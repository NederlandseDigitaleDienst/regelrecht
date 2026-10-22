<script setup lang="ts">
import { ref, computed, shallowRef, nextTick, watch, watchEffect, onBeforeUnmount, inject } from 'vue';
import { useRoute, useRouter, onBeforeRouteUpdate, onBeforeRouteLeave } from 'vue-router';
import type { RouteLocationNormalized } from 'vue-router';
import * as yaml from 'js-yaml';
import type { NLDDButton } from '@nldd/design-system/button';
import type { NLDDModalDialog } from '@nldd/design-system/modal-dialog';
import type { NLDDSearchField } from '@nldd/design-system/search-field';
import type { NLDDTabBarItem } from '@nldd/design-system/tab-bar';
import ArticleText from './components/ArticleText.vue';
import MachineReadable from './components/MachineReadable.vue';
import YamlView from './components/YamlView.vue';
import ActionSheet from './components/ActionSheet.vue';
import SearchPopover from './components/SearchPopover.vue';
import AddLawSheet from './components/AddLawSheet.vue';
import DocumentList from './components/DocumentList.vue';
import DocumentEditor from './components/DocumentEditor.vue';
import TrajectDetailsPane from './components/TrajectDetailsPane.vue';
import TrajectStructureSheet from './components/TrajectStructureSheet.vue';
import TrajectMembersPane from './components/TrajectMembersPane.vue';
import InviteMembersSheet from './components/InviteMembersSheet.vue';
import TasksCategoriesPane from './components/TasksCategoriesPane.vue';
import TasksListPane from './components/TasksListPane.vue';
import TasksSidebarItem from './components/TasksSidebarItem.vue';
import UploadConfirmDialog from './components/UploadConfirmDialog.vue';
import { useAuth, ensureAuthReady } from './composables/useAuth';
import {
  readLocalFavorites,
  toggleLocalFavorite,
  moveLocalFavoritesToAccount,
} from './composables/useLocalFavorites';
import { useCorpusLaws } from './composables/useCorpusLaws';
// useEnrichState (bovenop useTaskActions), niet useTasks: LibraryView heeft
// alleen gerichte refreshes nodig - na een geannuleerde job, na een eigen
// aanvraag - en mag daarvoor niet de 30s-poll aanzetten (die hoort bij de
// taken-componenten, en anonieme bezoekers pollen nooit).
import { useEnrichState } from './composables/useEnrichState';
import { lawFetchInit } from './composables/useLaw';
import { useTrajects, refreshTrajects } from './composables/useTrajects';
import { lawsListUrl, lawUrl, lawUploadUrl, changedLawsUrl, favoritesUrl, favoriteUrl } from './composables/corpusUrls';
import { SUPPORT_EMAIL, paneChromeVisible } from './constants';
import { registerSearchPopover, setLibraryEmpty, openSearch } from './composables/useAppChrome';
import { homeTarget } from './composables/useLastVisitedRoute';
import { useDocumentsManager } from './composables/useDocumentsManager';
import { useTrajectDocumentJobs } from './composables/useTrajectDocumentJobs';
import { useDocumentUpload } from './composables/useDocumentUpload';
import { useAddActions } from './composables/useAddActions';
import { useDocumentTaskReview } from './composables/useDocumentTaskReview';
import { humanizeLawId } from './lib/lawName';
import { sameTraject } from './lib/taskReview';
import { apiFetch, apiFetchJson, ApiError } from './lib/apiFetch';
import { uploadMultipart } from './lib/uploadMultipart';
import { useLatest } from './lib/useLatest';
import { holdRetryFloor, RETRY_MIN_SPINNER_MS } from './lib/retryFeedback';
import { singleParam } from './lib/routeParams';
import type { RequestError } from './lib/apiFetch';
import type { ExecutionAction, LawArticle, LawDocument } from './lib/lawFormat';
import type { CorpusLaw } from './composables/useCorpusLaws';
import type { RunningTaskJob } from './composables/useTasks';
import type { UploadResult } from './lib/uploadMultipart';

/** What TasksCategoriesPane emits on `select` (its own key names). */
interface CategorySelection {
  categorie: string;
  lawId: string | null;
}

/** A "Recent bekeken" entry as stored in localStorage. */
interface RecentLaw {
  law_id: string;
  name: string;
}

/** One block of the primary sidebar. */
interface SidebarSection {
  key: string;
  title: string;
  laws: CorpusLaw[];
  empty?: boolean;
}

/** A `legal_basis` entry of a rulework (the law format's keys). */
interface LegalBasisEntry {
  law_id: string;
  article?: string | number | null;
  description?: string | null;
}

/** The parsed rulework, with the extra top-level fields this view reads by name. */
interface LibraryLawDocument extends LawDocument {
  law_id?: string;
  legal_basis?: LegalBasisEntry[];
}

/** A paused navigation or an in-view action waiting on the unsaved-changes guard. */
type PendingLeave =
  | { type: 'inview'; run: () => void }
  | { type: 'route'; resolve: (proceed: boolean) => void };

const { authenticated, loading: authLoading } = useAuth();

// Provided by AppShell: shows the login-warning popover anchored to an element,
// so "Bewerken" gates on login the same way the Editor tab does.
const showLoginWarning = inject<((anchorEl: EventTarget | null | undefined, redirectHref?: string) => void) | null>(
  'showLoginWarning',
  null,
);
// Wire on the trigger's @pointerdown.capture so a re-tap on "Bewerken" toggles
// the login warning closed instead of reopening it (see AppShell). No-op default.
const onLoginTriggerPointerdown = inject('onLoginTriggerPointerdown', () => {});
// The detail-pane "Bewerken" button, used as the popover anchor for edit
// actions that don't originate from a click on the button itself.
const editButton = ref<NLDDButton | null>(null);


const route = useRoute();
const router = useRouter();

// Active traject (null = global browse). Derived from the URL via
// `route.params.trajectRef`, so the new `library-traject` route makes the
// bibliotheek traject-aware without any extra plumbing.
const { activeTrajectRef, activeTraject } = useTrajects();

// Label of the back-button that returns to the primary sidebar from underlying
// pages: the name of the tab you are in. A back-button reads more naturally
// as the tab than as the traject's own name (which the heading shows).
const LIBRARY_HOME_BACK_TEXT = computed(() => (activeTrajectRef.value ? 'Traject' : 'Corpus juris'));

// Primary-sidebar heading: the active traject's name, or 'Corpus juris' for the
// public/global corpus (logged-out, or logged-in without a chosen traject).
// Replaces the old fixed 'RegelRecht' now that Home is traject-aware.
const sidebarTitle = computed(() =>
  activeTrajectRef.value ? activeTraject.value?.name || 'Traject…' : 'Corpus juris',
);
// Laadtekst die aansluit op wat er stráks in de kop staat: binnen een traject
// laden we dat traject, daarbuiten de globale corpus. Gekoppeld aan
// activeTrajectRef (niet aan ingelogd-zijn): ingelogd zonder gekozen traject is
// nog steeds de Corpus juris-scope, en de kop zegt dat ook.
const sidebarLoadingText = computed(() =>
  activeTrajectRef.value ? 'Traject laden' : 'Corpus juris laden',
);

// --- Werkdocumenten (folded into Home) ----------------------------------
// The active traject's werkdocumenten live inside Home: a "Werkdocumenten"
// entry in the primary sidebar opens the document list in the secondary sidebar
// and the editor in main (route `werkdocumenten-traject`). Ported from the old
// standalone WerkdocumentenView; DocumentEditor self-contains the rename / save
// / delete / conflict UI, so LibraryView only wires the manager, list and the
// unsaved-changes guard.
// Werkdocumenten upload (ported from main #918): file picker -> server-side
// markdown conversion -> poll the conversion jobs. Created before the manager
// so it can reserve converting target names against create/rename collisions.
const docJobs = useTrajectDocumentJobs(activeTrajectRef);
const { jobs: conversionJobs, cancelJob: cancelConversionJob } = docJobs;
// Verrijken kan ook vanuit de corpusweergave, met dezelfde twee knoppen als in
// een traject. De knop staat er dus altijd zodra er een wet is; ontbreekt de
// login of het traject, dan routeert hij net als "Bewerken" ernaast in plaats
// van te verdwijnen. Zonder traject is er namelijk niets om het voorstel ín te
// landen: het endpoint hangt onder /api/trajects/{ref}/.
const canEnrich = computed(() => !!selectedLawId.value);

// Wat er nog ontbreekt voordat verrijken hier kan. Loopt gelijk op met de gate
// in `enrichSelectedLaw`, zodat de tekst boven de knoppen belooft wat de klik
// daarna doet. De lege staat gebruikt het voor de uitleg en voor de vorm van
// het knoplabel.
const enrichNeeds = computed(() => {
  if (!authenticated.value) return 'login';
  if (!activeTrajectRef.value) return 'traject';
  return '';
});

const enrichError = ref('');

async function enrichSelectedLaw(anchorEl: EventTarget | null) {
  if (!selectedLawId.value) return;
  if (!authenticated.value || !activeTrajectRef.value) {
    // openEditor regelt beide: login-popover op deze knop, of de editor-route
    // die de trajectkiezer toont.
    openEditor(anchorEl);
    return;
  }
  enrichError.value = '';
  try {
    // requestEnrich ververst zelf de takenlijst, dus de pane staat daarna in de
    // bezig-staat. alreadyRunning heeft daarom geen eigen melding nodig: dat is
    // precies wat de pane dan toont.
    const { alreadyRunning, tooMany } = await requestEnrich();
    if (tooMany) enrichError.value = 'Je hebt te veel verrijkingen tegelijk lopen.';
    else if (!alreadyRunning) enrichError.value = '';
  } catch {
    enrichError.value = 'Verrijking aanvragen mislukt.';
  }
}

const docsMgr = useDocumentsManager(
  activeTrajectRef,
  // A job without a target path reserves nothing: the manager only asks
  // whether a given string path is reserved, so leaving the nulls out matches.
  () => conversionJobs.value.map((j) => j.target_path).filter((p): p is string => p !== null),
);
const {
  documents: docList,
  listLoading: docsLoading,
  listError: docsError,
  currentPath: openDocPath,
  hasChanges: docHasChanges,
  saving: docSaving,
  open: openDoc,
  startNew: startNewDoc,
  close: closeDoc,
  refreshList: refreshDocList,
} = docsMgr;

const isWerkdocMode = computed(() => route.name === 'werkdocumenten-traject');
const hasOpenDoc = computed(() => !!openDocPath.value);
// A conversion job has no .md to open yet; clicking its row "opens" it in the
// main pane — a loading state while running, a failure + retry when failed.
// This holds that job's target path; viewedJob resolves it to the job.
const viewingJobPath = ref<string | null>(null);
const viewedJob = computed(() => conversionJobs.value.find((j) => j.target_path === viewingJobPath.value));
// A failure never stays in the polled jobs: the endpoint returns
// pending/processing only (failures surface as a job_failed task instead - see
// list_traject_document_jobs, which the editor-api always calls with
// include_failed = false since #939 dropped the tasks.job_review flag), so the
// job simply vanishes and `viewedJob` goes undefined. Set by the jobs watcher
// when a viewed job disappeared without leaving its .md behind. Resets whenever
// the viewed path changes, so it can't leak onto the next job.
const viewingJobFailed = ref(false);
watch(viewingJobPath, () => { viewingJobFailed.value = false; });
const viewedJobFailed = computed(() => viewingJobFailed.value || viewedJob.value?.status === 'failed');
// Highlight either the open document or the conversion job being viewed.
const docSelectedPath = computed(() => openDocPath.value || viewingJobPath.value);
// When conversion jobs change: refresh the list so a finished job's .md replaces
// its pending row. If the user is viewing that job, keep the view while the job
// is still listed (running, or failed while the flag keeps failures here); once
// it drops out, open the freshly converted document - but only once the .md is
// actually there, because dropping out means completed OR failed.
watch(conversionJobs, async (now, prev) => {
  // Any job leaving the list can have produced a document, so refresh on a
  // departure rather than on a shrinking count - a job leaving and another
  // starting in the same poll keeps the length equal.
  const departed = prev.some((j) => !now.some((n) => n.target_path === j.target_path));
  if (departed) await refreshDocList();
  // Deep-link / refresh: the URL may address a path that only now turns out to be
  // a conversion job (jobs load a tick after the page). Switch from the failing
  // real-doc open to its job view.
  const routed = isWerkdocMode.value && route.params.docPath ? String(route.params.docPath) : null;
  if (routed && viewingJobPath.value !== routed && now.some((j) => j.target_path === routed)) {
    viewingJobPath.value = routed;
    if (openDocPath.value === routed) closeDoc();
  }
  const p = viewingJobPath.value;
  if (!p) return;
  if (now.some((j) => j.target_path === p)) return; // still listed — keep its view
  // Gone from the polled jobs. Only a written .md proves it completed: a failed
  // conversion drops out too and never wrote one, so opening it would 404 into
  // a generic load error instead of saying it failed.
  if (!docList.value.some((d) => d.path === p)) {
    viewingJobFailed.value = true;
    return;
  }
  viewingJobPath.value = null;
  openDoc(p);
});

// Resolve a werkdoc URL path to a view: a conversion job (running/failed) shows
// its job view (no fetch); anything else opens as a real document. On a fresh
// deep-link the jobs aren't loaded yet, so a job path first opens as a doc and
// the watcher above corrects it once the jobs arrive.
function showWerkdocPath(p: string | null) {
  if (!p) { viewingJobPath.value = null; closeDoc(); return; }
  // Al bewust op de job-weergave gezet (bijv. "Bekijk document" vanuit een
  // lopende taak, of de conversionJobs-watcher): behoud die, ook al is de
  // jobs-lijst nog niet gepolld - anders zou dit 'm als gewoon document openen
  // en de 404/‘mislukt’-staat laten opflitsen. De watcher bevestigt de job
  // zodra de poll landt, of opent de .md zodra de conversie klaar is.
  if (viewingJobPath.value === p) return;
  if (conversionJobs.value.some((j) => j.target_path === p)) {
    viewingJobPath.value = p;
    if (openDocPath.value) closeDoc();
    return;
  }
  viewingJobPath.value = null;
  if (openDocPath.value !== p) openDoc(p);
}
// `confirm: true`: het gekozen bestand wacht in `pendingDocUpload` op de
// bevestigingsdialoog, die vertelt of er AI aan te pas komt en (waar er iets te
// kiezen valt) om toestemming vraagt.
const {
  fileInput: docFileInput,
  uploadError: docUploadError,
  uploadRetryable: docUploadRetryable,
  pendingFile: pendingDocUpload,
  onUpload: onDocUpload,
  onFileChange: onDocFileChange,
  confirmUpload: confirmDocUpload,
  cancelUpload: cancelDocUpload,
} = useDocumentUpload(docsMgr.uploadDocument, (result) => {
  docJobs.refresh();
  // De conversie is ook een wacht-taak. Zonder deze refresh wacht de takenlijst
  // op zijn eigen 30s-poll en staat de taak die je zojuist in gang zette er nog
  // niet in als je er meteen naartoe navigeert.
  refreshTasks();
  // Only called on success; the failure branch of the union has no targetPath.
  const targetPath = result?.ok ? result.targetPath : undefined;
  if (!targetPath) return;
  // Vanuit de takenlijst ("Probeer opnieuw") staan we niet in werkdoc-modus, en
  // `showUploadedJob` zet alleen state die dáár rendert - de gebruiker zou dan
  // op zijn takenlijst blijven staan zonder enig teken dat de upload aankwam.
  // Navigeer in dat geval naar de job-weergave, dezelfde bestemming die
  // "Bekijk document" opent.
  if (!isWerkdocMode.value) {
    if (activeTrajectRef.value) {
      router.push({
        name: 'werkdocumenten-traject',
        params: { trajectRef: activeTrajectRef.value, docPath: targetPath },
      });
    }
    return;
  }
  showUploadedJob(targetPath);
}, { confirm: true });
// Poll conversion jobs only while the werkdocumenten sidebar is open.
watch(
  isWerkdocMode,
  (on) => {
    if (on) { docJobs.startPolling(); }
    else { docJobs.stopPolling(); viewingJobPath.value = null; }
  },
  { immediate: true },
);
onBeforeUnmount(() => docJobs.stopPolling());

// Surface an upload failure in a modal (not inline); dismissing clears it.
const uploadErrorModalEl = ref<NLDDModalDialog | null>(null);
watch(docUploadError, async (err) => {
  await nextTick();
  if (err) uploadErrorModalEl.value?.show?.();
  else uploadErrorModalEl.value?.hide?.();
});
function dismissUploadError() {
  docUploadError.value = null;
}

// Same modal treatment for a failed cancel: the job is still converting, so
// leaving the user back at the list with no word would have the UI claim
// something it didn't do.
const jobCancelError = ref<string | null>(null);
const jobCancelErrorModalEl = ref<NLDDModalDialog | null>(null);
watch(jobCancelError, async (err) => {
  await nextTick();
  if (err) jobCancelErrorModalEl.value?.show?.();
  else jobCancelErrorModalEl.value?.hide?.();
});
function dismissJobCancelError() {
  jobCancelError.value = null;
}
function retryUpload() {
  docUploadError.value = null;
  nextTick(() => onDocUpload());
}

