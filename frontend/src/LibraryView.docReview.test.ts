// Targeted tests for the document-review orchestration in LibraryView.vue
// (rejectDocReview / onDocSaved / clearDocReviewQuery / the proposal-seeding
// watch). Like the other LibraryView.*.test.ts files, this one mounts the view
// `shallow` with every composable it touches replaced by a controllable stub
// from test-utils/libraryViewHarness.ts - deliberately narrow: it exercises
// only the review-flow surface, not the rest of the view (laws browsing,
// search, traject panes, ...), which stay in their default/empty state.
import { describe, it, expect, vi, beforeEach } from 'vitest';
import { shallowMount, type VueWrapper } from '@vue/test-utils';
import { ref } from 'vue';
import type { DocError } from './composables/useTrajectDocuments';
import * as harness from './test-utils/libraryViewHarness';

/** The route fields LibraryView reads; tests reassign params and query freely. */
interface RouteStub {
  name: string;
  params: Record<string, string>;
  query: Record<string, string>;
  fullPath: string;
}

/** The review-task fields LibraryView reads. */
interface ReviewTaskStub {
  id: string;
  payload: { target_path: string; traject_ref: string };
}

/**
 * Setup state the tests call through `wrapper.vm`. LibraryView exposes
 * nothing, but test-utils proxies the setup state onto `vm`.
 */
interface LibraryViewInternals {
  onViewTaskJob(path: string): void;
  rejectDocReview(): Promise<void>;
  onDocSaved(savedPath?: string | null): Promise<void>;
}
const internals = (wrapper: VueWrapper) => wrapper.vm as unknown as LibraryViewInternals;

// --- vue-router: no real router install. `onBeforeRouteUpdate`/
// `onBeforeRouteLeave` are just named exports LibraryView calls once at
// setup to register a callback - stubbing them as no-ops absorbs that
// registration without needing real guard machinery (same technique
// TasksListPane.test.js / MobileTrajectSheet.test.js use for useRoute/useRouter).
const routeState: RouteStub = {
  name: 'werkdocumenten-traject',
  params: { trajectRef: 'traject-abcd1234', docPath: '' },
  query: {},
  fullPath: '/traject/traject-abcd1234/werkdocumenten',
};
const replaceMock = vi.fn();
const pushMock = vi.fn();
vi.mock('vue-router', () =>
  harness.vueRouter(
    () => routeState,
    () => ({ replace: replaceMock, push: pushMock }),
  ),
);
vi.mock('./composables/useAuth', () => harness.auth());
vi.mock('./composables/useTrajects', () => harness.trajectsModule());
vi.mock('./lib/apiFetch', () => harness.apiFetchModule());
vi.mock('./composables/useFeatureFlags', () => harness.featureFlags());

// The manager stub deliberately keeps `currentPath`/`currentBody`/`docLoading`/
// `docError` as module-level refs so tests can drive the exact state the
// review-flow watch reacts to, and assert on `dropDraft`/`open` (aliased
// `openDoc` in LibraryView) being called (or not).
const openDoc = vi.fn().mockResolvedValue(undefined);
const dropDraft = vi.fn();
const currentPath = ref<string | null>('report.md');
const currentBody = ref('');
const docLoading = ref(false);
const docError = ref<DocError | null>(null);
vi.mock('./composables/useDocumentsManager', () =>
  harness.documentsManager(() => ({
    currentPath,
    currentBody,
    docLoading,
    docError,
    open: openDoc,
    dropDraft,
  })),
);
vi.mock('./composables/useTrajectDocumentJobs', () => harness.trajectDocumentJobs());
vi.mock('./composables/useDocumentUpload', () => harness.documentUpload());

