// Focused tests for the review-mode orchestration in EditorView.vue (the
// `?task=` watches, `applyProposedContent` and its cleanup). EditorView.vue
// has no test harness of its own, so this file mounts it `shallow` with every
// composable it touches replaced by a controllable stub - as
// LibraryView.docReview.test.ts does for the document review - except the open
// regelwerk, which is the real useOpenLaw over an in-memory corpus.
// Deliberately narrow: only the review surface; the rest of the view (panes,
// notes, scenarios, graph) stays in its empty default state.
import { describe, it, expect, vi, beforeEach, afterEach } from 'vitest';
import { flushPromises, shallowMount } from '@vue/test-utils';
import type { VueWrapper } from '@vue/test-utils';
import { ref, reactive, nextTick } from 'vue';
import type { EditorChanges } from './composables/useAppChrome';
import type { OpenLaw } from './composables/useOpenLaw';
import { installTestCorpus } from './test-utils/corpusSetup';
import { testingPinia } from './test-utils/testingPinia';
import { allFlagsOn } from './test-utils/featureFlagState';

// --- vue-router: no real router. `route` is reactive, because the whole bug
// hangs on a watch on `route.query.task`: a test must be able to change it the
// way a tab switch in the real editor does.
const routeState = reactive({
  name: 'editor-traject',
  params: { trajectRef: 'kieswet-kandidaatstelling-86070ac7', lawId: 'kieswet', articleNumber: 'B 5' },
  query: {},
  fullPath: '/trajecten/kieswet-kandidaatstelling-86070ac7/editor/kieswet/B 5',
});
const replaceMock = vi.fn();
const pushMock = vi.fn();
vi.mock('vue-router', () => ({
  useRoute: () => routeState,
  useRouter: () => ({
    replace: (...a: unknown[]) => replaceMock(...a),
    push: (...a: unknown[]) => pushMock(...a),
    resolve: () => ({ href: '#' }),
  }),
  onBeforeRouteUpdate: vi.fn(),
  onBeforeRouteLeave: vi.fn(),
}));

vi.mock('./composables/useAuth', () => ({
  useAuth: () => ({ authenticated: ref(true), oidcConfigured: ref(true), login: vi.fn() }),
}));




// --- useOpenLaw: the real module, over an in-memory corpus with the law as
// saved. The test drives it the way the view does (select an article, open
// another law) through the instance the view created.
const TRAJECT = 'kieswet-kandidaatstelling-86070ac7';
const SAVED_LAW_YAML = [
  '$id: kieswet',
  'articles:',
  "  - number: 'B 1'",
  '    text: B1 opgeslagen',
  "  - number: 'B 5'",
  '    text: B5 opgeslagen',
].join('\n');
// A second law in the same traject that happens to use the same article
// numbers - common enough ("B 1", "1", "2" exist in half the corpus). That is
// what makes it interesting: switching law then leaves the article number as
// it is, and only the law underneath changes.
const OTHER_LAW_YAML = [
  '$id: andere-wet',
  'articles:',
  "  - number: 'B 1'",
  '    text: B1 andere wet',
  "  - number: 'B 5'",
  '    text: B5 andere wet',
].join('\n');
let openLaw: OpenLaw | null = null;
const save = vi.fn();
vi.mock('./composables/useOpenLaw', async (importOriginal) => {
  const actual = await importOriginal<typeof import('./composables/useOpenLaw')>();
  return {
    ...actual,
    useOpenLaw: (...args: Parameters<typeof actual.useOpenLaw>) => {
      const real = actual.useOpenLaw(...args);
      openLaw = real;
      return {
        ...real,
        save: (...a: Parameters<typeof real.save>) => {
          save(...a);
          return real.save(...a);
        },
      };
    },
  };
});
/** The module instance the mounted view uses. */
function law(): OpenLaw {
  if (!openLaw) throw new Error('EditorView created no useOpenLaw');
  return openLaw;
}

vi.mock('./composables/useEngine', () => ({
  useEngine: () => ({
    ready: ref(false),
    initError: ref(null),
    initEngine: vi.fn().mockResolvedValue(null),
    getEngine: vi.fn(),
    loadLawYaml: vi.fn(),
    unloadAllLaws: vi.fn(),
  }),
}));

