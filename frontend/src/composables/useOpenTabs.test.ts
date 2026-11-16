import { describe, it, expect, beforeEach } from 'vitest';
import { ref } from 'vue';
import { useOpenTabs } from './useOpenTabs';

// useOpenTabs buckets the editor's open tabs per traject and derives the active
// bar from the active traject ref. These pin the bucket isolation that fixes
// the cross-traject leak, plus the closeTab/dropLaw/persistence contracts.

const A = 'traject-a-00000001';
const B = 'traject-b-00000002';
const TAB_A1 = { kind: 'article', lawId: 'law_a', articleNumber: '1' } as const;
const TAB_A2 = { kind: 'article', lawId: 'law_a', articleNumber: '2' } as const;
const TAB_C1 = { kind: 'article', lawId: 'law_c', articleNumber: '1' } as const;
const SCENARIO_A = { kind: 'scenario', lawId: 'law_a', filename: 'basis.feature' } as const;

const tabsKey = (ref_: string) => `regelrecht-open-tabs:${ref_}`;
const activeKey = (ref_: string) => `regelrecht-active-tab:${ref_}`;
// Parse a stored value; a missing key parses as null, like JSON.parse(null).
const stored = (key: string): unknown => JSON.parse(localStorage.getItem(key) ?? 'null');

beforeEach(() => {
  localStorage.clear();
});

