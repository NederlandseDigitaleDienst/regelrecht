import { describe, it, expect } from 'vitest';
import { createTabRestore } from './useTabRestore';
import type { LoadError } from './useTabRestore';
import type { OpenTab } from '../lib/openTabsStorage';

// useTabRestore drives "open the right article when you enter a traject". These
// pin the four landing branches, the 404 prune-cascade, the never-prune rules
// (5xx / unconfirmed membership) and the superseded-claim guard.

const REF = 'traject-a-00000001';
const T1: OpenTab = { kind: 'article', lawId: 'law_1', articleNumber: '1' };
const T2: OpenTab = { kind: 'article', lawId: 'law_2', articleNumber: '3' };
const S1: OpenTab = { kind: 'scenario', lawId: 'law_1', filename: 'basis.feature' };

/**
 * Build a restore instance over a fake bucket + a fake `switchLaw` that sets
 * `error` from a `laws` status map. `laws[lawId]` is 404 | 500 | 'network' |
 * undefined(ok). `switchLaw` resolves to whether THIS call won the shared
 * staleness race (defaults to always winning; `switchWon: false` simulates a
 * concurrent caller superseding it). `canPrune` defaults to true.
 */
interface HarnessOptions {
  tabs?: OpenTab[];
  active?: OpenTab | null;
  laws?: Record<string, 404 | 500 | 'network'>;
  canPrune?: boolean;
  switchWon?: boolean;
}

/** What routeForTab builds in this harness. */
interface FakeLocation {
  lawId: string | null;
  articleNumber: string | null;
  filename?: string;
}

interface HarnessState {
  tabs: OpenTab[];
  active: OpenTab | null;
  error: { value: LoadError | null };
  replaced: FakeLocation[];
  dropped: string[];
  activeSetTo: (OpenTab | null)[];
  switched: [string, string | null][];
  cleared: number;
}

function harness({
  tabs = [],
  active = null,
  laws = {},
  canPrune = true,
  switchWon = true,
}: HarnessOptions = {}) {
  const state: HarnessState = {
    tabs: [...tabs],
    active,
    error: { value: null },
    replaced: [],
    dropped: [],
    activeSetTo: [],
    switched: [],
    cleared: 0,
  };
  const restore = createTabRestore({
    tabsFor: () => state.tabs,
    activeTabFor: () => state.active,
    setActiveTab: (_ref, tab) => {
      state.active = tab;
      state.activeSetTo.push(tab);
    },
    dropLaw: (_ref, lawId) => {
      state.tabs = state.tabs.filter((t) => t.lawId !== lawId);
      state.dropped.push(lawId);
      if (state.active && state.active.lawId === lawId) state.active = null;
    },
    switchLaw: async (lawId, article) => {
      state.switched.push([lawId, article]);
      const status = laws[lawId];
      // Model `error` as whatever is VISIBLE after the await. When this call is
      // superseded (`switchWon: false`) the value on the shared ref belongs to
      // the winning caller - surface it anyway, so the test proves restore
      // ignores a foreign error (no wrong prune / URL stamp) purely on `won`.
      state.error.value =
        status === 404 ? { status: 404 }
        : status === 500 ? { status: 500 }
        : status === 'network' ? new TypeError('fetch failed')
        : null;
      return switchWon;
    },
    clearLaw: () => { state.cleared++; },
    error: state.error,
    router: { replace: (t: FakeLocation) => state.replaced.push(t) },
    routeForTab: (tab): FakeLocation => {
      if (!tab) return { lawId: null, articleNumber: null };
      if (tab.kind === 'scenario') return { lawId: tab.lawId, articleNumber: null, filename: tab.filename };
      return { lawId: tab.lawId, articleNumber: tab.articleNumber };
    },
    canPrune: () => canPrune,
  });
  return { restore, state };
}