vi.mock('./composables/useNotes', () => ({
  useNotes: () => ({
    notesForArticle: ref([]),
    issues: ref([]),
    error: ref(null),
    reload: vi.fn(),
  }),
  useResolvedDraftNotes: () => ({ draftNotesForArticle: ref([]) }),
}));

vi.mock('./composables/useDraftNotes', () => ({
  useDraftNotes: () => ({
    drafts: ref([]),
    draftCount: ref(0),
    addDraft: vi.fn(),
    removeDraft: vi.fn(),
    exportYaml: vi.fn(),
    exportYamlFromNotes: vi.fn(),
    publishNote: vi.fn(),
  }),
}));

vi.mock('./composables/useOpenTabs', () => ({
  useOpenTabs: () => ({
    tabs: ref([]),
    activeTab: ref(null),
    publishedTrajectRef: ref(null),
    tabsFor: () => [],
    activeTabFor: () => null,
    findTab: () => null,
    openTab: vi.fn(),
    setActiveTab: vi.fn(),
    closeTab: vi.fn(),
    reorderTabs: vi.fn(),
    dropLaw: vi.fn(),
  }),
}));

vi.mock('./composables/useTabRestore', () => ({
  createTabRestore: () => ({ restoreForTraject: vi.fn().mockResolvedValue(undefined) }),
}));

vi.mock('./composables/useEnrichState', () => ({
  useEnrichState: () => ({
    isEnriching: ref(false),
    reviewReady: ref(false),
    reviewArticleForPane: ref(''),
    openReviewForLaw: vi.fn(),
    openTasksForLaw: vi.fn(),
    requestEnrich: vi.fn().mockResolvedValue({}),
  }),
}));

vi.mock('./composables/useLastVisitedRoute', () => ({
  lastHomePath: ref({ name: 'home' }),
  homeTarget: () => ({ name: 'home' }),
}));

vi.mock('./lib/apiFetch', () => ({
  apiFetch: vi.fn().mockResolvedValue({ ok: true, status: 200, headers: { get: () => null }, json: async () => ({}) }),
  apiFetchJson: vi.fn().mockResolvedValue({}),
  ApiError: class ApiError extends Error {},
}));

// The app shell renders the review bar and the changes bar; EditorView only
// publishes to them. So `editorChanges` is exactly what the user sees:
// `review: true` = the review view is on screen.
let editorChanges: EditorChanges | null = null;
vi.mock('./composables/useAppChrome', () => ({
  registerSearchPopover: vi.fn(),
  setEditorChrome: vi.fn(),
  registerTabActions: vi.fn(),
  setEditorChanges: (state: EditorChanges | null) => {
    editorChanges = state;
  },
  registerEditorActions: vi.fn(),
  clearEditorChrome: vi.fn(),
}));

// useTaskReview itself stays REAL - this file tests exactly how the composable
// works together with the view's watches. Only the HTTP layer below is a stub.
const fetchTask = vi.fn();
const resolveTask = vi.fn();
const fetchJobTasks = vi.fn();
const applyEnrichment = vi.fn();
vi.mock('./composables/useTasks', () => ({
  useTaskActions: () => ({
    fetchTask: (...a: unknown[]) => fetchTask(...a),
    resolveTask: (...a: unknown[]) => resolveTask(...a),
    fetchJobTasks: (...a: unknown[]) => fetchJobTasks(...a),
    applyEnrichment: (...a: unknown[]) => applyEnrichment(...a),
    refresh: vi.fn(),
    requestEnrich: vi.fn(),
    running: ref([]),
    tasks: ref([]),
  }),
  useTasks: () => ({ tasks: ref([]), refresh: vi.fn() }),
  usePollWhile: vi.fn(),
}));

import EditorView from './EditorView.vue';

const PROPOSAL_YAML = [
  '$id: kieswet',
  'articles:',
  '  - number: B 5',
  '    text: B5 voorstel uit verrijking',
].join('\n');

