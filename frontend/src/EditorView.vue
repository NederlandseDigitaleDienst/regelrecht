<script setup lang="ts">
import { ref, computed, reactive, watch, watchEffect, nextTick, onBeforeUnmount } from 'vue';
import { storeToRefs } from 'pinia';
import { useRoute, useRouter, onBeforeRouteLeave, onBeforeRouteUpdate } from 'vue-router';
import * as yaml from 'js-yaml';
import { useOpenLaw } from './composables/useOpenLaw';
import { useCorpus } from './corpus';
import { useEngine } from './composables/useEngine';
import { useAuth } from './composables/useAuth';
import { useTrajectsStore } from './stores/trajects';
import { useFeatureFlagsStore } from './stores/featureFlags';
import { useEnrichState } from './composables/useEnrichState';
import { useTaskReview } from './composables/useTaskReview';
import { useNotes, useResolvedDraftNotes } from './composables/useNotes';
import { useDraftNotes } from './composables/useDraftNotes';
import { lastHomePath, homeTarget } from './composables/useLastVisitedRoute';
import { useAppChromeStore } from './stores/appChrome';
import { SUPPORT_EMAIL } from './constants';
import { apiFetchJson } from './lib/apiFetch';
import { RETRY_MIN_SPINNER_MS } from './lib/retryFeedback';
import { humanizeLawId } from './lib/lawName';
import { quoteContext } from './lib/quoteContext';
import { linkedOutput, linksByOutput, type LinkedFragment } from './regelrijm/regelrijmLinks';
import type { Path } from '@regelrecht/frontend-shared/regelrijm/index';
import { humanize } from './utils/outputFormat';
import { useLatest } from './lib/useLatest';
import { useOpenTabs } from './composables/useOpenTabs';
import { createTabRestore } from './composables/useTabRestore';
import { proposalDivergence } from './lib/taskReview';
import ArticleText from './components/ArticleText.vue';
import ArticleTextEditor from './components/ArticleTextEditor.vue';
import NoteCreator from './components/NoteCreator.vue';
import NotesLoadError from './components/NotesLoadError.vue';
import NoteCard from './components/NoteCard.vue';
import QuotedFragment from './components/QuotedFragment.vue';
import { cpToUtf16, utf16ToCp } from './composables/useTextSelection';
import ActionSheet from './components/ActionSheet.vue';
import EditSheet from './components/EditSheet.vue';
import SearchPopover from './components/SearchPopover.vue';
import MachineReadable from './components/MachineReadable.vue';
import MachineEmptyState from './components/MachineEmptyState.vue';
import ScenarioBuilder from './components/ScenarioBuilder.vue';
import ExecutionTraceView from './components/ExecutionTraceView.vue';
import { buildOutputTypeMap } from './utils/articleMapping';
import LawGraphView from './components/LawGraphView.vue';
import { singleParam } from './lib/routeParams';
import type { ComponentPublicInstance } from 'vue';
import type { NLDDSheet } from '@nldd/design-system/sheet';
import type { NLDDModalDialog } from '@nldd/design-system/modal-dialog';
import type { NLDDPopover } from '@nldd/design-system/popover';
import type { Annotation } from '@nldd/design-system/text-editor';
import type { Note } from './composables/useNotes';
import type { EditorEngine } from './composables/useEngine';
import type { Verdict } from './composables/useTaskReview';
import type { JobTaskSummary } from './composables/useTasks';
import type { OpenTab } from './lib/openTabsStorage';
import type { RequestError } from './lib/apiFetch';
import type { ExecutionAction, LawArticle, LawDocument, MachineReadableSection } from './lib/lawFormat';
import type { ScenarioExecutedPayload } from './components/ScenarioBuilder.vue';
import type { EditSheetItem } from './components/EditSheet.vue';
import type { MachineReadableDelete, MachineReadableSave } from './lib/machineReadableEdits';

/** A pane's position target in the pane menu. */
type PaneMoveTarget = 'left' | 'right' | 'start' | 'end';

/** A [start, end) code-point range into the article text. */
interface CodePointRange {
  start: number;
  end: number;
}

/** The fields of a note body the look-ahead popover reads (W3C TextualBody or SpecificResource). */
interface NoteBody {
  value?: string;
  source?: string;
}

/** Notes sharing one quoted fragment, with that fragment's quoteContext() parts. */
interface NoteGroup {
  start: number;
  end: number | null;
  quote: string;
  before: string;
  after: string;
  ellipsisBefore: string;
  ellipsisAfter: string;
  notes: Note[];
}

/** What a mounted ArticleTextEditor exposes (its defineExpose). */
type TextEditorInstance = InstanceType<typeof ArticleTextEditor>;


const { authenticated, oidcConfigured } = useAuth();
const featureFlags = useFeatureFlagsStore();
// What this view publishes to the shell: tabs, the changes bar, the PR badge.
const {
  registerSearchPopover,
  setEditorChrome,
  registerTabActions,
  setEditorChanges,
  registerEditorActions,
  clearEditorChrome,
} = useAppChromeStore();

// Per-pane view selection. Each pane independently picks one of the
// available views (Tekst, Machine, Scenario's, YAML). Same view can be
// in multiple panes (e.g. two YAML panes for diff). Layout always has
// one pane per view; nldd-side-by-side-split-view auto-hides panes
// from the right when the viewport is too narrow, so left = highest
// priority. Visibility flags via panel.* feature flags filter what's
// pickable in the menu.
const VIEW_DEFINITIONS = [
  { id: 'text', flag: 'panel.article_text', label: 'Tekst' },
  { id: 'machine', flag: 'panel.machine_readable', label: 'Machine' },
  { id: 'scenario', flag: 'panel.scenario_form', label: "Scenario's" },
  { id: 'yaml', flag: 'panel.yaml_editor', label: 'YAML' },
  { id: 'notes', flag: 'panel.notes', label: 'Notities' },
];

const availableViews = computed(() => VIEW_DEFINITIONS.filter(v => featureFlags.isEnabled(v.flag)));

function viewLabel(viewId: string) {
  return VIEW_DEFINITIONS.find(v => v.id === viewId)?.label ?? viewId;
}

const PANE_VIEWS_KEY = 'regelrecht-pane-views';
const paneViews = ref<string[]>(loadPaneViews());

function loadPaneViews(): string[] {
  try {
    const stored = JSON.parse(localStorage.getItem(PANE_VIEWS_KEY) ?? 'null');
    // Only accept entries we still recognise - a stale value left over
    // from a removed view (e.g. 'form' before scenario was renamed) or
    // an externally injected string would otherwise produce a pane with
    // no v-if branch matching it, briefly flashing an empty body before
    // the availableViews watcher corrects it on the next tick.
    const knownIds = new Set(VIEW_DEFINITIONS.map(v => v.id));
    if (
      Array.isArray(stored) &&
      stored.length > 0 &&
      stored.every(v => typeof v === 'string' && knownIds.has(v))
    ) {
      return stored;
    }
  } catch {
    /* fall through to default */
  }
  return VIEW_DEFINITIONS.map(v => v.id);
}

// Every paneViews mutation goes through `paneViews.value = next`
// (setPaneView, the availableViews sync watcher), so the top-level ref
// identity always changes. No deep:true needed; in-place mutations are
// not used and shouldn't be relied on.
watch(paneViews, (val) => {
  localStorage.setItem(PANE_VIEWS_KEY, JSON.stringify(val));
});

// Sync paneViews with availableViews when a flag flips:
// - Drop panes whose view is no longer available (flag off → pane gone)
// - Append a pane for any view that was JUST enabled by this change
//   (in current available, not in previous) - so re-enabling a flag
//   brings the pane back instead of silently leaving the user without
//   it. The previous version compared "missing from paneViews" against
//   the full available set, which spuriously appended every available
//   view a user happened not to have open whenever any unrelated flag
//   flipped.
// When everything is filtered out, reset to one pane per available view
// as the safe default.
watch(availableViews, (views, oldViews) => {
  const allowedIds = new Set(views.map(v => v.id));
  const filtered = paneViews.value.filter(v => allowedIds.has(v));
  // On the very first run (immediate: true) oldViews is undefined; treat
  // it as "no diff" so we don't append every available view on top of
  // whatever the user had restored from localStorage.
  const previousIds = new Set((oldViews ?? []).map(v => v.id));
  const newlyEnabled = views
    .filter(v => oldViews !== undefined && !previousIds.has(v.id))
    .map(v => v.id);
  const next = [...filtered, ...newlyEnabled];
  if (next.length === 0 && views.length > 0) {
    paneViews.value = views.map(v => v.id);
    return;
  }
  if (
    next.length !== paneViews.value.length ||
    next.some((v, i) => v !== paneViews.value[i])
  ) {
    paneViews.value = next;
  }
}, { immediate: true });

function setPaneView(idx: number, viewId: string) {
  const next = [...paneViews.value];
  next[idx] = viewId;
  paneViews.value = next;
}

// Reorder this pane within the left→right pane order. `target`:
// 'left' | 'right' | 'start' | 'end'. Persists (paneViews → localStorage) and
// survives flag flips (the availableViews sync filter preserves order).
function movePane(idx: number, target: PaneMoveTarget) {
  const next = [...paneViews.value];
  const [moved] = next.splice(idx, 1);
  const to =
    target === 'left' ? idx - 1
      : target === 'right' ? idx + 1
        : target === 'start' ? 0
          : next.length; // 'end'
  // idx is a rendered pane's index, so the splice above removed one entry.
  next.splice(Math.max(0, Math.min(next.length, to)), 0, moved!);
  paneViews.value = next;
}

// All edit operations are gated behind SSO + an active traject. The
// editor renders in two shapes:
//   - `/editor/{lawId?}/{articleNumber?}`         → read-only view,
//     `canEdit` is false; save buttons disabled.
//   - `/editor/{trajectRef}/{lawId?}/{article?}`  → full edit mode,
//     `canEdit` is true; writes land in that traject's branch.
// Pick a traject in the TrajectMenu to flip from the first shape to
// the second.
const { activeTrajectRef, activeTraject, trajectMissing } = storeToRefs(useTrajectsStore());
const canEdit = computed(
  () => (!oidcConfigured.value || authenticated.value) && activeTrajectRef.value !== null,
);
// The Tekst editor pane is editable whenever the user has write access. The
// old `editor.article_text_edit` flag that gated this separately is merged into
// the pane itself - the pane IS the editable text view, so this now just tracks
// canEdit (kept as a named alias for the text-edit affordances below).
const canEditArticleText = computed(() => canEdit.value);

const route = useRoute();
const router = useRouter();

// De review-taak uit de URL (`?task=`). Staat hier hoog omdat `editorRouteFor`
// hem meeneemt: zolang een taak in beoordeling is, reist hij mee met elke
// navigatie binnen de editor (tab-, artikel- en wetwissel). De hele
// review-modus verderop hangt aan deze ene bron.
const reviewTaskIdParam = computed(() =>
  typeof route.query.task === 'string' ? route.query.task : null,
);

// Bibliotheek tab / "naar bibliotheek" buttons: restore the last library
// position but re-stamp it with the currently active traject, so the
// traject survives the Editor→Bibliotheek switch (it lives in the URL).
const libraryTabTarget = computed(() => lastHomePath.value);
const libraryTabHref = computed(() => router.resolve(libraryTabTarget.value).href);

// --- The open regelwerk and its unsaved edits (initial load from the route) ---
const openLaw = useOpenLaw({
  lawId: singleParam(route.params.lawId),
  articleNumber: singleParam(route.params.articleNumber),
  trajectRef: singleParam(route.params.trajectRef),
});
const {
  law,
  lawId,
  rawYaml,
  articles,
  lawName,
  selectedArticle,
  selectedArticleNumber,
  selectArticle,
  clear: clearLaw,
  lawTrajectRef,
  loading,
  error,
  saving: lawSaving,
  saveError: lawSaveError,
  reload: reloadLaw,
  seedFromYaml,
  currentEtag,
  lastSavedPr,
  editedText,
  machineReadable,
  yamlSource,
  parseError,
  editedArticle,
  hasMachineReadable,
  currentLawYaml,
  isMachineReadableDirty,
  articleDirty,
  lastSaveTouchedText,
  lastSaveTouchedMachine,
  activeAction,
  activeActionPath,
  activeActionIsNew,
} = openLaw;

// --- Multi-law tab state (persisted per traject in localStorage) ---
// Tab state is DERIVED from the active traject rather than a single ref that a
// switch reassigns: `useOpenTabs` keeps a per-traject bucket and every mutation
// takes an explicit trajectRef, so a late/stale write lands in its own bucket
// and is invisible instead of leaking into the traject the route switched to.
// The `openTabs`/`activeTab` computeds re-read the bucket for whatever traject
// is active now, so a route commit flips the bar with nobody reassigning it.
const {
  tabs: openTabs,
  activeTab,
  publishedTrajectRef,
  tabsFor,
  activeTabFor,
  findTab: findTabIn,
  openTab,
  setActiveTab,
  closeTab: closeTabIn,
  reorderTabs: reorderTabsIn,
  dropLaw,
} = useOpenTabs(activeTrajectRef);

// Restore-on-entry flow (see useTabRestore): opens the last active article of a
// traject when its editor is entered without a law in the URL, pruning tabs
// whose law 404s in this traject. `canPrune` gates pruning on confirmed traject
// membership (the list is still loading at mount, when every law-GET 404s).
const tabRestore = createTabRestore({
  tabsFor,
  activeTabFor,
  setActiveTab,
  dropLaw,
  switchLaw: (id, articleNumber, trajectRef) => openLaw.open({ lawId: id, articleNumber, trajectRef }),
  clearLaw,
  error,
  router,
  editorRouteFor,
  canPrune: () => activeTraject.value != null,
});

// When the active traject changes (router.push to /editor/{otherRef}/…)
// the URL stays on the same component; re-open the law through the new
// traject's backends. The display names need no handling here - the corpus
// reader follows the route's traject itself. `open` takes the traject
// explicitly, so it reads through the new traject before the route has
// committed.
//
// Also flush the WASM engine: it caches loaded laws by id only, so
// without this a scenario run after a traject switch would evaluate
// the open law against the *previous* traject's dependencies. The
// dependency walker re-loads on demand on the next run, so a single
// `unloadAllLaws` is enough - no per-dep bookkeeping needed.
//
// Two switches in quick succession (rapid back/forward across trajects) can
// overlap. `open` resolves to false when a later open superseded it, and the
// reconciliation below then stops. The auto-open latch is a separate matter:
// only the latest traject switch may lower it, also when a tab click
// superseded its open, or the net would stay off for good.
const claimTrajectSwitch = useLatest();
watch(activeTrajectRef, async (next) => {
  const ownsLatch = claimTrajectSwitch();
  // Stand the auto-open robustness net down while the bar is reconciled for
  // `next`, so it doesn't race the restore below.
  suppressTabAutoOpen = true;
  try {
    await reconcileTraject(next);
  } finally {
    await nextTick();
    if (ownsLatch()) suppressTabAutoOpen = false;
  }
});

async function reconcileTraject(next: string | null): Promise<void> {
  unloadAllLaws();
  // Tabs are DERIVED from the active traject: the `openTabs`/`activeTab`
  // computeds have already flipped to `next`'s bucket now the route committed,
  // so there is nothing to swap here. Resolve the swapped-in tabs' labels
  // through the new traject.
  backfillTabLabels(next);
  const urlLawId = singleParam(route.params.lawId);
  if (!urlLawId) {
    // No law in the URL (the switcher dropped us on the traject root): restore
    // the last active article of the traject we just entered, pruning any tab
    // whose law 404s here, or land on the neutral root.
    await tabRestore.restoreForTraject(next, { hasLawInUrl: false });
    return;
  }
  // A URL that still names a law in the new traject can only come from a deep
  // link or browser back/forward (the traject switcher navigates to the bare
  // root - see trajectSwitchTarget). The route guard only opens a law when the
  // URL's lawId *changes*; a cross-traject navigation that keeps the SAME
  // lawId leaves the open law pointing at the PREVIOUS traject's copy. Open it
  // again here whenever it isn't this traject's copy, so we never adopt stale,
  // wrong-traject content - and a later save PUTs to the right traject.
  if (lawTrajectRef.value !== next || lawId.value !== urlLawId) {
    const won = await openLaw.open({
      lawId: urlLawId,
      articleNumber: singleParam(route.params.articleNumber),
      trajectRef: next,
    });
    // A later open superseded us: don't reconcile the bar against it.
    if (!won) return;
  }
  // Only reflect the URL's law as this traject's active tab when it actually
  // loaded FOR this traject. When the traject doesn't have the law, the open
  // 404s: `error` is set and the previous traject's law is still open.
  // Adopting that would add a foreign law as an open+active tab and persist it
  // under this traject. The error view renders the "niet beschikbaar" dialog
  // for the failed case.
  const loaded =
    !error.value &&
    lawTrajectRef.value === next &&
    lawId.value === singleParam(route.params.lawId) &&
    selectedArticleNumber.value != null;
  if (loaded) {
    // openTab de-dups, caps at MAX_TABS and marks it active - all under `next`.
    // `loaded` checked that lawId equals the URL's law id.
    openTab(next, { lawId: lawId.value as string, articleNumber: String(selectedArticleNumber.value) });
  } else {
    // Failed cross-traject deep link: don't leave the previous traject's law
    // marked active in this traject's bar (it isn't in this bucket here).
    setActiveTab(next, null);
  }
}