describe('createTabRestore', () => {
  it('does nothing for the traject-less read-only editor', async () => {
    const { restore, state } = harness({ tabs: [T1] });
    await restore.restoreForTraject(null, { hasLawInUrl: false });
    expect(state.activeSetTo).toEqual([]);
    expect(state.replaced).toEqual([]);
  });

  it('does nothing when the URL already carries a law (deep link wins)', async () => {
    const { restore, state } = harness({ tabs: [T1] });
    await restore.restoreForTraject(REF, { hasLawInUrl: true });
    expect(state.activeSetTo).toEqual([]);
    expect(state.replaced).toEqual([]);
  });

  it('restores the remembered active article and syncs the URL', async () => {
    const { restore, state } = harness({ tabs: [T1, T2], active: T2 });
    await restore.restoreForTraject(REF, { hasLawInUrl: false });
    expect(state.active).toEqual(T2);
    expect(state.replaced).toEqual([{ lawId: 'law_2', articleNumber: '3' }]);
  });

  it('falls back to the first tab when nothing is remembered', async () => {
    const { restore, state } = harness({ tabs: [T1, T2], active: null });
    await restore.restoreForTraject(REF, { hasLawInUrl: false });
    expect(state.active).toEqual(T1);
    expect(state.replaced).toEqual([{ lawId: 'law_1', articleNumber: '1' }]);
  });

  it('lands on the neutral root with no tabs', async () => {
    const { restore, state } = harness({ tabs: [] });
    await restore.restoreForTraject(REF, { hasLawInUrl: false });
    expect(state.cleared).toBe(1);
    expect(state.active).toBeNull();
    expect(state.replaced).toEqual([{ lawId: null, articleNumber: null }]);
  });

  it('prunes a 404 tab, tries the next candidate, then the neutral root', async () => {
    // Remembered T1 404s; T2 also 404s -> both pruned -> neutral root.
    const { restore, state } = harness({
      tabs: [T1, T2],
      active: T1,
      laws: { law_1: 404, law_2: 404 },
    });
    await restore.restoreForTraject(REF, { hasLawInUrl: false });
    expect(state.dropped).toEqual(['law_1', 'law_2']);
    expect(state.tabs).toEqual([]);
    expect(state.replaced).toEqual([{ lawId: null, articleNumber: null }]);
  });

  it('prunes the 404 tab then opens the surviving one', async () => {
    const { restore, state } = harness({
      tabs: [T1, T2],
      active: T1,
      laws: { law_1: 404 },
    });
    await restore.restoreForTraject(REF, { hasLawInUrl: false });
    expect(state.dropped).toEqual(['law_1']);
    expect(state.tabs).toEqual([T2]);
    expect(state.replaced).toEqual([{ lawId: 'law_2', articleNumber: '3' }]);
  });

  it('never prunes on a 5xx: leaves the tab and its error dialog', async () => {
    const { restore, state } = harness({ tabs: [T1], active: T1, laws: { law_1: 500 } });
    await restore.restoreForTraject(REF, { hasLawInUrl: false });
    expect(state.dropped).toEqual([]);
    expect(state.tabs).toEqual([T1]);
    // No URL rewrite to a law - the error stays up.
    expect(state.replaced).toEqual([]);
  });

  it('never prunes on a network error (no .status)', async () => {
    const { restore, state } = harness({ tabs: [T1], active: T1, laws: { law_1: 'network' } });
    await restore.restoreForTraject(REF, { hasLawInUrl: false });
    expect(state.dropped).toEqual([]);
    expect(state.tabs).toEqual([T1]);
    expect(state.replaced).toEqual([]);
  });

  it('does not prune before traject membership is confirmed (canPrune=false)', async () => {
    const { restore, state } = harness({
      tabs: [T1],
      active: T1,
      laws: { law_1: 404 },
      canPrune: false,
    });
    await restore.restoreForTraject(REF, { hasLawInUrl: false });
    expect(state.dropped).toEqual([]);
    expect(state.tabs).toEqual([T1]);
  });

  it('a superseded restore drops its writes', async () => {
    const { restore, state } = harness({ tabs: [T1], active: T1 });
    // Two overlapping restores: the first is superseded by the second before
    // its switchLaw resolves, so only the second reaches router.replace.
    const first = restore.restoreForTraject(REF, { hasLawInUrl: false });
    const second = restore.restoreForTraject(REF, { hasLawInUrl: false });
    await Promise.all([first, second]);
    expect(state.replaced).toEqual([{ lawId: 'law_1', articleNumber: '1' }]);
  });

  it('drops its writes when a concurrent caller wins the shared switchLaw race', async () => {
    // switchLaw shares one staleness token across all callers; a concurrent
    // selectTab / route-switch (the user clicking another tab mid-restore) can
    // win it, making OUR switchLaw a no-op that returns false and leaves
    // error/law reflecting the OTHER call. Restore must not stamp the URL for a
    // candidate that never loaded, nor prune it on a foreign error.
    const { restore, state } = harness({
      tabs: [T1],
      active: T1,
      // Even if the shared error looked like a 404, a superseded call must not
      // prune: the 404 belongs to whatever the winning caller requested.
      laws: { law_1: 404 },
      switchWon: false,
    });
    await restore.restoreForTraject(REF, { hasLawInUrl: false });
    expect(state.replaced).toEqual([]);
    expect(state.dropped).toEqual([]);
    expect(state.tabs).toEqual([T1]);
  });
  it('restores a scenario tab: opens its law without an article and goes to the file', async () => {
    const { restore, state } = harness({ tabs: [T2, S1], active: S1 });
    await restore.restoreForTraject(REF, { hasLawInUrl: false });
    expect(state.switched).toEqual([['law_1', null]]);
    expect(state.replaced).toEqual([{ lawId: 'law_1', articleNumber: null, filename: 'basis.feature' }]);
  });
});