function reviewTaskDetail(overrides = {}) {
  return {
    id: 'taak-1',
    task_type: 'job_review',
    status: 'open',
    payload: {
      law_id: 'kieswet',
      article: 'B 5',
      source_etag: 'etag-1',
      yaml_path: 'laws/kieswet.yaml',
    },
    results: [{ path: 'laws/kieswet.yaml', content: PROPOSAL_YAML }],
    ...overrides,
  };
}

// Lets loads from the in-memory corpus and the watches after them run.
async function settle(times = 4) {
  for (let i = 0; i < times; i += 1) {
    await flushPromises();
    await nextTick();
  }
}

// The Tekst pane reads `activeFormats` from its ArticleTextEditor ref for the
// bold/italic control; a bare shallow stub lacks it, so this one publishes it.
const ArticleTextEditorStub = {
  name: 'ArticleTextEditor',
  template: '<div />',
  setup() {
    return { activeFormats: { bold: false, italic: false }, clearHistory: () => {} };
  },
};

/** The state EditorView last published to the changes bar; fails the test when it published none. */
function changes(): EditorChanges {
  if (!editorChanges) throw new Error('EditorView published no editor changes');
  return editorChanges;
}

/** The setup-scope functions these tests call on the mounted view (script setup exposes them on `vm` in tests). */
interface EditorViewInternals {
  editorRouteFor(lawId: string | null, articleNumber: string | null): { query?: unknown };
  handleLawSave(): Promise<void>;
  rejectReview(): Promise<void>;
  editedText: string;
}
const internals = (wrapper: VueWrapper) => wrapper.vm as unknown as EditorViewInternals;

let mounted: VueWrapper | null = null;
function mountEditor() {
  mounted = shallowMount(EditorView, {
    global: {
      stubs: { teleport: true, ArticleTextEditor: ArticleTextEditorStub },
      plugins: [testingPinia({ initialState: { featureFlags: allFlagsOn() } })],
    },
  });
  return mounted;
}

// Also after a failed assertion: a view left mounted keeps listening to the
// shared reactive route and then pollutes the next test.
afterEach(() => {
  mounted?.unmount();
  mounted = null;
});

beforeEach(() => {
  fetchTask.mockReset().mockResolvedValue(reviewTaskDetail());
  resolveTask.mockReset().mockResolvedValue(undefined);
  fetchJobTasks.mockReset();
  applyEnrichment.mockReset().mockResolvedValue({ accepted: 1, total: 1 });
  replaceMock.mockReset();
  pushMock.mockReset();
  save.mockReset();
  openLaw = null;
  editorChanges = null;
  installTestCorpus(
    { '': {}, [TRAJECT]: { laws: { kieswet: SAVED_LAW_YAML, 'andere-wet': OTHER_LAW_YAML }, etags: { kieswet: 'etag-1' } } },
    TRAJECT,
  );
  routeState.params = {
    trajectRef: 'kieswet-kandidaatstelling-86070ac7',
    lawId: 'kieswet',
    articleNumber: 'B 5',
  };
  routeState.query = {};
});

