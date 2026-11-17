import { computed, ref } from 'vue';
import type { Ref } from 'vue';
import {
  loadSavedTabs,
  saveTabs,
  loadSavedActiveTab,
  saveActiveTab,
  tabKey,
  MAX_TABS,
} from '../lib/openTabsStorage';
import type { OpenTab, ScenarioTab } from '../lib/openTabsStorage';

type TrajectRef = string | null | undefined;

/**
 * A tab as callers pass it in: an article (`kind` may be left out, and
 * `articleNumber` is normalised to a string) or a scenario file.
 */
export type TabInput =
  | { kind?: 'article'; lawId: string; articleNumber: string | number }
  | ScenarioTab;

interface TabBucket {
  tabs: OpenTab[];
  active: OpenTab | null;
}

/**
 * The editor's open-tab store, bucketed per traject.
 *
 * The previous design kept a single `openTabs` ref and, on a traject switch,
 * reassigned it to the new traject's saved set (`openTabs.value =
 * loadSavedTabs(next)`). Every writer read the active traject ref *at write
 * time* (`saveTabs(activeTrajectRef.value, …)`) - a global computed read after
 * an `await`, so a late/stale write could land under whichever traject the
 * route had meanwhile switched to. That is the root of the cross-traject leak.
 *
 * Here tab state is instead *derived* from the active traject: a
 * `Map<trajectRef, { tabs, active }>`, lazily hydrated from localStorage, and
 * every mutation takes an **explicit** `trajectRef`. A late write for traject A
 * lands in A's bucket and is simply invisible while B is active, rather than
 * corrupting B. The reactive `tabs` / `activeTab` computeds re-read the bucket
 * for whatever traject is active *now*, so a route commit flips the bar without
 * anyone reassigning it.
 *
 * A single reactive `version` counter is the only dependency: buckets are plain
 * objects mutated in place (always a NEW array + `version.value++`, never an
 * in-place array splice), so consumers stay cheap and there is no per-tab
 * reactivity to leak. No lifecycle hooks, so this is directly unit-testable.
 *
 * A tab is an article or a scenario file; see `OpenTab`.
 */