describe('useOpenTabs', () => {
  it('isolates buckets: opening in A while B is active leaves tabs.value alone and writes A', () => {
    const active = ref(B);
    const store = useOpenTabs(active);
    expect(store.tabs.value).toEqual([]);

    store.openTab(A, TAB_A1);

    // The active (B) bar is untouched...
    expect(store.tabs.value).toEqual([]);
    // ...while A's own bucket + key got the write.
    expect(store.tabsFor(A)).toEqual([TAB_A1]);
    expect(stored(tabsKey(A))).toEqual([TAB_A1]);
    expect(localStorage.getItem(tabsKey(B))).toBeNull();
  });

  it('lands a stale write in its own bucket, invisible to the active traject', () => {
    const active = ref(A);
    const store = useOpenTabs(active);
    store.openTab(A, TAB_A1);
    // The route switched to B while a late write for A is still coming.
    active.value = B;
    store.openTab(A, TAB_A2);
    expect(store.tabs.value).toEqual([]); // B stays empty
    expect(store.tabsFor(A)).toEqual([TAB_A1, TAB_A2]);
  });

  it('lazily hydrates a bucket from localStorage', () => {
    localStorage.setItem(tabsKey(A), JSON.stringify([TAB_A1, TAB_C1]));
    localStorage.setItem(activeKey(A), JSON.stringify(TAB_C1));
    const active = ref(A);
    const store = useOpenTabs(active);
    expect(store.tabs.value).toEqual([TAB_A1, TAB_C1]);
    expect(store.activeTab.value).toEqual(TAB_C1);
  });

  it('caps a bucket at MAX_TABS (20), dropping the oldest', () => {
    const active = ref(A);
    const store = useOpenTabs(active);
    for (let i = 0; i < 25; i++) store.openTab(A, { lawId: `law_${i}`, articleNumber: '1' });
    expect(store.tabs.value).toHaveLength(20);
    expect(store.tabs.value[0]).toEqual({ kind: 'article', lawId: 'law_5', articleNumber: '1' });
    expect(store.tabs.value[19]).toEqual({ kind: 'article', lawId: 'law_24', articleNumber: '1' });
  });

  it('normalises a numeric articleNumber on openTab', () => {
    const active = ref(A);
    const store = useOpenTabs(active);
    store.openTab(A, { kind: 'article', lawId: 'law_a', articleNumber: 5 });
    expect(store.tabs.value).toEqual([{ kind: 'article', lawId: 'law_a', articleNumber: '5' }]);
  });

  it('openTab activates the newly opened tab', () => {
    const active = ref(A);
    const store = useOpenTabs(active);
    store.openTab(A, TAB_A1);
    store.openTab(A, TAB_A2);
    expect(store.activeTab.value).toEqual(TAB_A2);
    expect(store.tabs.value).toEqual([TAB_A1, TAB_A2]);
  });

  it('closeTab promotes the right neighbour then the left, and PERSISTS the new active tab', () => {
    const active = ref(A);
    const store = useOpenTabs(active);
    store.openTab(A, TAB_A1);
    store.openTab(A, TAB_A2);
    store.openTab(A, TAB_C1); // tabs [A1, A2, C1]
    store.setActiveTab(A, TAB_A2); // middle active

    const repl = store.closeTab(A, TAB_A2);
    expect(repl).toEqual(TAB_C1); // right neighbour
    expect(store.activeTab.value).toEqual(TAB_C1);
    // Persisted - closing the active tab and reloading must not resurrect it.
    expect(stored(activeKey(A))).toEqual(TAB_C1);

    const repl2 = store.closeTab(A, TAB_C1); // no right neighbour -> left
    expect(repl2).toEqual(TAB_A1);
  });

  it('closeTab honours the caller-provided next pick (the bar own choice)', () => {
    const active = ref(A);
    const store = useOpenTabs(active);
    store.openTab(A, TAB_A1);
    store.openTab(A, TAB_A2);
    store.openTab(A, TAB_C1);
    store.setActiveTab(A, TAB_C1);
    expect(store.closeTab(A, TAB_C1, TAB_A1)).toEqual(TAB_A1);
  });

  it('closing a non-active tab keeps the active tab', () => {
    const active = ref(A);
    const store = useOpenTabs(active);
    store.openTab(A, TAB_A1);
    store.openTab(A, TAB_A2); // active A2
    expect(store.closeTab(A, TAB_A1)).toEqual(TAB_A2);
    expect(store.activeTab.value).toEqual(TAB_A2);
  });

  it('dropLaw removes every tab of a law, clears the active tab, cleans both keys', () => {
    const active = ref(A);
    const store = useOpenTabs(active);
    store.openTab(A, TAB_A1);
    store.openTab(A, TAB_A2);
    store.openTab(A, TAB_C1);
    store.setActiveTab(A, TAB_A1); // active is law_a

    store.dropLaw(A, 'law_a');
    expect(store.tabs.value).toEqual([TAB_C1]);
    expect(store.activeTab.value).toBeNull();
    expect(stored(tabsKey(A))).toEqual([TAB_C1]);
    expect(localStorage.getItem(activeKey(A))).toBeNull();
  });

  it('dropLaw keeps a different active tab', () => {
    const active = ref(A);
    const store = useOpenTabs(active);
    store.openTab(A, TAB_A1);
    store.openTab(A, TAB_C1); // active C1
    store.dropLaw(A, 'law_a');
    expect(store.tabs.value).toEqual([TAB_C1]);
    expect(store.activeTab.value).toEqual(TAB_C1);
  });

  it('reorderTabs moves a tab and persists the new order', () => {
    const active = ref(A);
    const store = useOpenTabs(active);
    store.openTab(A, TAB_A1);
    store.openTab(A, TAB_A2);
    store.openTab(A, TAB_C1);
    store.reorderTabs(A, 0, 2);
    expect(store.tabs.value).toEqual([TAB_A2, TAB_C1, TAB_A1]);
    expect(stored(tabsKey(A))).toEqual([TAB_A2, TAB_C1, TAB_A1]);
  });

  it('publishedTrajectRef tracks the active ref', () => {
    const active = ref(A);
    const store = useOpenTabs(active);
    expect(store.publishedTrajectRef.value).toBe(A);
    active.value = B;
    expect(store.publishedTrajectRef.value).toBe(B);
  });
  it('opens a scenario tab next to the law\'s article tabs and finds it', () => {
    const store = useOpenTabs(ref(A));
    store.openTab(A, TAB_A1);
    store.openTab(A, SCENARIO_A);
    store.openTab(A, SCENARIO_A);
    expect(store.tabs.value).toEqual([TAB_A1, SCENARIO_A]);
    expect(store.activeTab.value).toEqual(SCENARIO_A);
    expect(store.findTab(A, SCENARIO_A)).toEqual(SCENARIO_A);
    expect(store.findTab(A, { lawId: 'law_a', articleNumber: 1 })).toEqual(TAB_A1);
  });

  it('dropLaw also closes the law\'s scenario tabs', () => {
    const store = useOpenTabs(ref(A));
    store.openTab(A, TAB_A1);
    store.openTab(A, TAB_C1);
    store.openTab(A, SCENARIO_A);
    store.dropLaw(A, 'law_a');
    expect(store.tabs.value).toEqual([TAB_C1]);
    expect(store.activeTab.value).toBeNull();
  });
});