// --- Wet of regel toevoegen uit een geüpload document ----------------------
// De "+" onder de wettenlijst (alleen in een traject): upload een PDF/Word-
// document dat de backend omzet naar een basis-wet en automatisch verrijkt;
// het resultaat komt terug als law_create-taak in het Taken-paneel. De
// multipart-POST en foutclassificatie delen we met de werkdocument-upload
// via `uploadMultipart`.
async function uploadLawDocument(file: File): Promise<UploadResult> {
  if (!activeTrajectRef.value) {
    return { ok: false, error: 'Geen actief traject.', retryable: false };
  }
  return uploadMultipart(lawUploadUrl(activeTrajectRef.value), file);
}
const lawUploadStarted = ref(false);
function dismissLawUploadStarted() {
  lawUploadStarted.value = false;
}
const {
  fileInput: lawFileInput,
  uploadError: lawUploadError,
  uploadRetryable: lawUploadRetryable,
  onUpload: onLawUpload,
  onFileChange: onLawFileChange,
} = useDocumentUpload(uploadLawDocument, () => {
  lawUploadStarted.value = true;
  refreshTasks();
});
// Zelfde modal-behandeling als de werkdocument-upload, met een eigen modal
// zodat de retry-actie de juiste picker heropent.
const lawUploadErrorModalEl = ref<NLDDModalDialog | null>(null);
watch(lawUploadError, async (err) => {
  await nextTick();
  if (err) lawUploadErrorModalEl.value?.show?.();
  else lawUploadErrorModalEl.value?.hide?.();
});
function dismissLawUploadError() {
  lawUploadError.value = null;
}
function retryLawUpload() {
  lawUploadError.value = null;
  nextTick(() => onLawUpload());
}

// --- Wet toevoegen uit het centrale corpus (promote / traject-harvest) -----
// De zoek-flow achter "Zoeken in het centrale corpus…": promoten kopieert de
// wet naar de traject-repo, en voor een niet-gevonden wet start een BWB-id
// een traject-scoped harvest via het taken-mechanisme (AddLawSheet).
const addLawSheetRef = ref<InstanceType<typeof AddLawSheet> | null>(null);
const inviteMembersSheetRef = ref<InstanceType<typeof InviteMembersSheet> | null>(null);
function openAddLawSearch() {
  addLawSheetRef.value?.show();
}

// De universele "Toevoegen"-knop in de header (AppShell) vuurt intenties; hier
// worden ze omgezet in de bestaande acties. LibraryView is altijd gemount binnen
// een traject, dus de tellers worden altijd opgevangen.
const addActions = useAddActions();
watch(() => addActions.addLaw.value, () => openAddLawSearch());
watch(() => addActions.newWerkdoc.value, () => headerAddWerkdoc('new'));
watch(() => addActions.uploadWerkdoc.value, () => headerAddWerkdoc('upload'));
// Alleen de sheet openen, geen navigatie: de InviteMembersSheet leeft hier en is
// altijd bereikbaar. Na een geslaagde invite seint hij membersChanged, waar de
// leden-pane op herlaadt.
watch(() => addActions.inviteMembers.value, () => inviteMembersSheetRef.value?.show());

// Werkdocument nieuw/upload vanuit de header: zorg eerst voor werkdoc-modus (de
// nieuw-document-editor en de file-picker leven daar), voer daarna de actie uit.
async function headerAddWerkdoc(kind: 'new' | 'upload') {
  if (!isWerkdocMode.value) {
    goToWerkdocumenten();
    await nextTick();
  }
  if (kind === 'new') onDocNew();
  else onDocUpload();
}
// Na een geslaagde promote (via de AddLawSheet): index verversen (de wet staat
// nu in de traject-repo) en de wet openen — dezelfde afronding als een
// afgeronde harvest in de globale zoeker (onHarvestAvailable).
async function onLawPromoted(lawId: string) {
  await loadIndex();
  selectLaw(lawId);
}
// Een gestarte traject-harvest is async: bevestig met dezelfde banner-vorm
// als de document-upload dat de voortgang in het Taken-paneel verschijnt.
const lawHarvestStarted = ref(false);
function onTrajectHarvestRequested() {
  lawHarvestStarted.value = true;
}
function dismissLawHarvestStarted() {
  lawHarvestStarted.value = false;
}

// Name the open document in the unsaved-changes guard so it's clear what's at
// risk (falls back to a generic phrasing if the name isn't resolved yet).
const docNavGuardText = computed(() => {
  const name = docsMgr.displayTitle(openDocPath.value);
  return name
    ? `'${name}' heeft wijzigingen die nog niet zijn opgeslagen. Als je verdergaat, gaan ze verloren.`
    : 'Dit document heeft wijzigingen die nog niet zijn opgeslagen. Als je verdergaat, gaan ze verloren.';
});

// Enter the werkdocumenten section (from the primary sidebar / traject menu).
function goToWerkdocumenten() {
  if (!activeTrajectRef.value) return;
  router.push({ name: 'werkdocumenten-traject', params: { trajectRef: activeTrajectRef.value } });
}

// --- Taken (folded into Home) -----------------------------------------------
// The user's taken live inside Home too: a "Taken"-item in the primary sidebar
// (below Instellingen and Werkdocumenten) opens the categories in the secondary
// sidebar (route `taken-traject`), and the chosen category's task list in main -
// the Instellingen shape, not the werkdocumenten one. A task itself still never
// opens in main: "Beoordelen" navigates to the editor or the addressed
// werkdocument.
const isTakenMode = computed(() => route.name === 'taken-traject');
const takenCategorie = computed(() => singleParam(route.params.categorie) || null);
// Only meaningful under `wet`; the route allows the segment either way, so pin
// it here rather than let a stray /taken/alle/kieswet filter anything. Note the
// param is `contextLawId`, not `lawId` - see the route comment: `lawId` would
// make this view open the law as well as filter by it.
const takenLawId = computed(() =>
  takenCategorie.value === 'wet' ? (singleParam(route.params.contextLawId) || null) : null,
);
function goToTaken() {
  if (!activeTrajectRef.value) return;
  router.push({ name: 'taken-traject', params: { trajectRef: activeTrajectRef.value } });
}
// "Annuleer conversie" vanuit de takenlijst. Hergebruikt de foutmodal van de
// job-weergave: een mislukte cancel mag niet stil blijven, want de rij blijft
// dan staan terwijl de UI doet alsof er iets gebeurde. `useTasks` merkt de
// verdwenen job pas op bij zijn volgende poll (30s), dus refresh meteen.
async function onCancelTaskJob(job: RunningTaskJob) {
  const res = await cancelConversionJob(job.job_id);
  if (!res?.ok) {
    jobCancelError.value = res?.error || 'Annuleren mislukt.';
    return;
  }
  await refreshTasks();
}

// "Bekijk document" vanuit een lopende conversie-taak. Zet viewingJobPath vóór
// de navigatie, zodat de resolutie het pad niet eerst als gewoon document opent
// (de jobs-lijst is bij binnenkomst nog niet gepolld) - dat gaf de flits tussen
// "aan het converteren" en "mislukt". showWerkdocPath respecteert een al
// gezette viewingJobPath, dus de job-weergave staat er meteen; de
// conversionJobs-watcher bevestigt de job zodra de poll landt.
function onViewTaskJob(path: string) {
  if (!activeTrajectRef.value || !path) return;
  viewingJobPath.value = path;
  router.push({ name: 'werkdocumenten-traject', params: { trajectRef: activeTrajectRef.value, docPath: path } });
}

function goToTakenCategorie({ categorie, lawId }: CategorySelection) {
  if (!activeTrajectRef.value) return;
  router.push({
    name: 'taken-traject',
    params: { trajectRef: activeTrajectRef.value, categorie, contextLawId: lawId || undefined },
  });
}
// Titel van de gekozen categorie, voor zowel de top-title-bar als de kop in
// main. Een wet-context draagt de weergavenaam (de route draagt alleen het
// law_id); useCorpusLaws deelt zijn scope-cache met TasksCategoriesPane, dus
// dit is geen tweede fetch.
const { displayName: corpusLawName } = useCorpusLaws(activeTrajectRef);
const TAKEN_TITELS: Record<string, string> = {
  alle: 'Alle taken',
  prioriteit: 'Prioriteit',
  wachten: 'Wachten op',
  werkdocumenten: 'Werkdocumenten',
  // Alleen bereikbaar via een handmatig getypte /taken/wet zonder id; het panel
  // linkt altijd naar één wet.
  wet: 'Regelwerken',
};
const takenTitle = computed(() => {
  if (!takenCategorie.value) return undefined;
  if (takenLawId.value) return corpusLawName(takenLawId.value);
  return TAKEN_TITELS[takenCategorie.value];
});

// --- Instellingen (traject details + leden, folded into Home) ---------------
// De structuurcontrole is dezelfde Instellingen-plek met een sheet erover, dus
// hij telt hier als Instellingen > Algemeen: die pane hoort onder de sheet te
// staan, en het adres is meteen de open-stand van de sheet.
const isStructureCheck = computed(() => route.name === 'traject-structuur-controle');
const isInstellingenMode = computed(
  () => route.name === 'instellingen-traject' || isStructureCheck.value,
);
const instellingenTab = computed(
  () => (isStructureCheck.value ? 'details' : singleParam(route.params.tab) || null),
);

// True on the library routes proper (/trajecten/{ref} and /corpus/…) - i.e.
// none of the sibling non-corpus modes that LibraryView also hosts. Kept as one
// computed so the "no usable content" states below (initial-loading,
// empty-library and, crucially, the index-error takeover) can't drift apart:
// a failed corpus index must only take over the library itself, never /taken,
// /instellingen or /werkdocumenten, which don't read the wettenindex at all.
const isLibraryMode = computed(
  () => !isWerkdocMode.value && !isInstellingenMode.value && !isTakenMode.value,
);
function goToInstellingen(tab?: string) {
  if (!activeTrajectRef.value) return;
  router.push({ name: 'instellingen-traject', params: { trajectRef: activeTrajectRef.value, tab } });
}
// Sluiten vervangt het adres in plaats van er een stap aan toe te voegen: de
// knop pushte al, dus Terug hoort de sheet te sluiten en niet opnieuw te openen.
function closeStructureCheck() {
  if (!activeTrajectRef.value) return;
  router.replace({
    name: 'instellingen-traject',
    params: { trajectRef: activeTrajectRef.value, tab: 'details' },
  });
}

// Deleting or leaving the traject drops your access - go to the public Home
// (Corpus juris) and refresh the traject list.
function onTrajectGone() {
  refreshTrajects();
  router.push({ name: 'home' });
}

// Mirror the open document OR the conversion job being viewed into the URL
// (refresh / bookmark / back), so a converting/failed werkdocument has its own
// address and survives a refresh. Guard the redundant replace the initial open
// would trigger (URL already names it). Only the PATH is normalized: de query
// rijdt mee, anders zou deze mirror een binnenkomende `?task=<id>`
// (document-review-taak) direct weer strippen voordat de review-activatie hem
// heeft kunnen lezen.
watch(docSelectedPath, (p) => {
  if (!isWerkdocMode.value) return;
  const target = {
    name: 'werkdocumenten-traject',
    params: { trajectRef: activeTrajectRef.value, docPath: p || '' },
    query: route.query,
  };
  if (router.resolve(target).href !== route.fullPath) {
    router.replace(target).catch(() => {});
  }
});

// Unsaved-changes guard for in-view document navigation (pick another document,
// "nieuw", back). Mirrors the old WerkdocumentenView.
const docNavGuardEl = ref<NLDDModalDialog | null>(null);
const docEditorEl = ref<InstanceType<typeof DocumentEditor> | null>(null);
// One guard, two triggers, one modal (blijven / opslaan / negeren): in-view
// actions (pick another document, "nieuw", back) queue a `run` callback;
// route-level leaves (pick a law, switch tab/traject, browser back) queue a
// `resolve` for the paused navigation.
let pendingLeave: PendingLeave | null = null; // { type: 'inview', run } | { type: 'route', resolve }
function guardedDocNavigate(run: () => void) {
  if (hasOpenDoc.value && docHasChanges.value) {
    pendingLeave = { type: 'inview', run };
    docNavGuardEl.value?.show?.();
  } else {
    run();
  }
}
// Route guard: true = proceed now, Promise<boolean> = ask first (the modal
// resolves it). Lets the open document's own URL sync (same doc) through.
function guardDirtyDoc(to: RouteLocationNormalized): true | Promise<boolean> {
  // Guard ONLY when a werkdocument is actually open in werkdoc mode AND dirty -
  // i.e. exactly "an open, edited document, navigating away". Never on the
  // document list, never in corpus mode, never on a stale in-memory doc (the
  // manager can keep a left-open doc's state without it being on screen).
  if (!isWerkdocMode.value || !hasOpenDoc.value || !docHasChanges.value) return true;
  if (to.name === 'werkdocumenten-traject'
      && String(to.params.docPath || '') === (openDocPath.value || '')) {
    return true;
  }
  return new Promise<boolean>((resolve) => {
    pendingLeave = { type: 'route', resolve };
    docNavGuardEl.value?.show?.();
  });
}
function resolveDocGuard(proceed: boolean) {
  const p = pendingLeave;
  pendingLeave = null;
  docNavGuardEl.value?.hide?.();
  if (!p) return;
  if (p.type === 'route') p.resolve(proceed);
  else if (proceed) p.run();
}
function cancelDocLeave() { resolveDocGuard(false); }
function confirmDocLeave() {
  // "Negeer wijzigingen en sluit" = truly discard: drop the local draft and
  // revert the body, so reopening the document doesn't resurrect the changes.
  docsMgr.dropDraft();
  resolveDocGuard(true);
}
async function saveDocAndLeave() {
  const ok = await docEditorEl.value?.saveDocument();
  if (!ok) return; // save failed - stay open, DocumentEditor shows the error
  resolveDocGuard(true);
}
function onDocSelect(path: string | null) {
  // A conversion job (running or failed) has no .md to open: show its state in
  // the main pane instead of fetching (which would 404). The conversion watcher
  // opens the real document once a running job finishes.
  if (conversionJobs.value.some((j) => j.target_path === path)) {
    if (viewingJobPath.value === path) return;
    guardedDocNavigate(() => { closeDoc(); viewingJobPath.value = path; });
    return;
  }
  if (path === openDocPath.value) return;
  // A null path is a job row's (DocumentList emits a job's target_path), and
  // took the job branch above; a document row always carries a path.
  guardedDocNavigate(() => { viewingJobPath.value = null; openDoc(path as string); });
}
function onDocNew() {
  guardedDocNavigate(() => { viewingJobPath.value = null; startNewDoc(); });
}
// A fresh upload arrives as a conversion job, not a document - its .md only
// exists once the conversion finishes. Select it anyway, so the main pane shows
// the upload the user just made (converting, then the document itself via the
// jobs watcher, or the failure) instead of leaving them on whatever was open.
// `targetPath` comes from the upload response; only it knows where the
// conversion will land. Same navigation as clicking the row, so same guard.
function showUploadedJob(path: string) {
  guardedDocNavigate(() => { closeDoc(); viewingJobPath.value = path; });
}
// Cancel the conversion the user is currently viewing (from its loading view).
// Clear viewingJobPath first so the conversion watcher doesn't mistake the
// cancelled (vanished) job for a job that failed - it leaves the list either
// way, and without its .md the watcher would otherwise report a failure the
// user caused on purpose.
async function onCancelViewingJob() {
  const p = viewingJobPath.value;
  if (!p) return;
  const job = conversionJobs.value.find((j) => j.target_path === p);
  viewingJobPath.value = null;
  if (!job) return;
  const res = await cancelConversionJob(job.id);
  if (!res?.ok) jobCancelError.value = res?.error || 'Annuleren mislukt.';
}
// Retry a failed conversion the user is viewing: discard the failed attempt and
// reopen the upload picker so they can pick the file again. A cancel that fails
// stops the retry: the old attempt still holds its name (the upload derives a
// collision-free one against reserved paths), so carrying on would quietly file
// the retry under a different name next to the attempt it was meant to replace.
async function onRetryViewingJob() {
  const job = conversionJobs.value.find((j) => j.target_path === viewingJobPath.value);
  viewingJobPath.value = null;
  if (job) {
    const res = await cancelConversionJob(job.id);
    if (!res?.ok) {
      jobCancelError.value = res?.error || 'Annuleren mislukt.';
      return;
    }
  }
  onDocUpload();
}
function onDocBack() {
  // closeDoc nult currentPath; de docSelectedPath-mirror (zie boven) vervangt de
  // URL dan naar de kale lijst. Geen expliciete navigatie nodig - dat zou de
  // mirror alleen dubbelen.
  guardedDocNavigate(() => { viewingJobPath.value = null; closeDoc(); });
}

// --- Review-modus (job_review-taak, payload.kind === 'document') --------
// `?task=<id>` on the werkdocumenten route:
// show a document-conversion job_review task's proposed markdown as an
// unsaved edit on the addressed document, the same way EditorView seeds a
// law-review proposal into the article panes. Mirrors EditorView's
// `useTaskReview` wiring; see the comment on useDocumentTaskReview.js for
// why the fetch/resolve logic lives in the composable while the seeding
// (this component's job, since it owns `docsMgr`) lives here.
const {
  reviewTask: docReviewTask,
  proposedContent: docReviewProposedContent,
  loadError: docReviewLoadError,
  loadReview: loadDocReview,
  approveAfterSave: docReviewApproveAfterSave,
  reject: docReviewRejectInternal,
} = useDocumentTaskReview();
const docReviewActive = computed(() => !!docReviewTask.value);
// Guards against re-firing loadDocReview for a task id already attempted -
// approve/reject null out `docReviewTask`, which would otherwise look
// indistinguishable from "not loaded yet" and re-trigger against the task
// we just resolved.
let docReviewAttemptedForTaskId: string | null = null;