// Notes (RFC-005/RFC-018) for the current law, resolved against its text.
// `lawValidFrom` names the version on screen: the engine holds every version
// of the law (loadDependency), so the resolver needs to know which one the
// user is looking at instead of defaulting to the newest. Corpus YAML quotes
// the date ('2025-01-01' → string), but an unquoted date would reach us as a
// JS Date via js-yaml, so coerce defensively.
const lawValidFrom = computed(() => {
  const v = law.value?.valid_from;
  if (typeof v === 'string') return v;
  if (v instanceof Date) return v.toISOString().slice(0, 10);
  return null;
});
const {
  notesForArticle: committedNotesForArticle,
  issues: noteIssues,
  error: notesLoadError,
  reload: reloadNotes,
} = useNotes(lawId, selectedArticle, activeTrajectRef, lawValidFrom);

// Notes render as annotations inside the editable Tekst editor (they used to
// live in a separate read-only "Tekst + notities" pane, now dropped). The
// resolver's code-point spans are converted to the editor's annotation overlay
// in `editorAnnotations`; authoring runs off the toolbar "Reactie" button.

// Note authoring (RFC-018 write path). Draft notes live in localStorage per
// law until exported; they resolve and highlight live alongside committed
// ones, so the author sees the new note anchored immediately. The WASM engine
// is handed to NoteCreator for selector-uniqueness checks; useNotes already
// initialises it, this just exposes the instance once ready (null until then,
// NoteCreator guards on it).
const noteEngine = ref<EditorEngine | null>(null);
useEngine()
  .initEngine()
  .then((e) => {
    noteEngine.value = e;
  })
  .catch(() => {});
const {
  drafts: draftNotes,
  draftCount,
  addDraft,
  removeDraft,
  exportYaml,
  exportYamlFromNotes,
  publishNote,
} = useDraftNotes(lawId, activeTrajectRef);
const { draftNotesForArticle } = useResolvedDraftNotes(
  draftNotes,
  lawId,
  selectedArticle,
  activeTrajectRef,
  lawValidFrom,
);
// Authoring is part of the notes pane (the old separate `notes.create` flag is
// folded in): wherever the pane is available, you can create notes in it.
// Note creation is a writer action (it persists to the traject sidecar), so it
// follows the same write-access gate as law-text editing. Whether the pane is
// offered at all is the panel.notes flag (VIEW_DEFINITIONS above).
const canCreateNotes = computed(() => canEdit.value);
// Committed + draft notes share the highlight path. Draft entries already
// carry __draft so the popover can mark them unsaved.
const notesForArticle = computed(() => [
  ...committedNotesForArticle.value,
  ...draftNotesForArticle.value,
]);

// A stable id per note so an annotation-click maps back to it. Committed notes
// carry a W3C id; a draft may not, so fall back to its position.
function noteKey(note: Note | null | undefined, i: number): string {
  // A W3C annotation id is an IRI string; notes are not validated (see Note).
  return (note?.id as string | undefined) || `note-${i}`;
}
const noteById = computed(() => {
  const m = new Map<string, Note>();
  notesForArticle.value.forEach((entry, i) => m.set(noteKey(entry.note, i), entry.note));
  return m;
});
// Notes → DS annotations for the editable Tekst editor. Convert the resolver's
// code-point spans to UTF-16 offsets against the SAVED text (not editedText) so
// this stays stable while typing - the editor maps the anchors through edits
// itself. Recomputes on article switch / save, when the notes re-resolve.
const editorAnnotations = computed(() => {
  const text = selectedArticle.value?.text || '';
  const out: Annotation[] = [];
  notesForArticle.value.forEach((entry, i) => {
    const id = noteKey(entry.note, i);
    for (const span of entry.spans) {
      const start = cpToUtf16(text, span.start);
      const end = cpToUtf16(text, span.end);
      out.push({ id, start, end, quote: text.slice(start, end) });
    }
  });
  return out;
});

// RegelRijm (flag panel.regelrijm): per output, the fragments of the saved
// article text that a linking note ties to that rule, for the Machine pane.
// Same resolved notes as the highlights, so both show the same fragments.
const regelrijmLinks = computed(() => {
  if (!featureFlags.isEnabled('panel.regelrijm')) return new Map<string, LinkedFragment[]>();
  return linksByOutput(notesForArticle.value, {
    lawId: [law.value?.$id, lawId.value],
    text: selectedArticle.value?.text || '',
  });
});

// The action a linking note points at in this article, for "Actie openen" in
// the note popover; null for any other note or with the flag off.
function linkedAction(note: Note): ExecutionAction | null {
  if (!featureFlags.isEnabled('panel.regelrijm')) return null;
  const output = linkedOutput(note, [law.value?.$id, lawId.value]);
  return output ? (machineReadable.value?.execution?.actions?.find((a) => a?.output === output) ?? null) : null;
}

function openLinkedAction(note: Note) {
  const action = linkedAction(note);
  if (!action) return;
  annotationPopEl.value?.hide?.();
  handleOpenAction(action);
}

// NoteCreator (authoring popover) state, opened by the toolbar comment button
// on the current editor selection.
const noteCreator = reactive<{
  open: boolean;
  range: CodePointRange | null;
  editing: Note | null;
  initialNote: Note | null;
}>({ open: false, range: null, editing: null, initialNote: null });

// - Note sheet (a right sheet hosting the badge note list and the editor) ------
// The editor (NoteCreator) is rendered as sheet content. Two entry points:
// - annotation badge → open at the 'list' view (referenced text + note cards),
//   from which editing/adding pushes the 'edit' view with a back button;
// - notities pane → open directly at 'edit' (the pane is already the list), whose
//   top bar shows a close button, not a back button.
const noteSheetEl = ref<NLDDSheet | null>(null);
const noteSheetView = ref<'list' | 'edit'>('list'); // 'list' | 'edit'
const noteEditFromList = ref(false); // edit entered from the in-sheet list

function openNoteEditor({ range, editing, initialNote, fromList }: {
  range: CodePointRange | null;
  editing: Note | null;
  initialNote: Note | null;
  fromList: boolean;
}) {
  noteCreator.range = range;
  noteCreator.editing = editing;
  noteCreator.initialNote = initialNote;
  noteCreator.open = true;
  noteEditFromList.value = fromList;
  noteSheetView.value = 'edit';
  noteSheetEl.value?.show?.(); // no-op when already open (came from the list)
}
function resetNoteCreator() {
  noteCreator.open = false;
  noteCreator.range = null;
  noteCreator.editing = null;
  noteCreator.initialNote = null;
}
// Back button (edit → list): only offered when editing started from the list.
function noteEditorBack() {
  resetNoteCreator();
  noteSheetView.value = 'list';
}
// Cancel the editor: back to the list if that is where it came from, else close.
function noteEditCancel() {
  if (noteEditFromList.value) noteEditorBack();
  else noteSheetEl.value?.hide?.();
}
// The sheet finished closing (dismiss/Esc/backdrop or our own hide()).
function onNoteSheetClose() {
  resetNoteCreator();
  noteSheetView.value = 'list';
  activeNotes.value = [];
}
// After a create/edit, re-read the badge list's notes for the group so a new or
// edited note shows immediately.
function refreshActiveNotes(span: CodePointRange | null) {
  if (!span) return;
  const g = noteGroups.value.find((gr) => gr.start === span.start && gr.end === span.end);
  if (g) activeNotes.value = [...g.notes];
}

function startNoteFromSelection(idx: number) {
  const refs = textEditorRefs[idx];
  if (!refs) return;
  const sel = refs.getSelection();
  if (sel.empty) return;
  const text = editedText.value || '';
  openNoteEditor({
    range: { start: utf16ToCp(text, sel.start), end: utf16ToCp(text, sel.end) },
    editing: null,
    initialNote: null,
    fromList: false,
  });
}
function onNoteCreated(note: Note, share: boolean) {
  const span = noteCreator.range;
  const backToList = noteEditFromList.value;
  let stored: Note;
  if (noteCreator.editing) {
    // Edit: replace the draft in place. Keep the original authorship/timestamp
    // (and id, if any); take the freshly built body/target/workflow.
    const orig = noteCreator.editing;
    const merged: Note = { ...note, __draft: true };
    if (orig.creator) merged.creator = orig.creator;
    if (orig.created) merged.created = orig.created;
    if (orig.id) merged.id = orig.id;
    const i = draftNotes.value.indexOf(orig);
    if (i >= 0) removeDraft(i);
    stored = addDraft(merged);
  } else {
    stored = addDraft(note);
  }
  resetNoteCreator();
  // "Deel met anderen binnen dit traject" was on: commit the just-created draft
  // to the traject right away. No extra confirm - the switch was the deliberate,
  // default-off opt-in. Needs traject write access (the switch only shows then).
  if (share && stored && canEdit.value) {
    void publishOneNote(stored);
  }
  // From the badge list: return to it (refreshed). Otherwise close the sheet.
  if (backToList) {
    refreshActiveNotes(span);
    noteSheetView.value = 'list';
  } else {
    noteSheetEl.value?.hide?.();
  }
}
// "Nog een notitie" on a note group: open the editor pre-set to that group's
// existing fragment (its span, already in codepoint offsets), so the new note
// targets the same quoted text without re-selecting. `fromList` marks the
// in-sheet badge list as the origin (back returns there).
function startNoteForGroup(group: NoteGroup | null | undefined, fromList = false) {
  if (group?.start == null || group?.end == null) return; // unanchored group
  openNoteEditor({
    range: { start: group.start, end: group.end },
    editing: null,
    initialNote: null,
    fromList,
  });
}
// Edit an existing draft note: open the editor pre-filled, anchored to its span.
function startEditNote(group: NoteGroup | null | undefined, note: Note, fromList = false) {
  if (group?.start == null || group?.end == null) return;
  openNoteEditor({
    range: { start: group.start, end: group.end },
    editing: note,
    initialNote: note,
    fromList,
  });
}

const activeNotes = ref<Note[]>([]);
// Notes grouped by the fragment they annotate (their primary span), so every
// comment on the same quote sits together under one quote header. Groups are
// ordered by where the fragment starts in the article text; a note without a
// resolvable span falls into a trailing "unanchored" group.
const noteGroups = computed(() => {
  const text = selectedArticle.value?.text || '';
  const groups = new Map<string, NoteGroup>();
  for (const entry of notesForArticle.value) {
    const span = entry?.spans?.[0];
    const key = span ? `${span.start}-${span.end}` : 'unanchored';
    if (!groups.has(key)) {
      // A little sentence-aware context around the fragment. Only the fragment
      // itself is italicised in the template; the context words and the ellipses
      // are not.
      const ctx = span
        ? quoteContext(text, cpToUtf16(text, span.start), cpToUtf16(text, span.end))
        : { quote: '', before: '', after: '', ellipsisBefore: '', ellipsisAfter: '' };
      groups.set(key, {
        start: span?.start ?? Number.MAX_SAFE_INTEGER,
        end: span?.end ?? null,
        ...ctx,
        notes: [],
      });
    }
    // Set just above when the key was new.
    groups.get(key)!.notes.push(entry.note);
  }
  return [...groups.values()].sort((a, b) => a.start - b.start);
});
// A draft note lives in this browser's own localStorage, so it is the user's
// by construction - every draft is deletable. Ownership (creator.id) is only
// meaningful for committed notes in the shared repo; gating drafts on it broke
// per-note delete for pre-login drafts (no creator) and legacy drafts (whose
// creator.id was stored undefined by an earlier bug), leaving only the bulk
// wipe. Committed notes are not __draft, so they still have no delete path.
function canDeleteNote(note: Note | null | undefined) {
  return !!note?.__draft;
}
function deleteNote(note: Note) {
  const i = draftNotes.value.indexOf(note);
  if (i >= 0) removeDraft(i);
}
// Deleting a draft is irreversible (it is the only copy), so confirm first. Same
// pending-ref + show()/hide() pattern as the share modal.
const deleteModalEl = ref<NLDDModalDialog | null>(null);
const deletePending = ref<Note | null>(null);
watch(deletePending, (note) => {
  const el = deleteModalEl.value;
  if (!el) return;
  if (note && typeof el.show === 'function') el.show();
  else if (!note && typeof el.hide === 'function') el.hide();
});
function askDeleteNote(note: Note) {
  deletePending.value = note;
}
function cancelDelete() {
  if (deletePending.value === null) return; // idempotent: @close + button
  deletePending.value = null;
}
function confirmDelete() {
  const note = deletePending.value;
  deletePending.value = null;
  if (note) deleteNote(note);
}
// The note group (with its span) a given note belongs to, so the badge popover
// can drive an edit's re-anchoring the same way the Notities pane does.
function groupForNote(note: Note) {
  return noteGroups.value.find((g) => g.notes.includes(note)) ?? null;
}
// The group the open badge sheet shows (all its notes share one span), so the
// sheet can offer "Notitie toevoegen" on that same fragment.
const activeGroup = computed(() =>
  // The length check guarantees a first note.
  activeNotes.value.length ? groupForNote(activeNotes.value[0]!) : null,
);
// Sharing ("Delen") applies to your still-local drafts (a committed note is
// already on the traject branch) and needs a traject to write to.
function canPublishNote(note: Note | null | undefined) {
  return !!note?.__draft && canEdit.value;
}
// Publishing writes the note to the repo (Git) and can't be undone - it can't
// be made private again - so it goes through a confirm modal. Same show()/hide()
// pattern as other DS modal dialogs: a pending ref (the note to publish) drives
// the open/close via a watch, and @close just clears it.
const publishModalEl = ref<NLDDModalDialog | null>(null);
const publishPending = ref<Note | null>(null);
watch(publishPending, (note) => {
  const el = publishModalEl.value;
  if (!el) return;
  if (note && typeof el.show === 'function') el.show();
  else if (!note && typeof el.hide === 'function') el.hide();
});
function askPublishNote(note: Note) {
  publishPending.value = note;
}
function cancelPublish() {
  if (publishPending.value === null) return; // idempotent: @close + button
  publishPending.value = null;
}
function confirmPublish() {
  const note = publishPending.value;
  publishPending.value = null;
  if (note) publishOneNote(note);
}
async function publishOneNote(note: Note) {
  if (savingNotes.value) return;
  savingNotes.value = true;
  notesSaveError.value = null;
  notesSaveStatus.value = null;
  try {
    const { pr, noChange } = await publishNote(note);
    notesSaveStatus.value = noChange
      ? 'Notitie was al gedeeld.'
      : pr
        ? `Gedeeld in PR #${pr.number}.`
        : 'Notitie gedeeld.';
    await reloadNotes();
  } catch (e) {
    // Reads `.message` off whatever was thrown, as before.
    notesSaveError.value = (e as { message?: string } | null)?.message || 'Delen mislukt';
  } finally {
    savingNotes.value = false;
  }
}
// Look-ahead popover shown on a badge click: a flat preview of the comments with
// a CTA into the full sheet. The DS popover positions against a real element, so
// we anchor it to the badge's viewport rect via one reused invisible element.
const annotationPopEl = ref<NLDDPopover | null>(null);
let annotationAnchorEl: HTMLDivElement | null = null;
// A note's comment text and author, mirroring NoteCard, for the preview.
function noteText(note: Note) {
  // Notes are not validated (see Note), so the body shape is assumed here.
  const noteBody = note?.body as NoteBody | NoteBody[] | null | undefined;
  const body = Array.isArray(noteBody) ? noteBody[0] : noteBody;
  return body?.value || body?.source || '';
}
// The comment of a note that also links to a rule: its commenting body (or an
// untagged text body), not a tag and not the link, wherever it stands.
function noteComment(note: Note): string {
  type Body = { type?: string; purpose?: string; value?: unknown };
  const raw = (note as { body?: Body | Body[] } | null)?.body;
  const bodies: (Body | undefined)[] = Array.isArray(raw) ? raw : [raw];
  const c = bodies.find((b) => b?.type === 'TextualBody' && (b.purpose ? b.purpose === 'commenting' : true) && typeof b.value === 'string' && b.value.trim());
  return (c?.value as string | undefined) ?? '';
}
function noteAuthor(note: Note) {
  // The { id, name } shape or a legacy plain string, as in NoteCard; not validated.
  const creator = note?.creator as { name?: string } | string | null | undefined;
  return (creator as { name?: string } | null | undefined)?.name ?? creator ?? '';
}
// Clicking an annotation's badge opens the look-ahead popover anchored to the
// badge; its CTA then opens the full sheet at the list view. A merged badge can
// carry several ids.
function onEditorAnnotationClick(ids: string | string[], rect?: DOMRect | null) {
  const idList = [...new Set(Array.isArray(ids) ? ids : [ids])];
  activeNotes.value = idList.map((id) => noteById.value.get(id)).filter((n): n is Note => Boolean(n));
  if (!activeNotes.value.length) return;
  if (!rect) {
    // No rect to anchor to (e.g. keyboard activation) - skip the popover.
    openNoteListSheet();
    return;
  }
  if (!annotationAnchorEl) {
    annotationAnchorEl = document.createElement('div');
    annotationAnchorEl.setAttribute('aria-hidden', 'true');
    annotationAnchorEl.style.cssText = 'position:fixed;pointer-events:none;z-index:-1;';
    document.body.appendChild(annotationAnchorEl);
  }
  annotationAnchorEl.style.left = `${rect.left}px`;
  annotationAnchorEl.style.top = `${rect.top}px`;
  annotationAnchorEl.style.width = `${rect.width}px`;
  annotationAnchorEl.style.height = `${rect.height}px`;
  const pop = annotationPopEl.value;
  if (pop) {
    pop.anchorElement = annotationAnchorEl;
    pop.show?.();
  }
}
function openNoteListSheet() {
  noteSheetView.value = 'list';
  noteSheetEl.value?.show?.();
}
// CTA in the look-ahead popover: close it and open the full sheet.
function openSheetFromAnnotation() {
  annotationPopEl.value?.hide?.();
  openNoteListSheet();
}
onBeforeUnmount(() => {
  annotationAnchorEl?.remove();
  annotationAnchorEl = null;
});