// The composable under test's caller-side orchestration: kept as
// module-level refs (identity-equal to what LibraryView destructures) so
// tests can set `reviewTask.value` / `proposedContent.value` directly and
// assert on `loadError.value` after a failed resolve.
const reviewTask = ref<ReviewTaskStub | null>(null);
const proposedContent = ref<string | null>(null);
const loadError = ref<string | null>(null);
const loadReview = vi.fn();
const approveAfterSave = vi.fn();
const rejectTask = vi.fn();
vi.mock('./composables/useDocumentTaskReview', () =>
  harness.documentTaskReview(() => ({
    reviewTask,
    proposedContent,
    loadError,
    loadReview,
    approveAfterSave,
    reject: rejectTask,
  })),
);

import LibraryView from './LibraryView.vue';

function mountLibrary() {
  return shallowMount(LibraryView, { global: { stubs: { teleport: true } } });
}

beforeEach(() => {
  reviewTask.value = null;
  proposedContent.value = null;
  loadError.value = null;
  loadReview.mockReset().mockResolvedValue(undefined);
  approveAfterSave.mockReset();
  rejectTask.mockReset();
  openDoc.mockReset().mockResolvedValue(undefined);
  dropDraft.mockReset();
  replaceMock.mockReset().mockReturnValue(Promise.resolve());
  pushMock.mockReset();
  currentPath.value = 'report.md';
  currentBody.value = '';
  docLoading.value = false;
  docError.value = null;
  routeState.name = 'werkdocumenten-traject';
  routeState.params = { trajectRef: 'traject-abcd1234', docPath: '' };
  routeState.query = {};
});

function openReview(overrides: Partial<ReviewTaskStub['payload']> = {}) {
  reviewTask.value = {
    id: 't1',
    payload: { target_path: 'report.md', traject_ref: 'traject-abcd1234', ...overrides },
  };
  routeState.query = { task: 't1' };
}

// De "opent onbekend werkdocument → geen 404"-fix: een 404-staat toont een
// eigen centrale melding (niet de kale editor of "Geen document open"), en de
// URL is leidend - een adresseerd-maar-niet-open document wordt heropend.
describe('LibraryView werkdocument not-found', () => {
  function showsEmptyState(wrapper: VueWrapper) {
    // "Geen document open" staat als attribuut op nldd-inline-dialog (text="…").
    return wrapper.html().includes('Geen document open');
  }

  it('toont een centrale 404-melding voor een niet-bestaand document, niet de editor', async () => {
    currentPath.value = 'ghost.md';
    docError.value = { kind: 'not-found', message: 'Document niet gevonden' };
    routeState.params = { trajectRef: 'traject-abcd1234', docPath: 'ghost.md' };
    routeState.query = {};
    const wrapper = mountLibrary();
    await wrapper.vm.$nextTick();
    // De eigen 404-tak rendert, niet DocumentEditor.
    expect(wrapper.findComponent({ name: 'DocumentEditor' }).exists()).toBe(false);
    expect(wrapper.html()).toContain('bestaat niet');
    expect(showsEmptyState(wrapper)).toBe(false);
  });

  it('houdt de 404-melding weg zolang er een review-taak (?task=) loopt', async () => {
    // Tijdens een review is de not-found juist het startpunt voor het voorstel.
    currentPath.value = 'ghost.md';
    docError.value = { kind: 'not-found', message: 'Document niet gevonden' };
    routeState.params = { trajectRef: 'traject-abcd1234', docPath: 'ghost.md' };
    routeState.query = { task: 't1' };
    const wrapper = mountLibrary();
    await wrapper.vm.$nextTick();
    expect(wrapper.html()).not.toContain('bestaat niet');
  });

  it('onViewTaskJob landt deterministisch op de job-weergave, niet op een document-open', async () => {
    // "Bekijk document" vanuit een lopende conversie-taak: het pad mag niet
    // eerst als gewoon document geopend worden (→ 404/mislukt-race), maar
    // meteen als job-weergave ("Aan het converteren…").
    currentPath.value = null;
    routeState.params = { trajectRef: 'traject-abcd1234', docPath: '' };
    routeState.query = {};
    const wrapper = mountLibrary();
    await wrapper.vm.$nextTick();
    openDoc.mockClear();
    pushMock.mockClear();

    internals(wrapper).onViewTaskJob('nota.md');
    await wrapper.vm.$nextTick();

    expect(pushMock).toHaveBeenCalledWith({
      name: 'werkdocumenten-traject',
      params: { trajectRef: 'traject-abcd1234', docPath: 'nota.md' },
    });
    // Deterministisch: het pad wordt niet als document geopend, en de
    // job-weergave staat er (converterend, want de job is nog niet mislukt).
    expect(openDoc).not.toHaveBeenCalled();
    expect(wrapper.html()).toContain('Aan het converteren');
  });
});

