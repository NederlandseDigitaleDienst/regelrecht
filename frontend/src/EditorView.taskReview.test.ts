// Gerichte tests voor de review-modus-orkestratie in EditorView.vue (de
// `?task=`-watches, `applyProposedContent` en het opruimen daarvan).
// EditorView.vue is een view van ~3000 regels zonder eigen testharnas, dus dit
// bestand mount hem `shallow` met elke composable die hij aanraakt vervangen
// door een stuurbare stub - net als LibraryView.docReview.test.js doet voor de
// werkdocument-review. Bewust smal: alleen het review-oppervlak, de rest van de
// view (panes, notities, scenario's, graaf) blijft in zijn lege standaardstaat.
import { describe, it, expect, vi, beforeEach, afterEach } from 'vitest';
import { shallowMount } from '@vue/test-utils';
import type { VueWrapper } from '@vue/test-utils';
import { ref, computed, reactive, nextTick } from 'vue';
import type { EditorChanges } from './composables/useAppChrome';

// --- vue-router: geen echte router. `route` is reactief, want de hele bug
// hangt aan een watch op `route.query.task`: die moet in een test net zo
// kunnen wisselen als bij een tabwissel in de echte editor.
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

vi.mock('./composables/useFeatureFlags', () => ({
  useFeatureFlags: () => ({ isEnabled: () => true }),
}));

vi.mock('./composables/useTrajects', () => ({
  useTrajects: () => ({
    activeTrajectRef: ref('kieswet-kandidaatstelling-86070ac7'),
    activeTraject: ref({ name: 'Kieswet-Kandidaatstelling' }),
    trajectMissing: ref(false),
  }),
  refreshTrajects: vi.fn(),
}));

// --- useLaw: de wet zoals hij opgeslagen staat. `selectedArticle` volgt
// `selectedArticleNumber`, zodat een artikelwissel in de test hetzelfde
// aanvoelt als in de view.
const SAVED_LAW_YAML = [
  '$id: kieswet',
  'articles:',
  "  - number: 'B 1'",
  '    text: B1 opgeslagen',
  "  - number: 'B 5'",
  '    text: B5 opgeslagen',
].join('\n');
const SAVED_ARTICLES = [
  { number: 'B 1', text: 'B1 opgeslagen', machine_readable: null },
  { number: 'B 5', text: 'B5 opgeslagen', machine_readable: null },
];
// Een tweede wet in hetzelfde traject die tóevallig dezelfde artikelnummers
// gebruikt - doodgewoon ("B 1", "1", "2" bestaan in half de corpus). Precies
// dat maakt hem interessant: bij een wetwissel verandert `selectedArticleNumber`
// dan niet, alleen de wet eronder.
const OTHER_LAW_ARTICLES = [
  { number: 'B 1', text: 'B1 andere wet', machine_readable: null },
  { number: 'B 5', text: 'B5 andere wet', machine_readable: null },
];
const articles = ref(SAVED_ARTICLES);
const lawId = ref('kieswet');
// Wissel van wet zoals `switchLaw` dat doet: de wet eronder verandert, het
// artikelnummer in de URL kan hetzelfde blijven.
function switchToLaw(id: string, lawArticles: typeof SAVED_ARTICLES) {
  lawId.value = id;
  articles.value = lawArticles;
}
const selectedArticleNumber = ref('B 5');
const selectedArticle = computed(
  () => articles.value.find((a) => String(a.number) === String(selectedArticleNumber.value)) ?? null,
);
const loading = ref(false);
const lawError = ref(null);
const currentEtag = ref('etag-1');
const seedFromYaml = vi.fn();
const saveLaw = vi.fn().mockResolvedValue(undefined);
const reloadLaw = vi.fn().mockResolvedValue(undefined);
vi.mock('./composables/useLaw', () => ({
  useLaw: () => ({
    law: ref({ $id: 'kieswet', valid_from: '2025-01-01' }),
    lawId,
    rawYaml: ref(SAVED_LAW_YAML),
    articles,
    lawName: ref('Kieswet'),
    selectedArticle,
    selectedArticleNumber,
    switchLaw: vi.fn().mockResolvedValue(true),
    clearLaw: vi.fn(),
    lawTrajectRef: ref('kieswet-kandidaatstelling-86070ac7'),
    loading,
    error: lawError,
    saving: ref(false),
    saveError: ref(null),
    saveLaw: (...a: unknown[]) => saveLaw(...a),
    reloadLaw: (...a: unknown[]) => reloadLaw(...a),
    seedFromYaml,
    createLaw: vi.fn().mockResolvedValue(undefined),
    currentEtag,
    lastSavedPr: ref(null),
  }),
  fetchLaw: vi.fn(),
}));