const docReviewTaskIdParam = computed(() =>
  isWerkdocMode.value && typeof route.query.task === 'string' ? route.query.task : null,
);

// Het geadresseerde werkdocument bestaat niet: openDoc's 404-tak liet een
// 'not-found' docError achter (useTrajectDocuments.js). Toon dan een eigen
// centrale melding i.p.v. de kale editor-body. Niet tijdens een review: een
// review wijst normaal naar een nog-niet-bestaand document, en dan is de
// 'not-found' juist het startpunt voor het voorstel (zie de watcher hierboven,
// die 'm wist zodra er een voorstel is). Zolang er nog een ?task= loopt, laten
// we de review-flow het scherm bezitten - mislukt die, dan haalt hij ?task=
// eruit en valt het alsnog hier naartoe.
const docNotFoundActive = computed(() =>
  isWerkdocMode.value
  && docsMgr.docError.value?.kind === 'not-found'
  && !docReviewActive.value
  && !docReviewTaskIdParam.value,
);

// Fires once the addressed document has finished its (possibly 404) open -
// `openDoc(docPath)` (route-driven, see the initial-load/onBeforeRouteUpdate
// wiring below) already sets `currentPath`/`docError` before this can act,
// so this only has to wait for the per-document `docLoading` to clear (not
// `docsLoading`, which tracks the sidebar's document *list* fetch).
watch(
  [docsMgr.docLoading, openDocPath, docReviewTaskIdParam],
  ([isDocLoading, docPath, taskId]) => {
    if (isDocLoading || !docPath || !taskId) return;
    if (docReviewAttemptedForTaskId === taskId) return;
    docReviewAttemptedForTaskId = taskId;
    loadDocReview(taskId).then(() => {
      if (!docReviewProposedContent.value) {
        // Geen voorstel om te seeden. Alleen als de load écht faalde (de
        // composable laat `reviewTask` dan leeg) halen we ?task= uit de URL en
        // laten we de natuurlijke document-staat het overnemen - voor het
        // normaal niet-bestaande doeldocument is dat de 404-melding, geen
        // sticky foutbanner die daarna elk document vervuilt.
        if (!docReviewTask.value) {
          docReviewLoadError.value = null;
          clearDocReviewQuery();
        }
        return;
      }
      // Stale-callback guard: `docPath`/`activeTrajectRef` may have moved on
      // (another document opened, or the traject switched) while the fetch
      // was in flight. Re-check against the task's own payload - a mismatch
      // means this response no longer addresses what's on screen, so leave
      // both the body and any docError alone rather than seeding the wrong
      // document (or wiping a real 'not-found'/'load-error' for it).
      const payload = docReviewTask.value?.payload;
      if (
        payload?.target_path !== openDocPath.value ||
        !sameTraject(payload?.traject_ref, activeTrajectRef.value)
      ) {
        return;
      }
      // The target document doesn't exist yet on the branch (the usual
      // case - a conversion is never pushed) - openDoc's 404 branch left a
      // 'not-found' docError blocking the editor body (see
      // useTrajectDocuments.js); clear it so the proposal renders as a
      // brand-new document instead of a blocking "Document niet gevonden".
      // When the document DOES already exist, docError is left as-is
      // (openDoc already loaded the real savedBody/etag) and the proposal
      // simply overwrites currentBody as a draft-seed, so the existing
      // conflict mechanism (currentEtag as If-Match on save) keeps working.
      if (docsMgr.docError.value?.kind === 'not-found') {
        docsMgr.docError.value = null;
      }
      docsMgr.currentBody.value = docReviewProposedContent.value;
    });
  },
  { immediate: true },
);

// Drop `?task=` from the URL once the review is resolved (approved or
// rejected) so a refresh/back-navigation doesn't re-open review mode. Guarded
// on still being on the werkdocumenten route with a `?task=` query: a
// save-and-leave (saveDocAndLeave below) already lets the user's chosen
// navigation through before this resolves (`onDocSaved` awaits the task
// resolve, which finishes after `resolveDocGuard` has moved the route/doc
// on) - without the guard this `replace` would stomp that navigation and
// drag the user back to the just-approved document.
function clearDocReviewQuery() {
  if (route.name !== 'werkdocumenten-traject' || typeof route.query.task !== 'string') return;
  router.replace({
    name: 'werkdocumenten-traject',
    params: { trajectRef: activeTrajectRef.value, docPath: openDocPath.value || '' },
  });
}

// "Verwerpen" in the review banner: resolve the task as rejected and
// re-fetch the document's real server state (whichever it is - still
// nonexistent, or its unmodified saved body) so the seeded proposal is
// thrown away, mirroring EditorView's `discardArticle()` on reject. Wrapped
// in try/catch (EditorView's law-review reject doesn't need this - it has no
// resolve-failure path exercised in practice - but this one does): a failed
// resolve must NOT be treated as a successful reject, so the seeded proposal
// stays in place and `openDoc`/`clearDocReviewQuery` are skipped entirely -
// otherwise the banner would disappear while the task is still open,
// leaving the user with no way back to it.
async function rejectDocReview() {
  const path = docReviewTask.value?.payload?.target_path;
  // Clear a prior failure so a retry (or a later success) drops the stale
  // critical banner instead of leaving it up after the reject succeeds.
  docReviewLoadError.value = null;
  try {
    await docReviewRejectInternal();
  } catch (e) {
    console.warn('Verwerpen van documentvoorstel mislukt:', e);
    docReviewLoadError.value = 'Verwerpen van het voorstel is mislukt. Probeer het opnieuw.';
    return;
  }
  // Drop the seeded proposal's draft (localStorage + in-memory body) BEFORE
  // re-opening: `openDoc` re-reads the server state, but the debounced draft
  // persistence in useTrajectDocuments already wrote the seeded proposal to
  // localStorage by the time the user clicks Verwerpen - without this,
  // reopening the (still-nonexistent, for the common case) document would
  // resurrect the rejected proposal as a 'draft-present' notice, and a
  // rejected proposal for a document that never existed would leave an
  // orphan draft behind indefinitely.
  docsMgr.dropDraft();
  if (path) await openDoc(path);
  clearDocReviewQuery();
}

// A successful save of the exact document under review IS the approval
// (spec: save first, then resolve) - hooked onto DocumentEditor's 'saved'
// event, its existing save-success point, rather than duplicating the save
// path. `savedPath` is the path DocumentEditor actually just saved (see its
// `emit('saved', savedPath)` call sites) - falls back to `openDocPath.value`
// for safety, though every emitter passes it explicitly. Also gates on the
// traject: `docReviewTask` is a task fetched for a specific traject_ref, so
// a traject switch that happens to land on a document with the same path
// must not resolve a review that belongs to the other traject. Wrapped in
// try/catch: a failed resolve leaves the task open (by design - the assignee
// still has to act on it) without crashing the save flow, which already
// succeeded on the server.
async function onDocSaved(savedPath?: string | null) {
  if (!docReviewActive.value) return;
  const path = savedPath ?? openDocPath.value;
  const payload = docReviewTask.value?.payload;
  if (payload?.target_path !== path || !sameTraject(payload?.traject_ref, activeTrajectRef.value))
    return;
  try {
    await docReviewApproveAfterSave();
  } catch (e) {
    console.warn('Goedkeuren van documentvoorstel mislukt:', e);
    return;
  }
  // A successful approval clears any stale reject-failure banner too.
  docReviewLoadError.value = null;
  clearDocReviewQuery();
}

const docReviewBannerVariant = computed(() => (docReviewLoadError.value ? 'critical' : 'neutral'));
// Hoe het voorstel tot stand kwam. Dit is nadrukkelijk iets anders dan het
// vinkje bij de upload: dat zei wat mócht, dit zegt wat er is gebeurd - alleen
// de pipeline weet dat (een .docx die pandoc niet aankon kan alsnog via het
// taalmodel zijn gegaan). Een oudere taak draagt het veld niet; dan zwijgen we
// erover in plaats van iets te beweren.
const DOC_CONVERSION_ORIGIN: Record<string, string> = {
  llm: 'Omgezet met AI.',
  pandoc: 'Omgezet met pandoc, zonder AI.',
  pdftotext: 'Omgezet met pdftotext, zonder AI.',
};
const docReviewBannerSupportingText = computed(() => {
  if (docReviewLoadError.value) return docReviewLoadError.value;
  // `converted_with` is a document-conversion field the shared payload type does not list.
  const convertedWith = (docReviewTask.value?.payload as { converted_with?: string | null } | null | undefined)
    ?.converted_with;
  const origin = convertedWith ? DOC_CONVERSION_ORIGIN[convertedWith] : undefined;
  const action = 'Opslaan keurt het voorstel goed, Verwerpen wijst af.';
  return origin ? `${origin} ${action}` : action;
});

// Keep the user's traject scope across in-app navigations. A traject with a law
// stays on `library-traject`, a traject without one on `traject-home`; publicly,
// a law drills into `corpus-juris`, otherwise the bare `home`. See homeTarget.
function libraryRouteFor(params: { lawId?: string | null; articleNumber?: string } = {}) {
  return homeTarget({
    trajectRef: activeTrajectRef.value || undefined,
    lawId: params.lawId,
    articleNumber: params.articleNumber,
  });
}
function editorRouteFor(lawIdVal: string | null, articleNumber: string | null | undefined) {
  // Without an active traject the editor isn't reachable directly - the
  // editor requires a traject. Send the user to the chooser, carrying the
  // law as query so it opens right after a traject is picked.
  return activeTrajectRef.value
    ? { name: 'editor-traject', params: { trajectRef: activeTrajectRef.value, lawId: lawIdVal, articleNumber } }
    : { name: 'editor', query: { law: lawIdVal || undefined, article: articleNumber || undefined } };
}

const laws = ref<CorpusLaw[]>([]);
const favorites = ref<Set<string> | null>(null);
// Law ids edited in the active traject (branch-vs-base diff). `null` until
// loaded / when no traject is active; a Set once the endpoint resolves.
const changedLawIds = ref<Set<string> | null>(null);
// De sectie "In dit traject" toont wat dit traject zelf heeft aangeraakt: de
// diff van de trajectbranch tegen zijn base. Bewerken en een wet hierheen
// halen gaan allebei via het writable-own schrijfpad, dus allebei staan ze in
// die diff. Wat de branch van zijn base erfde staat er niet in: dat is niet het
// werk van dit traject.
//
// Boven TRAJECT_LAWS_COLLAPSED wordt het een muur tekst, dus dan de eerste
// twintig met een uitklapknop.
const TRAJECT_LAWS_COLLAPSED = 20;
// Staat de lijst uitgeklapt? Per traject, dus gereset bij een wissel.
const trajectLawsExpanded = ref(false);
// Alfabetisch op weergavenaam. De backend ordent op law_id, en dat is een
// andere string dan wat de gebruiker leest.
const trajectSectionLaws = computed(() => {
  const changed = changedLawIds.value;
  if (!activeTrajectRef.value || !changed?.size) return [];
  return laws.value
    .filter(law => changed.has(law.law_id))
    .sort((a, b) => displayName(a).localeCompare(displayName(b), 'nl'));
});
const loading = ref(true);
// Whatever the failed index load threw (an ApiError, or a network error).
const indexError = ref<unknown>(null);
const searchPopoverRef = ref<InstanceType<typeof SearchPopover> | null>(null);
// The toolbar search control lives in the AppShell; register our popover so
// the shell's search button/field opens it.
registerSearchPopover(searchPopoverRef);

const selectedLawId = ref<string | null>(null);
// Which sidebar section the law was opened from, so only that instance is
// highlighted (a law can sit in both Favorieten and Recent bekeken). Cleared on
// non-sidebar selects (search / deep-link); highlightSection then falls back to
// the law's first occurrence.
const selectedSection = ref<string | null>(null);
const selectedLaw = shallowRef<LibraryLawDocument | null>(null);
const selectedLawLoading = ref(false);
const lawError = ref<RequestError | null>(null);

const selectedArticleNumber = ref<string | null>(null);

// De verrijk-/beoordeel-staat van de geselecteerde wet, gedeeld met EditorView:
// wat de lege Machine-/YAML-pane meldt en waar zijn knoppen heen gaan. Hier en
// niet bovenaan het bestand: de composable legt meteen een watch op zijn bron,
// dus selectedLawId/selectedArticleNumber moeten al bestaan - eerder aanroepen
// gooit een ReferenceError in setup en dan rendert de hele view niets.
const {
  isEnriching,
  reviewReady,
  reviewArticleForPane,
  openReviewForLaw,
  openTasksForLaw,
  requestEnrich,
  refreshTasks,
} = useEnrichState({
  trajectRef: activeTrajectRef,
  lawId: selectedLawId,
  articleNumber: selectedArticleNumber,
});

// Recently-viewed laws (most-recent-first), persisted across sessions. Stored
// as { law_id, name } so a law that fails to load in the active traject - and
// therefore never enters the corpus index - still stays reachable + labelled.
//
// One list per traject, plus one for the global corpus. Two trajects hold
// different laws, so a law you opened in the one has no business turning up
// under "Recent bekeken" in the other - it would point at something that
// corpus may not even have.
const RECENT_LAWS_KEY = 'regelrecht-recent-laws';
const MAX_RECENT_LAWS = 12;
function recentLawsKey(trajectRef: string | null | undefined) {
  return `${RECENT_LAWS_KEY}:${trajectRef || 'corpus'}`;
}
function loadRecentLaws(trajectRef: string | null | undefined): RecentLaw[] {
  try {
    const raw = JSON.parse(localStorage.getItem(recentLawsKey(trajectRef)) || '[]');
    return Array.isArray(raw) ? raw.filter(r => r && r.law_id) : [];
  } catch {
    return [];
  }
}
// The list from before this was scoped mixed every traject together, so there
// is nothing to migrate it into. Drop it instead of leaving it behind forever.
try { localStorage.removeItem(RECENT_LAWS_KEY); } catch { /* ignore */ }
const recentLaws = ref(loadRecentLaws(activeTrajectRef.value));
// Switching traject keeps this view mounted (same route, other param), so the
// list has to follow the switch itself; there is no remount to reload it.
watch(activeTrajectRef, (trajectRef) => {
  recentLaws.value = loadRecentLaws(trajectRef);
});
function recordRecentLaw(lawId: string, name: string | null | undefined) {
  if (!lawId) return;
  const entry = { law_id: lawId, name: name || humanizeLawId(lawId) };
  recentLaws.value = [entry, ...recentLaws.value.filter(r => r.law_id !== lawId)].slice(0, MAX_RECENT_LAWS);
  try {
    localStorage.setItem(recentLawsKey(activeTrajectRef.value), JSON.stringify(recentLaws.value));
  } catch { /* storage unavailable - keep the in-memory list */ }
}
// Detail view (tekst/machine/yaml) is reflected in the URL hash so the
// state is bookmarkable and shareable. English keys in the hash because
// they're stable identifiers, not labels.
const VIEW_TO_HASH: Record<string, string> = { tekst: '#text', machine: '#machine', yaml: '#yaml' };
const HASH_TO_VIEW: Record<string, string> = { '#text': 'tekst', '#machine': 'machine', '#yaml': 'yaml' };

const detailView = computed<string>({
  get() {
    return HASH_TO_VIEW[route.hash] ?? 'tekst';
  },
  set(value: string) {
    // Reject anything we don't recognise rather than silently stripping
    // the hash - every call site today hard-codes a literal, so an
    // unknown value is a programmer error. Warn in dev so a future
    // contributor adding a tab without updating VIEW_TO_HASH catches
    // it immediately; production silently no-ops to avoid console noise.
    const hash = VIEW_TO_HASH[value];
    if (!hash) {
      if (import.meta.env.DEV) {
        console.warn(`[detailView] ignoring unknown value: ${value}`);
      }
      return;
    }
    if (hash !== route.hash) {
      router.replace({ path: route.path, query: route.query, hash });
    }
  },
});
// nldd-tab-bar fires `tabchange` on BOTH pointer click and arrow-key
// activation (content-switching tabs auto-activate on arrow - the ARIA
// pattern). Driving detailView from this single event keeps the keyboard, the
// selected tab, and the visible panel in lockstep; a per-item @click never
// fired on arrow, which is why the highlight moved but the view lagged.
// `:selected` stays the controlled-in binding so a hash-driven detailView
// change (e.g. a deep link to #yaml) still reflects on the tabs.
function onDetailTabChange(e: CustomEvent<{ item?: NLDDTabBarItem } | null>) {
  const view = e.detail?.item?.dataset?.view;
  if (view) detailView.value = view;
}

const activeAction = ref<ExecutionAction | null>(null);