const exporting = ref(false);
// Local YYYY-MM-DD-HH-mm-ss stamp for download filenames - hyphens throughout
// (no underscores, no colons), sortable, and unique enough to avoid clobbering
// an earlier export from the same article.
function fileTimestamp() {
  const d = new Date();
  const p = (n: number) => String(n).padStart(2, '0');
  return `${d.getFullYear()}-${p(d.getMonth() + 1)}-${p(d.getDate())}-${p(d.getHours())}-${p(d.getMinutes())}-${p(d.getSeconds())}`;
}

// Law id as a filename part, hyphenated (the corpus slug uses underscores).
function lawSlug() {
  return (lawId.value || 'regelwerk').replace(/_/g, '-');
}

// Trigger a browser download of `text` as `filename`. Revoke on a later tick:
// a programmatic anchor download starts asynchronously, and revoking
// synchronously after click() races the browser fetching the blob (unreliable
// in Safari/Firefox).
function downloadYaml(text: string, filename: string) {
  const blob = new Blob([text], { type: 'text/yaml' });
  const url = URL.createObjectURL(blob);
  const a = document.createElement('a');
  a.href = url;
  a.download = filename;
  a.click();
  setTimeout(() => URL.revokeObjectURL(url), 0);
}

// Whole-law export: every committed note (from the sidecar) plus the drafts.
async function exportNotes() {
  if (exporting.value) return; // a second click would download a duplicate
  exporting.value = true;
  try {
    downloadYaml(await exportYaml(), `${lawSlug()}-annotations-${fileTimestamp()}.yaml`);
  } finally {
    exporting.value = false;
  }
}

// Article-scoped export: only the notes that resolve to the open article
// (committed + drafts), which EditorView already has as notesForArticle.
function exportArticleNotes() {
  if (exporting.value) return;
  exporting.value = true;
  try {
    const notes = notesForArticle.value.map((e) => e.note);
    const nr = selectedArticleNumber.value || 'artikel';
    downloadYaml(exportYamlFromNotes(notes), `${lawSlug()}-artikel-${nr}-annotations-${fileTimestamp()}.yaml`);
  } finally {
    exporting.value = false;
  }
}

// Note write-back: PUT the new drafts to editor-api, which appends them to
// the sidecar on the active traject's branch (same write model as law and
// scenario edits since #632). No source picker - the traject's own corpus
// config decides where the notes land. The "PR #N" toolbar badge is driven
// by the shared lastSavedPr ref, so a save that produced a PR lights it up
// with no extra wiring here.
const savingNotes = ref(false);
const notesSaveError = ref<string | null>(null);
// Explicit success signal: a PR-less / NoChange save must not look like
// the work vanished (the drafts get cleared either way).
const notesSaveStatus = ref<string | null>(null);
// The save status/error describe the LAST publish. A NEW draft appearing
// afterwards makes the confirmation stale ("Notitie gedeeld" next to a fresh
// unsaved draft is contradictory), so clear it then. Sharing itself removes the
// just-shared draft - clearing on a DECREASE would wipe the very confirmation
// publishOneNote is about to set. Only react to an increase; the count going
// down is a share/delete completing.
watch(draftCount, (count, prev) => {
  if (count > prev) {
    notesSaveStatus.value = null;
    notesSaveError.value = null;
  }
});

const resultSheetOpen = ref(false);
const graphSheetOpen = ref(false);

// --- Corpus search (reuse the shared SearchPopover) ---
const searchPopoverRef = ref(null);
// The toolbar search control lives in the AppShell; register our popover so
// the shell's search button/field opens it.
registerSearchPopover(searchPopoverRef);

// Display names from the corpus reader - the same per-traject list
// MachineReadable reads, so an editor mount fires ONE laws-list fetch.
// Traject switches follow the route; fetch failures degrade to the
// humanized law id.
const corpus = useCorpus();
const corpusDisplayName = corpus.displayName;

/**
 * Display name for the failed law on the error inline-dialog. Tries the
 * corpus index first; falls back to the URL slug (via `humanizeLawId`
 * inside `displayName`) so the user always sees a concrete identifier.
 */
const failedLawName = computed(() => {
  const id = lawId.value;
  if (!id) return '';
  return corpusDisplayName(id);
});

// True when the law fetch errored with 404 (law missing or not in active traject's corpus).
const lawErrorIs404 = computed(() => error.value?.status === 404);

/**
 * Retry the failed law fetch. `open` clears `error` and re-runs the
 * fetch; failed responses don't enter the cache so a retry actually hits
 * the network again.
 */
function retryLoadLaw() {
  if (!lawId.value) return;
  openLaw.open({ lawId: lawId.value, articleNumber: selectedArticleNumber.value, minLoadingMs: RETRY_MIN_SPINNER_MS });
}

function onSearchSelectLaw(lawIdVal: string) {
  // Open in the library - search currently only matches law names. As
  // soon as article-level search lands, we can route directly into the
  // editor (with the chosen article as the active tab).
  router.push(libraryRouteFor(lawIdVal));
}

const resultSheetEl = ref<NLDDSheet | null>(null);
watch(resultSheetOpen, async (open) => {
  await nextTick();
  if (open) resultSheetEl.value?.show();
  else resultSheetEl.value?.hide();
});
const graphSheetEl = ref<NLDDSheet | null>(null);
watch(graphSheetOpen, async (open) => {
  await nextTick();
  if (open) graphSheetEl.value?.show();
  else graphSheetEl.value?.hide();
});

// The per-traject tab store (`useOpenTabs`) and the restore-on-entry flow
// (`useTabRestore`) are initialised near the useOpenLaw block above; the storage
// layer lives in lib/openTabsStorage.js (per-traject keys, sanitising reads,
// try/catch safe defaults). What follows are the editor-side wrappers and
// watches that drive them.

/**
 * Build a router target for the editor that preserves the current
 * traject scope. With a traject in the URL the user stays in
 * `editor-traject` (full edit mode). EditorView only ever mounts under a
 * traject (the editor requires one), so the no-traject branch should not
 * fire here - but for safety it routes to the chooser with the law as
 * query rather than the (now removed) no-traject editor.
 */
function editorRouteFor(
  lawIdVal: string | null | undefined,
  articleNumber: string | null | undefined,
  { keepReviewTask = true }: { keepReviewTask?: boolean } = {},
) {
  const trajectRef = singleParam(route.params.trajectRef);
  // Een lopende beoordeling reist mee met de navigatie: `?task=` blijft op elke
  // editor-URL staan tot de taak is afgehandeld (`clearReviewQuery` bouwt de
  // enige route zonder). Zo overleeft de beoordeling een uitstapje naar een
  // ander artikel - en een refresh onderweg - in plaats van stilletjes weg te
  // vallen bij de eerste tabwissel. Zonder wet is er niets om bij te horen.
  const task = keepReviewTask && lawIdVal ? reviewTaskIdParam.value : null;
  if (trajectRef) {
    return {
      name: 'editor-traject',
      params: { trajectRef, lawId: lawIdVal, articleNumber },
      ...(task ? { query: { task } } : {}),
    };
  }
  return {
    name: 'editor',
    query: { law: lawIdVal || undefined, article: articleNumber || undefined, task: task || undefined },
  };
}

/**
 * Build a router target for the bibliotheek that preserves the current
 * traject scope, mirroring editorRouteFor. Used when leaving the editor
 * for the library (e.g. opening a search result) so the active traject
 * follows the user instead of being dropped.
 */
function libraryRouteFor(lawIdVal: string) {
  return homeTarget({ trajectRef: route.params.trajectRef, lawId: lawIdVal });
}

// Cache for law names, keyed by `${trajectRef}::${lawId}`. Keyed per traject
// (not by lawId alone) because a concept edit can give the same law a different
// name per traject, so a bare-lawId cache would show the wrong label after a
// switch.
const lawNames = ref<Record<string, string>>({});
function lawNameKey(trajectRef: string | null | undefined, lawIdVal: string) {
  return `${trajectRef || ''}::${lawIdVal}`;
}

// Set true for the tick a traject switch takes to settle, so the auto-open
// robustness net below doesn't race the restore. A plain (non-reactive) latch
// is enough: the switch sets it before mutating and clears it after nextTick.
let suppressTabAutoOpen = false;

function tabKey(tab: OpenTab) {
  return `${tab.lawId}:${tab.articleNumber}`;
}

// Same-shape wrapper over the store's explicit-ref lookup, scoped to the active
// traject (the only traject EditorView's watches ever add to).
function findTab(lawIdVal: string, articleNumber: string | number) {
  return findTabIn(activeTrajectRef.value, lawIdVal, articleNumber);
}

// Add tab when initial law loads
watch([() => lawId.value, selectedArticle], ([id, article]) => {
  if (!id || !article) return;
  // Only record a tab for the traject the law was actually loaded through.
  // During a cross-traject route change (browser back/forward, or any nav to a
  // law URL in another traject) `open` loads the law a tick before the
  // route - and thus `activeTrajectRef` - commits to the new traject. Keying on
  // `activeTrajectRef.value` here would append this law to (and persist it
  // under) the traject we're *leaving*, leaking a foreign law into its tab bar.
  // Defer to watch(activeTrajectRef), which reconciles the bar under the
  // correct traject once it commits.
  if (lawTrajectRef.value !== activeTrajectRef.value) return;
  // openTab de-dups, caps at MAX_TABS and marks it active - all under the active
  // traject's own bucket.
  openTab(activeTrajectRef.value, { lawId: id, articleNumber: String(article.number) });
  if (lawName.value) {
    lawNames.value = { ...lawNames.value, [lawNameKey(activeTrajectRef.value, id)]: lawName.value };
  }
});

// Also populate lawNames when lawName resolves
watch(lawName, (name) => {
  if (name && lawId.value) {
    lawNames.value = {
      ...lawNames.value,
      [lawNameKey(activeTrajectRef.value, lawId.value)]: name,
    };
  }
});

// Resolves to false when the user chose to stay on the article that is open
// (unsaved edits), so the tab bar can put its selection back.
async function selectTab(tab: OpenTab): Promise<boolean> {
  if (!isOpenArticle(tab.lawId, tab.articleNumber) && !(await confirmLeave())) return false;
  setActiveTab(activeTrajectRef.value, tab);
  // Drop an open action edit; it belongs to the article we are leaving.
  if (activeAction.value) {
    handleActionClose();
  }
  if (tab.lawId === lawId.value) {
    selectArticle(tab.articleNumber);
  } else {
    // Fetch through the tab's own traject (every open tab belongs to the
    // active traject). Passing it explicitly matters when this runs right
    // after a traject switch with no law in the URL: the open law's traject
    // is still the previous one, so without this the law would be read
    // through the old traject's scope.
    const won = await openLaw.open({
      lawId: tab.lawId,
      articleNumber: tab.articleNumber,
      trajectRef: singleParam(route.params.trajectRef) || null,
    });
    if (!won) return true; // a later tab or route switch took over
    lawNames.value = {
      ...lawNames.value,
      [lawNameKey(activeTrajectRef.value, tab.lawId)]: lawName.value,
    };
  }
  // Sync the URL so deep-linking and browser back/forward stay in step.
  // `replace` (not `push`) keeps history clean - a tab switch isn't
  // navigation the user wants to undo with the back button.
  router.replace(editorRouteFor(tab.lawId, tab.articleNumber));
  return true;
}

// --- Leaving an article with unsaved edits ---------------------------------
// Only the selected article's edits are kept, so anything that leaves it -
// another tab, article, law or traject, closing its tab, back/forward,
// another page - asks first when that would lose someone's work. An untouched
// review proposal is not such work (useOpenLaw's `hasUnsavedWork`).
const leaveModalEl = ref<NLDDModalDialog | null>(null);
let pendingLeave: Promise<boolean> | null = null;
let answerPendingLeave: ((leave: boolean) => void) | null = null;

function isOpenArticle(lawIdVal: string, articleNumber: string | number): boolean {
  return lawIdVal === lawId.value && String(articleNumber) === String(selectedArticleNumber.value);
}

/**
 * Whether the user may leave the open article: at once when nothing would be
 * lost, else once they choose in the dialog. Choosing to leave throws the
 * edits away, so the navigation that follows asks nothing more. A second
 * request while the dialog is open waits for the same answer.
 */
function confirmLeave(): Promise<boolean> {
  if (!openLaw.hasUnsavedWork.value) return Promise.resolve(true);
  if (pendingLeave) return pendingLeave;
  const dialog = leaveModalEl.value;
  // Without the design system there is no dialog to ask with; let it go.
  if (!dialog?.show) return Promise.resolve(true);
  pendingLeave = new Promise<boolean>((resolve) => {
    answerPendingLeave = resolve;
  });
  dialog.show();
  return pendingLeave;
}

function answerLeave(leave: boolean) {
  const answer = answerPendingLeave;
  answerPendingLeave = null;
  pendingLeave = null;
  // The dialog's close event after a button click finds nothing pending.
  if (!answer) return;
  if (leave) discardArticle();
  leaveModalEl.value?.hide?.();
  answer(leave);
}

onBeforeRouteLeave(async () => {
  if (!(await confirmLeave())) return false;
});

// Reloading or closing the browser tab: the browser asks with its own words.
// Registered only while there is unsaved work, because a beforeunload
// listener keeps some browsers from caching the page for back/forward.
function warnBeforeUnload(event: BeforeUnloadEvent) {
  event.preventDefault();
}
watch(
  () => openLaw.hasUnsavedWork.value,
  (unsaved) => {
    if (unsaved) window.addEventListener('beforeunload', warnBeforeUnload);
    else window.removeEventListener('beforeunload', warnBeforeUnload);
  },
  { immediate: true },
);
onBeforeUnmount(() => window.removeEventListener('beforeunload', warnBeforeUnload));

// On the INITIAL mount, restore the last active article of this traject when
// the URL carries no law (a fresh load / refresh returns the user where they
// were); a law in the URL is a deep link and wins. A traject switch and browser
// back/forward run the same restore through their own hooks (the
// watch(activeTrajectRef) else-branch and the bare-root route watch below). Replaces
// the old openSavedActiveTab. Pruning a 404'ing tab is gated on confirmed
// traject membership, so at mount - before the traject list has loaded - the
// restore leaves tabs in place and a later entry re-runs it.
tabRestore
  .restoreForTraject(activeTrajectRef.value, { hasLawInUrl: !!route.params.lawId })
  .catch(console.warn);

// Robustness net: whenever there is no active tab but tabs are open (e.g. the
// active tab was closed while others remain), open the first one so the panes
// show instead of the empty state. The empty state then only appears with
// genuinely no open tabs. NEVER when the URL names an article: that is an
// explicit open, and if it fails the user must see the "niet beschikbaar /
// niet geladen" dialog for the law they asked for - not silently land on some
// other tab.
watch([activeTab, openTabs], ([tab, tabs]) => {
  if (route.params.articleNumber) return;
  // A traject switch reconciles the bar via restore; don't race it here.
  if (suppressTabAutoOpen) return;
  // tabs.length > 0 guarantees a first tab.
  if (!tab && tabs.length > 0) selectTab(tabs[0]!).catch(console.warn);
});

// A URL naming a traject the user has no access to (deleted, never existed, or
// access revoked) redirects to the chooser with a name-agnostic banner, rather
// than a dead-end error page inside the editor. The name is unknown here (no
// access = no lookup), so the banner stays generic.
watch(trajectMissing, (missing) => {
  if (missing) router.replace({ name: 'trajecten', query: { melding: 'traject-missing' } });
}, { immediate: true });

// Browser back/forward (or any external navigation) - pull state from
// URL. Local mutations from selectTab already match the destination,
// so the guards below short-circuit; the work only happens for true
// URL changes.
//
// trajectRef-only changes are intentionally NOT handled here: the
// `watch(activeTrajectRef)` above already unloads the engine and opens the
// law, and opening it twice in the same tick would burn an extra fetch (the
// first loses useOpenLaw's stale-open race, but still hits the network).
// This guard handles the law / article portion only.
onBeforeRouteUpdate(async (to) => {
  const newLawId = singleParam(to.params.lawId);
  const newArticle = singleParam(to.params.articleNumber);
  // A navigation away from the open article (another article, law or
  // traject, or the bare root) asks first when that would lose edits. One
  // that only changes the query (`?task=`) stays on the article.
  const leavesArticle =
    (newLawId ?? null) !== (lawId.value ?? null) ||
    (!!newArticle && String(newArticle) !== String(selectedArticleNumber.value)) ||
    (singleParam(to.params.trajectRef) ?? null) !== (singleParam(route.params.trajectRef) ?? null);
  if (leavesArticle && !(await confirmLeave())) return false;
  // The bare editor root is restored by the watch below, after the navigation
  // has committed.
  if (!newLawId) return;
  if (newLawId !== lawId.value) {
    const won = await openLaw.open({
      lawId: newLawId,
      articleNumber: newArticle,
      trajectRef: singleParam(to.params.trajectRef) || null,
    });
    if (!won) return;
    lawNames.value = {
      ...lawNames.value,
      [lawNameKey(singleParam(to.params.trajectRef) || null, newLawId)]: lawName.value,
    };
  } else if (newArticle && String(newArticle) !== String(selectedArticleNumber.value)) {
    selectArticle(newArticle);
  }
});