export function useOpenTabs(activeTrajectRef: Readonly<Ref<TrajectRef>>) {
  // Map<string, { tabs: Tab[], active: Tab | null }>. Keyed on the traject ref
  // coerced to a string ('' for the traject-less read-only editor).
  const buckets = new Map<string, TabBucket>();
  // The single reactive dependency. Bumped on every mutation; the computeds and
  // the explicit-ref readers below take it as their only reactive input.
  const version = ref(0);

  function keyOf(trajectRef: TrajectRef): string {
    return trajectRef ?? '';
  }

  // Lazy hydration: a bucket is loaded from localStorage the first time it is
  // touched, so opening tab A while B is active never reads B's storage.
  function bucket(trajectRef: TrajectRef): TabBucket {
    const key = keyOf(trajectRef);
    let b = buckets.get(key);
    if (!b) {
      b = { tabs: loadSavedTabs(trajectRef), active: loadSavedActiveTab(trajectRef) };
      buckets.set(key, b);
    }
    return b;
  }

  function normalize(tab: TabInput): OpenTab {
    if (tab.kind === 'scenario') return { kind: 'scenario', lawId: tab.lawId, filename: tab.filename };
    return { kind: 'article', lawId: tab.lawId, articleNumber: String(tab.articleNumber) };
  }

  // --- Explicit-ref reads (used by the restore flow, which always names the
  //     traject it is restoring). Read `version` so they stay reactive when
  //     called from within a computed/watch. ---
  function tabsFor(trajectRef: TrajectRef): OpenTab[] {
    version.value;
    return bucket(trajectRef).tabs;
  }
  function activeTabFor(trajectRef: TrajectRef): OpenTab | null {
    version.value;
    return bucket(trajectRef).active;
  }
  function findTab(trajectRef: TrajectRef, tab: TabInput): OpenTab | null {
    const key = tabKey(normalize(tab));
    return tabsFor(trajectRef).find((t) => tabKey(t) === key) ?? null;
  }

  // --- Reactive views for the currently active traject. ---
  const tabs = computed(() => tabsFor(activeTrajectRef.value));
  const activeTab = computed(() => activeTabFor(activeTrajectRef.value));
  // The traject ref that the current `tabs` / `activeTab` belong to, published
  // alongside them so the bar can key its rebuild on the SAME value that moves
  // with the tabs (not a sibling ref that flips a tick earlier).
  const publishedTrajectRef = computed(() => {
    version.value;
    return activeTrajectRef.value;
  });

  // --- Mutations (explicit trajectRef; never mutate an array in place). ---

  /**
   * Add a tab to a traject's bucket (de-duplicated, `MAX_TABS`-capped) and mark
   * it active. Persists both keys it touches.
   */
  function openTab(trajectRef: TrajectRef, tab: TabInput): OpenTab {
    const b = bucket(trajectRef);
    const t = normalize(tab);
    if (!b.tabs.some((x) => tabKey(x) === tabKey(t))) {
      const next = [...b.tabs, t];
      b.tabs = next.length > MAX_TABS ? next.slice(-MAX_TABS) : next;
      saveTabs(trajectRef, b.tabs);
    }
    b.active = t;
    saveActiveTab(trajectRef, t);
    version.value++;
    return t;
  }

  /** Set (or clear, when `tab` is null) a traject's active tab and persist it. */
  function setActiveTab(trajectRef: TrajectRef, tab: TabInput | null | undefined): OpenTab | null {
    const b = bucket(trajectRef);
    b.active = tab ? normalize(tab) : null;
    saveActiveTab(trajectRef, b.active);
    version.value++;
    return b.active;
  }

  /**
   * Remove a tab. When it was the active one, promote a replacement: the
   * caller's explicit `next` (the bar's own pick), else the neighbour to the
   * right, else the one to the left, else null. Persists the new active tab too
   * - closing the active tab and reloading must not resurrect it.
   */
  function closeTab(trajectRef: TrajectRef, tab: TabInput, next: OpenTab | null = null): OpenTab | null {
    const b = bucket(trajectRef);
    const target = tabKey(normalize(tab));
    const index = b.tabs.findIndex((x) => tabKey(x) === target);
    if (index === -1) return b.active;
    const remaining = b.tabs.filter((x) => tabKey(x) !== target);
    b.tabs = remaining;
    saveTabs(trajectRef, remaining);
    if (b.active && tabKey(b.active) === target) {
      // Removing index `i` shifts the right neighbour into `i`.
      b.active = next ?? remaining[index] ?? remaining[index - 1] ?? null;
      saveActiveTab(trajectRef, b.active);
    }
    version.value++;
    return b.active;
  }

  /** Move a tab within a traject's bucket and persist the new order. */
  function reorderTabs(trajectRef: TrajectRef, fromIndex: number, toIndex: number): void {
    const b = bucket(trajectRef);
    if (
      fromIndex < 0 || fromIndex >= b.tabs.length ||
      toIndex < 0 || toIndex >= b.tabs.length ||
      fromIndex === toIndex
    ) {
      return;
    }
    const next = [...b.tabs];
    // fromIndex is bounds-checked above, so splice removes exactly one tab.
    const [moved] = next.splice(fromIndex, 1) as [OpenTab];
    next.splice(toIndex, 0, moved);
    b.tabs = next;
    saveTabs(trajectRef, next);
    version.value++;
  }

  /**
   * Drop every tab pointing at `lawId` from a traject, its scenario tabs
   * included (the self-heal for a law that 404's in this traject), clear the
   * active tab when it was one of them, and re-persist both keys.
   */
  function dropLaw(trajectRef: TrajectRef, lawId: string): void {
    const b = bucket(trajectRef);
    b.tabs = b.tabs.filter((x) => x.lawId !== lawId);
    saveTabs(trajectRef, b.tabs);
    if (b.active && b.active.lawId === lawId) {
      b.active = null;
    }
    saveActiveTab(trajectRef, b.active);
    version.value++;
  }

  return {
    tabs,
    activeTab,
    publishedTrajectRef,
    tabsFor,
    activeTabFor,
    findTab,
    openTab,
    setActiveTab,
    closeTab,
    reorderTabs,
    dropLaw,
  };
}