// Curated sidebar sections (in render order). Each entry is
// `{ key, title, laws }`. Empty sections are never pushed, so the template
// can iterate without per-section emptiness checks.
//
//   - "Bewerkt in dit traject" comes first: it's the small, high-signal,
//     context-specific set, so it sits above favorites.
//     Only present when a traject is active and the diff is non-empty.
//   - "Favorieten": the user's personal favorites.
//   - "Recent bekeken": the view history.
//   - "Traject" comes LAST: the full contents of the traject's own repo. It is
//     the longest and least selective of the four, so it belongs at the bottom
//     - but without it a fresh traject, or someone else's, shows an empty menu,
//     because the diff is empty and favorites/recent are personal.
//
// There is deliberately NO full-corpus fallback: the central corpus is the
// full BWB corpus (thousands of laws), so dumping it into the sidebar isn't
// useful and is exactly the "huge pile" we don't want loaded here. When
// nothing is curated yet, the template shows a search CTA instead - full
// browse lives in the search popover.
const sidebarSections = computed(() => {
  const list = laws.value;
  const sections: SidebarSection[] = [];

  // Favorieten eerst. Het is jouw bewuste selectie, en sinds die per traject
  // wordt bewaard hoort hij net zo bij deze plek als de sectie eronder.
  const favs = favorites.value;
  if (favs) {
    const favList = list.filter(law => favs.has(law.law_id));
    if (favList.length > 0) {
      sections.push({ key: 'favorites', title: 'Favorieten', laws: favList });
    }
  }

  // "In dit traject" staat er ook als hij leeg is: de kop hoort bij de plek en
  // eronder staat dan wat je moet doen om hem te vullen. Als sectie in de lijst
  // en niet als los blok, zodat de volgorde op één plek geregeld is.
  if (activeTrajectRef.value) {
    const all = trajectSectionLaws.value;
    if (all.length > 0) {
      const shown = trajectLawsExpanded.value ? all : all.slice(0, TRAJECT_LAWS_COLLAPSED);
      sections.push({ key: 'traject', title: 'In dit traject', laws: shown });
    } else if (!loading.value && changedLawIds.value) {
      // Alleen als de diff echt is opgehaald. `fetchChangedLawIds` slikt elke
      // fout en geeft dan `null`, en dat is iets anders dan een lege set: bij
      // een 500 of een netwerkfout zou "Nog niets bewerkt." een uitspraak doen
      // die de backend juist niet kon doen. Dan valt de sectie weg, precies
      // zoals hij dat vóór deze staat ook deed.
      sections.push({ key: 'traject', title: 'In dit traject', laws: [], empty: true });
    }
  }

  // "Recent bekeken" sits below the curated groups and faithfully shows the
  // view history: a law stays here even when it's also a favorite or edited in
  // this traject, so it can appear in both sections (favoriting a law must not
  // remove it from recent). Each id resolves to its richer index entry when
  // available, otherwise to the stored { law_id, name } (e.g. a law not present
  // in the active traject).
  if (recentLaws.value.length > 0) {
    const recent = recentLaws.value.map(r => indexedLaw(r.law_id) || r);
    sections.push({ key: 'recent', title: 'Recent bekeken', laws: recent });
  }


  return sections;
});

// A law opened from elsewhere (search, deep link) can sit past the collapsed
// slice, leaving the selection with no visible anchor anywhere in the menu.
// Unfold only then: `highlightSection` reads the sections as they are shown,
// so a law that is also starred or recently viewed is already highlighted in
// one of those - expanding the whole list for it would be exactly the wall of
// text the collapsing exists to prevent.
watch([selectedLawId, trajectSectionLaws], ([lawId, all]) => {
  if (!lawId || trajectLawsExpanded.value || !all) return;
  // Reading a computed inside a watcher with explicit sources adds no
  // dependency, so this cannot feed back into the watcher.
  if (highlightSection.value) return;
  if (all.findIndex(law => law.law_id === lawId) >= TRAJECT_LAWS_COLLAPSED) {
    trajectLawsExpanded.value = true;
  }
});

// Label of the collapse toggle under "In dit traject", or `null` when the list
// fits and there is nothing to fold. Counts the laws the section actually
// holds, so the button promises exactly what clicking it reveals.
const trajectExpanderText = computed(() => {
  const total = trajectSectionLaws.value.length;
  if (!activeTrajectRef.value || total <= TRAJECT_LAWS_COLLAPSED) return null;
  return trajectLawsExpanded.value ? 'Toon minder' : `Toon alle ${total}`;
});

// "No usable content" states, shown full-page (like EditorView's no-content
// states) instead of inside the split-view, so the error/CTA spans the full
// width rather than the narrow sidebar. isInitialLoading covers the first load
// before anything resolves; indexError is handled at the same top level.
const isInitialLoading = computed(
  () => loading.value && !selectedLawId.value && sidebarSections.value.length === 0 && isLibraryMode.value,
);

// De paginabrede lege staat vervangt de split-view, dus hij mag alleen aan als
// er echt niets te tonen is. In een traject is dat nooit zo: Instellingen,
// Werkdocumenten en Taken staan altijd in het menu, los van welke wetten er
// zijn. Zonder deze uitzondering krijgt een traject waarin nog niets is
// bewerkt een lege pagina in plaats van zijn eigen menu.
const isEmptyLibrary = computed(
  () => !loading.value && !indexError.value && !selectedLawId.value && sidebarSections.value.length === 0 && !activeTrajectRef.value && isLibraryMode.value,
);


// Longest backend explanation we'll surface verbatim before clamping. The real
// 502 body is the backend's own Dutch sentence ("De bibliotheek van dit traject
// is niet beschikbaar: de traject-repo kon niet worden gescand (…)") with
// GitHub's Trees error interpolated, which comfortably exceeds a few hundred
// chars; the old 300 cap silently dropped the whole thing to the useless
// generic fallback. Clamp-with-ellipsis instead so the reason still reaches the
// user while a runaway body can't flood the dialog.
const INDEX_ERROR_MAX_CHARS = 600;

// Supporting text for the index-error surfaces (fullscreen pane + non-library
// warning banner): prefer the backend's own (Dutch) explanation over the
// generic fallback. The `<`/`{` guard still rejects an HTML/JSON body from an
// intermediary proxy (those start with those chars) so a raw proxy dump never
// leaks; a human-readable sentence with JSON interpolated *inside* it does not
// start with either and comes through, clamped to a clean length.
const indexErrorSupportingText = computed(() => {
  const err = indexError.value;
  const body = err && typeof err === 'object' ? (err as { body?: unknown }).body : undefined;
  if (typeof body === 'string') {
    const text = body.trim();
    if (text && !text.startsWith('<') && !text.startsWith('{')) {
      return text.length <= INDEX_ERROR_MAX_CHARS
        ? text
        : `${text.slice(0, INDEX_ERROR_MAX_CHARS).trimEnd()}…`;
    }
  }
  return 'De gegevens konden niet worden opgehaald.';
});

// Tell the shell whether the library is empty so it can show the just-in-time
// search coach-mark on the toolbar field; reset on unmount so it doesn't linger
// on the editor route.
watchEffect(() => setLibraryEmpty(isEmptyLibrary.value));
onBeforeUnmount(() => setLibraryEmpty(false));

const articles = computed(() => selectedLaw.value?.articles ?? []);

// "Algemeen" rides the existing :articleNumber segment as a reserved value, so
// deep-linking and the browser back button work without a new route. The
// schema types article numbers as free-form strings, so a real article could
// in theory be called 'algemeen' too; when that happens the real article wins
// and the pane is simply not reachable for that law.
const ALGEMEEN_KEY = 'algemeen';
const isAlgemeen = computed(
  () =>
    selectedArticleNumber.value === ALGEMEEN_KEY &&
    !articles.value.some((article) => article.number === ALGEMEEN_KEY),
);

// A header field may be an internal reference like '#wet_naam': the value is
// produced by an action in one of the articles, not written in the header.
function resolveLawReference(value: unknown): unknown {
  if (typeof value !== 'string' || !value.startsWith('#')) return value ?? null;
  const outputName = value.slice(1);
  for (const article of articles.value) {
    const actions = article.machine_readable?.execution?.actions;
    if (!actions) continue;
    for (const action of actions) {
      if (action.output === outputName) return action.value;
    }
  }
  return null;
}

const lawName = computed(() => {
  if (!selectedLaw.value) return '';
  const nameRef = selectedLaw.value.name;
  const resolved = resolveLawReference(nameRef);
  // The YAML is unchecked; a name (or the action output it points at) is a string.
  if (resolved) return resolved as string;
  return humanizeLawId(selectedLaw.value.$id || selectedLaw.value.law_id || '');
});

/**
 * Metadata for a law id from whatever the sidebar has loaded: the `?ids=` set
 * (favorites + edits) first, then the traject's own list.
 *
 * The second lookup matters because the "Traject" section makes laws clickable
 * that are in neither of those id sets. Without it they resolve to
 * `humanizeLawId`, which is wrong for every law whose YAML carries a dynamic
 * `name: '#output_ref'` - the backend resolves those into `display_name`, and
 * the humanized id is a different string entirely.
 */
function indexedLaw(lawId: string): CorpusLaw | null {
  return laws.value.find(l => l.law_id === lawId) ?? null;
}

// Display name resolved from the index. Used in the load-error state where
// `selectedLaw` is null and `lawName` would be empty.
const indexedLawName = computed(() => {
  if (!selectedLawId.value) return '';
  const law = indexedLaw(selectedLawId.value);
  return law ? displayName(law) : humanizeLawId(selectedLawId.value);
});

// Title for the law pane. While loading, `lawName` (derived from the still-loaded
// PREVIOUS law) is stale, so the old title would linger when switching laws;
// `indexedLawName` tracks `selectedLawId` and updates immediately, so use it
// during load and the fully-resolved `lawName` once the new law is in.
const lawTitle = computed(() =>
  selectedLawLoading.value ? indexedLawName.value : (lawName.value || indexedLawName.value),
);

// Track the active law in "Recent bekeken" - including one that fails to load,
// so the sidebar reflects what the user is looking at even when nothing is
// curated yet. Re-runs as the name resolves to upgrade the label from the
// humanized id to the real name.
//
// Switching traject keeps the open law on screen and re-resolves its name, so
// this watch fires again. That is a switch and not a visit: without the guard
// below, every switch prepends the law you were already looking at to the list
// of the corpus you just moved into, and the two lists converge again.
let lastRecorded: { trajectRef: string | null; lawId: string | null } = { trajectRef: null, lawId: null };
watch([selectedLawId, lawName, indexedLawName], () => {
  const lawId = selectedLawId.value;
  if (!lawId) return;
  const trajectRef = activeTrajectRef.value ?? null;
  const switchedContext = lawId === lastRecorded.lawId && trajectRef !== lastRecorded.trajectRef;
  lastRecorded = { trajectRef, lawId };
  if (switchedContext) return;
  recordRecentLaw(lawId, lawName.value || indexedLawName.value);
}, { immediate: true });

const selectedArticle = computed(() => {
  if (!selectedArticleNumber.value || isAlgemeen.value) return null;
  return articles.value.find(
    (a) => String(a.number) === String(selectedArticleNumber.value)
  ) ?? null;
});

// The general-information rows, built from what the corpus document actually
// carries. Fields the law leaves empty are dropped rather than shown blank:
// across the corpus only the first handful is filled everywhere. Identifiers
// that belong to an external source (bwb_id, and the source url) stay out:
// RegelRecht is meant to become the record itself, not a view onto another one.
const REGULATORY_LAYER_LABELS: Record<string, string> = {
  GRONDWET: 'Grondwet',
  WET: 'Wet',
  AMVB: 'Algemene maatregel van bestuur',
  KONINKLIJK_BESLUIT: 'Koninklijk besluit',
  MINISTERIELE_REGELING: 'Ministeriële regeling',
  BELEIDSREGEL: 'Beleidsregel',
  EU_VERORDENING: 'EU-verordening',
  EU_RICHTLIJN: 'EU-richtlijn',
  VERDRAG: 'Verdrag',
  UITVOERINGSBELEID: 'Uitvoeringsbeleid',
  GEMEENTELIJKE_VERORDENING: 'Gemeentelijke verordening',
  PROVINCIALE_VERORDENING: 'Provinciale verordening',
  WATERSCHAPS_VERORDENING: 'Waterschapsverordening',
  REDUCTIE: 'Reductie',
};

function formatLawDate(value: unknown): string | null {
  if (!value || typeof value !== 'string' || value.startsWith('#')) return null;
  // Parse the calendar date by hand: new Date('YYYY-MM-DD') means UTC
  // midnight, and formatting that in a timezone behind UTC renders the day
  // before the legal date. Anchoring both the parse and the formatting to UTC
  // keeps the written date identical in every timezone.
  const match = /^([0-9]{4})-([0-9]{2})-([0-9]{2})$/.exec(value);
  if (!match) return value;
  const parsed = new Date(Date.UTC(Number(match[1]), Number(match[2]) - 1, Number(match[3])));
  if (Number.isNaN(parsed.getTime())) return value;
  return parsed.toLocaleDateString('nl-NL', {
    day: 'numeric',
    month: 'long',
    year: 'numeric',
    timeZone: 'UTC',
  });
}

const algemeenRows = computed(() => {
  const law = selectedLaw.value;
  if (!law) return [];
  const rows = [
    { label: 'Naam', value: lawName.value || indexedLawName.value },
    { label: 'Soort regeling', value: REGULATORY_LAYER_LABELS[law.regulatory_layer as string] || law.regulatory_layer },
    { label: 'Officiële titel', value: law.officiele_titel },
    { label: 'Bevoegd gezag', value: resolveLawReference(law.competent_authority) },
    { label: 'Publicatiedatum', value: formatLawDate(law.publication_date) },
    { label: 'Inwerkingtreding', value: formatLawDate(resolveLawReference(law.valid_from)) },
    { label: 'Geldig tot en met', value: formatLawDate(law.valid_to) },
    { label: 'Gemeentecode', value: law.gemeente_code },
    { label: 'Provinciecode', value: law.provincie_code },
    { label: 'Waterschapscode', value: law.waterschap_code },
    { label: 'CELEX-nummer', value: law.celex_nummer },
    { label: 'ELI', value: law.eli },
    { label: 'Tractatenblad', value: law.tractatenblad_id },
    { label: 'UNTS-nummer', value: law.unts_nummer },
    { label: 'Staatscourant', value: law.stcrt_id },
    { label: 'Organisatie', value: law.organisation },
    { label: 'Aantal artikelen', value: articles.value.length ? String(articles.value.length) : null },
  ];
  return rows.filter((row) => row.value);
});

// `legal_basis` points at other laws in the corpus, so these become links.
const algemeenLegalBasis = computed(() => selectedLaw.value?.legal_basis ?? []);

// True when the URL points at an article that doesn't exist in the
// loaded law. Distinct from "no article selected" (where no article
// number is in the URL).
const articleNotFound = computed(() =>
  !!(selectedLaw.value && selectedArticleNumber.value && !isAlgemeen.value && !selectedArticle.value)
);

// 404 means the law isn't in the active traject's corpus; the error UI shows a traject-specific message.
const lawErrorIs404 = computed(() => lawError.value?.status === 404);

// Reflect navigation depth in the document title, most-specific first so the
// browser tab keeps the deepest level when truncated:
//   "Art. 5 · Wet op de zorgtoeslag · 15 juni test · RegelRecht"
//   "beleidsnota · 15 juni test · RegelRecht"      (open werkdocument)
//   "Werkdocumenten · 15 juni test · RegelRecht"   (werkdoc list, none open)
//   "Leden · 15 juni test · RegelRecht"            (traject-instellingen)
// The traject name is appended on a traject-scoped browse (like the editor) so
// the tab and history show which traject you are in; it is null on the public
// no-traject library, so it drops out there. Bare "RegelRecht" is the fallback
// (the Home landing). We omit the "Bibliotheek:" prefix (unlike the editor) - browsing is
// the implicit default and the leaf name carries enough context.
// Always set (no early return) - router.afterEach used to set a static
// fallback but it raced with this effect on tab/article switches.
watchEffect(() => {
  const detail = [];
  if (isWerkdocMode.value) {
    // Leaf-first with a leading status glyph, so it stays visible when the tab
    // strip truncates: ○ converting, △ failed, ● unsaved edits. Alle drie
    // geometrische markers van gelijke grootte (○/● zijn een ontworpen paar,
    // open vs gevuld), bewust geen ⋯-leesteken ertussen. A viewed conversion job
    // names the tab by its target, an open document by its name, else the section.
    if (viewingJobPath.value) {
      const glyph = viewedJobFailed.value ? '△' : '○';
      detail.push(`${glyph} ${docsMgr.displayTitle(viewingJobPath.value)}`);
    } else if (hasOpenDoc.value && openDocPath.value) {
      detail.push(`${docHasChanges.value ? '● ' : ''}${docsMgr.displayTitle(openDocPath.value)}`);
    } else {
      detail.push('Werkdocumenten');
    }
  } else if (isInstellingenMode.value) {
    detail.push(
      instellingenTab.value === 'leden' ? 'Leden'
        : instellingenTab.value === 'details' ? 'Algemeen'
          : 'Instellingen',
    );
  } else {
    if (selectedArticle.value) detail.push(`Art. ${selectedArticle.value.number}`);
    // Use lawTitle so a law switch shows the new law immediately (lawName is
    // stale mid-load); it also falls back to the index/URL name on a load error.
    const name = lawTitle.value;
    if (name) detail.push(name);
  }
  if (activeTraject.value?.name) detail.push(activeTraject.value.name);
  document.title = detail.length > 0
    ? `${detail.join(' · ')} · RegelRecht`
    : 'RegelRecht';
});

function displayName(law: CorpusLaw): string {
  // Prefer the API's resolved `display_name`: laws can have a dynamic
  // `name: "#output_ref"` in YAML that the backend resolves via the
  // matching action output. Without this check we'd render the raw
  // `#output_ref` string for those laws.
  if (law.display_name) return law.display_name;
  if (law.name) return law.name;
  return humanizeLawId(law.law_id);
}