// Back/forward, the Home/Editor tab, or closing the last tab to the bare editor
// root WITHIN the same traject: watch(activeTrajectRef) won't fire (ref
// unchanged), so restore the last active article here; a trajectRef change is
// left to that watch. This must run after the navigation has committed, never
// from a route guard: a restore that ends on the root calls
// `router.replace(root)`, and from inside the guard for a navigation to that
// same root it starts a new navigation that runs the guard again. Closing the
// last tab hung the page that way. Once committed, that replace is a duplicate
// and does nothing. Only while the editor is the current route: leaving it for
// another page of the same traject has no law in the URL either.
watch(
  () => [singleParam(route.params.lawId), singleParam(route.params.trajectRef) || null] as const,
  ([lawIdInUrl, trajectRef], [, previousTrajectRef]) => {
    if (route.name !== 'editor-traject') return;
    if (lawIdInUrl || trajectRef !== previousTrajectRef) return;
    tabRestore.restoreForTraject(trajectRef, { hasLawInUrl: false }).catch(console.warn);
  },
);

// `next` is the tab bar's own replacement pick, handed over by its tabdismiss
// event (see AppShell's onTabDismiss). Follow it: the bar has already marked
// that item selected, so choosing differently here leaves both lit. Callers
// without a bar (no pick to follow) get the same rule the bar applies: the
// neighbour to the right, else the one to the left.
// Resolves to false when the user chose to keep the article open (unsaved
// edits), so the tab bar can put its selection back.
async function closeTab(tab: OpenTab, next: OpenTab | null = null): Promise<boolean> {
  const wasActive = !!activeTab.value && tabKey(activeTab.value) === tabKey(tab);
  if (wasActive && !(await confirmLeave())) return false;
  // The store removes the tab, promotes the replacement (the bar's `next` pick,
  // else right-then-left) and persists both keys - closing the active tab and
  // reloading must not resurrect it.
  const replacement = closeTabIn(activeTrajectRef.value, tab, next);
  if (!wasActive) return true;
  // Drive selectTab so the promoted tab's law actually loads into the panes.
  if (replacement) selectTab(replacement).catch(console.warn);
  else {
    // Last tab closed: clear the panes and sit on the traject's editor root.
    clearLaw();
    router.replace(editorRouteFor(null, null));
  }
  return true;
}

function tabDisplayName(tab: OpenTab) {
  return lawNames.value[lawNameKey(activeTrajectRef.value, tab.lawId)] || humanizeLawId(tab.lawId);
}

// Publish the editor-only chrome (federated "PR #N" indicator + document
// tabs) to the AppShell, which can't own these statically. Tab actions are
// registered once; the values stay in sync reactively while mounted and are
// cleared on unmount so the library never shows a stale PR badge or tab bar.
// Drag/keyboard reorder from the mobile tabs sheet. The nldd-list dispatches
// nldd-reorder with array indices; mirror the move into openTabs so the new
// order persists (and the md+ document-tab-bar follows it).
function reorderTabs(fromIndex: number, toIndex: number) {
  reorderTabsIn(activeTrajectRef.value, fromIndex, toIndex);
}

registerTabActions({
  key: tabKey,
  displayName: tabDisplayName,
  // A failure is logged and treated as done: the bar keeps what it shows.
  select: (tab) => selectTab(tab).catch((e) => { console.warn(e); return true; }),
  close: (tab, next) => closeTab(tab, next).catch((e) => { console.warn(e); return true; }),
  reorder: reorderTabs,
});
watchEffect(() => {
  setEditorChrome({
    pr: lastSavedPr.value,
    tabs: openTabs.value,
    activeTab: activeTab.value,
    // Publish the traject the tabs belong to IN THE SAME effect that publishes
    // the tabs, so the bar can key its rebuild on a value that moves together
    // with `documentTabs` - not a sibling ref that flips a tick earlier and
    // would rebuild the bar against the old tabs (leaving a spooktab).
    trajectRef: publishedTrajectRef.value,
  });
});
onBeforeUnmount(clearEditorChrome);

// Load lawNames for the open tabs (parallel, deduplicated). Reads go through
// the active traject, which callers pass as `trajectRef` to key the labels, so
// tab labels match what the editor pane shows after a save. Runs on startup
// and again after a traject switch, since a switch swaps in that traject's own
// tab set whose labels still need resolving.
function backfillTabLabels(trajectRef: string | null | undefined) {
  const uniqueLawIds = [...new Set(tabsFor(trajectRef).map(t => t.lawId))];
  Promise.all(uniqueLawIds.map(async (id) => {
    try {
      const entry = await corpus.law(id);
      // Drop late results from a superseded switch: a fast A -> B -> C swap
      // could otherwise land B's label on C's tab when both trajects have the
      // same law open (its name may differ per traject after a concept edit).
      if (trajectRef !== activeTrajectRef.value) return;
      lawNames.value = { ...lawNames.value, [lawNameKey(trajectRef, id)]: entry.lawName };
    } catch { /* ignore */ }
  }));
}
backfillTabLabels(activeTrajectRef.value);

// --- Engine ---
const {
  ready: engineReady,
  initError: engineInitError,
  initEngine,
  getEngine,
  loadLawYaml,
  unloadAllLaws,
} = useEngine();
initEngine().catch(() => {});

// The engine-loading watch lives below, next to `currentLawYaml`, so it
// observes in-memory edits rather than only the persisted `rawYaml`.

// --- Trace state (receives trace from last executed scenario) ---
const lastTraceText = ref<string | null>(null);
const lastResult = ref<ScenarioExecutedPayload['result']>(null);
const lastError = ref<string | null>(null);
const lastExpectations = ref<ScenarioExecutedPayload['expectations']>({});
const lastScenarioName = ref('');
// The scenario's entry output (e.g. "is_rechthebbende"). The graph view
// uses this to pin its "▶ start" marker to the right leaf.
const lastOutputName = ref<string | null>(null);
// Declared units of the opened law's outputs, so the result sheet renders a
// value as money only when its type_spec says so. Scenarios execute the opened
// law, so its articles carry the outputs the trace reports.
const outputTypeMap = computed(() => buildOutputTypeMap(articles.value));
// Loading state of the last scenario and a bound re-run callback, so the
// result sheet can show "running…" / an error with a reload action.
const lastRunning = ref(false);
const lastReload = ref<(() => void) | null>(null);

function handleScenarioExecuted({ result, traceText, error, running, expectations, scenarioName, outputName, reload, view }: ScenarioExecutedPayload) {
  lastResult.value = result;
  // The engine hands trace text and errors back untyped; ExecutionTraceView
  // renders them as text, so they are passed on as they are.
  lastTraceText.value = traceText as string | null;
  lastError.value = (error || null) as string | null;
  lastRunning.value = !!running;
  lastReload.value = typeof reload === 'function' ? reload : null;
  lastExpectations.value = expectations || {};
  lastScenarioName.value = scenarioName || '';
  lastOutputName.value = outputName || null;
  // Open exactly one of the two sheets - opening the second on top of
  // the first would leave the previous one as a stale layer that
  // re-appears when the user dismisses the foreground sheet.
  if (view === 'graph') {
    resultSheetOpen.value = false;
    graphSheetOpen.value = true;
  } else {
    graphSheetOpen.value = false;
    resultSheetOpen.value = true;
  }
}

// Clear the captured trace whenever the active law changes - otherwise
// LawGraphView would re-flatten the old trace under the new lawId,
// misattribute every step to the new law, and pin the "▶ start" badge
// to a leaf that just happens to share the previous output's name.
// The trace and graph sheets close along with the trace they were
// showing: leaving them open over an empty graph or a fresh law the
// user just navigated into is more confusing than auto-dismissing.
watch(lawId, () => {
  lastResult.value = null;
  lastTraceText.value = null;
  lastError.value = null;
  lastRunning.value = false;
  lastReload.value = null;
  lastExpectations.value = {};
  lastScenarioName.value = '';
  lastOutputName.value = null;
  resultSheetOpen.value = false;
  graphSheetOpen.value = false;
});

// --- Editor state ---
// The sheet for a definition or row of the Machine pane. The edits it makes
// live in `useOpenLaw`; which item is open is this view's concern.
const activeEditItem = ref<EditSheetItem | null>(null);

// Per-pane refs to the ArticleTextEditor instance so the header toolbar can
// render the formatting controls (Bold/Italic/lists) next to the existing
// pane-view dropdown rather than the editor drawing its own duplicate
// label dropdown inside the body. Functional ref keeps the map in sync as
// panes mount/unmount.
const textEditorRefs = reactive<Record<string, TextEditorInstance>>({});
function setTextEditorRef(idx: number) {
  return (el: Element | ComponentPublicInstance | null) => {
    if (el) {
      // The ref sits on an ArticleTextEditor, so the instance is one.
      textEditorRefs[idx] = el as TextEditorInstance;
    } else {
      delete textEditorRefs[idx];
    }
  };
}

// --- Tekst-opmaak (segmented controls in de header-toolbar) ---
// De ArticleTextEditor (Tiptap) per pane is de bron van waarheid; deze helpers
// vertalen activeFormats <-> de segmented-control-waardes en sturen de
// toggle-commando's aan. Lezen van activeFormats in de template houdt de
// controls reactief in sync met de selectie.

// Vet/Schuin: checkbox-control - de geselecteerde waardes zijn de actieve
// inline-formats (beide kunnen tegelijk aan staan).
function boldItalicValues(idx: number) {
  const refs = textEditorRefs[idx];
  if (!refs) return [];
  const values = [];
  if (refs.activeFormats.bold) values.push('bold');
  if (refs.activeFormats.italic) values.push('italic');
  return values;
}
function onInlineFormatChange(idx: number, e: Event) {
  const refs = textEditorRefs[idx];
  if (!refs) return;
  // nldd-segmented-control `item-change`: detail { value, checked }.
  const { value } = (e as CustomEvent<{ value: string; checked: boolean }>).detail;
  if (value === 'bold') refs.toggleBold();
  else if (value === 'italic') refs.toggleItalic();
}

// Lijst: radio-control met none/bullet/ordered. Tiptap's toggle-commando's
// converteren tussen lijsttypes en heffen de actieve lijst op, dus elke
// overgang is met één toggle te maken.
function listValue(idx: number) {
  const refs = textEditorRefs[idx];
  if (!refs) return 'none';
  if (refs.activeFormats.bulletList) return 'bullet';
  if (refs.activeFormats.orderedList) return 'ordered';
  return 'none';
}
function setList(idx: number, target: string) {
  const refs = textEditorRefs[idx];
  if (!refs) return;
  const current = listValue(idx);
  if (target === current) return;
  if (target === 'bullet') refs.toggleBulletList();
  else if (target === 'ordered') refs.toggleOrderedList();
  else if (refs.activeFormats.bulletList) refs.toggleBulletList();
  else if (refs.activeFormats.orderedList) refs.toggleOrderedList();
}
function onListChange(idx: number, e: Event) {
  // nldd-segmented-control `change` for a radio control: detail { value }.
  setList(idx, (e as CustomEvent<{ value: string }>).detail.value);
}

const dumpOpts = { lineWidth: 80, noRefs: true };

// Another article closes the item sheet; the module drops the edits itself.
watch(selectedArticle, () => {
  activeEditItem.value = null;
});

// Load current law into engine. Reacts to currentLawYaml so in-memory
// edits are immediately visible to scenarios. Goes through
// useEngine.loadLawYaml so the engine's scope-tracking map sees this
// load under the right traject - otherwise a later loadDependency call
// for the same law id would treat it as already-current and skip the
// refetch on a traject switch.
// `currentLawYaml` re-dumps on every keystroke, so reacting directly would
// reload the WASM engine per keystroke. Debounce the reload ~300ms after the
// last edit; keep the first load (no previous yaml) synchronous so the initial
// load isn't delayed. Subsequent transitions from an existing yaml - keystroke
// edits, article switches, traject switches - debounce.
let engineLoadDebounce: ReturnType<typeof setTimeout> | undefined;

// The engine's verdict on the current YAML, shown to the author as a
// page-wide banner. `loadLaw` rejects a law that fails its type check
// (RFC-037, e.g. "article 2: 'huur' is not nullable ...") the same way it
// rejects an unparseable one, and it has already dropped the previous copy
// by then, so every scenario would otherwise fail with "Law not found"
// without saying why. The WASM binding throws a plain string.
const engineLoadError = ref<string | null>(null);

async function reloadEngineLaw(lawYaml: string | null, isReady: boolean) {
  if (!isReady || !lawYaml) return;
  try {
    await loadLawYaml(lawYaml, lawId.value, activeTrajectRef.value);
    engineLoadError.value = null;
  } catch (e) {
    // Reads `.message` off whatever was thrown, as before.
    engineLoadError.value = typeof e === 'string' ? e : ((e as { message?: string } | null)?.message || String(e));
    // Plain first argument: the law id is user text, not a format string.
    console.warn('Failed to load law into engine:', lawId.value, e);
  }
}

watch(
  [currentLawYaml, engineReady],
  ([lawYaml, isReady], prev) => {
    clearTimeout(engineLoadDebounce);
    // First load (no previous yaml): run immediately so scenarios see the law
    // without delay. Any change from an existing yaml debounces.
    if (!prev || !prev[0]) {
      reloadEngineLaw(lawYaml, isReady);
      return;
    }
    engineLoadDebounce = setTimeout(() => reloadEngineLaw(lawYaml, isReady), 300);
  },
  { immediate: true },
);

onBeforeUnmount(() => clearTimeout(engineLoadDebounce));

// --- Review-modus (job_review-taak) -----------------------------------
// `?task=<id>`: show a job_review task's
// proposed law YAML as an unsaved edit rather than fetching it
// separately. The proposal is applied to the first article where it
// diverges from the saved law (seeding `editedText`/`machineReadable`,
// the same pane-local "current" refs a manual edit would touch), so the
// existing dirty-tracking and Wijzigingenbalk (Opslaan/Wijzigingen-
// ongedaan) drive the review UI the same way they drive a manual edit.
// Seeden is artikel-scoped: de taak wijst het artikel aan (`payload.article`)
// en de editor toont dat.
//
// Opslaan schrijft niet meer. Een verrijking is de eenheid die geschreven
// wordt, dus "Neem voorstel over" legt het oordeel over dít onderdeel vast en
// brengt je naar het volgende; pas als elk onderdeel een oordeel heeft, gaat
// alles in één keer naar de backend, die er één commit van maakt. Tot dat
// moment verandert er niets aan de wet.
const {
  reviewTask,
  proposedContent: reviewProposedContent,
  loadError: reviewLoadError,
  partIndex: reviewPartIndex,
  partCount: reviewPartCount,
  undecidedParts: reviewUndecidedParts,
  loadReview,
  decide: decideReviewPart,
  processEnrichment,
  reset: resetReview,
  approveAfterSave,
  reject: rejectReviewInternal,
} = useTaskReview();
// `law_create`-taak: de wet bestaat nog niet in het traject; het voorstel
// wordt integraal in de editor geseed en Opslaan gaat via het create-pad
// (POST) in plaats van de PUT.
const reviewIsLawCreate = computed(
  () => reviewTask.value?.payload?.kind === 'law_create',
);
// Het artikel waar deze taak over gaat. De worker maakt sinds de opsplitsing
// één review-taak per gewijzigd artikel en zet het nummer in de payload, zodat
// een beoordelaar zich uitspreekt over wat er werkelijk veranderd is.
//
// Zonder nummer gaat de taak over de hele wet. Dat blijft bestaan voor een
// `law_create` (de wet ís het voorstel), voor een voorstel dat de worker niet
// kon opsplitsen, en voor taken van vóór deze wijziging.
const reviewArticleNumber = computed(() => {
  const number = reviewTask.value?.payload?.article;
  return number == null ? null : String(number);
});
// Kijk je op dit moment naar waar de taak over gaat? De taak reist mee met de
// navigatie (zie `editorRouteFor`), zodat een uitstapje naar een ander artikel
// de beoordeling niet weggooit - maar de beoordeel-weergave hoort alleen te
// staan boven het artikel waarover je je uitspreekt. Anders hangt de balk over
// vreemde inhoud en keurt "Opslaan" de taak goed met een artikel dat de
// verrijking nooit heeft aangeraakt.
//
// Een taak zonder artikelnummer gaat over de hele wet: die slaat het voorstel
// integraal op (`reviewSavesWholeLaw`), dus dan is elk artikel van die wet een
// geldige plek om hem te beoordelen.
const reviewOnTarget = computed(() => {
  const task = reviewTask.value;
  if (!task) return false;
  const taskLawId = task.payload?.law_id;
  if (taskLawId && lawId.value && taskLawId !== lawId.value) return false;
  const number = reviewArticleNumber.value;
  if (number == null) return true;
  return String(selectedArticleNumber.value ?? '') === number;
});
// De beoordeel-weergave (balk, Opslaan-als-goedkeuren, Verwerpen) staat aan.
// Bewust smaller dan "er is een taak geladen": zie `reviewOnTarget`.
const reviewActive = computed(() => !!reviewTask.value && reviewOnTarget.value);

// Een taak die één artikel aanwijst, accordeert dat artikel zoals het in de
// panes staat - inclusief wat de beoordelaar er zelf nog aan veranderde. Een
// taak zonder artikelnummer accordeert het voorstel in zijn geheel, want daar
// is geen kleinere eenheid om over te beslissen.
const reviewSavesWholeLaw = computed(() => reviewActive.value && !reviewArticleNumber.value);
// Guards against re-firing loadReview for a task id already attempted -
// approve/reject null out `reviewTask`, which would otherwise look
// indistinguishable from "not loaded yet" and re-trigger against the
// task we just resolved.
let reviewAttemptedForTaskId: string | null = null;