describe('LibraryView document-review flow', () => {
  // Fix 1: "Verwerpen" must throw away the seeded draft before reopening the
  // document - otherwise the debounced localStorage persistence in
  // useTrajectDocuments resurrects the rejected proposal as a 'draft-present'
  // notice (or leaves an orphan draft for a document that never existed).
  it('rejectDocReview drops the draft before reopening the document, then clears the task query', async () => {
    rejectTask.mockResolvedValue(undefined);
    openReview();
    const wrapper = mountLibrary();
    await wrapper.vm.$nextTick();

    const order: string[] = [];
    dropDraft.mockImplementation(() => order.push('dropDraft'));
    openDoc.mockImplementation(() => {
      order.push('openDoc');
      return Promise.resolve();
    });

    await internals(wrapper).rejectDocReview();

    expect(rejectTask).toHaveBeenCalled();
    expect(order).toEqual(['dropDraft', 'openDoc']);
    expect(openDoc).toHaveBeenCalledWith('report.md');
    expect(replaceMock).toHaveBeenCalledWith(
      expect.objectContaining({ name: 'werkdocumenten-traject' }),
    );
  });

  // Fix 6: a failed reject-resolve must not be treated as a successful
  // reject - the draft/document state stays untouched and the failure is
  // surfaced, not silently swallowed.
  it('rejectDocReview leaves the draft and task open when the resolve call fails', async () => {
    rejectTask.mockRejectedValue(new Error('network'));
    openReview();
    const wrapper = mountLibrary();
    await wrapper.vm.$nextTick();

    await internals(wrapper).rejectDocReview();

    expect(dropDraft).not.toHaveBeenCalled();
    expect(openDoc).not.toHaveBeenCalled();
    expect(replaceMock).not.toHaveBeenCalled();
    expect(loadError.value).toMatch(/mislukt/i);
  });

  // A retry after a failed reject must clear the stale critical banner: the
  // second call succeeds, so the "Verwerpen ... mislukt" error may not linger.
  it('rejectDocReview clears a prior reject-failure banner on a successful retry', async () => {
    rejectTask.mockResolvedValue(undefined);
    openReview();
    loadError.value = 'Verwerpen van het voorstel is mislukt. Probeer het opnieuw.';
    const wrapper = mountLibrary();
    await wrapper.vm.$nextTick();

    await internals(wrapper).rejectDocReview();

    expect(rejectTask).toHaveBeenCalled();
    expect(loadError.value).toBeNull();
  });

  // Approving (Opslaan) after a prior reject-failure must also drop the stale
  // banner - otherwise it stays visible while the task is already resolved.
  it('onDocSaved clears a prior reject-failure banner on a successful approval', async () => {
    approveAfterSave.mockResolvedValue(undefined);
    openReview();
    loadError.value = 'Verwerpen van het voorstel is mislukt. Probeer het opnieuw.';
    const wrapper = mountLibrary();
    await wrapper.vm.$nextTick();

    await internals(wrapper).onDocSaved('report.md');

    expect(approveAfterSave).toHaveBeenCalled();
    expect(loadError.value).toBeNull();
  });

  // Fix 2 + Fix 4: onDocSaved only approves when the saved path AND traject
  // match the task under review.
  it('onDocSaved approves and clears the task query when the saved path matches', async () => {
    approveAfterSave.mockResolvedValue(undefined);
    openReview();
    const wrapper = mountLibrary();
    await wrapper.vm.$nextTick();

    await internals(wrapper).onDocSaved('report.md');

    expect(approveAfterSave).toHaveBeenCalled();
    expect(replaceMock).toHaveBeenCalledWith(
      expect.objectContaining({ name: 'werkdocumenten-traject' }),
    );
  });

  it('onDocSaved ignores a save of a different path (e.g. renamed away from target_path)', async () => {
    openReview();
    const wrapper = mountLibrary();
    await wrapper.vm.$nextTick();

    await internals(wrapper).onDocSaved('other.md');

    expect(approveAfterSave).not.toHaveBeenCalled();
    expect(replaceMock).not.toHaveBeenCalled();
  });

  it('onDocSaved ignores a review task that belongs to a different traject', async () => {
    openReview({ traject_ref: 'ander-traject-9999zzzz' });
    const wrapper = mountLibrary();
    await wrapper.vm.$nextTick();

    await internals(wrapper).onDocSaved('report.md');

    expect(approveAfterSave).not.toHaveBeenCalled();
  });

  // Fix 6: a failed approve-resolve must not crash the (already-succeeded)
  // save flow; the task simply stays open.
  it('onDocSaved leaves the task open without crashing when the resolve call fails', async () => {
    approveAfterSave.mockRejectedValue(new Error('network'));
    openReview();
    const wrapper = mountLibrary();
    await wrapper.vm.$nextTick();

    await expect(internals(wrapper).onDocSaved('report.md')).resolves.toBeUndefined();
    expect(replaceMock).not.toHaveBeenCalled();
  });

  // Fix 3: a save-and-leave already lets the user's chosen navigation
  // through before the approve-resolve settles; clearDocReviewQuery must not
  // drag them back once the route has moved off the werkdocumenten/task URL.
  it('clearDocReviewQuery does not stomp a navigation the user already made', async () => {
    approveAfterSave.mockResolvedValue(undefined);
    openReview();
    const wrapper = mountLibrary();
    await wrapper.vm.$nextTick();

    routeState.name = 'library-traject';
    await internals(wrapper).onDocSaved('report.md');

    expect(approveAfterSave).toHaveBeenCalled();
    expect(replaceMock).not.toHaveBeenCalled();
  });

  // Fix 2: the seeding `.then()` callback must re-validate against the
  // task's own payload before writing into the editor body - a stale
  // response arriving after the user has moved to another document must not
  // seed the wrong one.
  it('seeds the proposal into the document body when the task matches the open document', async () => {
    openReview();
    loadReview.mockImplementation(() => {
      proposedContent.value = '# Voorstel';
      return Promise.resolve();
    });

    const wrapper = mountLibrary();
    await wrapper.vm.$nextTick();
    await wrapper.vm.$nextTick();

    expect(loadReview).toHaveBeenCalledWith('t1');
    expect(currentBody.value).toBe('# Voorstel');
  });

  it('skips seeding when the open document changed while the task fetch was in flight', async () => {
    openReview();
    let resolveLoad: ((value?: unknown) => void) | undefined;
    loadReview.mockImplementation(
      () =>
        new Promise((resolve) => {
          resolveLoad = resolve;
        }),
    );

    const wrapper = mountLibrary();
    await wrapper.vm.$nextTick();
    expect(loadReview).toHaveBeenCalledWith('t1');

    // The user navigated to a different document before the fetch resolved.
    currentPath.value = 'other.md';
    proposedContent.value = '# Voorstel';
    // Assigned by the mockImplementation, which the mount just called.
    resolveLoad!();
    await wrapper.vm.$nextTick();
    await wrapper.vm.$nextTick();

    expect(currentBody.value).toBe('');
  });
});