function articleDescription(article: LawArticle): string {
  if (!article.text) return '';
  // split() always returns at least one element.
  const firstLine = article.text.split('\n')[0]!.replace(/\*\*/g, '');
  return firstLine.length > 80 ? firstLine.slice(0, 80) + '...' : firstLine;
}

// Favorieten zijn per plek: de set van het actieve traject, of die van Corpus
// juris als je nergens in zit. `trajectRef` komt uit de aanroeper zodat een
// laadronde en de toewijzing erna over dezelfde scope gaan.
// Schrijft zodra zijn eigen antwoord binnen is, en niet pas als de traagste van
// de twee ophaalacties klaar is: changed-laws gaat langs GitHub en favorieten
// niet, dus daarop wachten zou de sterren onnodig laat laten verschijnen.
//
// Daarom draagt hij de staleness-check van `loadIndex` zelf. Sinds favorieten
// per traject zijn kunnen twee antwoorden het oneens zijn, en zonder deze
// controle overschrijft een traag antwoord van het vórige traject dat van het
// huidige.
async function loadFavorites(trajectRef: string | null | undefined, isCurrent: () => boolean) {
  // Settle the login state first: before /auth/status answers, `authenticated`
  // reads false, and a logged-in user would briefly see the browser's set.
  if (authLoading.value) {
    await ensureAuthReady();
    if (!isCurrent()) return;
  }
  // Not logged in: Corpus juris favorites live in this browser.
  if (!authenticated.value) {
    favorites.value = trajectRef ? new Set() : readLocalFavorites();
    return;
  }
  try {
    // Logged in: whatever was starred before logging in moves to the account
    // first, so the list below already contains it.
    if (!trajectRef) await moveLocalFavoritesToAccount();
    const favIds = await apiFetchJson<string[]>(favoritesUrl(trajectRef), {
      errorMessage: (status) => `Failed to load favorites: ${status}`,
    });
    if (!isCurrent()) return;
    favorites.value = new Set(favIds);
  } catch (e) {
    // Endpoint unavailable (403, or 503 without a database) - no favorites.
    // Only server errors are worth a console trace.
    if (e instanceof ApiError && e.status >= 500) console.warn(e.message);
  }
}

// Fetch the set of law ids edited in the active traject. Returns `null`
// when there's no traject (global browse has no "changed" notion) or on
// any failure - the "Bewerkt in dit traject" section then simply stays
// hidden instead of surfacing an error in the sidebar. The backend returns
// an empty array (not an error) when nothing has been saved yet, which maps
// to an empty Set and a hidden section all the same.
async function fetchChangedLawIds(trajectRef: string | null | undefined): Promise<Set<string> | null> {
  if (!trajectRef) return null;
  try {
    return new Set(await apiFetchJson<string[]>(changedLawsUrl(trajectRef)));
  } catch {
    // Any failure (HTTP or network) just hides the section - see above.
    return null;
  }
}

// The laws of the traject's OWN source - the one repo it writes to, unique
// per traject and recognisable by `priority === 0` (the same convention
// `useLawPromote` and `AddLawSheet` already use). Federated sources are
// deliberately left out: the central one is the full BWB corpus.
//
// Why not reuse `useCorpusLaws` (already instantiated above, and it does hold
// `source_priority`): its list is the UNFILTERED route, which the backend
// sorts by `law_id` across every source and then truncates at the limit. A
// traject that federates the central BWB corpus can therefore lose its own
// laws past that cap - they simply sort too late - and filtering the result on
// `source_priority === 0` would silently show a partial section. `?source=`
// takes the filtered branch, which orders by source priority precisely so the
// cap cannot starve the own repo (see `list_corpus_laws_in_scope`). The
// `/sources` call stays regardless: it is the only place carrying `law_count`
// (exact past any cap) and `index_error`.
//
// Returns `{ laws, count }`:
//   - `laws: null` means "no section": no traject, no own source, a source
//     that could not be scanned (`index_error` - the existing index-error
//     surface reports that failure), an empty source, or a source past the
//     threshold. In the last case `count` carries the number for the hint.
//   - a fetch failure resolves to the same "no section", without an error in
//     the menu: the same stance as `fetchChangedLawIds`.
const togglingFavorites = ref(new Set<string>());

function onFavoriteClick() {
  toggleFavorite(selectedLawId.value);
}

async function toggleFavorite(lawId: string | null) {
  if (!lawId) return;
  // Not logged in: star in this browser (see useLocalFavorites), then refresh
  // the sidebar so the law appears under Favorieten.
  if (!authenticated.value) {
    favorites.value = toggleLocalFavorite(lawId);
    loadIndex();
    return;
  }
  if (togglingFavorites.value.has(lawId)) return;

  togglingFavorites.value.add(lawId);
  const isFav = favorites.value?.has(lawId);

  // Optimistic update
  const updated = new Set(favorites.value || []);
  if (isFav) updated.delete(lawId);
  else updated.add(lawId);
  favorites.value = updated;

  const revert = () => {
    const reverted = new Set(favorites.value);
    if (isFav) reverted.add(lawId);
    else reverted.delete(lawId);
    favorites.value = reverted;
  };

  try {
    const method = isFav ? 'DELETE' : 'PUT';
    await apiFetch(favoriteUrl(activeTrajectRef.value, lawId), { method });
    // Re-resolve the sidebar's id-set so a newly-favorited law (whose
    // metadata isn't loaded yet, since we only fetch favorites + edits by
    // id) appears in the Favorieten section without a manual reload. The
    // traject's own laws are untouched by starring, so they are not refetched.
    loadIndex();
  } catch {
    // HTTP or network failure - roll the optimistic toggle back.
    revert();
  } finally {
    togglingFavorites.value.delete(lawId);
  }
}

const claimLoadIndex = useLatest();

/**
 * Reload what the sidebar needs.
 */
async function loadIndex() {
  const isCurrent = claimLoadIndex();
  // Drop any previous scope's failure before we start. The error describes one
  // specific index load, so it must not outlive the run that produced it:
  // besides `retryLoadCorpus`, `loadIndex` is also reached from the
  // `activeTrajectRef` watcher, and without this a broken traject's error would
  // ride along to the next traject the user switches to - warning about (or,
  // on the library routes, blocking) a traject whose wetten loaded fine. The
  // `catch` below re-raises it for this run if this load fails too.
  indexError.value = null;
  // Snapshot the traject so the changed-laws fetch and its assignment below
  // both refer to the scope this run started in.
  const trajectRef = activeTrajectRef.value;
  try {
    // Resolve the small id sets the sidebar actually needs: the user's
    // personal favorites and (in a traject) the laws this traject touched on
    // its own branch. Both are id-only.
    const [, changedIds] = await Promise.all([
      loadFavorites(trajectRef, isCurrent),
      fetchChangedLawIds(trajectRef),
    ]);
    if (!isCurrent()) return;
    changedLawIds.value = changedIds;

    // Fetch metadata for just those ids via `?ids=` - never the whole corpus.
    // The central corpus is the full BWB corpus (thousands of laws); loading
    // it here only to filter out a handful would be wasteful and would miss
    // any favorite/edit that sorts past a page cap. Full browse lives in the
    // search popover instead.
    const ids = new Set([...(favorites.value || []), ...(changedIds || [])]);
    if (ids.size === 0) {
      // In een traject is "niets samengesteld" niet hetzelfde als "alles in
      // orde": als de traject-repo niet gescand kan worden weigert de backend
      // de traject-bibliotheek expliciet (428 koppel-flow / 502 met uitleg).
      // Probe die status ook zonder ids, zodat de gebruiker de foutstatus
      // ziet in plaats van een normaal ogende lege bibliotheek.
      if (trajectRef) {
        await apiFetch(lawsListUrl(trajectRef, 'limit=1'), {
          errorMessage: (status) => `Failed to load corpus: ${status}`,
        });
        if (!isCurrent()) return;
      }
      laws.value = [];
      return;
    }
    const query = `ids=${encodeURIComponent([...ids].join(','))}&limit=1000`;
    const res = await apiFetch(lawsListUrl(trajectRef, query), {
      errorMessage: (status) => `Failed to load corpus: ${status}`,
    });
    // Gate before and after json(): skip parsing for stale 200s, and catch races during it.
    if (!isCurrent()) return;
    const corpusLaws = (await res.json()) as CorpusLaw[];
    if (!isCurrent()) return;
    laws.value = corpusLaws.sort((a, b) => a.law_id.localeCompare(b.law_id));
  } catch (e) {
    if (!isCurrent()) return;
    indexError.value = e;
  } finally {
    if (isCurrent()) {
      loading.value = false;
    }
  }
}

const claimLoadLaw = useLatest();

async function loadLaw(lawId: string, { minLoadingMs = 0 }: { minLoadingMs?: number } = {}) {
  const isCurrent = claimLoadLaw();
  const startedAt = Date.now();
  let failed = false;
  try {
    selectedLawLoading.value = true;
    const res = await apiFetch(lawUrl(activeTrajectRef.value, lawId), lawFetchInit);
    // Gate before and after `res.text()`: skip the body read for stale 200s, and catch races during it.
    if (!isCurrent()) return;
    const text = await res.text();
    if (!isCurrent()) return;
    // Unchecked YAML: the types say what the view reads, nothing validates it.
    selectedLaw.value = yaml.load(text) as LibraryLawDocument | null;
    // selectedArticleNumber is set from the route on initial mount and via
    // onBeforeRouteUpdate; we don't validate here so an invalid number
    // surfaces as the articleNotFound error state instead of being silently
    // stripped from the URL.
  } catch (e) {
    if (!isCurrent()) return;
    failed = true;
    selectedLaw.value = null;
    // apiFetch throws an ApiError (with status), a network failure a TypeError,
    // a bad YAML body a YAMLException: all Errors.
    lawError.value = e as RequestError;
  } finally {
    if (isCurrent()) {
      // On a failed retry, hold the spinner briefly so the click reads as
      // feedback instead of the error dialog snapping straight back.
      await holdRetryFloor({ startedAt, minMs: minLoadingMs, failed });
      if (isCurrent()) selectedLawLoading.value = false;
    }
  }
}

function retryLoadLaw() {
  if (!selectedLawId.value) return;
  // No explicit selectedLawLoading.value = true here (unlike
  // retryLoadCorpus → loadIndex): loadLaw sets it as the first
  // statement inside its try block, which runs synchronously before
  // any await yields. The next reactivity flush sees both
  // lawError = null and selectedLawLoading = true together, so the
  // template can't briefly fall through to the "Selecteer een wet"
  // empty state.
  lawError.value = null;
  loadLaw(selectedLawId.value, { minLoadingMs: RETRY_MIN_SPINNER_MS });
}

function retryLoadCorpus() {
  // Clearing indexError is loadIndex's job (it does so for every caller, not
  // just this one). loading is not: loadIndex only flips it back to false in
  // its finally block, it never sets it to true. So after the first failure
  // (loading is false, indexError is truthy) we have to flip the spinner back
  // on here, otherwise the retry shows the error pane until the next
  // round-trip resolves.
  loading.value = true;
  loadIndex();
}

function editInEditor() {
  if (!selectedLawId.value || !selectedArticleNumber.value) return;
  activeAction.value = null;
  if (!authenticated.value) {
    gateEditorLogin(editButton.value);
    return;
  }
  // Carry the active traject so "Bewerken" opens the editable
  // editor-traject view instead of the read-only editor.
  router.push(editorRouteFor(selectedLawId.value, selectedArticleNumber.value));
}

// "Bewerken" button in the detail pane: same traject-aware target as
// editInEditor, exposed as a location + href so the anchor is real (and
// middle-click / open-in-new-tab works) while the click stays SPA.
const editLawTarget = computed(() =>
  editorRouteFor(selectedLawId.value, selectedArticleNumber.value || undefined),
);
const editLawHref = computed(() => router.resolve(editLawTarget.value).href);

// Editor requires login. Instead of letting the route guard bounce an
// unauthenticated user to SSO, show the same login-warning popover the Editor
// tab uses (anchored to `anchorEl`), returning to this article after login.
function gateEditorLogin(anchorEl: EventTarget | null | undefined) {
  showLoginWarning?.(anchorEl, editLawHref.value);
}

// Open the editor for the selected article, gating on login. `anchorEl` is the
// button that was clicked - it anchors the login popover. Shared by "Bewerken"
// and by the Machine/YAML empty-state "aanmaken" button: this pane is
// read-only, so creating a machine-readable version means going to the editor.
function openEditor(anchorEl: EventTarget | null) {
  if (!authenticated.value) {
    gateEditorLogin(anchorEl);
    return;
  }
  router.push(editLawTarget.value);
}

function onEditClick(e: MouseEvent) {
  openEditor(e.currentTarget);
}

function selectLaw(lawId: string, focusAfter = false) {
  // Default to no section context (search / programmatic); selectLawFromSection
  // re-sets it after this call for sidebar clicks.
  selectedSection.value = null;
  if (lawId !== selectedLawId.value || lawError.value) {
    selectedLawId.value = lawId;
    selectedArticleNumber.value = null;
    activeAction.value = null;
    lawError.value = null;
    router.push(libraryRouteFor({ lawId }));
    loadLaw(lawId);
  }

  // When triggered from the search popover we want focus to land on the
  // newly-selected sidebar item - not on the popover trigger that
  // popover._returnFocus restores to. Schedule on nextTick so the popover
  // has fully closed (sync) and Vue has rendered the selected state, then
  // walk the list-item shadow DOM to focus its inner button (the host
  // doesn't delegate focus). Scope by section so a law present in two sections
  // focuses the clicked instance (selectedSection is set by the time this runs).
  if (focusAfter) {
    nextTick(() => {
      const sectionSel = selectedSection.value ? `[data-section="${CSS.escape(selectedSection.value)}"]` : '';
      const item = document.querySelector(`${sectionSel}[data-law-id="${CSS.escape(lawId)}"]`);
      const action = item?.shadowRoot?.querySelector<HTMLElement>('.list-item__action');
      action?.focus?.();
    });
  }
}

// Sidebar click: record which section it came from, then select (keeping the
// focus-restore that recent-reordering needs).
function selectLawFromSection(lawId: string, sectionKey: string) {
  selectLaw(lawId, sectionKey === 'recent');
  selectedSection.value = sectionKey;
}

// The single sidebar instance to highlight: the clicked section (when it still
// holds the law), else the law's first occurrence - so exactly one instance is
// selected, including on a deep-link where no section was clicked.
const highlightSection = computed(() => {
  const id = selectedLawId.value;
  if (!id) return null;
  const sections = sidebarSections.value;
  const clicked = sections.find(s => s.key === selectedSection.value);
  if (clicked?.laws.some(l => l.law_id === id)) return clicked.key;
  return sections.find(s => s.laws.some(l => l.law_id === id))?.key ?? null;
});

// A Grondslag entry points at an article in another law: select that law and
// land on the authorizing article itself, instead of dropping the reader at
// the law's front door to re-find it. A dangling article reference degrades
// to the law's own empty state.
function selectLawArticle(lawId: string, articleNumber: string | number | null | undefined) {
  if (!articleNumber) {
    selectLaw(lawId);
    return;
  }
  const articleStr = String(articleNumber);
  selectedSection.value = null;
  if (lawId !== selectedLawId.value || lawError.value) {
    selectedLawId.value = lawId;
    lawError.value = null;
    loadLaw(lawId);
  }
  selectedArticleNumber.value = articleStr;
  activeAction.value = null;
  router.push(libraryRouteFor({ lawId, articleNumber: articleStr }));
}

function selectArticle(number: string | number) {
  const articleStr = String(number);
  if (articleStr === selectedArticleNumber.value) return;
  selectedArticleNumber.value = articleStr;
  activeAction.value = null;
  router.replace({
    ...libraryRouteFor({ lawId: selectedLawId.value, articleNumber: articleStr }),
    hash: route.hash,
  });
}

function selectAlgemeen() {
  selectArticle(ALGEMEEN_KEY);
}

// Article filter: a toolbar toggle reveals the field, so the list stays
// undisturbed until someone asks for it.
const articleFilterOpen = ref(false);
const articleFilter = ref('');
const articleFilterField = ref<NLDDSearchField | null>(null);

function toggleArticleFilter() {
  articleFilterOpen.value = !articleFilterOpen.value;
  if (articleFilterOpen.value) {
    nextTick(() => articleFilterField.value?.focus?.());
  } else {
    articleFilter.value = '';
  }
}

// Machine-uitvoerbaar = het artikel draagt een uitvoerbare regel, niet alleen
// een definitie: `execution` is wat de engine kan draaien.
function isMachineReadable(article: LawArticle): boolean {
  // The Machine tab renders whatever machine_readable holds, and a machine
  // version with only definitions (no execution yet) is still a machine
  // version - the tag and the filter follow that same notion.
  const mr = article.machine_readable;
  if (!mr) return false;
  return !!mr.execution || Object.keys(mr.definitions ?? {}).length > 0;
}

const ARTICLE_FILTERS = [
  { key: 'alles', label: 'Alles' },
  { key: 'machine', label: 'Met machine-versie' },
  { key: 'geen-machine', label: 'Zonder machine-versie' },
];
const articleTypeFilter = ref('alles');

function setArticleTypeFilter(key: string) {
  articleTypeFilter.value = key;
}

function resetArticleFilters() {
  articleTypeFilter.value = 'alles';
  articleFilter.value = '';
}