// Drop `?task=` from the URL once the review is resolved (approved or
// rejected) so a refresh/back-navigation doesn't re-open review mode.
// Rebuilt from the CURRENT law/article (not `route.params`, which still
// names whatever article the URL originally pointed at): applyProposedContent
// may have moved `selectedArticleNumber` to the article the proposal
// actually touches, and `onBeforeRouteUpdate` would otherwise see the
// stale route param disagree with `selectedArticleNumber` and snap the
// editor back to the pre-review article right after resolving.
function clearReviewQuery() {
  router.replace(
    editorRouteFor(lawId.value, selectedArticleNumber.value, { keepReviewTask: false }),
  );
}

// Whether the proposal seeded anything (false when every proposed article
// matches the saved law, or the only differences are articles the saved
// law doesn't have - see the comment on the loop below).
const reviewSeeded = ref(false);
// Whether the proposal touches article(s) beyond the single one seeded into
// the editor panes (or, when nothing was seedable, the proposal touches
// anything at all). The visible editor is always single-article-scoped, but
// Opslaan now approves the FULL proposal regardless - this only drives the
// banner copy that points the reviewer at the YAML panel for the rest.
const reviewHasHiddenChanges = ref(false);
// The article the proposal should land in (`payload.article`, or - for a
// proposal about the whole law - the article `proposalDivergence` picked), and
// the article it actually sits in right now. The two part as soon as you look
// at another article: useOpenLaw then drops the edits and the panes show the
// saved law again. That difference is exactly the signal to seed again on
// return.
const reviewSeedTarget = ref<string | null>(null);
// Where the proposal actually sits in the panes right now, as `law::article`.
// The law belongs in the key: two laws in one traject can share article
// numbers ("B 5", "1", "2" are not unique), and a law switch then leaves
// `selectedArticleNumber` as it is while the panes went back to the saved law.
// On the number alone the proposal would not go in again on return.
const reviewSeededAt = ref<string | null>(null);
function reviewSeedKey(number: string | null | undefined) {
  return `${lawId.value ?? ''}::${number ?? ''}`;
}
watch([lawId, selectedArticleNumber], ([, nr]) => {
  if (reviewSeededAt.value && reviewSeededAt.value !== reviewSeedKey(nr)) {
    reviewSeededAt.value = null;
  }
});

function applyProposedContent(proposedYaml: string) {
  reviewSeeded.value = false;
  reviewHasHiddenChanges.value = false;
  let proposed: LawDocument | null;
  try {
    // Unchecked YAML; `articles` is checked with Array.isArray below.
    proposed = yaml.load(proposedYaml) as LawDocument | null;
  } catch {
    return; // malformed proposal - leave the saved content in place
  }
  const proposedArticles: LawArticle[] = Array.isArray(proposed?.articles) ? proposed.articles : [];
  // v1 can only seed an EXISTING article as an unsaved edit (same
  // single-article model as `currentLawYaml`'s KNOWN LIMITATION below,
  // which has no way to splice in an article the saved law doesn't have),
  // and can't show a removed article either - proposalDivergence folds both
  // into `hiddenChanges` so the banner points at the YAML panel for them.
  const { target, hiddenChanges } = proposalDivergence(
    articles.value,
    proposedArticles,
    reviewArticleNumber.value,
  );
  reviewHasHiddenChanges.value = hiddenChanges;
  if (!target) return; // nothing seedable differs - nothing to seed
  reviewSeeded.value = true;
  reviewSeedTarget.value = String(target.number);
  // Set before seeding: the watches that clean up on an article switch and
  // re-seed on return run in the next flush, and must already see this as
  // seeded - otherwise they would seed over it again.
  reviewSeededAt.value = reviewSeedKey(String(target.number));
  // Selects the article and puts the proposal in its edits, in one call. The
  // proposal is the leave guard's baseline: until the reviewer edits it,
  // leaving loses nothing, since it is seeded again on return.
  openLaw.seedProposal(target);
}

// Variant + copy for the review-mode bar (a low, full-width nldd-banner -
// see PR #935 UX feedback; it used to be a page-height nldd-inline-dialog
// that ate half the viewport). Opslaan always approves the FULL proposal
// (see `handleLawSave`), so the copy says so explicitly; when the proposal
// touches more than the single seeded article (or nothing could be seeded
// at all), point the reviewer at the YAML panel (`panel.yaml_editor`) for
// the parts the article-scoped editor can't show. A bar has no room for
// paragraphs, so copy stays to a heading line + one short supporting line.
const REVIEW_HIDDEN_CHANGES_NOTE = 'Zie ook het YAML-paneel voor wijzigingen buiten dit artikel.';
// Accent, not neutral: a generated proposal is the one thing on screen that
// asks for a decision, and a neutral bar reads as background chrome.
const reviewBannerVariant = computed(() => {
  if (reviewLoadError.value) return 'critical';
  if (reviewIsLawCreate.value) return 'accent';
  if (reviewActive.value && !reviewSeeded.value) return 'warning';
  return 'accent';
});
const reviewBannerText = computed(() =>
  reviewIsLawCreate.value ? 'Nieuw regelwerk uit documentconversie' : 'Voorstel uit verrijking',
);
const reviewBannerSupportingText = computed(() => {
  if (reviewLoadError.value) return reviewLoadError.value;
  if (reviewIsLawCreate.value) {
    return 'Controleer het regelwerk; Opslaan voegt het toe aan het traject, Verwerpen wijst af.';
  }
  if (!reviewSeeded.value) {
    return reviewArticleNumber.value
      ? `Artikel ${reviewArticleNumber.value} wijkt niet af van het voorstel; er valt niets te wijzigen.`
      : 'Voorstel raakt alleen inhoud die hier niet zichtbaar is - bekijk het YAML-paneel.';
  }
  // Artikel-scoped: het oordeel gaat over dit artikel zoals het hier staat,
  // dus eigen aanpassingen gaan wél mee.
  if (!reviewSavesWholeLaw.value) {
    return `Overnemen bewaart artikel ${reviewArticleNumber.value} zoals het hier staat, Verwerpen laat het ongemoeid.`;
  }
  return (
    'Overnemen neemt het volledige voorstel over (eigen aanpassingen gaan niet mee), Verwerpen wijst af.' +
    (reviewHasHiddenChanges.value ? ` ${REVIEW_HIDDEN_CHANGES_NOTE}` : '')
  );
});

// Which panes carry the proposal, for de review-melding. De verrijking schrijft
// machine_readable en scenario's, niet de wettekst, dus Tekst hoort hier niet
// in. YAML toont altijd de volledige proposal; Machine alleen als het seeden
// lukte. Scenario's staat er bewust niet bij: de worker staged wel
// `features/*.feature`, maar useTaskReview pakt alleen de wet-YAML uit de
// resultaat-blobs, dus daar komt nooit een scenario aan.
const reviewChangedPanes = computed(() => {
  const panes = [];
  if (reviewSeeded.value) panes.push('Machine');
  panes.push('YAML');
  return panes;
});
const reviewStatusText = computed(() => {
  if (reviewLoadError.value) return reviewLoadError.value;
  const what = reviewIsLawCreate.value
    ? 'Dit is een nieuw regelwerk uit documentconversie'
    : 'Dit is een gegenereerd voorstel';
  const panes = reviewChangedPanes.value;
  const list =
    panes.length > 1 ? `${panes.slice(0, -1).join(', ')} en ${panes.at(-1)}` : panes[0];
  return `${what}. Beoordeel ${list}.${reviewProgressNote.value}`;
});

// Waar de beoordelaar in de verrijking staat, en - zolang er nog onderdelen
// open staan - dat de wet nog niet is bijgewerkt. Zonder die tweede zin leest
// "Neem voorstel over" als "opgeslagen", terwijl er tot het laatste oordeel
// niets is weggeschreven.
const reviewProgressNote = computed(() => {
  if (reviewIsLawCreate.value || reviewPartCount.value <= 1) return '';
  const position =
    reviewPartIndex.value > 0
      ? ` Onderdeel ${reviewPartIndex.value} van ${reviewPartCount.value}.`
      : '';
  return reviewUndecidedParts.value.length > 1
    ? `${position} Het regelwerk wordt pas bijgewerkt als alle onderdelen zijn beoordeeld.`
    : `${position} Dit is het laatste onderdeel; daarna wordt het regelwerk bijgewerkt.`;
});

// Fires once the law + its first article have finished loading (whether
// that's the initial load or a tab-restore open), so it works
// regardless of how the route.query.task navigation happened to arrive.
// `reviewTaskIdParam` is itself a source: navigating from the taken-lijst
// to a wet that is ALREADY open (the target law/article is unchanged) is a
// query-only route change - `loading`/`selectedArticle` never flip, so
// without this source the watch would simply never re-fire and the review
// would never activate.
watch(
  [loading, selectedArticle, reviewTaskIdParam],
  ([isLoading, article, taskId]) => {
    if (isLoading || !article || !taskId) return;
    if (reviewAttemptedForTaskId === taskId) return;
    reviewAttemptedForTaskId = taskId;
    loadReview(taskId).then(() => {
      if (!reviewProposedContent.value) return;
      // Alleen seeden waar de taak over gaat - dezelfde vraag als voor de balk,
      // dus dezelfde `reviewOnTarget`. Een taak die één artikel aanwijst hoort
      // in dát artikel te landen, en `applyProposedContent` springt er zo nodig
      // heen. Meestal ben je er al (de takenlijst linkt erheen), maar doordat de
      // taak meereist met de navigatie staat `?task=` ook op de route van een
      // ander artikel - en van een andere wet. Kom je daar binnen (een refresh
      // terwijl je even elders keek), dan wint wat er op het scherm staat: niet
      // ongevraagd wegspringen, en zeker het voorstel niet in vreemde inhoud
      // laten landen. Het artikelnummer alleen is daarvoor geen toets: nummers
      // als "B 5" bestaan in meerdere wetten. Het seed-doel wordt wel
      // vastgelegd, zodat de her-seed-watch het oppakt zodra je er terug bent.
      if (!reviewOnTarget.value) {
        reviewSeedTarget.value = reviewArticleNumber.value;
        return;
      }
      applyProposedContent(reviewProposedContent.value);
    });
  },
  { immediate: true },
);

// `law_create`-variant van de watch hierboven: de wet bestaat nog niet, dus
// de load eindigt in een 404 en `selectedArticle` komt er nooit - de normale
// watch kan niet vuren. Bij een 404 mét ?task= halen we de taak alsnog op;
// is het een law_create-taak voor precies deze wet, dan wordt het voorstel
// integraal geseed (`seedFromYaml`) in plaats van gespliced - de hele wet ÍS
// het voorstel. Elke andere combinatie laat de bestaande 404-foutstaat staan.
watch(
  [error, reviewTaskIdParam],
  ([err, taskId]) => {
    if (!taskId || err?.status !== 404) return;
    if (reviewAttemptedForTaskId === taskId) return;
    reviewAttemptedForTaskId = taskId;
    loadReview(taskId).then(() => {
      const task = reviewTask.value;
      if (
        task?.payload?.kind === 'law_create' &&
        task?.payload?.law_id === lawId.value &&
        reviewProposedContent.value
      ) {
        seedFromYaml(reviewProposedContent.value);
        // Niets is als dirty edit geseed: de banner levert de primaire
        // opslaan-knop (zie `reviewSeeded` in de template), niet de
        // Wijzigingenbalk.
        reviewSeeded.value = false;
        reviewHasHiddenChanges.value = false;
      }
    });
  },
  { immediate: true },
);

// De keerzijde van de twee watches hierboven: `?task=` is niet alleen hoe een
// review begint, het is ook wat hem gaande houdt. Elke artikel- of wetwissel in
// de editor (een tabwissel, `selectTab`) stampt de URL opnieuw met
// `editorRouteFor`, en die draagt geen query - de taak valt er dus vanzelf uit
// zodra je iets anders bekijkt. Zonder deze watch bleef `reviewTask` staan: de
// beoordeel-balk hing over een artikel waar de taak niet over gaat, terwijl het
// geseede voorstel allang was teruggezet door `watch(selectedArticle)`. Erger
// dan lelijk: "Opslaan" keurde de taak dan goed met de inhoud van dát andere
// artikel, en zolang de modus aanstond hield `reviewReady` de knop "Beoordeel
// voorstel" verborgen waarmee je terug had gekund.
//
// De guard gaat mee terug op null: kom je later met dezelfde taak terug (via de
// takenlijst of "Beoordeel voorstel"), dan hoort het voorstel opnieuw geladen en
// geseed te worden in plaats van als "al geprobeerd" te worden overgeslagen.
watch(reviewTaskIdParam, (taskId) => {
  if (taskId) return;
  reviewAttemptedForTaskId = null;
  resetReview();
  reviewSeeded.value = false;
  reviewHasHiddenChanges.value = false;
  reviewSeedTarget.value = null;
  reviewSeededAt.value = null;
});

// Terug op het artikel van de taak: zet het voorstel er weer in. De panes zijn
// bij de artikelwissel teruggezet naar de opgeslagen wet, dus zonder dit zie je
// een beoordeling zonder iets te beoordelen - balk aan, voorstel weg. Dat was
// de tweede helft van de gemelde bug.
//
// `law_create` blijft erbuiten: daar is de wet zelf het voorstel en is het
// integraal geseed (`seedFromYaml`), niet als artikel-splice.
watch([reviewOnTarget, selectedArticle], ([onTarget, article]) => {
  if (!onTarget || !article || reviewIsLawCreate.value) return;
  if (!reviewProposedContent.value) return;
  if (reviewSeededAt.value === reviewSeedKey(String(article.number))) return;
  // Waar het voorstel heen moet. Bij een taak over één artikel ligt dat vast;
  // bij een voorstel over de hele wet kiest `proposalDivergence` het artikel, en
  // is het doel dus pas bekend na de eerste keer seeden. Kwam je binnen op een
  // andere wet, dan is dat nog niet gebeurd - laat `applyProposedContent` het
  // dan alsnog bepalen (die springt zo nodig zelf naar het juiste artikel).
  if (reviewSeedTarget.value) {
    if (String(article.number) !== reviewSeedTarget.value) return;
  } else if (reviewArticleNumber.value) {
    // Artikel-taak zonder doel: `applyProposedContent` vond niets seedbaars.
    return;
  }
  applyProposedContent(reviewProposedContent.value);
});

// "Verwerpen": bij een `law_create` is er niets samen te stellen - die taak
// staat op zichzelf en wordt direct afgehandeld. Bij een verrijking is dit een
// oordeel over één onderdeel, net als "Overnemen".
async function rejectReview() {
  if (reviewIsLawCreate.value) {
    await rejectReviewInternal();
    // De wet bestaat niet (en komt er na verwerpen ook niet): terugroutes
    // naar de wetsroute zouden op een 404 landen, dus terug naar Home.
    router.replace(libraryTabTarget.value);
    return;
  }
  await decideReview('rejected');
}

// "Rond af, neem de rest niet over": de vastgelegde oordelen plus "niet
// overnemen" voor alles wat nog open staat, in één verwerking. Zonder deze weg
// zou een verrijking van dertig artikelen dertig klikken kosten voordat er ook
// maar iets geschreven kan worden.
async function rejectRestOfReview() {
  if (!reviewActive.value || reviewIsLawCreate.value) return;
  await processReview({ rejectRemaining: true });
}

/**
 * De inhoud die de beoordelaar met "Overnemen" accordeert.
 *
 * Voor een artikel-taak: het artikel zoals het nu in de panes staat - het
 * geseede voorstel plus wat de beoordelaar er zelf nog aan veranderde. Zelfde
 * splice-regels als `currentLawYaml`, maar dan alléén het artikel: wat dat
 * betekent voor het bestand stelt de backend samen, want alleen daar kan het
 * tegen precies de bytes waartegen de `If-Match` is gecontroleerd.
 *
 * `null` betekent "neem het ruwe voorstel over" - dat is wat de backend doet
 * als er geen inhoud meekomt. Dat is de juiste terugval wanneer de editor het
 * artikel niet kan tonen (het staat niet in de opgeslagen wet), want dan heeft
 * de beoordelaar er ook niets aan kunnen veranderen.
 */
function acceptedReviewContent() {
  if (reviewSavesWholeLaw.value) return reviewProposedContent.value;
  const base = selectedArticle.value;
  if (!base || String(base.number) !== reviewArticleNumber.value) return null;
  const patched = { ...base, text: editedText.value };
  if (machineReadable.value != null) {
    patched.machine_readable = machineReadable.value;
  } else {
    delete patched.machine_readable;
  }
  try {
    return yaml.dump(patched, dumpOpts);
  } catch {
    return null;
  }
}

// Eén oordeel over het onderdeel dat nu open staat. Is het het laatste, dan
// wordt de verrijking meteen verwerkt; anders schuift de editor door naar het
// volgende onbeoordeelde onderdeel.
async function decideReview(action: Verdict['action']) {
  const { done, next } = decideReviewPart(
    action,
    action === 'approved' ? acceptedReviewContent() : null,
  );
  if (done) {
    await processReview();
    return;
  }
  // No review task is loaded (the review ended while the reject-confirmation
  // dialog was open): there is nothing to decide, so keep the panes as they are.
  if (!next) return;
  // De geseede wijziging is vastgelegd in het oordeel; de panes gaan terug
  // naar de opgeslagen wet voordat het volgende onderdeel eroverheen komt.
  discardArticle();
  router.replace(reviewRouteForPart(next));
}

