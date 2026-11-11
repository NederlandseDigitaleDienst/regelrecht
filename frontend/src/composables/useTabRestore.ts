import { MAX_TABS } from '../lib/openTabsStorage';
import { useLatest } from '../lib/useLatest';
import type { OpenTab } from '../lib/openTabsStorage';

type TrajectRef = string | null;

/** A load error as useOpenLaw leaves it: a network error has no `status`. */
export type LoadError = { status?: number } | (Error & { status?: number });

/**
 * What `createTabRestore` needs from the editor; see the JSDoc below. `Loc` is
 * whatever `editorRouteFor` builds and `router.replace` accepts.
 */
export interface TabRestoreDeps<Loc> {
  tabsFor: (ref: TrajectRef) => OpenTab[];
  activeTabFor: (ref: TrajectRef) => OpenTab | null;
  setActiveTab: (ref: TrajectRef, tab: OpenTab | null) => unknown;
  dropLaw: (ref: TrajectRef, lawId: string) => void;
  switchLaw: (lawId: string, article: string, ref: TrajectRef) => Promise<boolean>;
  clearLaw: () => void;
  error: { readonly value: LoadError | null | undefined };
  router: { replace: (to: Loc) => unknown };
  editorRouteFor: (lawId: string | null, article: string | null) => Loc;
  canPrune: () => boolean;
}

/**
 * The editor's "open the right document when you enter a traject" flow.
 *
 * On entering a traject's editor WITHOUT a law in the URL - a fresh page load,
 * a traject switch, coming back through the Home/Editor tab, or browser
 * back/forward to the bare root - restore the last active article of *that*
 * traject and stamp it into the URL with `router.replace`. When the URL already
 * carries a law it wins (a deep link) and we do nothing.
 *
 * A remembered article whose law is a hard 404 in this traject is pruned - from
 * the bar and from localStorage - and the next candidate is tried, so polluted
 * storage from earlier builds heals itself. Only a `status === 404` prunes; a
 * network error or a 5xx leaves the tab in place (per construction: a fetch
 * that never reached the server throws a `TypeError` with no `.status`). With
 * no candidate left we land on the neutral root.
 *
 * Factored out of `EditorView.vue` (which has no test file) as a dependency-
 * injected factory so the whole cascade is unit-testable.
 *
 * @param {object} deps
 * @param {(ref: string|null) => Tab[]} deps.tabsFor - current tabs for a traject
 * @param {(ref: string|null) => Tab|null} deps.activeTabFor - saved active tab
 * @param {(ref: string|null, tab: Tab|null) => void} deps.setActiveTab
 * @param {(ref: string|null, lawId: string) => void} deps.dropLaw
 * @param {(lawId: string, article: string, ref: string|null) => Promise<boolean>} deps.switchLaw - resolves to whether THIS call won the shared staleness race
 * @param {() => void} deps.clearLaw
 * @param {{ value: { status?: number } | null }} deps.error - useOpenLaw's error ref
 * @param {{ replace: Function }} deps.router
 * @param {(lawId: string|null, article: string|null) => object} deps.editorRouteFor
 * @param {() => boolean} deps.canPrune - true once traject membership is confirmed
 */
export function createTabRestore<Loc>({
  tabsFor,
  activeTabFor,
  setActiveTab,
  dropLaw,
  switchLaw,
  clearLaw,
  error,
  router,
  editorRouteFor,
  canPrune,
}: TabRestoreDeps<Loc>) {
  // Supersede an in-flight restore when a newer one starts (rapid A->B->A
  // switching): the stale restore drops its writes instead of racing the new
  // traject's restore.
  const claim = useLatest();

  function landNeutral(trajectRef: TrajectRef): void {
    // No document to show: unload the previous law, clear the active tab and
    // sit on the traject's editor root ("Open een artikel vanuit de tabbalk of
    // Home").
    clearLaw();
    setActiveTab(trajectRef, null);
    router.replace(editorRouteFor(null, null));
  }

  /**
   * Restore the last active (or first) article for `trajectRef`.
   *
   * @param {string|null} trajectRef
   * @param {{ hasLawInUrl: boolean }} opts
   */
  async function restoreForTraject(
    trajectRef: TrajectRef,
    { hasLawInUrl }: { hasLawInUrl: boolean },
  ): Promise<void> {
    // The traject-less read-only editor buckets under '' and drives its own
    // `?law=` query; never auto-open there.
    if (!trajectRef) return;
    // A law in the URL is an explicit open (deep link / back-forward): it wins,
    // no restore, and the "niet beschikbaar in dit traject" page stays for a
    // law this traject lacks.
    if (hasLawInUrl) return;

    const isCurrent = claim();
    // The remembered article is preferred; capture it before any mutation so a
    // later prune of its law falls back to the first surviving tab.
    const remembered = activeTabFor(trajectRef);

    // Each round opens (and validates) one candidate; a 404 prunes it and loops.
    // `MAX_TABS` is the belt: every prune removes at least one tab, so the loop
    // can never outrun the bucket.
    for (let round = 0; round <= MAX_TABS; round++) {
      const current = tabsFor(trajectRef);
      if (current.length === 0) {
        landNeutral(trajectRef);
        return;
      }
      // Prefer the remembered article while it still exists; otherwise the
      // first surviving tab.
      const candidate =
        (remembered &&
          current.find(
            (t) => t.lawId === remembered.lawId && t.articleNumber === remembered.articleNumber,
          )) ||
        current[0]!; // non-empty: checked just above

      // Set active synchronously BEFORE the await (mirrors `selectTab`) so the
      // neutral empty state never flashes between rounds.
      setActiveTab(trajectRef, candidate);
      const won = await switchLaw(candidate.lawId, candidate.articleNumber, trajectRef);
      // A newer restore superseded us: drop every write from here on.
      if (!isCurrent()) return;
      // `switchLaw` shares one staleness token across ALL callers, so a
      // concurrent `selectTab` / route-switch (the user clicking a different
      // tab mid-restore) can win that race and make OUR call a silent no-op -
      // it returns `false` and leaves `error.value`/`law.value` reflecting the
      // OTHER call's result. Our own `isCurrent()` (a private restore token)
      // can't see that, so without this check we'd read a foreign `error`
      // (wrongly pruning `candidate`, or stamping the URL at a law that never
      // loaded). Drop our writes; the winning caller drives the bar/URL.
      if (!won) return;

      // Prune ONLY on a confirmed hard 404. `canPrune` gates on settled traject
      // membership: at mount the membership list is still loading and every
      // law-GET 404s, so this check must sit AFTER the await (subtle: pruning
      // before membership settles would wipe a traject's whole bar).
      if (error.value?.status === 404) {
        if (canPrune()) {
          dropLaw(trajectRef, candidate.lawId);
          continue;
        }
        // Membership not yet confirmed - leave the tab and stop; a later entry
        // (once the list loads) re-runs the restore and can prune then.
        return;
      }
      // Network error / 5xx: leave the tab and its error dialog in place.
      if (error.value) return;

      // Clean load: sync the URL so a refresh / back-forward stays in step.
      router.replace(editorRouteFor(candidate.lawId, candidate.articleNumber));
      return;
    }
    // Belt tripped (should be unreachable): settle on the neutral root.
    landNeutral(trajectRef);
  }

  return { restoreForTraject };
}