// Zeg wát er niets opleverde, niet alleen dát er niets is: de zoekterm en het
// typefilter kunnen allebei de oorzaak zijn en vragen om een ander antwoord.
const articleEmptyText = computed(() => {
  const term = articleFilter.value.trim();
  if (term) return `Geen artikelen gevonden voor "${term}"`;
  if (articleTypeFilter.value === 'machine') return 'Geen machine-uitvoerbare artikelen';
  if (articleTypeFilter.value === 'geen-machine') return 'Alle artikelen zijn machine-uitvoerbaar';
  return 'Geen artikelen';
});

// Alleen het typefilter krijgt een token: een zoekterm staat al zichtbaar in
// het veld, terwijl de filterkeuze anders onzichtbaar in een menu verdwijnt.
const activeTypeFilterLabel = computed(() =>
  articleTypeFilter.value === 'alles'
    ? ''
    : ARTICLE_FILTERS.find((f) => f.key === articleTypeFilter.value)?.label ?? ''
);

const articleFiltersActive = computed(
  () => articleTypeFilter.value !== 'alles' || !!articleFilter.value.trim()
);

// Filters horen bij de wet die je bekijkt, niet bij de sessie: van wet wisselen
// wist ze, ongeacht de route ernaartoe (klik, deeplink of de backknop).
watch(selectedLawId, () => {
  resetArticleFilters();
  articleFilterOpen.value = false;
});

const filteredArticles = computed(() => {
  const term = articleFilter.value.trim().toLowerCase();
  const byType = articles.value.filter((article) => {
    if (articleTypeFilter.value === 'machine') return isMachineReadable(article);
    if (articleTypeFilter.value === 'geen-machine') return !isMachineReadable(article);
    return true;
  });
  if (!term) return byType;
  return byType.filter((article) => {
    const number = `artikel ${article.number}`.toLowerCase();
    const description = (articleDescription(article) || '').toLowerCase();
    return number.includes(term) || description.includes(term) || String(article.text || '').toLowerCase().includes(term);
  });
});

// Web Share is absent on most desktop browsers, so the button only appears
// where the OS can actually take over. Checked once: it cannot change at runtime.
const canShare = typeof navigator !== 'undefined' && typeof navigator.share === 'function';

async function shareLaw() {
  if (!canShare) return;
  try {
    await navigator.share({ title: lawName.value || lawTitle.value, url: window.location.href });
  } catch (err) {
    // An abort is the user closing the sheet, not a failure worth surfacing.
    if ((err as Error | null | undefined)?.name !== 'AbortError') console.warn('Delen is niet gelukt', err);
  }
}

/**
 * Pane back-button handlers - URL-driven so browser back works the same
 * way as clicking the in-pane back button. Pushing the URL one level up
 * lets `onBeforeRouteUpdate` reactively pull the right local state into
 * sync. On sm the navigation-split-view shows the deepest pane with
 * has-content based on those state values.
 *
 * Listening at the nldd-navigation-split-view level (rather than per pane)
 * is more reliable: bubbling always reaches there. We use composedPath
 * to identify which pane the back originated from and route accordingly.
 *
 * `back` is the event fired by nldd-top-title-bar's back-button (not
 * `dismiss` - that's the X-style close button on the right).
 */
function onPaneBack(e: Event) {
  const path = e.composedPath();
  const pane = path.find((el): el is Element => (el as Element).tagName === 'NLDD-SPLIT-VIEW-PANE');
  if (!pane) return;
  const slot = pane.getAttribute('slot');
  // On any error state - corpus load failed (indexError) or this
  // specific law failed (lawError) - back from the main pane should
  // return to the library root, not /library/<lawId>. The latter would
  // route the user back into the same error they just dismissed.
  if (slot === 'main') {
    // Instellingen: back from a tab's content returns to the tab list
    // (bare /instellingen), which collapses to the secondary sidebar on narrow.
    if (isInstellingenMode.value) return goToInstellingen();
    // Taken: same shape, back from a category returns to the category list.
    // Without this the shared fallback below ran, and that routes on a law id
    // that taken mode does not have, so the button did nothing at all.
    if (isTakenMode.value) return goToTaken();
    return (lawError.value || indexError.value) ? goToLibraryRoot() : goToLawRoot();
  }
  if (slot === 'secondary-sidebar') return goToLibraryRoot();
}

function goToLawRoot() {
  if (selectedLawId.value) {
    router.push(libraryRouteFor({ lawId: selectedLawId.value }));
  }
}

function goToLibraryRoot() {
  router.push(libraryRouteFor());
}

function clearRecent() {
  // Deselect only if the open law was reachable *solely* via "Recent bekeken"
  // (not also a favorite / traject edit), so clearing the list doesn't leave a
  // selected-but-invisible law. Then return to the library home.
  const sel = selectedLawId.value;
  const stillShown =
    !!sel && ((favorites.value && favorites.value.has(sel)) ||
              (changedLawIds.value && changedLawIds.value.has(sel)));
  const deselect = !!sel && recentLaws.value.some(r => r.law_id === sel) && !stillShown;
  recentLaws.value = [];
  try { localStorage.removeItem(recentLawsKey(activeTrajectRef.value)); } catch { /* ignore */ }
  if (deselect) {
    // Clear the open law up front so the article sidebar + main reflow to the
    // empty state now. `selectedLawId` is a manual ref, not route-derived; a
    // plain router.push doesn't re-run setup (only a refresh does), so without
    // this the deselected law would linger until reload.
    selectedLawId.value = null;
    selectedLaw.value = null;
    selectedArticleNumber.value = null;
    activeAction.value = null;
    lawError.value = null;
    goToLibraryRoot();
  }
}

// Handle browser back/forward navigation
onBeforeRouteUpdate(async (to) => {
  if (!(await guardDirtyDoc(to))) return false;

  // Werkdocumenten section: open the addressed document (or clear to the list).
  // The corpus state falls through to the no-lawId branch below and is cleared.
  if (to.name === 'werkdocumenten-traject') {
    const p = to.params.docPath ? String(to.params.docPath) : null;
    if (p !== docSelectedPath.value) showWerkdocPath(p);
  }

  const newLawId = singleParam(to.params.lawId);
  const newArticle = singleParam(to.params.articleNumber);

  if (!newLawId) {
    // Navigated to /library with no lawId - clear state. No auto-select:
    // the empty state (Wetten Browser only) is a valid landing view.
    selectedLawId.value = null;
    selectedLaw.value = null;
    selectedArticleNumber.value = null;
    activeAction.value = null;
    lawError.value = null;
  } else if (newLawId !== selectedLawId.value) {
    selectedLawId.value = newLawId;
    selectedArticleNumber.value = null;
    activeAction.value = null;
    lawError.value = null;
    loadLaw(newLawId);
  } else if (newLawId === selectedLawId.value) {
    if (newArticle) {
      const articleStr = String(newArticle);
      if (articleStr !== selectedArticleNumber.value) {
        selectedArticleNumber.value = articleStr;
        activeAction.value = null;
      }
    } else {
      selectedArticleNumber.value = null;
      activeAction.value = null;
    }
  }
});

// Leaving a dirty document by any route change (a law in the sidebar, the tab
// bar, browser back) prompts the same save/discard guard as the in-view actions.
onBeforeRouteLeave(async (to) => {
  if (!(await guardDirtyDoc(to))) return false;
});

// selectedLawId is a manual ref, not route-derived. onBeforeRouteUpdate only
// fires for same-record param changes, so navigating to a no-law route
// (traject-home / corpus home) via the back button or a tab switch - a
// different route record - leaves the ref set and the article list lingers
// until refresh. Sync it reactively so the view reflows on any such navigation.
// Symmetrisch: een wet die uit de route verdwijnt ruimt op, een wet die erin
// verschijnt wordt geopend. Dat tweede stond alleen in de initiële load
// hieronder, en die draait niet opnieuw wanneer je vanuit de takenlijst hierheen
// navigeert: `taken-traject` en `library-traject` worden door dezelfde
// LibraryView gerenderd, dus Vue hergebruikt de instantie. Zonder deze tak bleef
// de view op "Geen selectie" staan terwijl de URL de wet wél noemde.
//
// Niet via selectLaw(): die doet een router.push naar de route waar we al zijn.
watch(() => singleParam(route.params.lawId), (lawId) => {
  if (!lawId) {
    if (selectedLawId.value) {
      selectedLawId.value = null;
      selectedLaw.value = null;
      selectedArticleNumber.value = null;
      activeAction.value = null;
      lawError.value = null;
    }
    return;
  }
  if (lawId === selectedLawId.value && !lawError.value) return;
  selectedSection.value = null;
  selectedLawId.value = lawId;
  selectedArticleNumber.value = route.params.articleNumber
    ? String(route.params.articleNumber)
    : null;
  activeAction.value = null;
  lawError.value = null;
  loadLaw(lawId);
});

// When a harvested law becomes available, reload the corpus and select it.
async function onHarvestAvailable(slug: string) {
  // Best-effort reload - a failure just means the index below may not
  // include the fresh law yet.
  await apiFetch('/api/corpus/reload', {
    method: 'POST',
    headers: { 'Content-Type': 'application/json' },
    body: JSON.stringify({ law_ids: [slug] }),
  }).catch(() => {});
  await loadIndex();
  selectLaw(slug);
}

// Initial load from route
const initialLawId = singleParam(route.params.lawId);
if (initialLawId) {
  selectedLawId.value = initialLawId;
  if (route.params.articleNumber) {
    selectedArticleNumber.value = String(route.params.articleNumber);
  }
  loadLaw(initialLawId);
}
// Werkdocumenten deep-link on first load: restore the addressed document, or a
// converting/failed job's view once the jobs load (the conversionJobs watcher).
if (isWerkdocMode.value && route.params.docPath) {
  showWerkdocPath(String(route.params.docPath));
}
loadIndex();

// The bibliotheek reads through the active traject's corpus
// (`/api/trajects/{ref}/corpus/...`) or the global corpus (`/api/corpus/...`)
// depending on `activeTrajectRef`. When the user switches traject in-place
// (e.g. picking another traject from the TrajectMenu while staying in the
// library, or "Geen traject"), the route param changes but the component
// stays mounted - so refetch the index and the open law through the new
// scope. Mirrors EditorApp's `watch(activeTrajectRef)`.
watch(activeTrajectRef, () => {
  // Drop the previous traject's changed-set immediately so the
  // "Bewerkt in dit traject" section doesn't briefly show stale entries
  // (filtered against the also-stale corpus) while the new index loads.
  // `loadIndex` repopulates it for the new scope, or leaves it null in
  // global browse.
  changedLawIds.value = null;
  // Favorieten zijn nu ook per traject, dus de set van het vorige mag hier niet
  // blijven staan terwijl de nieuwe onderweg is.
  favorites.value = null;
  // Ook de uitklapstand hoort bij het vórige traject: zonder deze reset opent
  // het volgende traject uitgeklapt omdat je hier ooit op de knop drukte.
  trajectLawsExpanded.value = false;
  loadIndex();
  if (selectedLawId.value) {
    lawError.value = null;
    loadLaw(selectedLawId.value);
  }
});
</script>