// De editor-route van een ander onderdeel van dezelfde verrijking, met de
// taak-id als `?task=` zodat de bestemming hem oppakt.
function reviewRouteForPart(part: JobTaskSummary) {
  const target = editorRouteFor(lawId.value, part.article ?? selectedArticleNumber.value);
  return { ...target, query: { ...(target.query ?? {}), task: part.id } };
}

// Verwerk de verrijking: één schrijfactie, één commit, alle taken dicht. De
// backend stelt de eindstand samen, dus de wet wordt daarna opnieuw geladen -
// de client kent de uitkomst niet.
const reviewProcessing = ref(false);
const reviewProcessError = ref<RequestError | null>(null);
async function processReview(options: { rejectRemaining?: boolean } = {}) {
  reviewProcessing.value = true;
  reviewProcessError.value = null;
  try {
    await processEnrichment(currentEtag.value, options);
    await reloadLaw();
    discardArticle();
    // De wettekst kan gewijzigd zijn, dus notities moeten opnieuw ankeren.
    void reloadNotes();
    clearReviewQuery();
  } catch (e) {
    // processEnrichment and reloadLaw reject with an Error (an ApiError for HTTP failures).
    reviewProcessError.value = e as RequestError;
  } finally {
    reviewProcessing.value = false;
  }
}

// --- Verrijken aanvragen (levert een job_review-taak op) -----------------
// Fire-and-forget request; the resulting job_review task shows up in the
// taken-lijst in Home on its next poll (TasksSidebarItem/TasksListPane poll
// via useTasks() every 30s). Gedeeld met LibraryView, dat dezelfde wet in
// dezelfde panes toont: useEnrichState houdt de bezig-/voorstel-staat bij en
// ververst de gedeelde takenlijst waar dat nodig is (mount, na een aanvraag),
// zonder de 30s-poll van useTasks() te starten voor elke editor-bezoeker.
const {
  isEnriching,
  reviewReady,
  reviewArticleForPane,
  openReviewForLaw,
  openTasksForLaw,
  requestEnrich,
} = useEnrichState({
  trajectRef: activeTrajectRef,
  lawId,
  articleNumber: selectedArticleNumber,
  // Niet melden dat er een voorstel klaarstaat terwijl je die taak al aan het
  // beoordelen bent.
  reviewActive,
});
const enrichFeedback = ref<{ variant: string; text: string } | null>(null); // { variant, text } | null
// An actual traject open (write access implies a traject, see `canEdit`
// above) and a law loaded - mirrors the gates other write actions in this
// view use.
const canEnrichLaw = computed(
  () => canEdit.value && !!activeTrajectRef.value && !!lawId.value,
);

// De verrijking schrijft machine_readable en scenario's, niet de wettekst. De
// knop hoort dus bij de panes waar dat resultaat landt, niet in de toolbar van
// de tekstverwerker (waar hij stond omdat die toevallig pane 0 is). In elke
// Machine- en YAML-pane, niet één keer: die twee staan niet altijd allebei
// open, en de pane die je wél ziet moet je verder kunnen helpen.
const isEnrichPane = (view: string) => view === 'machine' || view === 'yaml';

async function enrichLaw(_trigger?: EventTarget | null, provider?: string) {
  if (!activeTrajectRef.value || !lawId.value) return;
  try {
    const { alreadyRunning, tooMany } = await requestEnrich(provider);
    if (alreadyRunning) {
      enrichFeedback.value = { variant: 'warning', text: 'Er loopt al een verrijking voor dit regelwerk.' };
    } else if (tooMany) {
      enrichFeedback.value = { variant: 'warning', text: 'Je hebt te veel verrijkingen tegelijk lopen.' };
    } else {
      enrichFeedback.value = {
        variant: 'success',
        text: 'Verrijking van alle artikelen gestart - je krijgt een taak per gewijzigd artikel zodra het resultaat klaarstaat.',
      };
    }
  } catch (e) {
    enrichFeedback.value = { variant: 'critical', text: 'Verrijking aanvragen mislukt.' };
  }
}
function dismissEnrichFeedback() {
  enrichFeedback.value = null;
}

// Single save handler shared by the Tekst and Machine panes. The PUT writes
// the whole law YAML, so one click persists every in-memory edit for the
// selected article regardless of which pane surfaced the button.
//
// Review-modus van een verrijking gaat hier NIET langs: daar is Opslaan een
// oordeel over één onderdeel en geen schrijfactie (`decideReview`). Alleen een
// `law_create` slaat hier nog op - die taak staat op zichzelf, de wet bestaat
// nog niet en gaat via het create-pad (POST). `createLaw` accepteert
// willekeurige volledige-wet-YAML, dus dat is dezelfde aanroep met andere
// inhoud; er komt geen tweede save-pad bij.
async function handleLawSave() {
  if (reviewActive.value && !reviewIsLawCreate.value) {
    await decideReview('approved');
    return;
  }
  // A whole-law review proposal saves as it is, not spliced into the article.
  const content = reviewSavesWholeLaw.value ? reviewProposedContent.value : undefined;
  if (reviewSavesWholeLaw.value && !content) return;
  // law_create: the law does not exist in the traject yet, so the proposal
  // goes through the create path (POST); later saves take the normal PUT.
  const outcome = await openLaw.save({
    content: content ?? undefined,
    create: reviewActive.value && reviewIsLawCreate.value,
  });
  if (outcome.status === 'failed') {
    // The failure is surfaced via lawSaveError; log for dev visibility.
    console.warn('saveLaw failed:', lawSaveError.value);
    return;
  }
  // The user moved to another law while the save was out.
  if (outcome.status !== 'saved') return;
  // The article text changed, so notes must re-resolve against the saved copy:
  // their quote-based selectors re-anchor via the engine, so a note whose
  // quoted text merely moved comes back at its new place, while one whose own
  // text was edited orphans (surfaced in the issues list) rather than silently
  // re-attaching. Fire-and-forget.
  if (outcome.touchedText) void reloadNotes();
  // Review mode: a successful save IS the approval (save first, then
  // resolve). The changes bar has already cleared before the task disappears.
  if (reviewActive.value) {
    await approveAfterSave();
    clearReviewQuery();
  }
}

// Whole-law save failures surface as a single notification over the editor
// (not an inline dialog buried in one pane). `saveFailure` drives it: a new
// failure (a fresh Error) shows it again, a successful save (null) removes it.
// It is a critical notification, so it stays until dismissed; a message with
// one way out is no dialog. Het verwerken van een verrijking hangt er ook
// aan - dat is voor de gebruiker dezelfde gebeurtenis (mijn wijziging kwam er
// niet in) en verdient dezelfde melding, inclusief de 412-uitleg bij een
// conflict.
const saveFailure = computed(() => reviewProcessError.value ?? lawSaveError.value);
const saveErrorText = computed(() =>
  saveFailure.value ? saveFailure.value.message || String(saveFailure.value) : '',
);
// The failure the user dismissed. `lawSaveError` lives on until the next save,
// so dismissing remembers which failure it was rather than clearing it.
const dismissedSaveFailure = ref<unknown>(null);
const showSaveError = computed(
  () => !!saveFailure.value && saveFailure.value !== dismissedSaveFailure.value,
);
function dismissSaveError() {
  dismissedSaveFailure.value = saveFailure.value;
  reviewProcessError.value = null;
}

// Alias kept to minimise template churn; both panes ultimately call the
// same whole-law save.
const handleMachineReadableSave = handleLawSave;

// --- Wijzigingenbalk (article-level pending changes) -----------------------
// Tekst + Machine edits share one whole-law save, so "dirty" is article-wide.
// The AppShell renders the bar; the editor publishes the state and the
// Opslaan / Wijzigingen-ongedaan / undo / redo actions via the chrome store.

// Throw away every in-memory edit for the selected article.
function discardArticle() {
  if (!selectedArticle.value) return;
  openLaw.discard();
  // editedText flows into each Tekst pane's Tiptap content via its model watch
  // on the next tick; clear that history afterwards so Ctrl+Z can't step back
  // into the discarded edits and re-dirty the article.
  nextTick(() => {
    for (const refs of Object.values(textEditorRefs)) refs?.clearHistory?.();
  });
}

// Undo/redo route to the last-focused Tekst editor (falling back to the first
// mounted one). Tiptap owns the history; the Machine/YAML panes have none, so
// the buttons simply stay disabled when no text editor can undo.
const activeTextEditorIdx = ref<number | null>(null);
function onTextEditorFocus(idx: number) { activeTextEditorIdx.value = idx; }
const targetTextEditor = computed(() => {
  const active = activeTextEditorIdx.value != null
    ? textEditorRefs[activeTextEditorIdx.value]
    : null;
  if (active) return active;
  const firstKey = Object.keys(textEditorRefs)[0];
  return firstKey != null ? textEditorRefs[firstKey] : null;
});
const canUndoText = computed(() => targetTextEditor.value?.canUndo ?? false);
const canRedoText = computed(() => targetTextEditor.value?.canRedo ?? false);
function undoText() { targetTextEditor.value?.undo?.(); }
function redoText() { targetTextEditor.value?.redo?.(); }

// Stable actions registered once; the reactive state re-publishes via the
// watchEffect so the bar re-renders as dirty/saving/undo-availability shift.
registerEditorActions({
  save: handleLawSave,
  discard: discardArticle,
  undo: undoText,
  redo: redoText,
  reject: rejectReview,
  rejectRest: rejectRestOfReview,
});
watchEffect(() => {
  setEditorChanges({
    dirty: articleDirty.value,
    saving: lawSaving.value || reviewProcessing.value,
    canUndo: canUndoText.value,
    canRedo: canRedoText.value,
    // `reviewProcessing` telt mee zodat de balk niet halverwege het verwerken
    // verdwijnt: `processEnrichment` sluit de review-state zodra de backend
    // klaar is, terwijl het herladen van de wet dan nog loopt.
    review: reviewActive.value || reviewProcessing.value,
    reviewStatus: reviewActive.value || reviewLoadError.value ? reviewStatusText.value : null,
    reviewVariant: reviewBannerVariant.value,
    // Meer dan één onbeoordeeld onderdeel: dan is "rond af, neem de rest niet
    // over" een echte afkorting. Bij het laatste onderdeel doet Verwerpen
    // hetzelfde, en dan zou het een tweede knop voor dezelfde daad zijn.
    reviewCanRejectRest:
      reviewActive.value && !reviewIsLawCreate.value && reviewUndecidedParts.value.length > 1,
  });
});

// Reflect navigation depth + unsaved state in the document title:
//   "● Editor: Art. 5 · Wet op de zorgtoeslag · 15 juni test · RegelRecht"
// A leading dot flags unsaved changes - it stays visible even when the tab
// title is truncated to its start. Same glyph as the werkdoc-tab unsaved marker
// in LibraryView (● matches the ○ converting circle in size). Then the mode
// prefix, then most-specific to least-specific (article, law, traject) with the
// brand last. Lives here (after articleDirty) and is always set - a static
// router.afterEach fallback used to race this effect on tab/article switches.
watchEffect(() => {
  const detail = [];
  if (selectedArticle.value) detail.push(`Art. ${selectedArticle.value.number}`);
  if (lawName.value) detail.push(lawName.value);
  if (activeTraject.value?.name) detail.push(activeTraject.value.name);
  const base = detail.length > 0
    ? `Editor: ${detail.join(' · ')} · RegelRecht`
    : 'Editor · RegelRecht';
  document.title = articleDirty.value ? `● ${base}` : base;
});

function onYamlInput(event: Event) {
  // nldd-code-editor dispatches a CustomEvent with the new value in
  // event.detail.value (see the design-system 0.8.41 component). The
  // host's `value` property is updated before dispatch so
  // event.target.value would also work, but reading from detail keeps
  // the contract explicit and matches how the storybook docs the API.
  // `??` skips only null/undefined - a deliberate empty string passes
  // through as a valid "user cleared the editor" input.
  const text =
    (event as CustomEvent<{ value?: string } | null>).detail?.value ??
    (event.target as { value?: string } | null)?.value;
  if (text == null) {
    // Structurally broken event (no detail, no value on target). Don't
    // touch yamlSource - silently overwriting with the previous value
    // would swallow keystrokes a user can see in the textarea but
    // never sees committed to the reactive source. Surface it loudly
    // instead so the regression is obvious in dev console.
    console.warn('onYamlInput: unexpected event shape, ignoring', event);
    return;
  }
  openLaw.setYamlText(text);
}

// Edits from the Machine pane and its item sheet.
function handleSave(payload: MachineReadableSave) {
  openLaw.saveItem(payload);
}

function handleDelete(payload: MachineReadableDelete) {
  openLaw.deleteItem(payload);
}

function handleInitMr() {
  openLaw.initMachineReadable();
}

// A new action opens in the action sheet; it joins the article on save.
function handleAddAction() {
  openLaw.beginNewAction();
}

// A name in a RegelRijm reading that another article computes: go there, the
// way a deep link does (the route watcher switches law or article).
function handleRegelRijmNavigate({
  lawId: targetLaw,
  articleNumber,
}: {
  lawId: string;
  articleNumber: string;
}) {
  router.push(editorRouteFor(targetLaw, articleNumber));
}

// From a RegelRijm reading in the action sheet: a name leads to the rule that
// computes it. The sheet only follows a name when it has no unsaved edits.
function handleSheetNavigate(target: { lawId: string; articleNumber: string }) {
  handleActionClose();
  handleRegelRijmNavigate(target);
}

function handleSheetOpenOutput(output: string) {
  handleActionClose();
  const next = machineReadable.value?.execution?.actions?.find((a) => a?.output === output);
  if (next) handleOpenAction(next);
}

// `path`: from a RegelRijm reading, the part of the action to open at.
function handleOpenAction(action: ExecutionAction, path: Path | null = null) {
  openLaw.beginActionEdit(action, path);
}

// The sheet closed without saving: its edits were on a copy, so nothing to undo.
function handleActionClose() {
  openLaw.cancelActionEdit();
}

// The sheet's "Opslaan" is a real save: commit the action into the article,
// then save the law. A refused action (no output, an unfilled operation)
// keeps the sheet open with the reason in `parseError`. A failed PUT keeps
// the committed edits and falls back to the changes bar - no data loss.
async function handleActionSave() {
  if (openLaw.commitActionEdit()) return;
  await handleLawSave();
}

</script>