vi.mock('./composables/useCorpusLaws', () => ({
  useCorpusLaws: () => ({ displayName: (id: string) => id, refresh: vi.fn() }),
}));

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
  apiFetch: vi.fn().mockResolvedValue({ ok: true, status: 200, json: async () => ({}) }),
  apiFetchJson: vi.fn().mockResolvedValue({}),
  ApiError: class ApiError extends Error {},
}));

// De app-shell rendert de review-balk en de Wijzigingenbalk; EditorView
// publiceert er alleen naartoe. `editorChanges` is dus precies wat de
// gebruiker ziet: `review: true` = de beoordeel-weergave staat op het scherm.
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

// useTaskReview zelf blijft ECHT - dit bestand test juist de samenloop van de
// composable met de watches in de view. Alleen de HTTP-laag eronder is een stub.
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

async function settle(times = 4) {
  for (let i = 0; i < times; i += 1) await nextTick();
}

// De Tekst-pane leest `activeFormats` van zijn ArticleTextEditor-ref voor de
// vet/schuin-control; een kale shallow-stub heeft dat veld niet, dus geven we
// er een die het wél publiceert.
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
    global: { stubs: { teleport: true, ArticleTextEditor: ArticleTextEditorStub } },
  });
  return mounted;
}

// Ook na een gefaalde assertie: een gemount gebleven view blijft meeluisteren
// naar de gedeelde reactieve route en vervuilt dan de volgende test.
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
  seedFromYaml.mockReset();
  saveLaw.mockReset().mockResolvedValue(undefined);
  reloadLaw.mockReset().mockResolvedValue(undefined);
  editorChanges = null;
  switchToLaw('kieswet', SAVED_ARTICLES);
  selectedArticleNumber.value = 'B 5';
  loading.value = false;
  lawError.value = null;
  currentEtag.value = 'etag-1';
  routeState.params = {
    trajectRef: 'kieswet-kandidaatstelling-86070ac7',
    lawId: 'kieswet',
    articleNumber: 'B 5',
  };
  routeState.query = {};
});

