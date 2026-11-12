// How EditorView wires review mode in: the changes bar shows it, editor routes
// carry its task, and the save button approves through it. What review mode
// itself does is in composables/useProposalReview.test.ts. This file mounts
// the view `shallow` with every composable it touches replaced by a
// controllable stub, except the open regelwerk (the real useOpenLaw over an
// in-memory corpus) and review mode itself. Deliberately narrow: the rest of
// the view (panes, notes, scenarios, graph) stays in its empty default state.
import { describe, it, expect, vi, beforeEach, afterEach } from 'vitest';
import { flushPromises, shallowMount } from '@vue/test-utils';
import type { VueWrapper } from '@vue/test-utils';
import { ref, reactive, nextTick } from 'vue';
import { useAppChromeStore, type EditorChanges } from './stores/appChrome';
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
// The traject list as loaded, with the traject the route is in.
const LOADED_TRAJECTS = {
  trajects: [{
    id: '00000000-0000-0000-0000-000086070ac7',
    name: 'Kieswet-Kandidaatstelling',
    description: '',
    scope: '',
    status: 'bezig',
    role: 'owner',
    ref: TRAJECT,
  }],
  loading: false,
  error: null,
};

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
// publishes to them through the chrome store. So the store's `editorChanges`
// is exactly what the user sees: `review: true` = the review view is on
// screen. The store's actions run for real (see mountEditor).

// useProposalReview stays real; only the task API below is a stub.
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
  const { editorChanges } = useAppChromeStore();
  if (!editorChanges) throw new Error('EditorView published no editor changes');
  return editorChanges;
}

/** The setup-scope functions these tests call on the mounted view (script setup exposes them on `vm` in tests). */
interface EditorViewInternals {
  editorRouteFor(lawId: string | null, articleNumber: string | null): { query?: unknown };
  handleLawSave(): Promise<void>;
}
const internals = (wrapper: VueWrapper) => wrapper.vm as unknown as EditorViewInternals;

let mounted: VueWrapper | null = null;
function mountEditor() {
  mounted = shallowMount(EditorView, {
    global: {
      stubs: { teleport: true, ArticleTextEditor: ArticleTextEditorStub },
      // Real actions: the test reads what EditorView publishes to the chrome store.
      plugins: [testingPinia({ initialState: { featureFlags: allFlagsOn(), trajects: LOADED_TRAJECTS }, stubActions: false })],
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
  installTestCorpus(
    { '': {}, [TRAJECT]: { laws: { kieswet: SAVED_LAW_YAML }, etags: { kieswet: 'etag-1' } } },
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

  it('approves the proposal with the save button while the review is on', async () => {
    fetchTask.mockResolvedValue(reviewTaskDetail({ job_id: 'job-1' }));
    routeState.query = { task: 'taak-1' };
    const wrapper = mountEditor();
    await settle();

    // The task is the whole enrichment (its parts are not fetched here), so
    // this verdict is the last one and the enrichment is written right away.
    await internals(wrapper).handleLawSave();
    await settle();
    expect(save).not.toHaveBeenCalled();
    expect(applyEnrichment).toHaveBeenCalledWith(
      'job-1',
      [{ task_id: 'taak-1', action: 'approved', content: expect.stringContaining('B5 voorstel uit verrijking') }],
      'etag-1',
    );
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

  // Writing an enrichment that fails is the same event for the user (my change
  // did not get in), so it shows in the same notification, and dismissing it
  // clears the review's error.
  it('shows a failed enrichment write, and dismissing it clears the review error', async () => {
    fetchTask.mockResolvedValue(reviewTaskDetail({ job_id: 'job-1' }));
    applyEnrichment.mockRejectedValue(new Error('Het regelwerk is intussen gewijzigd.'));
    routeState.query = { task: 'taak-1' };
    const wrapper = mountEditor();
    await settle();

    await internals(wrapper).handleLawSave();
    await settle();
    expect(notification(wrapper).attributes('supporting-text')).toBe('Het regelwerk is intussen gewijzigd.');

    notification(wrapper).element.dispatchEvent(new CustomEvent('dismiss'));
    await settle();
    expect(notification(wrapper).exists()).toBe(false);
  });
});