<template>
        <!-- The URL names a traject the user has no membership for: it was
             deleted, never existed, or access was revoked. We do not
             distinguish these (no leak of traject existence). Takes
             precedence over every editor state below, including the law-404
             branch, so a missing traject never shows "<law> is niet
             beschikbaar in dit traject". -->
        <!-- A traject the user can't access redirects to /trajecten with a
             name-agnostic banner (see the trajectMissing watcher); render
             nothing here while that navigation commits. -->
        <nldd-page v-if="trajectMissing"></nldd-page>

        <!-- Neutral / root view: no active tab, nothing loading, nothing
             selected, no error to report. Two cases land here - genuinely no
             open tabs, and the moment right after a traject switch (tabs sit in
             the bar but none is active, since a switch never auto-opens one).
             Both show the same "pick a tab or go Home" prompt, so this is NOT
             keyed on `openTabs.length`. Deliberately NOT keyed on `!activeTab`
             alone either - that stays null for the WHOLE load of an explicit
             article URL (the add-tab watch only sets it once the article
             resolves), so keying on it would hide the panes behind a full-page
             state exactly when we want them up; `!loading && !selectedArticle`
             guards that. `!error` keeps a failed explicit open on its own error
             branch below. -->
        <nldd-page v-else-if="!activeTab && !loading && !selectedArticle && !error">
          <nldd-simple-section width="full">
            <nldd-inline-dialog text="Open een artikel vanuit de tabbalk of Home om te bewerken.">
              <nldd-button slot="actions" appearance="secondary" text="Naar Home" :href="libraryTabHref" @click.prevent="router.push(libraryTabTarget)"></nldd-button>
            </nldd-inline-dialog>
          </nldd-simple-section>
        </nldd-page>

        <!-- Error state - mirrors the library's law-load failure pattern.
             404s typically mean "the law isn't part of the active traject"
             (e.g. after a traject switch); we surface a traject-specific
             message and a quick "Naar bibliotheek" exit. Other failures
             keep the generic copy + retry. Gated on `!loading` so a stale
             error during a refetch yields to the panes (with their own
             per-pane loading) instead of flashing the error. -->
        <nldd-page v-else-if="error && !loading">
          <nldd-simple-section width="full">
            <nldd-inline-dialog
              v-if="lawErrorIs404"
              variant="alert"
              :text="`${failedLawName} is niet beschikbaar in dit traject`"
              supporting-text="Wissel van traject via het menu rechtsboven of ga terug naar het overzicht."
            >
              <nldd-button slot="actions" appearance="primary" text="Naar Home" :href="libraryTabHref" @click.prevent="router.push(libraryTabTarget)"></nldd-button>
              <nldd-button slot="actions" appearance="secondary" text="Probeer opnieuw" @click="retryLoadLaw"></nldd-button>
            </nldd-inline-dialog>
            <nldd-inline-dialog
              v-else
              variant="alert"
              :text="`${failedLawName} is niet geladen`"
              supporting-text="De gegevens konden niet worden opgehaald."
            >
              <nldd-button slot="actions" appearance="primary" text="Probeer opnieuw" @click="retryLoadLaw"></nldd-button>
              <nldd-button slot="actions" appearance="secondary" text="Neem contact op via e-mail" :href="`mailto:${SUPPORT_EMAIL}`"></nldd-button>
            </nldd-inline-dialog>
          </nldd-simple-section>
        </nldd-page>

        <!-- All editor flags off - paneViews is empty so the
             side-by-side view would render zero pane slots. Surface an
             explicit empty-state with a CTA to the settings menu so
             the user understands the editor isn't broken. -->
        <nldd-page v-else-if="paneViews.length === 0">
          <nldd-simple-section width="full">
            <nldd-inline-dialog text="Geen editors actief. Schakel ten minste één editor in via Instellingen."></nldd-inline-dialog>
          </nldd-simple-section>
        </nldd-page>

        <!-- One pane per entry in `paneViews`, wrapped in a template so the
             review/enrich banners can sit above it as siblings within this
             same v-else branch. Each pane independently picks its view via
             the dropdown in its header. The split-view auto-hides panes
             from the right when the viewport is too narrow. Hidden panes
             stay in the DOM so state is preserved when the viewport
             widens. -->
        <template v-else>
          <!-- Review mode (a job_review task) has no block of its own in
               the content any more. The notice goes to the bar-split-view in
               AppShell as `slot="review-notice"` (under the document tab bar,
               above `main`), and the decision sits in the changes bar below
               it. Both go through the chrome store, like the rest of the
               shell chrome, so neither scrolls with the panes. -->

          <!-- Feedback op een aangevraagde verrijking (zie de Machine/YAML-pane):
               same page-wide banner-in-container pattern as the review notice
               above, so feedback never pushes the editor into a tall narrow
               column. -->
          <nldd-container v-if="enrichFeedback" padding="8">
            <nldd-banner
              :variant="enrichFeedback.variant"
              :text="enrichFeedback.text"
              dismissible
              @dismiss="dismissEnrichFeedback"
            ></nldd-banner>
          </nldd-container>

          <!-- The engine refused the current YAML (parse or type-check error,
               RFC-037). Same page-wide pattern as the feedback above; not
               dismissible, it clears when a reload succeeds. -->
          <nldd-container v-if="engineLoadError" padding="8">
            <nldd-banner
              variant="critical"
              text="De engine kan dit regelwerk niet laden"
              :supporting-text="engineLoadError"
            ></nldd-banner>
          </nldd-container>

        <nldd-side-by-side-split-view :panes="String(paneViews.length)">
          <!-- Compound key: when a flag flip shifts which view sits at a
               given index, Vue would otherwise patch the existing pane in
               place - leaking ScenarioBuilder form state and engine
               results into a different view. Re-keying on the view id
               forces an unmount + remount on identity change, at the
               (acceptable) cost of losing pane scroll position. -->
          <!-- data-testid names the view, not the index: the empty states and
               their buttons live in more than one pane, so an e2e locator has to
               be able to say which pane it means. -->
          <nldd-split-view-pane
            v-for="(view, idx) in paneViews"
            :key="`${view}-${idx}`"
            :slot="`pane-${idx + 1}`"
            :data-testid="`pane-${view}`"
          >
            <!-- A page in a pane renders no main landmark of its own; the
                 first pane holds the primary content, so it carries it. -->
            <nldd-page
              sticky-header
              :landmarks="idx === 0 ? 'page' : 'auto'"
              :background="view === 'scenario' ? 'base' : undefined"
            >
              <nldd-container slot="header" padding="8" padding-bottom="0">
                <nldd-toolbar size="md" accessible-label="Paneelacties">
                  <!-- Weergave-keuze (alle panes). Hoogste prioriteit zodat
                       deze als laatste naar het overflow-menu verhuist. -->
                  <nldd-toolbar-item slot="start" label="Weergave" :priority="4">
                    <nldd-button size="md" expandable :text="viewLabel(view)">
                      <nldd-menu slot="popup">
                        <nldd-menu-item
                          v-for="opt in availableViews"
                          :key="opt.id"
                          type="radio"
                          :selected="view === opt.id || undefined"
                          :text="opt.label"
                          @select="setPaneView(idx, opt.id)"
                        ></nldd-menu-item>
                        <nldd-menu-divider></nldd-menu-divider>
                        <nldd-menu-item
                          text="Verplaats naar links"
                          icon="arrow-left"
                          :disabled="idx === 0 || undefined"
                          @select="movePane(idx, 'left')"
                        ></nldd-menu-item>
                        <nldd-menu-item
                          text="Verplaats naar rechts"
                          icon="arrow-right"
                          :disabled="idx === paneViews.length - 1 || undefined"
                          @select="movePane(idx, 'right')"
                        ></nldd-menu-item>
                        <nldd-menu-item
                          text="Verplaats uiterst links"
                          icon="arrow-left-to-line"
                          :disabled="idx === 0 || undefined"
                          @select="movePane(idx, 'start')"
                        ></nldd-menu-item>
                        <nldd-menu-item
                          text="Verplaats uiterst rechts"
                          icon="arrow-right-to-line"
                          :disabled="idx === paneViews.length - 1 || undefined"
                          @select="movePane(idx, 'end')"
                        ></nldd-menu-item>
                      </nldd-menu>
                    </nldd-button>
                    <nldd-menu-group slot="overflow" text="Weergave">
                      <nldd-menu-item
                        v-for="opt in availableViews"
                        :key="`ovf-view-${opt.id}`"
                        type="radio"
                        :selected="view === opt.id || undefined"
                        :text="opt.label"
                        @select="setPaneView(idx, opt.id)"
                      ></nldd-menu-item>
                      <nldd-menu-divider></nldd-menu-divider>
                      <nldd-menu-item
                        text="Verplaats naar links"
                        icon="arrow-left"
                        :disabled="idx === 0 || undefined"
                        @select="movePane(idx, 'left')"
                      ></nldd-menu-item>
                      <nldd-menu-item
                        text="Verplaats naar rechts"
                        icon="arrow-right"
                        :disabled="idx === paneViews.length - 1 || undefined"
                        @select="movePane(idx, 'right')"
                      ></nldd-menu-item>
                      <nldd-menu-item
                        text="Verplaats uiterst links"
                        icon="arrow-left-to-line"
                        :disabled="idx === 0 || undefined"
                        @select="movePane(idx, 'start')"
                      ></nldd-menu-item>
                      <nldd-menu-item
                        text="Verplaats uiterst rechts"
                        icon="arrow-right-to-line"
                        :disabled="idx === paneViews.length - 1 || undefined"
                        @select="movePane(idx, 'end')"
                      ></nldd-menu-item>
                    </nldd-menu-group>
                  </nldd-toolbar-item>
                  <!-- Vet/Schuin - checkbox segmented control (beide kunnen
                       tegelijk actief zijn). Bron van waarheid: de Tiptap-
                       editor; de control reflecteert de selectie. -->
                  <nldd-toolbar-item
                    v-if="view === 'text' && selectedArticle && textEditorRefs[idx]"
                    slot="end"
                    label="Tekststijl"
                    :priority="2"
                  >
                    <nldd-segmented-control
                      type="checkbox"
                      variant="icon"
                      size="md"
                      accessible-label="Tekststijl"
                      :disabled="!canEditArticleText || undefined"
                      :values.prop="boldItalicValues(idx)"
                      @item-change="onInlineFormatChange(idx, $event)"
                    >
                      <nldd-segmented-control-item value="bold" icon="bold" text="Vet"></nldd-segmented-control-item>
                      <nldd-segmented-control-item value="italic" icon="italic" text="Schuin"></nldd-segmented-control-item>
                    </nldd-segmented-control>
                    <nldd-menu-group slot="overflow" text="Tekststijl">
                      <nldd-menu-item
                        type="checkbox"
                        icon="bold"
                        text="Vet"
                        :selected="textEditorRefs[idx].activeFormats.bold || undefined"
                        :disabled="!canEditArticleText || undefined"
                        @select="textEditorRefs[idx].toggleBold()"
                      ></nldd-menu-item>
                      <nldd-menu-item
                        type="checkbox"
                        icon="italic"
                        text="Schuin"
                        :selected="textEditorRefs[idx].activeFormats.italic || undefined"
                        :disabled="!canEditArticleText || undefined"
                        @select="textEditorRefs[idx].toggleItalic()"
                      ></nldd-menu-item>
                    </nldd-menu-group>
                  </nldd-toolbar-item>
                  <!-- Lijsttype - radio segmented control: geen / opsomming /
                       genummerd. "Geen" (minus-small) heft de actieve lijst op. -->
                  <nldd-toolbar-item
                    v-if="view === 'text' && selectedArticle && textEditorRefs[idx]"
                    slot="end"
                    label="Lijst"
                    :priority="1"
                  >
                    <nldd-segmented-control
                      type="radio"
                      variant="icon"
                      size="md"
                      accessible-label="Lijst"
                      :disabled="!canEditArticleText || undefined"
                      :value="listValue(idx)"
                      @change="onListChange(idx, $event)"
                    >
                      <nldd-segmented-control-item value="none" icon="minus-small" text="Geen lijst"></nldd-segmented-control-item>
                      <nldd-segmented-control-item value="bullet" icon="bullet-list" text="Opsomming"></nldd-segmented-control-item>
                      <nldd-segmented-control-item value="ordered" icon="numbered-list" text="Genummerde lijst"></nldd-segmented-control-item>
                    </nldd-segmented-control>
                    <nldd-menu-group slot="overflow" text="Lijst">
                      <nldd-menu-item
                        type="radio"
                        icon="minus-small"
                        text="Geen lijst"
                        :selected="listValue(idx) === 'none' || undefined"
                        :disabled="!canEditArticleText || undefined"
                        @select="setList(idx, 'none')"
                      ></nldd-menu-item>
                      <nldd-menu-item
                        type="radio"
                        icon="bullet-list"
                        text="Opsomming"
                        :selected="listValue(idx) === 'bullet' || undefined"
                        :disabled="!canEditArticleText || undefined"
                        @select="setList(idx, 'bullet')"
                      ></nldd-menu-item>
                      <nldd-menu-item
                        type="radio"
                        icon="numbered-list"
                        text="Genummerde lijst"
                        :selected="listValue(idx) === 'ordered' || undefined"
                        :disabled="!canEditArticleText || undefined"
                        @select="setList(idx, 'ordered')"
                      ></nldd-menu-item>
                    </nldd-menu-group>
                  </nldd-toolbar-item>
                  <!-- Inspringen - vergroot/verklein de inspringing van de
                       geselecteerde regels. Laagste prioriteit (0), dus deze
                       verdwijnt als eerste naar de overflow. -->
                  <nldd-toolbar-item
                    v-if="view === 'text' && selectedArticle && textEditorRefs[idx]"
                    slot="end"
                    label="Inspringen"
                    :priority="0"
                  >
                    <!-- Grouped as a button-bar: the bar draws the divider and
                         propagates size/variant to its children. Individual
                         disabled stays per-button (canIndent vs canOutdent) since
                         the bar itself is never disabled. -->
                    <nldd-button-bar size="md">
                      <nldd-icon-button
                        icon="indent-increase"
                        text="Inspringen vergroten"
                        :disabled="!canEditArticleText || !textEditorRefs[idx].activeFormats.canIndent || undefined"
                        @mousedown.prevent
                        @click="textEditorRefs[idx].indent()"
                      ></nldd-icon-button>
                      <nldd-button-bar-divider></nldd-button-bar-divider>
                      <nldd-icon-button
                        icon="indent-decrease"
                        text="Inspringen verkleinen"
                        :disabled="!canEditArticleText || !textEditorRefs[idx].activeFormats.canOutdent || undefined"
                        @mousedown.prevent
                        @click="textEditorRefs[idx].outdent()"
                      ></nldd-icon-button>
                    </nldd-button-bar>
                    <nldd-menu-group slot="overflow" text="Inspringen">
                      <nldd-menu-item
                        icon="indent-increase"
                        text="Inspringen vergroten"
                        :disabled="!canEditArticleText || !textEditorRefs[idx].activeFormats.canIndent || undefined"
                        @select="textEditorRefs[idx].indent()"
                      ></nldd-menu-item>
                      <nldd-menu-item
                        icon="indent-decrease"
                        text="Inspringen verkleinen"
                        :disabled="!canEditArticleText || !textEditorRefs[idx].activeFormats.canOutdent || undefined"
                        @select="textEditorRefs[idx].outdent()"
                      ></nldd-menu-item>
                    </nldd-menu-group>
                  </nldd-toolbar-item>
                  <!-- Notitie toevoegen - annotatie op de huidige selectie.
                       Prioriteit tussen de mode-switcher (4) en de opmaak-
                       controls (Tekststijl 2, Lijst 1) in, zodat deze primaire
                       actie pas na de opmaak naar de overflow verdwijnt. -->
                  <nldd-toolbar-item
                    v-if="view === 'text' && selectedArticle && textEditorRefs[idx] && canCreateNotes"
                    slot="end"
                    label="Notitie"
                    :priority="3"
                  >
                    <nldd-icon-button
                      icon="comment"
                      text="Notitie toevoegen"
                      appearance="secondary"
                      size="md"
                      :disabled="textEditorRefs[idx].selectionEmpty || undefined"
                      @mousedown.prevent
                      @click="startNoteFromSelection(idx)"
                    ></nldd-icon-button>
                    <!-- No :disabled here: the toolbar clones overflow items, so
                         a live-changing disabled (selectionEmpty) would freeze at
                         clone time and stick. startNoteFromSelection no-ops on an
                         empty selection instead. -->
                    <nldd-menu-item
                      slot="overflow"
                      icon="comment"
                      text="Notitie toevoegen"
                      @select="startNoteFromSelection(idx)"
                    ></nldd-menu-item>
                  </nldd-toolbar-item>
                  <!-- Notities downloaden - expandable icon-button (area end) met
                       een menu: dit artikel, of de hele wet, als YAML. -->
                  <nldd-toolbar-item
                    v-if="view === 'notes' && selectedArticle && notesForArticle.length > 0"
                    slot="end"
                    label="Notities downloaden"
                    :priority="3"
                  >
                    <nldd-icon-button
                      icon="download"
                      text="Notities downloaden"
                      appearance="secondary"
                      size="md"
                      expandable
                    >
                      <nldd-menu slot="popup">
                        <nldd-menu-item icon="document" text="Artikel-notities als YAML" @select="exportArticleNotes"></nldd-menu-item>
                        <nldd-menu-item icon="document" text="Regelwerk-notities als YAML" @select="exportNotes"></nldd-menu-item>
                      </nldd-menu>
                    </nldd-icon-button>
                    <nldd-menu-group slot="overflow" text="Notities downloaden">
                      <nldd-menu-item icon="document" text="Artikel-notities als YAML" @select="exportArticleNotes"></nldd-menu-item>
                      <nldd-menu-item icon="document" text="Regelwerk-notities als YAML" @select="exportNotes"></nldd-menu-item>
                    </nldd-menu-group>
                  </nldd-toolbar-item>
                  <!-- YAML parse-status (Machine-readable pane). -->
                  <nldd-toolbar-item v-if="view === 'yaml' && parseError" slot="end" label="YAML">
                    <nldd-badge color="critical" text="YAML parse error"></nldd-badge>
                    <nldd-menu-item
                      slot="overflow"
                      text="YAML parse error"
                      disabled
                    ></nldd-menu-item>
                  </nldd-toolbar-item>
                  <!-- Verrijken hoort bij de panes die het resultaat tonen, maar
                       het is geen actie die je vaak aanraakt en de pane-toolbar
                       is smal. Als directe child van nldd-toolbar met
                       slot="overflow" is dit een pinned overflow item: hij staat
                       altijd achter de ...-knop en vecht dus nooit met de
                       pane-switcher om breedte. Is de pane leeg, dan staat de
                       ingang in de lege staat zelf. -->
                  <nldd-menu-item
                    v-if="canEnrichLaw && isEnrichPane(view) && hasMachineReadable"
                    slot="overflow"
                    icon="ai"
                    text="Verrijk dit regelwerk opnieuw"
                    @select="enrichLaw"
                  ></nldd-menu-item>
                </nldd-toolbar>
              </nldd-container>

              <!-- While the shared article/law load is in flight - or before an
                   article is picked at all - each pane shows its own type-named
                   indicator ("Tekst laden", …) so the chrome (the Weergave
                   toolbar above) is up immediately and the content fills in per
                   pane. `!selectedArticle` matters as much as `loading`: on an
                   explicit article URL the panes render before the article
                   resolves, and the view content below would otherwise get a
                   null article.
                   The scenario pane is EXCLUDED: it has a second phase (scenario
                   files + dependency laws) after this one, and two indicators
                   handing off would each restart the DS anti-flash timer
                   (spinner -> 1s blank -> spinner). It therefore renders during
                   the load too and owns both phases in one mounted indicator -
                   see its `article-loading` prop below. -->
              <nldd-simple-section v-if="(loading || !selectedArticle) && !(view === 'scenario' && lawId)" width="full">
                <nldd-activity-indicator :text="`${viewLabel(view)} laden`" show-text></nldd-activity-indicator>
              </nldd-simple-section>

              <!-- Tekst - WYSIWYG editor when the user can edit, otherwise the
                   read-only ArticleText display. The format controls in the
                   header toolbar guard on `textEditorRefs[idx]`, which only the
                   WYSIWYG component populates, so they auto-hide when read-only. -->
              <nldd-simple-section v-else-if="view === 'text'" width="full">
                <ArticleTextEditor
                  v-if="canEditArticleText"
                  :ref="setTextEditorRef(idx)"
                  :article="selectedArticle"
                  :editable="canEditArticleText"
                  :model-value="editedText"
                  :annotations="editorAnnotations"
                  @update:model-value="editedText = $event"
                  @focus="onTextEditorFocus(idx)"
                  @annotation-click="onEditorAnnotationClick"
                />
                <ArticleText v-else :article="selectedArticle" />

                <!-- The note editor and list live in one right sheet (the badge
                     look-ahead popover, below, is the lightweight step before it).
                     The badge list is the root view; editing/adding pushes an
                     editor view (with a back button) or, from the notities pane,
                     opens it as the root (with a close button). -->
                <nldd-sheet
                  :ref="(el: Element | ComponentPublicInstance | null) => (noteSheetEl = el as NLDDSheet | null)"
                  placement="right"
                  width="480px"
                  accessible-label="Notities"
                  @close="onNoteSheetClose"
                >
                  <!-- List view: the referenced text, a card per note, then add.
                       A sheet always hosts an nldd-page; the title bar is its
                       header (sticky) and carries the close button. -->
                  <template v-if="noteSheetView === 'list'">
                    <nldd-page sticky-header>
                      <nldd-top-title-bar
                        slot="header"
                        text="Notities"
                        dismiss-text="Sluit"
                        @dismiss="noteSheetEl?.hide?.()"
                      ></nldd-top-title-bar>
                      <nldd-container padding="16" data-testid="note-detail">
                        <QuotedFragment :fragment="activeGroup" />
                        <nldd-spacer v-if="activeGroup && activeGroup.quote" size="12"></nldd-spacer>
                        <nldd-collection layout="stack" gap="12">
                          <nldd-card v-for="(note, i) in activeNotes" :key="i">
                            <nldd-container padding="10">
                              <NoteCard
                                :note="note"
                                :can-edit="canEdit"
                                :saving="savingNotes"
                                @edit="startEditNote(groupForNote(note), note, true)"
                                @share="askPublishNote(note)"
                                @delete="askDeleteNote(note)"
                              />
                            </nldd-container>
                          </nldd-card>
                        </nldd-collection>
                        <template v-if="canCreateNotes && activeGroup && activeGroup.quote">
                          <nldd-spacer size="12"></nldd-spacer>
                          <nldd-button
                            appearance="secondary"
                            size="md"
                            width="full"
                            start-icon="add"
                            text="Notitie toevoegen"
                            @click="startNoteForGroup(activeGroup, true)"
                          ></nldd-button>
                        </template>
                      </nldd-container>
                    </nldd-page>
                  </template>

                  <!-- Editor view: back button when it came from the list, plus
                       the close button on the right in every case. -->
                  <template v-else>
                    <nldd-page sticky-header>
                      <nldd-top-title-bar
                        slot="header"
                        text="Notitie"
                        :back-text="noteEditFromList ? 'Notities' : ''"
                        dismiss-text="Annuleer"
                        @back="noteEditorBack"
                        @dismiss="noteSheetEl?.hide?.()"
                      ></nldd-top-title-bar>
                      <!-- Only in the Tekst pane, which renders once an article
                           (and so a law) is loaded: lawId is a string here. -->
                      <NoteCreator
                        v-if="noteCreator.open"
                        :range="noteCreator.range"
                        :raw-text="editedText"
                        :law-id="(lawId as string)"
                        :article="selectedArticle"
                        :engine="noteEngine"
                        :valid-from="lawValidFrom || ''"
                        :traject-ref="activeTrajectRef || ''"
                        :initial-note="noteCreator.initialNote"
                        @create="onNoteCreated"
                        @cancel="noteEditCancel"
                      />
                    </nldd-page>
                  </template>
                </nldd-sheet>

                <!-- Look-ahead popover on a badge click: a flat preview of the
                     comments (no referenced text) with a CTA into the sheet. -->
                <nldd-popover
                  :ref="(el: Element | ComponentPublicInstance | null) => (annotationPopEl = el as NLDDPopover | null)"
                  accessible-label="Notities"
                  placement="bottom-start"
                >
                  <nldd-container padding="16">
                    <template v-for="(note, i) in activeNotes" :key="i">
                      <nldd-spacer v-if="i > 0" size="16"></nldd-spacer>
                      <nldd-rich-text>
                        <p v-if="!linkedAction(note)">{{ noteText(note) || '-' }}</p>
                        <p v-else-if="noteComment(note)">{{ noteComment(note) }}</p>
                        <p v-if="linkedAction(note)">Gekoppeld aan de rekenregel {{ humanize(linkedAction(note)?.output ?? '') }}</p>
                      </nldd-rich-text>
                      <template v-if="noteAuthor(note)">
                        <nldd-spacer size="4"></nldd-spacer>
                        <nldd-identity :text="noteAuthor(note)"></nldd-identity>
                      </template>
                      <!-- RegelRijm: a linking note leads to its rule. -->
                      <template v-if="linkedAction(note)">
                        <nldd-spacer size="8"></nldd-spacer>
                        <nldd-button size="sm" text="Actie openen" data-testid="note-open-linked-action" @click="openLinkedAction(note)"></nldd-button>
                      </template>
                    </template>
                    <nldd-spacer size="16"></nldd-spacer>
                    <nldd-button
                      appearance="secondary"
                      width="full"
                      text="Bijdragen of bewerken"
                      @click="openSheetFromAnnotation"
                    ></nldd-button>
                  </nldd-container>
                </nldd-popover>

                <!-- Note management + the note list live in the Notities pane. -->
              </nldd-simple-section>

              <!-- Machine readable -->
              <nldd-simple-section v-else-if="view === 'machine'" width="full">
                <MachineReadable
                  :article="editedArticle"
                  :editable="canEdit"
                  :can-enrich="canEnrichLaw"
                  :enriching="isEnriching"
                  :review-ready="reviewReady"
                  :review-article="reviewArticleForPane"
                  :dirty="isMachineReadableDirty"
                  :saving="lawSaving"
                  regelrijm
                  :law="law"
                  :law-id="lawId"
                  :links="regelrijmLinks"
                  @open-action="handleOpenAction"
                  @navigate="handleRegelRijmNavigate"
                  @open-edit="activeEditItem = $event"
                  @init-mr="handleInitMr"
                  @enrich="enrichLaw"
                  @view-tasks="openTasksForLaw"
                  @review="openReviewForLaw"
                  @add-action="handleAddAction"
                  @save="handleMachineReadableSave"
                  @patch="handleSave"
                  @delete="handleDelete"
                />
              </nldd-simple-section>

              <!-- Scenario builder -->
              <template v-else-if="view === 'scenario'">
                <nldd-simple-section v-if="engineInitError" width="full">
                  <nldd-inline-dialog
                    variant="alert"
                    text="WASM engine niet geladen"
                    :supporting-text="`${engineInitError.message} - voer 'just wasm-build' uit om de WASM module te bouwen.`"
                  ></nldd-inline-dialog>
                </nldd-simple-section>
                <!-- Rendered with a law id or a loaded article, and a loaded
                     article means a loaded law: lawId is a string here. -->
                <ScenarioBuilder
                  v-else
                  :law-id="(lawId as string)"
                  :law-yaml="currentLawYaml"
                  :engine="getEngine()"
                  :ready="engineReady"
                  :article-loading="loading || !selectedArticle"
                  :articles="articles"
                  :traject-ref="activeTrajectRef"
                  @executed="handleScenarioExecuted"
                />
              </template>

              <!-- YAML -->
              <nldd-simple-section v-else-if="view === 'yaml'" width="full">
                <MachineEmptyState
                  v-if="!hasMachineReadable"
                  :review-ready="reviewReady"
                  :review-article="reviewArticleForPane"
                  :enriching="isEnriching"
                  :can-enrich="canEnrichLaw"
                  :can-write-here="canEdit"
                  @enrich="enrichLaw"
                  @write="handleInitMr"
                  @view-tasks="openTasksForLaw"
                  @review="openReviewForLaw"
                ></MachineEmptyState>
                <template v-else>
                  <nldd-code-editor
                    resize="auto"
                    language="yaml"
                    accessible-label="YAML"
                    :value="yamlSource"
                    @input="onYamlInput"
                  ></nldd-code-editor>
                  <nldd-banner v-if="parseError" variant="critical" :text="parseError"></nldd-banner>
                </template>
              </nldd-simple-section>

              <!-- Notities - every note for this article, one under the other,
                   plus the draft-management actions moved from the Tekst pane.
                   Deliberately minimal for now; to be fine-tuned later. -->
              <nldd-simple-section v-else-if="view === 'notes'" width="full">
                <!-- Pane-level resolve failure (fetch/engine/resolver threw).
                     Shown to every viewer, not only note authors: without it
                     the pane falls through to "Geen notities", which reads as
                     an empty sidecar instead of a failure. -->
                <!-- useNotes keeps whatever it caught (`unknown`): an Error or the
                     bare string a WASM call throws, which is what this prop takes. -->
                <NotesLoadError :error="notesLoadError as { message?: string } | string | null" />
                <template v-if="canCreateNotes">
                  <nldd-inline-dialog
                    v-if="noteIssues.length"
                    icon="exclamation-triangle"
                    icon-color="warning"
                    :text="`${noteIssues.length} notitie(s) niet verankerd`"
                    :supporting-text="noteIssues.map(i => i.reason).join('; ')"
                  ></nldd-inline-dialog>
                  <nldd-inline-dialog
                    v-if="draftCount > 0 && !canEdit"
                    icon="exclamation-triangle"
                    icon-color="warning"
                    data-testid="notes-no-traject"
                    text="Selecteer eerst een traject"
                    supporting-text="Delen werkt pas als er een actief traject is. Notities downloaden als YAML werkt wel."
                  ></nldd-inline-dialog>
                  <nldd-inline-dialog
                    v-if="notesSaveError"
                    variant="alert"
                    data-testid="notes-save-error"
                    text="Notities opslaan mislukt"
                    :supporting-text="notesSaveError"
                  ></nldd-inline-dialog>
                  <nldd-inline-dialog
                    v-if="notesSaveStatus && !notesSaveError"
                    variant="success"
                    data-testid="notes-save-status"
                    text="Opslaan gelukt"
                    :supporting-text="notesSaveStatus"
                  ></nldd-inline-dialog>
                </template>

                <nldd-inline-dialog
                  v-if="notesForArticle.length === 0 && !notesLoadError"
                  text="Geen notities voor dit artikel"
                ></nldd-inline-dialog>
                <template v-else>
                  <template v-for="(group, gi) in noteGroups" :key="gi">
                    <!-- Gap between groups only; the list must not open with a
                         spacer (no leading whitespace at the top of the pane). -->
                    <nldd-spacer v-if="gi > 0" size="24"></nldd-spacer>
                    <!-- Quoted fragment (group header) as rich-text, then a
                         spacer before the cards. -->
                    <QuotedFragment v-if="group.quote" :fragment="group" />
                    <nldd-rich-text v-else>
                      <p><i>Zonder verankering</i></p>
                    </nldd-rich-text>
                    <nldd-spacer size="10"></nldd-spacer>
                    <nldd-collection layout="stack" gap="12">
                      <nldd-card v-for="(note, ni) in group.notes" :key="ni">
                        <nldd-container padding="10">
                          <NoteCard
                            :note="note"
                            :can-edit="canEdit"
                            :saving="savingNotes"
                            @edit="startEditNote(group, note)"
                            @share="askPublishNote(note)"
                            @delete="askDeleteNote(note)"
                          />
                        </nldd-container>
                      </nldd-card>
                    </nldd-collection>
                    <!-- Add another note anchored to this same fragment, without
                         re-selecting the text in the editor. -->
                    <template v-if="canCreateNotes && group.quote">
                      <nldd-spacer size="12"></nldd-spacer>
                      <nldd-button
                        appearance="secondary"
                        size="md"
                        width="full"
                        start-icon="add"
                        text="Notitie toevoegen"
                        @click="startNoteForGroup(group)"
                      ></nldd-button>
                    </template>
                  </template>
                </template>
              </nldd-simple-section>
            </nldd-page>
          </nldd-split-view-pane>
        </nldd-side-by-side-split-view>
        </template>
  <!-- Overlays teleported to body: as light-DOM siblings of the split view they
       would be slotted into the main pane and pick up its ::slotted flex-grow,
       stealing height from the pane content. -->
  <Teleport to="body">
    <ActionSheet regelrijm :action="activeAction" :article="editedArticle" :editable="canEdit" :is-new="activeActionIsNew" :law="law" :law-id="lawId" :focus-path="activeActionPath" @close="handleActionClose" @save="handleActionSave" @navigate="handleSheetNavigate" @open-output="handleSheetOpenOutput" />
    <EditSheet :item="activeEditItem" :article="editedArticle" @save="handleSave" @close="activeEditItem = null" />
    <SearchPopover
      ref="searchPopoverRef"
      @select-law="onSearchSelectLaw"
    />
  </Teleport>

  <!-- Publishing a note is irreversible (it lands in the repo and can't be made
       private again), so confirm before the write. -->
  <nldd-modal-dialog
    ref="publishModalEl"
    text="Notitie delen?"
    supporting-text="Deze notitie wordt vastgelegd op de traject-branch in Git en is dan zichtbaar voor de andere traject-leden. Delen kan niet ongedaan worden gemaakt: de notitie kan daarna niet meer privé worden gemaakt."
    data-testid="publish-confirm"
    @close="cancelPublish"
  >
    <nldd-button slot="actions" appearance="primary" text="Houd privé" @click="cancelPublish"></nldd-button>
    <nldd-button slot="actions" appearance="secondary" text="Deel binnen traject" data-testid="publish-confirm-btn" @click="confirmPublish"></nldd-button>
  </nldd-modal-dialog>

  <!-- Deleting a draft is irreversible (it is the only copy), so confirm. The
       safe option (Annuleren) is the primary action. -->
  <nldd-modal-dialog
    ref="deleteModalEl"
    variant="alert"
    text="Notitie verwijderen?"
    supporting-text="Deze notitie wordt definitief verwijderd. Dit kan niet ongedaan worden gemaakt."
    data-testid="delete-confirm"
    @close="cancelDelete"
  >
    <nldd-button slot="actions" appearance="primary" text="Behoud notitie" @click="cancelDelete"></nldd-button>
    <nldd-button slot="actions" appearance="destructive" text="Verwijder" data-testid="delete-confirm-btn" @click="confirmDelete"></nldd-button>
  </nldd-modal-dialog>

  <!-- Leaving an article with unsaved edits (see confirmLeave). The safe
       option, staying, is the primary action. -->
  <nldd-modal-dialog
    ref="leaveModalEl"
    variant="alert"
    :text="`Je hebt niet-opgeslagen wijzigingen in artikel ${selectedArticleNumber ?? ''}`"
    supporting-text="Als je doorgaat, gaan ze verloren."
    data-testid="leave-confirm"
    @close="answerLeave(false)"
  >
    <nldd-button slot="actions" appearance="primary" text="Blijf hier" data-testid="leave-stay-btn" @click="answerLeave(false)"></nldd-button>
    <nldd-button slot="actions" appearance="destructive" text="Wijzigingen weggooien" data-testid="leave-discard-btn" @click="answerLeave(true)"></nldd-button>
  </nldd-modal-dialog>

  <!-- Whole-law save failure, shown over the whole editor rather than inline in
       one pane. Critical, so it stays until dismissed. -->
  <nldd-notification
    v-if="showSaveError"
    variant="critical"
    text="Opslaan mislukt"
    :supporting-text="saveErrorText"
    data-testid="save-error-notification"
    @dismiss="dismissSaveError"
  ></nldd-notification>

  <!-- Trace sheet - execution trace + expected outcomes for the most
       recently executed scenario. Opened from a scenario card's "Toon
       resultaat" button. -->
  <nldd-sheet
    ref="resultSheetEl"
    placement="bottom"
    @close="resultSheetOpen = false"
  >
    <nldd-page sticky-header>
      <nldd-top-title-bar slot="header" :text="lastScenarioName ? `Resultaat: ${lastScenarioName}` : 'Resultaat'" collapse-anchor="result-scenario-title" dismiss-text="Sluit" @dismiss="resultSheetOpen = false"></nldd-top-title-bar>
      <nldd-simple-section width="full">
        <nldd-title id="result-scenario-title" size="4"><h3>{{ lastScenarioName ? `Resultaat: ${lastScenarioName}` : 'Resultaat' }}</h3></nldd-title>
        <nldd-spacer size="16"></nldd-spacer>
        <ExecutionTraceView
          :result="lastResult"
          :trace-text="lastTraceText"
          :error="lastError"
          :running="lastRunning"
          :expectations="lastExpectations"
          :output-types="outputTypeMap"
          :can-reload="!!lastReload"
          @reload="lastReload && lastReload()"
        />
      </nldd-simple-section>
    </nldd-page>
  </nldd-sheet>

  <!-- Graph sheet - visual law graph with the scenario's trace overlay.
       Opened from a scenario card's "Graaf" button. -->
  <nldd-sheet
    ref="graphSheetEl"
    placement="bottom"
    @close="graphSheetOpen = false"
  >
    <nldd-page sticky-header>
      <nldd-top-title-bar slot="header" :text="lastScenarioName ? `Graaf: ${lastScenarioName}` : 'Graaf'" dismiss-text="Sluit" @dismiss="graphSheetOpen = false"></nldd-top-title-bar>
      <!-- Lazy-mount: building the Vue Flow graph for laws with many
           cross-law references is non-trivial and the sheet is hidden
           by default. Render only while the sheet is open; the remount
           cost on each subsequent open is acceptable next to dragging
           an unused Vue Flow tree behind every editor session. -->
      <LawGraphView
        v-if="graphSheetOpen"
        :law-id="lawId"
        :result="lastResult"
        :output-name="lastOutputName"
        :expectations="lastExpectations"
      />
    </nldd-page>
  </nldd-sheet>
</template>