describe('EditorView review-modus', () => {
  it('opent de beoordeel-weergave bij ?task= en seedt het voorstel', async () => {
    routeState.query = { task: 'taak-1' };
    mountEditor();
    await settle();

    expect(fetchTask).toHaveBeenCalledWith('taak-1');
    expect(changes().review).toBe(true);
    expect(changes().reviewStatus).toContain('gegenereerd voorstel');
    // Het voorstel staat als niet-opgeslagen wijziging in de panes.
    expect(changes().dirty).toBe(true);
  });

  // De bug: een artikelwissel (tabwissel in de editor) doet
  // `router.replace(editorRouteFor(...))` en dat laat `?task=` vallen, maar de
  // review-state bleef staan. Resultaat: de beoordeel-weergave hing over een
  // artikel waar de taak niet over gaat - inclusief een actieve "Sla voorstel
  // op" die de taak zou goedkeuren met de inhoud van dát andere artikel.
  it('sluit de beoordeel-weergave zodra ?task= uit de URL verdwijnt', async () => {
    routeState.query = { task: 'taak-1' };
    mountEditor();
    await settle();
    expect(changes().review).toBe(true);

    // Tabwissel naar een ander artikel: de URL houdt geen taak meer vast.
    routeState.params = { ...routeState.params, articleNumber: 'B 1' };
    routeState.query = {};
    selectedArticleNumber.value = 'B 1';
    await settle();

    expect(changes().review).toBe(false);
    expect(changes().reviewStatus).toBeNull();
  });

  // De taak reist mee met de navigatie: elke editor-route die de view bouwt
  // (tabwissel, artikelwissel, wetwissel) houdt `?task=` vast, zodat een
  // uitstapje - of een refresh onderweg - de beoordeling niet weggooit.
  it('houdt ?task= vast op elke editor-route zolang de taak open staat', async () => {
    routeState.query = { task: 'taak-1' };
    const wrapper = mountEditor();
    await settle();

    expect(internals(wrapper).editorRouteFor('kieswet', 'B 1')).toMatchObject({
      name: 'editor-traject',
      params: { lawId: 'kieswet', articleNumber: 'B 1' },
      query: { task: 'taak-1' },
    });
    // Zonder wet is er niets om bij te horen.
    expect(internals(wrapper).editorRouteFor(null, null).query).toBeUndefined();
  });

  // Het gevaarlijke geval: de balk hing over een ander artikel én "Sla voorstel
  // op" bleef werken. Die keurde de taak goed met de inhoud van dát artikel.
  it('zet de beoordeel-weergave uit op een ander artikel, ook met ?task= in de URL', async () => {
    routeState.query = { task: 'taak-1' };
    const wrapper = mountEditor();
    await settle();
    expect(changes().review).toBe(true);

    // Tabwissel: de taak blijft in de URL staan, het zicht verschuift.
    routeState.params = { ...routeState.params, articleNumber: 'B 1' };
    selectedArticleNumber.value = 'B 1';
    await settle();

    expect(changes().review).toBe(false);
    expect(changes().reviewStatus).toBeNull();

    // Opslaan is hier een gewone artikel-save, geen goedkeuring.
    await internals(wrapper).handleLawSave();
    await settle();
    expect(saveLaw).toHaveBeenCalled(); // de save gebeurt wel...
    expect(resolveTask).not.toHaveBeenCalled(); // ...maar keurt niets goed.
  });

  it('zet balk en voorstel terug zodra je terug bent op het artikel van de taak', async () => {
    routeState.query = { task: 'taak-1' };
    mountEditor();
    await settle();

    routeState.params = { ...routeState.params, articleNumber: 'B 1' };
    selectedArticleNumber.value = 'B 1';
    await settle();
    expect(changes().review).toBe(false);

    routeState.params = { ...routeState.params, articleNumber: 'B 5' };
    selectedArticleNumber.value = 'B 5';
    await settle();

    expect(changes().review).toBe(true);
    // Het voorstel staat er weer als niet-opgeslagen wijziging...
    expect(changes().dirty).toBe(true);
    // ...zonder de taak opnieuw op te halen: die was nooit weg.
    expect(fetchTask).toHaveBeenCalledTimes(1);
  });

  it('springt niet weg wanneer je met ?task= op een ander artikel binnenkomt', async () => {
    // Refresh terwijl je even bij B 1 keek: de taak reist mee in de URL, maar
    // het artikel uit de URL wint - geen ongevraagde sprong naar B 5.
    routeState.params = { ...routeState.params, articleNumber: 'B 1' };
    routeState.query = { task: 'taak-1' };
    selectedArticleNumber.value = 'B 1';
    mountEditor();
    await settle();

    expect(selectedArticleNumber.value).toBe('B 1');
    expect(changes().review).toBe(false);

    // En zodra je naar het artikel van de taak gaat, staat het voorstel er.
    routeState.params = { ...routeState.params, articleNumber: 'B 5' };
    selectedArticleNumber.value = 'B 5';
    await settle();

    expect(changes().review).toBe(true);
    expect(changes().dirty).toBe(true);
  });

  it('laat de taak wel los bij verwerpen', async () => {
    routeState.query = { task: 'taak-1' };
    const wrapper = mountEditor();
    await settle();

    await internals(wrapper).rejectReview();
    await settle();

    // A task without a job_id (its job was deleted) has no enrichment the
    // apply endpoint can find, so rejecting it releases the task itself.
    expect(applyEnrichment).not.toHaveBeenCalled();
    expect(resolveTask).toHaveBeenCalledWith('taak-1', 'rejected');
    // De enige route die de taak bewust NIET meeneemt.
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

  // Tweede helft van dezelfde bug: terug op het artikel van de taak was de
  // inhoud weg (de panes zijn opnieuw uit de opgeslagen wet gevuld) terwijl de
  // beoordeel-weergave er nog stond. Komt de taak wél terug in de URL, dan
  // hoort het voorstel opnieuw geladen en geseed te worden.
  it('laadt de taak opnieuw wanneer je met ?task= terugkeert', async () => {
    routeState.query = { task: 'taak-1' };
    mountEditor();
    await settle();
    expect(fetchTask).toHaveBeenCalledTimes(1);

    routeState.params = { ...routeState.params, articleNumber: 'B 1' };
    routeState.query = {};
    selectedArticleNumber.value = 'B 1';
    await settle();
    expect(changes().review).toBe(false);

    // Terug naar het artikel van de taak, via "Beoordeel voorstel" - dat is
    // een route mét ?task=.
    routeState.params = { ...routeState.params, articleNumber: 'B 5' };
    routeState.query = { task: 'taak-1' };
    selectedArticleNumber.value = 'B 5';
    await settle();

    expect(fetchTask).toHaveBeenCalledTimes(2);
    expect(changes().review).toBe(true);
    expect(changes().dirty).toBe(true);
  });

  // De taak reist mee met de navigatie, dus `?task=` staat ook in de URL van een
  // ANDERE wet. Een refresh daar laadt de taak opnieuw - en dan mag het voorstel
  // niet in die vreemde wet landen. Het artikelnummer alleen is geen bewijs dat
  // je goed zit: nummers als "B 5" bestaan in meerdere wetten.
  it('seedt het voorstel niet in een andere wet bij binnenkomen met ?task=', async () => {
    switchToLaw('andere-wet', OTHER_LAW_ARTICLES);
    routeState.query = { task: 'taak-1' };
    mountEditor();
    await settle();

    expect(changes().review).toBe(false);
    // Niets geseed: de panes staan nog op de opgeslagen tekst van de andere wet.
    expect(changes().dirty).toBe(false);
  });

  // Dezelfde valkuil bij terugkeren. Wissel je naar een wet die hetzelfde
  // artikelnummer heeft, dan verandert `selectedArticleNumber` niet - alleen de
  // wet eronder. De panes zijn dan wél teruggezet naar de opgeslagen wet, dus
  // terug op de taak moet het voorstel er opnieuw in.
  it('seedt opnieuw na een uitstapje naar een wet met hetzelfde artikelnummer', async () => {
    routeState.query = { task: 'taak-1' };
    mountEditor();
    await settle();
    expect(changes().review).toBe(true);
    expect(changes().dirty).toBe(true);

    // Wetwissel, zelfde artikelnummer: de beoordeling hoort uit beeld.
    switchToLaw('andere-wet', OTHER_LAW_ARTICLES);
    await settle();
    expect(changes().review).toBe(false);
    expect(changes().dirty).toBe(false);

    // Terug op de wet van de taak: balk én voorstel horen er weer te staan.
    switchToLaw('kieswet', SAVED_ARTICLES);
    await settle();
    expect(changes().review).toBe(true);
    expect(changes().dirty).toBe(true);
  });
});