describe('EditorView review mode', () => {
  it('opens the review view on ?task= and seeds the proposal', async () => {
    routeState.query = { task: 'taak-1' };
    mountEditor();
    await settle();

    expect(fetchTask).toHaveBeenCalledWith('taak-1');
    expect(changes().review).toBe(true);
    expect(changes().reviewStatus).toContain('gegenereerd voorstel');
    // The proposal is in the panes as an unsaved change.
    expect(changes().dirty).toBe(true);
  });

  // The bug: an article switch (a tab switch in the editor) does
  // `router.replace(editorRouteFor(...))`, which dropped `?task=`, but the
  // review state stayed. Result: the review view hung over an article the
  // task is not about - including an active "Sla voorstel op" that would
  // approve the task with the content of THAT other article.
  it('closes the review view as soon as ?task= leaves the URL', async () => {
    routeState.query = { task: 'taak-1' };
    mountEditor();
    await settle();
    expect(changes().review).toBe(true);

    // Tab switch to another article: the URL no longer holds the task.
    routeState.params = { ...routeState.params, articleNumber: 'B 1' };
    routeState.query = {};
    law().selectArticle('B 1');
    await settle();

    expect(changes().review).toBe(false);
    expect(changes().reviewStatus).toBeNull();
  });

  // The task travels with navigation: every editor route the view builds (tab,
  // article and law switches) keeps `?task=`, so a detour - or a refresh on
  // the way - does not throw the review away.
  it('keeps ?task= on every editor route while the task is open', async () => {
    routeState.query = { task: 'taak-1' };
    const wrapper = mountEditor();
    await settle();

    expect(internals(wrapper).editorRouteFor('kieswet', 'B 1')).toMatchObject({
      name: 'editor-traject',
      params: { lawId: 'kieswet', articleNumber: 'B 1' },
      query: { task: 'taak-1' },
    });
    // Without a law there is nothing to belong to.
    expect(internals(wrapper).editorRouteFor(null, null).query).toBeUndefined();
  });

  // The dangerous case: the bar hung over another article AND "Sla voorstel
  // op" kept working. It approved the task with the content of THAT article.
  it('turns the review view off on another article, also with ?task= in the URL', async () => {
    routeState.query = { task: 'taak-1' };
    const wrapper = mountEditor();
    await settle();
    expect(changes().review).toBe(true);

    // Tab switch: the task stays in the URL, the view moves.
    routeState.params = { ...routeState.params, articleNumber: 'B 1' };
    law().selectArticle('B 1');
    await settle();

    expect(changes().review).toBe(false);
    expect(changes().reviewStatus).toBeNull();

    // Saving here is an ordinary article save, not an approval.
    await internals(wrapper).handleLawSave();
    await settle();
    expect(save).toHaveBeenCalled(); // the save does happen...
    expect(resolveTask).not.toHaveBeenCalled(); // ...but approves nothing.
  });

  it('restores bar and proposal once you are back on the task article', async () => {
    routeState.query = { task: 'taak-1' };
    mountEditor();
    await settle();

    routeState.params = { ...routeState.params, articleNumber: 'B 1' };
    law().selectArticle('B 1');
    await settle();
    expect(changes().review).toBe(false);

    routeState.params = { ...routeState.params, articleNumber: 'B 5' };
    law().selectArticle('B 5');
    await settle();

    expect(changes().review).toBe(true);
    // The proposal is back as an unsaved change...
    expect(changes().dirty).toBe(true);
    // ...without fetching the task again: it was never gone.
    expect(fetchTask).toHaveBeenCalledTimes(1);
  });

  it('does not jump away when entering with ?task= on another article', async () => {
    // A refresh while looking at B 1: the task travels in the URL, but the
    // URL's article wins - no unasked-for jump to B 5.
    routeState.params = { ...routeState.params, articleNumber: 'B 1' };
    routeState.query = { task: 'taak-1' };
    mountEditor();
    await settle();

    expect(law().selectedArticleNumber.value).toBe('B 1');
    expect(changes().review).toBe(false);

    // And once you go to the task's article, the proposal is there.
    routeState.params = { ...routeState.params, articleNumber: 'B 5' };
    law().selectArticle('B 5');
    await settle();

    expect(changes().review).toBe(true);
    expect(changes().dirty).toBe(true);
  });

  it('does let go of the task on reject', async () => {
    routeState.query = { task: 'taak-1' };
    const wrapper = mountEditor();
    await settle();

    await internals(wrapper).rejectReview();
    await settle();

    // A task without a job_id (its job was deleted) has no enrichment the
    // apply endpoint can find, so rejecting it releases the task itself.
    expect(applyEnrichment).not.toHaveBeenCalled();
    expect(resolveTask).toHaveBeenCalledWith('taak-1', 'rejected');
    // The one route that deliberately does NOT carry the task.
    const lastReplace = replaceMock.mock.calls.at(-1);
    if (!lastReplace) throw new Error('router.replace was not called');
    const target = lastReplace[0];
    expect(target.query).toBeUndefined();
    expect(target.params).toMatchObject({ lawId: 'kieswet' });
  });

  // The reject-confirmation dialog can stay open while the review ends (the
  // task leaves the URL). Confirming it then has no task to decide on: it must
  // do nothing, rather than discard the panes and throw building the next route.
  it('does nothing when a reject is confirmed after the review has ended', async () => {
    routeState.query = { task: 'taak-1' };
    const wrapper = mountEditor();
    await settle();
    expect(changes().review).toBe(true);

    routeState.query = {};
    await settle();
    expect(changes().review).toBe(false);
    replaceMock.mockReset();
    internals(wrapper).editedText = 'een onopgeslagen wijziging';

    await expect(internals(wrapper).rejectReview()).resolves.toBeUndefined();
    await settle();

    expect(replaceMock).not.toHaveBeenCalled();
    expect(applyEnrichment).not.toHaveBeenCalled();
    // The panes keep what the user typed.
    expect(internals(wrapper).editedText).toBe('een onopgeslagen wijziging');
  });

  // Second half of the same bug: back on the task's article the content was
  // gone (the panes were refilled from the saved law) while the review view
  // was still up. When the task does come back in the URL, the proposal must
  // be loaded and seeded again.
  it('loads the task again when you return with ?task=', async () => {
    routeState.query = { task: 'taak-1' };
    mountEditor();
    await settle();
    expect(fetchTask).toHaveBeenCalledTimes(1);

    routeState.params = { ...routeState.params, articleNumber: 'B 1' };
    routeState.query = {};
    law().selectArticle('B 1');
    await settle();
    expect(changes().review).toBe(false);

    // Back to the task's article, through "Beoordeel voorstel" - a route
    // WITH ?task=.
    routeState.params = { ...routeState.params, articleNumber: 'B 5' };
    routeState.query = { task: 'taak-1' };
    law().selectArticle('B 5');
    await settle();

    expect(fetchTask).toHaveBeenCalledTimes(2);
    expect(changes().review).toBe(true);
    expect(changes().dirty).toBe(true);
  });

  // The task travels with navigation, so `?task=` is also in the URL of
  // ANOTHER law. A refresh there loads the task again - and then the proposal
  // must not land in that other law. The article number alone proves nothing:
  // numbers like "B 5" exist in several laws.
  it('does not seed the proposal into another law when entering with ?task=', async () => {
    routeState.params = { ...routeState.params, lawId: 'andere-wet' };
    routeState.query = { task: 'taak-1' };
    mountEditor();
    await settle();

    expect(changes().review).toBe(false);
    // Nothing seeded: the panes still show the other law's saved text.
    expect(changes().dirty).toBe(false);
  });

  // The same trap on return. Switching to a law with the same article number
  // leaves `selectedArticleNumber` as it is - only the law underneath changes.
  // The panes ARE reset to the saved law then, so back on the task the
  // proposal must go in again.
  it('seeds again after a detour to a law with the same article number', async () => {
    routeState.query = { task: 'taak-1' };
    mountEditor();
    await settle();
    expect(changes().review).toBe(true);
    expect(changes().dirty).toBe(true);

    // Law switch, same article number: the review belongs out of view.
    await law().open({ lawId: 'andere-wet', articleNumber: 'B 5' });
    await settle();
    expect(changes().review).toBe(false);
    expect(changes().dirty).toBe(false);

    // Back on the task's law: bar and proposal belong there again.
    await law().open({ lawId: 'kieswet', articleNumber: 'B 5' });
    await settle();
    expect(changes().review).toBe(true);
    expect(changes().dirty).toBe(true);
  });
});

describe('EditorView save-failure notification', () => {
  const notification = (wrapper: VueWrapper) => wrapper.find('[data-testid="save-error-notification"]');

  it('hides on dismiss and shows again for a new failure', async () => {
    const wrapper = mountEditor();
    await settle();
    expect(notification(wrapper).exists()).toBe(false);

    law().saveError.value = new Error('Conflict');
    await settle();
    expect(notification(wrapper).attributes('supporting-text')).toBe('Conflict');

    // The failure outlives the dismiss (it clears on the next save), so the
    // view has to remember that this one was dismissed.
    notification(wrapper).element.dispatchEvent(new CustomEvent('dismiss'));
    await settle();
    expect(notification(wrapper).exists()).toBe(false);

    law().saveError.value = new Error('Netwerkfout');
    await settle();
    expect(notification(wrapper).attributes('supporting-text')).toBe('Netwerkfout');
  });
});