<template>
        <!-- Buiten de panes: de bestandskiezer wordt vanuit twee modi geopend
             (de werkdocumenten-toolbar en "Probeer opnieuw" in de takenlijst).
             Stond hij in het werkdocumenten-paneel, dan is `docFileInput` in
             taken-modus null en klikt `onUpload()` stilletjes niets aan -
             optional chaining slikt dat zonder een kik. -->
        <!-- No `accept` filter: werkdocumenten accept any format. Markdown is
             stored as-is; every other format is converted server-side (a
             deterministic tool when one fits, otherwise the enricher) and comes
             back as a reviewable markdown werkdocument. -->
        <input ref="docFileInput" type="file" hidden @change="onDocFileChange" />

        <!-- Non-blocking corpus warning for the non-library modes (/taken,
             /instellingen, /werkdocumenten): those don't read the wettenindex,
             so a corpus 502 leaves their panes standing (the fullscreen page
             below is gated to library mode) and they get this banner instead.
             Een notificatie plaatst zichzelf in de gedeelde regio, dus hij
             hangt boven alle panes zonder dat dit component iets over layout
             hoeft te zeggen. `duration="0"` houdt hem staan tot je hem wegdoet:
             de melding beschrijft een toestand, geen gebeurtenis, en wegtellen
             zou de retry-knop met zich meenemen.
             Hij verdwijnt vanzelf zodra een index-load opnieuw begint - een
             retry hier, of een wissel naar een ander traject - en blijft weg
             tenzij die load ook faalt (loadIndex reset indexError vooraf). -->
        <nldd-notification
          v-if="indexError && !isLibraryMode"
          variant="warning"
          duration="0"
          text="De regelwerken van dit traject zijn niet geladen"
          :supporting-text="indexErrorSupportingText"
        >
          <nldd-button slot="actions" appearance="secondary" text="Probeer opnieuw" @click="retryLoadCorpus"></nldd-button>
        </nldd-notification>

        <!-- Full-page "no usable content" states (matching EditorView): shown
             instead of the split-view so the error / CTA spans the full width,
             not the narrow sidebar. The index-error takeover is gated to
             library mode: /taken, /instellingen and /werkdocumenten don't read
             the wettenindex, so a corpus 502 must leave them standing (they get
             the non-blocking warning banner above instead). -->
        <nldd-page v-if="indexError && isLibraryMode">
          <nldd-simple-section width="full">
            <nldd-inline-dialog
              variant="alert"
              text="De regelwerken zijn niet geladen"
              :supporting-text="indexErrorSupportingText"
            >
              <nldd-button slot="actions" appearance="primary" text="Probeer opnieuw" @click="retryLoadCorpus"></nldd-button>
              <nldd-button slot="actions" appearance="secondary" text="Neem contact op via e-mail" :href="`mailto:${SUPPORT_EMAIL}`"></nldd-button>
            </nldd-inline-dialog>
          </nldd-simple-section>
        </nldd-page>

        <nldd-page v-else-if="isInitialLoading">
          <nldd-simple-section width="full">
            <nldd-activity-indicator text="Laden" show-text></nldd-activity-indicator>
          </nldd-simple-section>
        </nldd-page>

        <!-- Nothing curated yet (no favorites, no traject edits, no open law):
             leave the canvas blank - the just-in-time coach-mark on the toolbar
             search field (AppShell) points the user at search. -->
        <nldd-page v-else-if="isEmptyLibrary"></nldd-page>

        <nldd-navigation-split-view v-else @back="onPaneBack">

          <nldd-split-view-pane slot="sidebar" has-content>
            <nldd-page sticky-header>
              <nldd-top-title-bar v-if="!loading" slot="header" :text="sidebarTitle" collapse-anchor="home-titel"></nldd-top-title-bar>

              <nldd-simple-section width="full">
                <nldd-title v-if="!loading" id="home-titel" size="3"><h3>{{ sidebarTitle }}</h3></nldd-title>
                <nldd-spacer v-if="!loading" size="16"></nldd-spacer>
                <nldd-activity-indicator v-if="loading" :text="sidebarLoadingText" show-text></nldd-activity-indicator>
                <template v-else>
                  <!-- Werkdocumenten (in a traject): a single entry that opens
                       the document list in the secondary sidebar + editor in
                       main, mirroring how a law drills into its articles. -->
                  <template v-if="activeTrajectRef">
                    <nldd-list appearance="simple">
                      <nldd-list-item size="md" button :current="isInstellingenMode || undefined" @click="goToInstellingen()">
                        <nldd-icon-cell size="20"><nldd-icon icon="gear"></nldd-icon></nldd-icon-cell>
                        <nldd-spacer-cell size="8"></nldd-spacer-cell>
                        <nldd-text-cell text="Instellingen"></nldd-text-cell>
                        <nldd-spacer-cell size="8"></nldd-spacer-cell>
                        <nldd-icon-cell size="20"><nldd-icon icon="chevron-right"></nldd-icon></nldd-icon-cell>
                      </nldd-list-item>
                      <nldd-list-item size="md" button :current="isWerkdocMode || undefined" @click="goToWerkdocumenten">
                        <nldd-icon-cell size="20"><nldd-icon icon="documents"></nldd-icon></nldd-icon-cell>
                        <nldd-spacer-cell size="8"></nldd-spacer-cell>
                        <nldd-text-cell text="Werkdocumenten"></nldd-text-cell>
                        <nldd-spacer-cell size="8"></nldd-spacer-cell>
                        <nldd-icon-cell size="20"><nldd-icon icon="chevron-right"></nldd-icon></nldd-icon-cell>
                      </nldd-list-item>
                      <TasksSidebarItem :current="isTakenMode" @click="goToTaken" />
                    </nldd-list>
                    <nldd-spacer size="24"></nldd-spacer>
                  </template>
                  <template
                    v-for="(section, sectionIndex) in sidebarSections"
                    :key="section.key"
                  >
                    <!-- Gap above every section after the first, so the
                         curated groups read as distinct blocks. -->
                    <nldd-spacer v-if="sectionIndex > 0" size="24"></nldd-spacer>
                    <template v-if="section.title">
                      <nldd-title size="5">
                        <h4>{{ section.title }}</h4>
                        <nldd-button
                          v-if="section.key === 'recent'"
                          slot="end"
                          size="xs"
                          appearance="accent-transparent"
                          text="Wis"
                          @click="clearRecent"
                        ></nldd-button>
                      </nldd-title>
                      <nldd-spacer size="8"></nldd-spacer>
                    </template>
                    <!-- Nog niets aangeraakt: de kop staat er al, hieronder
                         alleen de mededeling. Uitleg over hóe je hem vult lees
                         je één keer en daarna voor altijd, op de plek waar je
                         werk hoort te staan. -->
                    <nldd-rich-text v-if="section.empty" data-testid="traject-no-laws">
                      <p>Nog niets bewerkt.</p>
                    </nldd-rich-text>
                    <nldd-list v-else appearance="simple">
                      <nldd-list-item
                        v-for="law in section.laws"
                        :key="`${section.key}-${law.law_id}`"
                        size="md"
                        button
                        :data-law-id="law.law_id"
                        :data-section="section.key"
                        :current="(law.law_id === selectedLawId && section.key === highlightSection) || undefined"
                        @click="selectLawFromSection(law.law_id, section.key)"
                      >
                        <nldd-text-cell :text="displayName(law)" :supporting-text="law.source_name">
                        </nldd-text-cell>
                        <nldd-spacer-cell size="8"></nldd-spacer-cell>
                        <nldd-icon-cell size="20">
                          <nldd-icon icon="chevron-right"></nldd-icon>
                        </nldd-icon-cell>
                      </nldd-list-item>
                    </nldd-list>
                    <!-- Uitklapknop onder "In dit traject": die is de enige
                         die lang genoeg wordt om in te vouwen. -->
                    <template v-if="section.key === 'traject' && trajectExpanderText">
                      <nldd-spacer size="8"></nldd-spacer>
                      <nldd-button
                        data-testid="traject-laws-expander"
                        size="xs"
                        appearance="accent-transparent"
                        :text="trajectExpanderText"
                        @click="trajectLawsExpanded = !trajectLawsExpanded"
                      ></nldd-button>
                    </template>
                  </template>
                  <!-- "Regelwerk toevoegen" is verhuisd naar de universele "+" in de
                       header (AppShell); die opent de AddLawSheet via
                       useAddActions. De file-picker voor de upload-route blijft
                       hier, want de upload zelf (onLawFileChange) leeft in deze
                       view. -->
                  <template v-if="activeTrajectRef">
                    <input ref="lawFileInput" type="file" accept=".pdf,.doc,.docx" hidden @change="onLawFileChange" />
                    <template v-if="lawUploadStarted">
                      <nldd-spacer size="8"></nldd-spacer>
                      <nldd-banner
                        variant="success"
                        text="Conversie gestart"
                        supporting-text="Je krijgt een taak zodra het regelwerk klaarstaat voor beoordeling."
                        dismissible
                        @dismiss="dismissLawUploadStarted"
                      ></nldd-banner>
                    </template>
                    <template v-if="lawHarvestStarted">
                      <nldd-spacer size="8"></nldd-spacer>
                      <nldd-banner
                        variant="success"
                        text="Ophalen gestart"
                        supporting-text="De aanvraag staat bij Taken; je krijgt een taak zodra het regelwerk klaarstaat voor beoordeling."
                        dismissible
                        @dismiss="dismissLawHarvestStarted"
                      ></nldd-banner>
                    </template>
                  </template>
                </template>
              </nldd-simple-section>
            </nldd-page>
          </nldd-split-view-pane>

          <!-- Secondary Sidebar (instellingen mode): the settings tabs. -->
          <nldd-split-view-pane v-if="isInstellingenMode" slot="secondary-sidebar" has-content>
            <nldd-page sticky-header>
              <nldd-top-title-bar slot="header" text="Instellingen" :back-text="LIBRARY_HOME_BACK_TEXT" collapse-anchor="instellingen-titel"></nldd-top-title-bar>
              <nldd-simple-section width="full">
                <nldd-title id="instellingen-titel" size="3"><h3>Instellingen</h3></nldd-title>
                <nldd-spacer size="16"></nldd-spacer>
                <nldd-list appearance="simple">
                  <!-- Zelfde iconen als in TrajectMenu/MobileTrajectSheet: deze
                       twee openen dezelfde bestemmingen, dus ze horen er niet
                       anders uit te zien afhankelijk van waar je ze aanklikt. -->
                  <nldd-list-item size="md" button :current="instellingenTab === 'details' || undefined" @click="goToInstellingen('details')">
                    <nldd-icon-cell size="20"><nldd-icon icon="traject"></nldd-icon></nldd-icon-cell>
                    <nldd-spacer-cell size="8"></nldd-spacer-cell>
                    <nldd-text-cell text="Algemeen"></nldd-text-cell>
                    <nldd-spacer-cell size="8"></nldd-spacer-cell>
                    <nldd-icon-cell size="20"><nldd-icon icon="chevron-right"></nldd-icon></nldd-icon-cell>
                  </nldd-list-item>
                  <nldd-list-item size="md" button :current="instellingenTab === 'leden' || undefined" @click="goToInstellingen('leden')">
                    <nldd-icon-cell size="20"><nldd-icon icon="person-2"></nldd-icon></nldd-icon-cell>
                    <nldd-spacer-cell size="8"></nldd-spacer-cell>
                    <nldd-text-cell text="Leden"></nldd-text-cell>
                    <nldd-spacer-cell size="8"></nldd-spacer-cell>
                    <nldd-icon-cell size="20"><nldd-icon icon="chevron-right"></nldd-icon></nldd-icon-cell>
                  </nldd-list-item>
                </nldd-list>
              </nldd-simple-section>
            </nldd-page>
          </nldd-split-view-pane>

          <!-- Secondary Sidebar (taken mode): the categories, mirroring the
               Instellingen panel - the list itself lives in main. -->
          <nldd-split-view-pane v-else-if="isTakenMode" slot="secondary-sidebar" has-content>
            <nldd-page sticky-header>
              <nldd-top-title-bar slot="header" text="Taken" :back-text="LIBRARY_HOME_BACK_TEXT" collapse-anchor="taken-titel"></nldd-top-title-bar>
              <nldd-simple-section width="full">
                <nldd-title id="taken-titel" size="3"><h3>Taken</h3></nldd-title>
                <nldd-spacer size="16"></nldd-spacer>
                <TasksCategoriesPane
                  :traject-ref="activeTrajectRef"
                  :categorie="takenCategorie"
                  :law-id="takenLawId"
                  @select="goToTakenCategorie"
                ></TasksCategoriesPane>
              </nldd-simple-section>
            </nldd-page>
          </nldd-split-view-pane>

          <!-- Secondary Sidebar (werkdoc mode): the document list. -->
          <nldd-split-view-pane v-else-if="isWerkdocMode" slot="secondary-sidebar" has-content>
            <nldd-page sticky-header>
              <nldd-top-title-bar slot="header" text="Werkdocumenten" :back-text="LIBRARY_HOME_BACK_TEXT" collapse-anchor="werkdoc-titel"></nldd-top-title-bar>
              <nldd-simple-section width="full">
                <nldd-title id="werkdoc-titel" size="3"><h3>Werkdocumenten</h3></nldd-title>
                <nldd-spacer size="16"></nldd-spacer>
                <nldd-toolbar accessible-label="Documentacties">
                  <nldd-toolbar-item slot="start">
                    <nldd-icon-button
                      icon="plus-small"
                      text="Document toevoegen"
                      expandable
                      tooltip-timing="never"
                    >
                      <nldd-menu slot="popup">
                        <nldd-menu-item icon="new-text-document" text="Nieuw document" @select="onDocNew"></nldd-menu-item>
                        <nldd-menu-item icon="upload-to-cloud" text="Document uploaden…" @select="onDocUpload"></nldd-menu-item>
                      </nldd-menu>
                    </nldd-icon-button>
                  </nldd-toolbar-item>
                </nldd-toolbar>
                <nldd-spacer size="16"></nldd-spacer>
                <nldd-activity-indicator v-if="docsLoading && !docList.length && !conversionJobs.length" text="Documenten laden" show-text></nldd-activity-indicator>
                <nldd-inline-dialog v-else-if="docsError" variant="alert" text="Documenten niet geladen" :supporting-text="(docsError as Error).message"></nldd-inline-dialog>
                <DocumentList v-else-if="docList.length || conversionJobs.length" :documents="docList" :jobs="conversionJobs" :selected-path="docSelectedPath" @select="onDocSelect"></DocumentList>
                <nldd-inline-dialog v-else text="Geen werkdocumenten"></nldd-inline-dialog>
              </nldd-simple-section>
            </nldd-page>
          </nldd-split-view-pane>

          <!-- Secondary Sidebar: Artikelen Lijst - only when a law is
               selected. When deselected the pane is removed from the DOM
               so the navigation-split-view reflows to spatial mode and
               shows the sidebar (Wetten Browser) alongside main. -->
          <nldd-split-view-pane v-else-if="selectedLawId && !lawError" slot="secondary-sidebar" has-content>
            <nldd-page sticky-header>
              <nldd-top-title-bar
                slot="header"
                :text="paneChromeVisible(selectedLawLoading) ? lawTitle : undefined"
                :back-text="LIBRARY_HOME_BACK_TEXT"
                collapse-anchor="wet-titel"
              ></nldd-top-title-bar>

              <nldd-simple-section width="full">
                <nldd-title v-if="paneChromeVisible(selectedLawLoading)" id="wet-titel" size="3"><h3>{{ lawTitle }}</h3></nldd-title>
                <nldd-spacer v-if="paneChromeVisible(selectedLawLoading)" size="16"></nldd-spacer>
                <!-- Keyed on the chrome flag only, like the title above: the
                     favourite button runs entirely off `selectedLawId` (route)
                     + `favorites`, never the loaded law, so waiting for
                     `selectedLaw` only hid the toolbar during the load. -->
                <nldd-toolbar v-if="paneChromeVisible(selectedLawLoading)" accessible-label="Regelwerkacties">
                  <!-- priority: hoger blijft langer staan, lager verdwijnt als
                       eerste in het overflow-menu. -->
                  <nldd-toolbar-item slot="start" :priority="2">
                    <nldd-icon-button
                      :icon="favorites?.has(selectedLawId) ? 'star-filled' : 'star'"
                      :text="favorites?.has(selectedLawId) ? 'Verwijder uit favorieten' : 'Voeg toe aan favorieten'"
                      @click="onFavoriteClick"
                    >
                    </nldd-icon-button>
                  </nldd-toolbar-item>
                  <nldd-toolbar-item v-if="canShare" slot="start" :priority="1">
                    <nldd-icon-button icon="share" text="Delen" @click="shareLaw"></nldd-icon-button>
                  </nldd-toolbar-item>
                  <nldd-toolbar-item slot="end" :priority="4">
                    <nldd-icon-button
                      icon="search"
                      text="Artikelen zoeken"
                      :expanded="articleFilterOpen || undefined"
                      @click="toggleArticleFilter"
                    ></nldd-icon-button>
                  </nldd-toolbar-item>
                  <nldd-toolbar-item slot="end" :priority="3">
                    <nldd-icon-button icon="filter" text="Artikelen filteren">
                      <nldd-menu slot="popup">
                        <nldd-menu-item
                          v-for="option in ARTICLE_FILTERS"
                          :key="option.key"
                          type="radio"
                          :text="option.label"
                          :selected="articleTypeFilter === option.key || undefined"
                          @select="setArticleTypeFilter(option.key)"
                        ></nldd-menu-item>
                      </nldd-menu>
                    </nldd-icon-button>
                  </nldd-toolbar-item>
                </nldd-toolbar>
                <nldd-spacer v-if="paneChromeVisible(selectedLawLoading)" size="16"></nldd-spacer>
                <template v-if="paneChromeVisible(selectedLawLoading) && articleFilterOpen">
                  <nldd-search-field
                    ref="articleFilterField"
                    :value="articleFilter"
                    placeholder="Artikelen zoeken"
                    accessible-label="Artikelen zoeken"
                    @input="articleFilter = $event.detail?.value ?? ''"
                  ></nldd-search-field>
                  <nldd-spacer size="16"></nldd-spacer>
                </template>
                <nldd-activity-indicator v-if="selectedLawLoading" text="Regelwerk laden" show-text></nldd-activity-indicator>
                <nldd-inline-dialog v-else-if="!selectedLaw" text="Selecteer een regelwerk"></nldd-inline-dialog>
                <template v-else>
                <nldd-list appearance="simple">
                  <nldd-list-item size="md" button :current="isAlgemeen || undefined" @click="selectAlgemeen()">
                    <nldd-icon-cell size="20">
                      <nldd-icon icon="information"></nldd-icon>
                    </nldd-icon-cell>
                    <nldd-spacer-cell size="8"></nldd-spacer-cell>
                    <nldd-text-cell text="Algemeen"></nldd-text-cell>
                    <nldd-spacer-cell size="8"></nldd-spacer-cell>
                    <nldd-icon-cell size="20">
                      <nldd-icon icon="chevron-right"></nldd-icon>
                    </nldd-icon-cell>
                  </nldd-list-item>
                </nldd-list>
                <nldd-spacer size="24"></nldd-spacer>
                <nldd-title size="5">
                  <h4>Artikelen</h4>
                </nldd-title>
                <nldd-spacer size="8"></nldd-spacer>
                <template v-if="activeTypeFilterLabel">
                  <nldd-token
                    control="dismiss"
                    :text="activeTypeFilterLabel"
                    dismiss-text="Filter wissen"
                    @dismiss="setArticleTypeFilter('alles')"
                  ></nldd-token>
                  <nldd-spacer size="8"></nldd-spacer>
                </template>
                <!-- Geen lijst maar een losse dialog zodra er niets te tonen is:
                     die vult de resterende hoogte en zet de boodschap in het
                     midden, in plaats van als blokje bovenin te blijven hangen. -->
                <nldd-inline-dialog v-if="!filteredArticles.length" :text="articleEmptyText">
                  <nldd-button
                    v-if="articleFiltersActive"
                    slot="actions"
                    text="Toon alles"
                    @click="resetArticleFilters"
                  ></nldd-button>
                </nldd-inline-dialog>
                <nldd-list v-else appearance="simple">
                  <nldd-list-item
                    v-for="article in filteredArticles"
                    :key="article.number"
                    size="md"
                    button
                    :current="String(article.number) === String(selectedArticleNumber) || undefined"
                    @click="selectArticle(article.number)"
                  >
                    <nldd-text-cell :supporting-text="articleDescription(article)">
                      Artikel {{ article.number }}
                      <nldd-tag v-if="isMachineReadable(article)" size="sm" color="accent">Machine</nldd-tag>
                    </nldd-text-cell>
                    <nldd-spacer-cell size="8"></nldd-spacer-cell>
                    <nldd-icon-cell size="20">
                      <nldd-icon icon="chevron-right"></nldd-icon>
                    </nldd-icon-cell>
                  </nldd-list-item>
                </nldd-list>
                </template>
              </nldd-simple-section>
            </nldd-page>
          </nldd-split-view-pane>

          <!-- Main (instellingen mode): the selected settings pane. -->
          <nldd-split-view-pane v-if="isInstellingenMode" slot="main" :has-content="instellingenTab || undefined">
            <nldd-page landmarks="page" sticky-header>
              <nldd-top-title-bar
                slot="header"
                :text="instellingenTab === 'leden' ? 'Leden' : (instellingenTab === 'details' ? 'Algemeen' : undefined)"
                back-text="Instellingen"
                :collapse-anchor="instellingenTab ? 'instellingen-pane-titel' : undefined"
              ></nldd-top-title-bar>
              <TrajectDetailsPane
                v-if="instellingenTab === 'details'"
                :traject-id="activeTraject?.id"
                @deleted="onTrajectGone"
                @left="onTrajectGone"
              ></TrajectDetailsPane>
              <TrajectMembersPane
                v-else-if="instellingenTab === 'leden'"
                :traject-id="activeTraject?.id"
              ></TrajectMembersPane>
              <nldd-simple-section v-else width="full">
                <nldd-inline-dialog text="Kies een instelling"></nldd-inline-dialog>
              </nldd-simple-section>
            </nldd-page>
          </nldd-split-view-pane>

          <!-- Main (taken-modus): de takenlijst van de gekozen categorie. Een
               taak zelf opent nog steeds niet hier - "Beoordelen" navigeert naar
               de editor of het werkdocument. -->
          <nldd-split-view-pane v-else-if="isTakenMode" slot="main" :has-content="takenCategorie || undefined">
            <nldd-page landmarks="page" sticky-header>
              <nldd-top-title-bar
                slot="header"
                :text="takenTitle"
                back-text="Taken"
                :collapse-anchor="takenCategorie ? 'taken-pane-titel' : undefined"
              ></nldd-top-title-bar>
              <nldd-simple-section v-if="takenCategorie">
                <nldd-title id="taken-pane-titel" size="3"><h3>{{ takenTitle }}</h3></nldd-title>
                <nldd-spacer size="16"></nldd-spacer>
                <TasksListPane
                  :traject-ref="activeTrajectRef"
                  :categorie="takenCategorie"
                  :law-id="takenLawId"
                  @upload="onDocUpload"
                  @cancel-job="onCancelTaskJob"
                  @view-job="onViewTaskJob"
                ></TasksListPane>
              </nldd-simple-section>
              <nldd-simple-section v-else width="full">
                <nldd-inline-dialog text="Kies een categorie"></nldd-inline-dialog>
              </nldd-simple-section>
            </nldd-page>
          </nldd-split-view-pane>

          <!-- Main (werkdoc mode): the document editor, or a placeholder. -->
          <nldd-split-view-pane v-else-if="isWerkdocMode" slot="main" :has-content="hasOpenDoc || !!viewingJobPath || undefined">
            <nldd-page landmarks="page" v-if="viewingJobPath">
              <!-- Back to the document list, shown only while the sidebar is
                   stacked away - the job view is the whole screen then, with no
                   other way out. Mirrors DocumentEditor's own back item: the
                   split-view pane drives --context-back-button-display ('none'
                   while the sidebar is visible). -->
              <nldd-container slot="header" padding-inline="12" padding-top="12" sm-padding-inline="8" sm-padding-top="8">
                <nldd-toolbar size="md">
                  <nldd-toolbar-item slot="start" class="job-back">
                    <nldd-icon-button icon="chevron-left" text="Terug naar werkdocumenten" tooltip-timing="never" @click="onDocBack"></nldd-icon-button>
                  </nldd-toolbar-item>
                  <!-- Names the document here, where a document's name lives, so
                       the dialog below can stay about the conversion itself. The
                       path is known from the start, so no chrome guard: the title
                       is there before the conversion resolves either way. -->
                  <nldd-toolbar-title slot="center" align="center" :text="docsMgr.displayTitle(viewingJobPath)"></nldd-toolbar-title>
                </nldd-toolbar>
              </nldd-container>
              <nldd-simple-section width="full">
                <nldd-inline-dialog v-if="viewedJobFailed" variant="alert" text="Conversie mislukt" supporting-text="Het bestand kon niet worden omgezet naar tekst. Mogelijk is het beschadigd of geen leesbaar documenttype.">
                  <nldd-button slot="actions" appearance="secondary" text="Probeer opnieuw" @click="onRetryViewingJob"></nldd-button>
                </nldd-inline-dialog>
                <nldd-inline-dialog v-else variant="loading" text="Aan het converteren…" supporting-text="Dit gebeurt op de achtergrond. Je kunt gerust wegnavigeren en later terugkomen. Duurt het te lang, dan kun je de conversie annuleren.">
                  <nldd-button slot="actions" appearance="secondary" text="Annuleer conversie" @click="onCancelViewingJob"></nldd-button>
                </nldd-inline-dialog>
              </nldd-simple-section>
            </nldd-page>
            <!-- Werkdoc-404: het geadresseerde document bestaat niet. Staat vóór
                 de hasOpenDoc-tak, want openDoc's 404 zet wél currentPath (dus
                 hasOpenDoc is true) - zonder deze voorrang zou de editor met een
                 lege body renderen. Een echte centrale melding met een uitweg,
                 in de stijl van de artikel-404 verderop. -->
            <nldd-page landmarks="page" v-else-if="docNotFoundActive">
              <nldd-simple-section width="full">
                <nldd-inline-dialog
                  variant="alert"
                  :text="`Werkdocument ${docsMgr.displayTitle(openDocPath)} bestaat niet`"
                  supporting-text="Mogelijk is het verwijderd of klopt de link niet. Ga terug naar de lijst om een ander document te openen of een nieuw document te maken."
                >
                  <nldd-button slot="actions" appearance="primary" text="Terug naar werkdocumenten" @click="onDocBack"></nldd-button>
                </nldd-inline-dialog>
              </nldd-simple-section>
            </nldd-page>
            <nldd-page landmarks="page" v-else-if="hasOpenDoc" sticky-header sticky-footer>
              <!-- Review-modus (job_review-taak, payload.kind === 'document'):
                   a full-width, low bar above the document editor, same
                   pattern/variants as EditorView's law-review banner (PR
                   #935/#936). A bare nldd-container + nldd-banner in
                   DocumentEditor's default (body) slot, ahead of its own
                   nldd-simple-section - both land in nldd-page's body area,
                   in source order, so the banner sits above the editor. -->
              <!-- Alleen bij een actief voorstel: een mislukte review-load zet
                   geen banner meer (die viel eerder overal boven - zie de
                   watcher-failure-tak), en `docReviewLoadError` dient nu enkel
                   nog voor een mislukte Verwerpen-actie op een actieve review. -->
              <nldd-container v-if="docReviewActive" padding="8">
                <nldd-banner :variant="docReviewBannerVariant" text="Voorstel uit documentconversie" :supporting-text="docReviewBannerSupportingText">
                  <nldd-button
                    v-if="docReviewActive"
                    slot="actions"
                    appearance="secondary"
                    text="Verwerpen"
                    @click="rejectDocReview"
                  ></nldd-button>
                </nldd-banner>
              </nldd-container>
              <DocumentEditor ref="docEditorEl" :manager="docsMgr" @back="onDocBack" @saved="onDocSaved"></DocumentEditor>
            </nldd-page>
            <nldd-page landmarks="page" v-else>
              <nldd-simple-section width="full">
                <nldd-inline-dialog text="Geen document open"></nldd-inline-dialog>
              </nldd-simple-section>
            </nldd-page>
          </nldd-split-view-pane>

          <!-- Main: Artikel Detail -->
          <nldd-split-view-pane v-else slot="main" :has-content="selectedArticle || isAlgemeen || lawError || articleNotFound ? true : undefined">
            <nldd-page landmarks="page" sticky-header>
              <nldd-top-title-bar
                slot="header"
                :text="isAlgemeen ? 'Algemeen' : (selectedArticle ? `Artikel ${selectedArticle.number}` : undefined)"
                :supporting-text="isAlgemeen || selectedArticle ? lawName : undefined"
                :back-text="lawError ? LIBRARY_HOME_BACK_TEXT : (lawName || 'Terug')"
                :collapse-anchor="isAlgemeen ? 'algemeen-titel' : (selectedArticle ? 'article-titel' : undefined)"
              ></nldd-top-title-bar>

              <nldd-simple-section width="full" v-if="!selectedLawId">
                <!-- Generic: from Home you can also open a werkdocument, not just a law. -->
                <nldd-inline-dialog text="Geen selectie"></nldd-inline-dialog>
              </nldd-simple-section>
              <nldd-simple-section width="full" v-else-if="selectedLawLoading">
                <!-- Loading takes precedence over `lawError` to avoid flashing a stale error during a refetch. -->
                <nldd-activity-indicator text="Artikel laden" show-text></nldd-activity-indicator>
              </nldd-simple-section>
              <nldd-simple-section width="full" v-else-if="lawError">
                <!-- 404 = law not in active traject; give the user an exit instead of a generic error. -->
                <nldd-inline-dialog
                  v-if="lawErrorIs404"
                  variant="alert"
                  :text="`${indexedLawName} is niet beschikbaar in dit traject`"
                  supporting-text="Wissel van traject via het menu rechtsboven of ga terug naar het overzicht."
                >
                  <nldd-button slot="actions" appearance="primary" text="Naar overzicht" @click="goToLibraryRoot"></nldd-button>
                  <nldd-button slot="actions" appearance="secondary" text="Probeer opnieuw" @click="retryLoadLaw"></nldd-button>
                </nldd-inline-dialog>
                <nldd-inline-dialog
                  v-else
                  variant="alert"
                  :text="`${indexedLawName} is niet geladen`"
                  supporting-text="De gegevens konden niet worden opgehaald."
                >
                  <nldd-button slot="actions" appearance="primary" text="Probeer opnieuw" @click="retryLoadLaw"></nldd-button>
                  <nldd-button slot="actions" appearance="secondary" text="Neem contact op via e-mail" :href="`mailto:${SUPPORT_EMAIL}`"></nldd-button>
                </nldd-inline-dialog>
              </nldd-simple-section>
              <nldd-simple-section width="full" v-else-if="articleNotFound">
                <nldd-inline-dialog
                  variant="alert"
                  :text="`Artikel ${selectedArticleNumber} van ${lawName || indexedLawName} bestaat niet`"
                  supporting-text="Mogelijk klopt de URL niet. Neem contact op als je verwacht dat dit artikel wel bestaat."
                >
                  <nldd-button slot="actions" class="article-not-found__back-button" appearance="primary" text="Bekijk artikelen" @click="goToLawRoot"></nldd-button>
                  <nldd-button slot="actions" appearance="secondary" text="Neem contact op via e-mail" :href="`mailto:${SUPPORT_EMAIL}`"></nldd-button>
                </nldd-inline-dialog>
              </nldd-simple-section>
              <nldd-simple-section width="full" v-else-if="isAlgemeen">
                <nldd-title id="algemeen-titel" size="3">
                  <h3>Algemeen</h3>
                  <span slot="supporting-text">{{ lawName }}</span>
                </nldd-title>
                <nldd-spacer size="16"></nldd-spacer>
                <nldd-list appearance="box-tinted" accessible-label="Algemene informatie">
                  <nldd-list-item v-for="row in algemeenRows" :key="row.label">
                    <nldd-text-cell :text="row.label" width="200px"></nldd-text-cell>
                    <nldd-spacer-cell size="16"></nldd-spacer-cell>
                    <nldd-text-cell :text="row.value"></nldd-text-cell>
                  </nldd-list-item>
                </nldd-list>
                <template v-if="algemeenLegalBasis.length">
                  <nldd-spacer size="24"></nldd-spacer>
                  <nldd-title size="4">
                    <h4>Grondslag</h4>
                  </nldd-title>
                  <nldd-spacer size="8"></nldd-spacer>
                  <nldd-list appearance="simple">
                    <nldd-list-item
                      v-for="basis in algemeenLegalBasis"
                      :key="`${basis.law_id}-${basis.article}`"
                      size="md"
                      button
                      @click="selectLawArticle(basis.law_id, basis.article)"
                    >
                      <nldd-text-cell
                        :text="basis.article ? `Artikel ${basis.article}` : humanizeLawId(basis.law_id)"
                        :supporting-text="basis.description || (basis.article ? humanizeLawId(basis.law_id) : undefined)"
                      ></nldd-text-cell>
                      <nldd-spacer-cell size="8"></nldd-spacer-cell>
                      <nldd-icon-cell size="20">
                        <nldd-icon icon="chevron-right"></nldd-icon>
                      </nldd-icon-cell>
                    </nldd-list-item>
                  </nldd-list>
                </template>
              </nldd-simple-section>
              <nldd-simple-section width="full" v-else-if="!selectedArticle">
                <nldd-inline-dialog text="Selecteer een artikel"></nldd-inline-dialog>
              </nldd-simple-section>
              <template v-else>
                <nldd-simple-section width="full">
                  <nldd-title id="article-titel" size="3">
                    <h3>Artikel {{ selectedArticle.number }}</h3>
                    <span slot="supporting-text">{{ lawName }}</span>
                  </nldd-title>
                  <nldd-spacer size="16"></nldd-spacer>
                  <nldd-toolbar>
                    <nldd-toolbar-item slot="start">
                      <nldd-tab-bar size="md" @tabchange="onDetailTabChange">
                        <nldd-tab-bar-item data-view="tekst" :current="detailView === 'tekst' || undefined" text="Tekst"></nldd-tab-bar-item>
                        <nldd-tab-bar-item data-view="machine" :current="detailView === 'machine' || undefined" text="Machine"></nldd-tab-bar-item>
                        <nldd-tab-bar-item data-view="yaml" :current="detailView === 'yaml' || undefined" text="YAML"></nldd-tab-bar-item>
                      </nldd-tab-bar>
                    </nldd-toolbar-item>
                    <nldd-toolbar-item slot="end">
                      <nldd-button ref="editButton" v-if="selectedLawId" appearance="secondary" text="Bewerken" :href="authenticated ? editLawHref : undefined" @click.prevent="onEditClick" @pointerdown.capture="onLoginTriggerPointerdown"></nldd-button>
                    </nldd-toolbar-item>
                  </nldd-toolbar>
                  <nldd-spacer size="24"></nldd-spacer>
                  <KeepAlive>
                    <ArticleText v-if="detailView === 'tekst'" :article="selectedArticle" centered />
                    <MachineReadable v-else-if="detailView === 'machine'" :article="selectedArticle" :can-create="!!selectedLawId" :can-enrich="canEnrich" :needs="enrichNeeds" :enriching="isEnriching" :review-ready="reviewReady" :review-article="reviewArticleForPane" :enrich-error="enrichError" :create-href="authenticated ? editLawHref : undefined" @create-mr="openEditor" @enrich="enrichSelectedLaw" @view-tasks="openTasksForLaw" @review="openReviewForLaw" @open-action="activeAction = $event" />
                    <YamlView v-else-if="detailView === 'yaml'" :article="selectedArticle" :can-create="!!selectedLawId" :can-enrich="canEnrich" :needs="enrichNeeds" :enriching="isEnriching" :review-ready="reviewReady" :review-article="reviewArticleForPane" :enrich-error="enrichError" :create-href="authenticated ? editLawHref : undefined" @create-mr="openEditor" @enrich="enrichSelectedLaw" @view-tasks="openTasksForLaw" @review="openReviewForLaw" />
                  </KeepAlive>
                </nldd-simple-section>
              </template>
            </nldd-page>
          </nldd-split-view-pane>

        </nldd-navigation-split-view>
  <!-- Overlays teleported to body: as light-DOM siblings of the split view they
       would be slotted into the main pane and pick up its ::slotted flex-grow,
       stealing height from the pane content (a short page's sticky footer then
       floats mid-screen in document-scroll mode). -->
  <Teleport to="body">
    <!-- LibraryApp is a read-only browser; ActionSheet is mounted without editable
         so the output field is hidden and the footer button just closes the sheet. -->
    <ActionSheet :action="activeAction" :article="selectedArticle" :editable="false" @close="activeAction = null" @save="activeAction = null" @edit="editInEditor" />
    <SearchPopover
      ref="searchPopoverRef"
      @select-law="(lawId) => selectLaw(lawId, true)"
      @harvest-available="onHarvestAvailable"
    />
    <AddLawSheet
      ref="addLawSheetRef"
      @promoted="onLawPromoted"
      @harvest-requested="onTrajectHarvestRequested"
      @upload-requested="onLawUpload"
    />
    <InviteMembersSheet ref="inviteMembersSheetRef" :traject-id="activeTraject?.id" />
    <TrajectStructureSheet
      :traject-ref="activeTrajectRef"
      :open="isStructureCheck"
      @close="closeStructureCheck"
    />
  </Teleport>
  <!-- Unsaved-changes guard for in-view werkdocument navigation. -->
  <Teleport to="body">
    <nldd-modal-dialog
      ref="docNavGuardEl"
      variant="alert"
      text="Niet-opgeslagen wijzigingen"
      :supporting-text="docNavGuardText"
      @close="cancelDocLeave"
    >
      <nldd-button slot="actions" appearance="primary" text="Blijf document bewerken" @click="cancelDocLeave"></nldd-button>
      <nldd-button slot="actions" appearance="secondary" text="Sla wijzigingen op en sluit" :loading="docSaving || undefined" @click="saveDocAndLeave"></nldd-button>
      <nldd-button slot="actions" appearance="destructive" text="Negeer wijzigingen en sluit" @click="confirmDocLeave"></nldd-button>
    </nldd-modal-dialog>
  </Teleport>

  <!-- Tussen kiezen en versturen: wat er met dit bestand gaat gebeuren, en (waar
       er te kiezen valt) of AI mee mag doen. -->
  <UploadConfirmDialog
    :file="pendingDocUpload"
    @confirm="confirmDocUpload"
    @cancel="cancelDocUpload"
  />

  <Teleport to="body">
    <nldd-modal-dialog
      ref="uploadErrorModalEl"
      variant="alert"
      text="Uploaden mislukt"
      :supporting-text="docUploadError || ''"
      @close="dismissUploadError"
    >
      <nldd-button slot="actions" appearance="primary" text="Sluit" @click="dismissUploadError"></nldd-button>
      <nldd-button v-if="docUploadRetryable" slot="actions" appearance="secondary" text="Probeer opnieuw" @click="retryUpload"></nldd-button>
    </nldd-modal-dialog>
  </Teleport>

  <Teleport to="body">
    <nldd-modal-dialog
      ref="lawUploadErrorModalEl"
      variant="alert"
      text="Uploaden mislukt"
      :supporting-text="lawUploadError || ''"
      @close="dismissLawUploadError"
    >
      <nldd-button slot="actions" appearance="primary" text="Sluit" @click="dismissLawUploadError"></nldd-button>
      <nldd-button v-if="lawUploadRetryable" slot="actions" appearance="secondary" text="Probeer opnieuw" @click="retryLawUpload"></nldd-button>
    </nldd-modal-dialog>
  </Teleport>

  <Teleport to="body">
    <nldd-modal-dialog
      ref="jobCancelErrorModalEl"
      variant="alert"
      text="Annuleren mislukt"
      :supporting-text="`De conversie loopt door op de achtergrond. ${jobCancelError || ''}`"
      @close="dismissJobCancelError"
    >
      <nldd-button slot="actions" appearance="primary" text="Sluit" @click="dismissJobCancelError"></nldd-button>
    </nldd-modal-dialog>
  </Teleport>
</template>

<style>
/* Unscoped on purpose: nldd-navigation-split-view is a custom element
   with its own shadow root, but the `.full-stack` class is reflected on
   the host element from light-DOM space, so a scoped selector here
   wouldn't match any longer than the scoping attribute the Vue compiler
   would inject. The class name is namespaced (`article-not-found__…`)
   to make accidental collisions outside this view unlikely.

   "Bekijk artikelen" alleen tonen wanneer de artikelenlijst niet naast
   de main pane zichtbaar is (full-stack mode = single pane op mobile). */
nldd-navigation-split-view:not(.full-stack) .article-not-found__back-button {
  display: none;
}

/* The conversion-job view's back item, shown only once the split-view pane
   drops its hide-back state (the document list is stacked away). Same signal
   nldd-top-title-bar and DocumentEditor's own back item use; the pane sets
   --context-back-button-display: none while the list is visible.
   :not([hidden]) yields to the toolbar's own overflow hiding. */
.job-back:not([hidden]) {
  display: var(--context-back-button-display, inline-flex);
}

</style>
