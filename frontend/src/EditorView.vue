<script setup lang="ts">
import { ref, computed, reactive, watch, watchEffect, nextTick, onBeforeUnmount } from 'vue';
import { useRoute, useRouter, onBeforeRouteUpdate } from 'vue-router';
import * as yaml from 'js-yaml';
import { withDecisionField } from '@regelrecht/frontend-shared/produces';
import { useLaw, fetchLaw } from './composables/useLaw';
import { useCorpusLaws } from './composables/useCorpusLaws';
import { useEngine } from './composables/useEngine';
import { useAuth } from './composables/useAuth';
import { useTrajects } from './composables/useTrajects';
import { useFeatureFlags } from './composables/useFeatureFlags';
import { useEnrichState } from './composables/useEnrichState';
import { useTaskReview } from './composables/useTaskReview';
import { useNotes, useResolvedDraftNotes } from './composables/useNotes';
import { useDraftNotes } from './composables/useDraftNotes';
import { lastTrajectPath, homeTarget, trajectTabTarget } from './composables/useLastVisitedRoute';
import {
  registerSearchPopover,
  setEditorChrome,
  registerTabActions,
  setEditorChanges,
  registerEditorActions,
  clearEditorChrome,
} from './composables/useAppChrome';
import { SUPPORT_EMAIL } from './constants';
import { apiFetch, apiFetchJson } from './lib/apiFetch';
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
import type { EditSheetItem, EditSheetSave } from './components/EditSheet.vue';

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

/** A save from EditSheet, or a field patch from MachineReadable (`section: 'produces'`). */
type MrSavePayload = EditSheetSave | { section: 'produces'; key: string; data: string | null };

/** A delete from MachineReadable: a definition by name, or a row by index. */
type MrDeletePayload =
  | { section: 'definition'; key: string }
  | { section: 'parameter' | 'input' | 'output' | 'action'; index: number };

/** The operation-node slots findIncompleteOperation walks; the tree comes from unchecked YAML. */
interface OperationLike {
  operation?: string;
  values?: unknown;
  conditions?: unknown;
  cases?: unknown;
  subject?: unknown;
  value?: unknown;
  default?: unknown;
  date_of_birth?: unknown;
  reference_date?: unknown;
}

const { authenticated, oidcConfigured } = useAuth();
const { isEnabled } = useFeatureFlags();

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

const availableViews = computed(() => VIEW_DEFINITIONS.filter(v => isEnabled(v.flag)));

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
const { activeTrajectRef, activeTraject, trajectMissing } = useTrajects();
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

// "Naar traject" buttons: the Traject tab's target for this editor's traject,
// so they land on the same page the tab would.
const libraryTabTarget = computed(() =>
  trajectTabTarget(router, lastTrajectPath.value, (route.params.trajectRef as string | undefined) ?? null),
);
const libraryTabHref = computed(() => router.resolve(libraryTabTarget.value).href);

// --- Initial law load (from route params) ---
const {
  law,
  lawId,
  rawYaml,
  articles,
  lawName,
  selectedArticle,
  selectedArticleNumber,
  switchLaw,
  clearLaw,
  lawTrajectRef,
  loading,
  error,
  saving: lawSaving,
  saveError: lawSaveError,
  saveLaw,
  reloadLaw,
  seedFromYaml,
  createLaw,
  currentEtag,
  lastSavedPr,
} = useLaw(
  singleParam(route.params.lawId),
  singleParam(route.params.articleNumber),
  singleParam(route.params.trajectRef),
);

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
  switchLaw,
  clearLaw,
  error,
  router,
  editorRouteFor,
  canPrune: () => activeTraject.value != null,
});

// When the active traject changes (router.push to /editor/{otherRef}/…)
// the URL stays on the same component; re-fetch the open law through the
// new traject's backends. The corpus list needs no handling here -
// `useCorpusLaws(activeTrajectRef)` re-scopes reactively on the same
// change. `switchLaw` crosses trajects too via its third argument so the
// law cache key stays correct.
//
// Also flush the WASM engine: it caches loaded laws by id only, so
// without this a scenario run after a traject switch would evaluate
// the open law against the *previous* traject's dependencies. The
// dependency walker re-loads on demand on the next run, so a single
// `unloadAllLaws` is enough - no per-dep bookkeeping needed.
//
// This handler is an async chain (`await switchLaw` on a deep link, then
// `await nextTick`) that mutates shared state, so two switches in quick
// succession (rapid back/forward across trajects) can overlap. Guard it with
// the same `useLatest()` pattern `selectTab`/`onBeforeRouteUpdate` use, so a
// stale invocation can't land its writes - or lower the auto-open latch a newer
// switch raised - after the newer one, which would leave the bar showing the
// previous traject's tabs while the route is on the new one.
const claimTrajectSwitch = useLatest();
watch(activeTrajectRef, async (next) => {
  const isCurrent = claimTrajectSwitch();
  // A traject switch reconciles the bar for `next`: stand the auto-open
  // robustness net down for this tick so it doesn't race the restore below.
  suppressTabAutoOpen = true;
  unloadAllLaws();
  // Tabs are DERIVED from the active traject: the `openTabs`/`activeTab`
  // computeds have already flipped to `next`'s bucket now the route committed,
  // so there is nothing to swap here. Resolve the swapped-in tabs' labels
  // through the new traject.
  backfillTabLabels(next);
  const urlLawId = singleParam(route.params.lawId);
  if (urlLawId) {
    // A URL that still names a law in the new traject can only come from a deep
    // link or browser back/forward now (the traject switcher navigates to the
    // bare root - see trajectSwitchTarget). Make sure that law is loaded THROUGH
    // the new traject before adopting it as a tab.
    //
    // onBeforeRouteUpdate only (re)loads when the URL's lawId *changes*; a
    // cross-traject nav that keeps the SAME lawId (a bookmarked/back-forward URL
    // differing only in trajectRef, or a law shared across trajects) skips it,
    // leaving law.value / selectedArticleNumber / useLaw's currentTrajectRef
    // pointed at the PREVIOUS traject's copy. Reload here whenever the open law
    // isn't already this traject's copy, so we never adopt stale, wrong-traject
    // content - and so a later save PUTs to the right traject. When the id and
    // traject already match (the lawId-changed deep link, loaded by
    // onBeforeRouteUpdate), this is skipped - no duplicate fetch.
    if (lawTrajectRef.value !== next || lawId.value !== urlLawId) {
      await switchLaw(urlLawId, singleParam(route.params.articleNumber), next);
      // A newer switch superseded us mid-load: drop our writes so we don't
      // reconcile the bar against a traject the route already left.
      if (!isCurrent()) return;
    }
    // Only reflect the URL's law as this traject's active tab when it actually
    // loaded FOR this traject. When the traject doesn't have the law, switchLaw
    // 404s: `error` is set and lawId/selectedArticleNumber still hold the
    // previous traject's law. Adopting that would add a foreign law as an
    // open+active tab and persist it under this traject - re-introducing the
    // exact leak this PR fixes. Require a clean load, for this traject, whose id
    // matches the URL; the error view renders the "niet beschikbaar" dialog for
    // the failed case.
    const loaded =
      !error.value &&
      lawTrajectRef.value === next &&
      lawId.value === singleParam(route.params.lawId) &&
      selectedArticleNumber.value != null;
    if (loaded) {
      // openTab de-dups, caps at MAX_TABS and marks it active - all under `next`.
      // `loaded` checked that lawId equals the URL's law id: a string, unless
      // the route left the law during the await above.
      openTab(next, { lawId: lawId.value as string, articleNumber: String(selectedArticleNumber.value) });
    } else {
      // Failed cross-traject deep link: don't leave the previous traject's law
      // marked active in this traject's bar (it isn't in this bucket here).
      setActiveTab(next, null);
    }
  } else {
    // No law in the URL (the switcher dropped us on the traject root): restore
    // the last active article of the traject we just entered, pruning any tab
    // whose law 404s here, or land on the neutral root. This deliberately
    // replaces the previous "always neutral on switch" behaviour.
    await tabRestore.restoreForTraject(next, { hasLawInUrl: false });
    if (!isCurrent()) return;
  }
  await nextTick();
  // Only the latest switch may lower the latch: a stale invocation clearing it
  // here would re-enable the auto-open net mid-way through a newer switch.
  if (!isCurrent()) return;
  suppressTabAutoOpen = false;
});

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
  if (!isEnabled('panel.regelrijm')) return new Map<string, LinkedFragment[]>();
  return linksByOutput(notesForArticle.value, {
    lawId: [law.value?.$id, lawId.value],
    text: selectedArticle.value?.text || '',
  });
});

// The action a linking note points at in this article, for "Actie openen" in
// the note popover; null for any other note or with the flag off.
function linkedAction(note: Note): ExecutionAction | null {
  if (!isEnabled('panel.regelrijm')) return null;
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

// Shared corpus list via `useCorpusLaws` - the same per-scope cache
// MachineReadable reads, so an editor mount fires ONE laws-list fetch
// instead of a private duplicate GET. Traject switches re-scope
// reactively; fetch failures degrade to the humanized law id.
const {
  displayName: corpusDisplayName,
  refresh: refreshCorpusLaws,
} = useCorpusLaws(activeTrajectRef);

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
 * Retry the failed law fetch. switchLaw clears `error` and re-runs the
 * fetch; failed responses don't enter the cache so a retry actually hits
 * the network again.
 */
function retryLoadLaw() {
  if (!lawId.value) return;
  switchLaw(lawId.value, selectedArticleNumber.value, undefined, { minLoadingMs: RETRY_MIN_SPINNER_MS });
}

function onSearchSelectLaw(lawIdVal: string) {
  // Open in the library - search currently only matches law names. As
  // soon as article-level search lands, we can route directly into the
  // editor (with the chosen article as the active tab).
  router.push(libraryRouteFor(lawIdVal));
}

async function onSearchHarvestAvailable(slug: string) {
  // Best-effort reload - a failure just means the list below may not
  // include the fresh law yet.
  await apiFetch('/api/corpus/reload', {
    method: 'POST',
    headers: { 'Content-Type': 'application/json' },
    body: JSON.stringify({ law_ids: [slug] }),
  }).catch(() => {});
  // Bust the shared per-scope cache so the fresh law shows up with its
  // real display name instead of the humanized slug fallback.
  await refreshCorpusLaws();
  router.push(libraryRouteFor(slug));
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
// (`useTabRestore`) are initialised near the useLaw block above; the storage
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
  // law URL in another traject) `switchLaw` loads the law a tick before the
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

// Shared between selectTab and the route guard below: a tab switch and a
// URL-driven switch supersede each other.
const claimTabSwitch = useLatest();

async function selectTab(tab: OpenTab) {
  const isCurrent = claimTabSwitch();
  setActiveTab(activeTrajectRef.value, tab);
  // Restore snapshot if the user is mid-edit, otherwise the partial mutations
  // would persist into the new tab's view.
  if (activeAction.value) {
    handleActionClose();
  }
  if (tab.lawId === lawId.value) {
    selectedArticleNumber.value = tab.articleNumber;
  } else {
    // Fetch through the tab's own traject (every open tab belongs to the
    // active traject). Passing it explicitly matters when this restore runs
    // right after a traject switch with no law in the URL: `switchLaw`'s
    // internal traject is still the previous one, so without this the law
    // would be read through the old traject's scope.
    await switchLaw(tab.lawId, tab.articleNumber, singleParam(route.params.trajectRef) || null);
    if (!isCurrent()) return; // stale, another switch started
    lawNames.value = {
      ...lawNames.value,
      [lawNameKey(activeTrajectRef.value, tab.lawId)]: lawName.value,
    };
  }
  // Sync the URL so deep-linking and browser back/forward stay in step.
  // `replace` (not `push`) keeps history clean - a tab switch isn't
  // navigation the user wants to undo with the back button.
  router.replace(editorRouteFor(tab.lawId, tab.articleNumber));
}

// On the INITIAL mount, restore the last active article of this traject when
// the URL carries no law (a fresh load / refresh returns the user where they
// were); a law in the URL is a deep link and wins. A traject switch and browser
// back/forward run the same restore through their own hooks (the
// watch(activeTrajectRef) else-branch and onBeforeRouteUpdate below). Replaces
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
// `watch(activeTrajectRef)` above already does `unloadAllLaws` +
// `switchLaw`, and triggering switchLaw twice in
// the same tick would burn an extra fetch (the first await loses
// useLaw's stale-switch race, but still hits the network). This guard
// handles the law / article portion only.
onBeforeRouteUpdate(async (to) => {
  const newLawId = singleParam(to.params.lawId);
  const newArticle = singleParam(to.params.articleNumber);
  if (!newLawId) {
    // Back/forward (or the Traject/Editor tab) to the bare editor root WITHIN the
    // same traject: the watch(activeTrajectRef) won't fire (ref unchanged), so
    // restore the last active article here. A trajectRef change is left to that
    // watch (which restores too). Not awaited and not blocking: restore's own
    // `router.replace` fires only after its `switchLaw` await, by which point
    // this navigation to the bare root has already committed.
    const trajectChanged =
      (to.params.trajectRef || null) !== (route.params.trajectRef || null);
    if (!trajectChanged) {
      tabRestore
        .restoreForTraject(singleParam(to.params.trajectRef) || null, { hasLawInUrl: false })
        .catch(console.warn);
    }
    return;
  }
  if (newLawId !== lawId.value) {
    const isCurrent = claimTabSwitch();
    await switchLaw(newLawId, newArticle, singleParam(to.params.trajectRef) || null);
    if (!isCurrent()) return;
    lawNames.value = {
      ...lawNames.value,
      [lawNameKey(singleParam(to.params.trajectRef) || null, newLawId)]: lawName.value,
    };
  } else if (newArticle && String(newArticle) !== String(selectedArticleNumber.value)) {
    selectedArticleNumber.value = String(newArticle);
  }
});

// `next` is the tab bar's own replacement pick, handed over by its tabdismiss
// event (see AppShell's onTabDismiss). Follow it: the bar has already marked
// that item selected, so choosing differently here leaves both lit. Callers
// without a bar (no pick to follow) get the same rule the bar applies: the
// neighbour to the right, else the one to the left.
function closeTab(tab: OpenTab, next: OpenTab | null = null) {
  const wasActive = !!activeTab.value && tabKey(activeTab.value) === tabKey(tab);
  // The store removes the tab, promotes the replacement (the bar's `next` pick,
  // else right-then-left) and persists both keys - closing the active tab and
  // reloading must not resurrect it.
  const replacement = closeTabIn(activeTrajectRef.value, tab, next);
  if (!wasActive) return;
  // Drive selectTab so the promoted tab's law actually loads into the panes.
  if (replacement) selectTab(replacement).catch(console.warn);
  else {
    // Last tab closed: clear the panes and sit on the traject's editor root.
    clearLaw();
    router.replace(editorRouteFor(null, null));
  }
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
  select: (tab) => { selectTab(tab).catch(console.warn); },
  close: closeTab,
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
// the given traject so tab labels match what the editor pane shows after a
// save. Runs on startup and again after a traject switch, since a switch swaps
// in that traject's own tab set whose labels still need resolving.
function backfillTabLabels(trajectRef: string | null | undefined) {
  const uniqueLawIds = [...new Set(tabsFor(trajectRef).map(t => t.lawId))];
  Promise.all(uniqueLawIds.map(async (id) => {
    try {
      const entry = await fetchLaw(trajectRef, id);
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
const activeAction = ref<ExecutionAction | null>(null);
// From a RegelRijm reading: the path of the part of the action to open at.
const activeActionPath = ref<Path | null>(null);
// True while the open action sheet is for a freshly added action, so its
// Save button is always offered (no edit required to create it).
const activeActionIsNew = ref(false);
const activeEditItem = ref<EditSheetItem | null>(null);
const parseError = ref<string | null>(null);

const machineReadable = ref<MachineReadableSection | null>(null);

// The YAML pane edits the machine-readable as text. There is something to edit
// when the article carries machine_readable, or one was created/edited this
// session (the live ref). With neither, the empty code editor is replaced by a
// message - like the Machine pane and the read-only YAML view. Deliberately not
// keyed on the live ref alone: clearing or mid-typing invalid YAML nulls it, and
// that must not yank the editor out from under the user.
const hasMachineReadable = computed(
  () => !!machineReadable.value || !!selectedArticle.value?.machine_readable,
);
const yamlSource = ref('');
// In-memory markdown for the currently selected article's `text` field.
// Seeded on article switch alongside machineReadable so the Tekst and Machine
// panes reset in lockstep when the user tabs to a different article.
const editedText = ref('');

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

watch(selectedArticle, (article) => {
  activeAction.value = null;
  activeEditItem.value = null;
  const mr = article?.machine_readable;
  machineReadable.value = mr ? structuredClone(mr) : null;
  yamlSource.value = mr ? yaml.dump(mr, dumpOpts) : '';
  editedText.value = article?.text ?? '';
  parseError.value = null;
}, { immediate: true });

const editedArticle = computed(() => {
  if (!selectedArticle.value) return null;
  return {
    ...selectedArticle.value,
    text: editedText.value,
    machine_readable: machineReadable.value,
  };
});

// Parse rawYaml once per law load into a reusable document skeleton. The
// computed below splices in the currently edited article's
// machine_readable on every reactive change, so without this cache each
// keystroke in the YAML textarea would re-parse the whole ~25-200 KiB law
// on the main thread. Hoisting the parse to a computed keyed only on
// rawYaml drops that cost to one parse per load.
const parsedRawLaw = computed(() => {
  if (!rawYaml.value) return null;
  try {
    // Unchecked YAML: a law body is a mapping, the code below guards the rest.
    return yaml.load(rawYaml.value) as LawDocument | null;
  } catch {
    return null;
  }
});

// Reactive "edited" law YAML: rawYaml with the currently selected article's
// machine_readable substituted in. This is what flows into the engine and
// into ScenarioBuilder, so in-memory edits re-execute scenarios without a
// round-trip through the backend.
//
// Only the currently selected article's machine_readable is swapped - edits
// on other articles are not tracked across tab switches (existing behavior
// of the editor state model).
//
// KNOWN LIMITATION: when this value is sent to `saveLaw` (via the Machine
// panel save button), the body is the `yaml.dump` output of the
// reconstructed document - which strips YAML comments and may reorder
// top-level keys compared to `rawYaml`. The YAML-pane edit path preserves
// the user's exact text via `yamlSource`, so it does not have this drift.
// Today's corpus is harvester-generated and comment-free, so the impact is
// zero in practice; revisit if hand-annotated laws are introduced (e.g.
// keep an "as-typed" base alongside `rawYaml` and only re-dump the edited
// article).
const currentLawYaml = computed(() => {
  if (!rawYaml.value) return null;
  if (!selectedArticle.value) return rawYaml.value;
  const base = parsedRawLaw.value;
  if (!base) return rawYaml.value;
  try {
    // Shallow-clone the doc and the articles array so our splice doesn't
    // mutate the memoized `parsedRawLaw` value - Vue would consider the
    // computed still fresh but the next read would see our substituted
    // article instead of the original.
    const doc = { ...base };
    const docArticles = Array.isArray(base.articles) ? [...base.articles] : null;
    if (!docArticles) return rawYaml.value;
    const idx = docArticles.findIndex(
      (a) => String(a.number) === String(selectedArticleNumber.value),
    );
    if (idx < 0) return rawYaml.value;
    // Short-circuit when neither pane has been touched: a `yaml.dump`
    // round-trip can cosmetically reformat the parsed doc (key ordering,
    // string quoting), and `handleActionSave` calls `saveLaw(currentLawYaml)`
    // without checking dirty flags, so a no-op action-modal save on a
    // pristine article would otherwise produce a reformat-only commit.
    // idx >= 0 was checked above.
    const baseArticle = docArticles[idx]!;
    const textPristine = editedText.value === (baseArticle.text ?? '');
    const baselineMr = baseArticle.machine_readable ?? null;
    const currentMr = machineReadable.value ?? null;
    const mrPristine = baselineMr === currentMr
      || JSON.stringify(baselineMr) === JSON.stringify(currentMr);
    if (textPristine && mrPristine) return rawYaml.value;
    // Only splice fields that have diverged from the base - passing
    // `machineReadable.value` verbatim when it's null would erase the
    // article's machine_readable from the serialized doc, and similarly
    // for text. The dirty computeds below drive this same contract.
    const patched = { ...baseArticle };
    if (!textPristine) {
      patched.text = editedText.value;
    }
    if (machineReadable.value != null) {
      patched.machine_readable = machineReadable.value;
    } else if (baselineMr != null) {
      // The user opened an article that had machine_readable and cleared the
      // YAML editor. The spread above carried the original key over; we have
      // to drop it explicitly so the save persists the deletion rather than
      // silently round-tripping the original content.
      delete patched.machine_readable;
    }
    docArticles[idx] = patched;
    doc.articles = docArticles;
    return yaml.dump(doc, dumpOpts);
  } catch {
    return rawYaml.value;
  }
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

// Dirty state: the selected article's in-memory machine_readable differs
// from the article's saved copy. `machineReadable.value` starts as a deep
// JSON clone of `selectedArticle.machine_readable` (see the `watch` above),
// so for field-based edits the two share the same key order and
// `JSON.stringify` is a cheap, accurate structural comparison.
//
// Note: the YAML-pane edit path (`onYamlInput`) replaces `machineReadable`
// with a fresh `yaml.load(text)` object whose key order comes from the
// textarea, so a no-op round-trip can flip this flag to `true` even when
// the semantic content is unchanged. That's a conservative false positive
// - the worst case is an enabled save button - so we accept it rather
// than pay for a canonical YAML dump on every keystroke.
const isMachineReadableDirty = computed(() => {
  if (!selectedArticle.value) return false;
  const saved = selectedArticle.value.machine_readable ?? null;
  const current = machineReadable.value ?? null;
  if (saved == null && current == null) return false;
  try {
    return JSON.stringify(saved) !== JSON.stringify(current);
  } catch {
    return true;
  }
});

const isArticleTextDirty = computed(() => {
  if (!selectedArticle.value) return false;
  return (selectedArticle.value.text ?? '') !== (editedText.value ?? '');
});

// Tracks which pane(s) had dirty edits at the time of the most recent save
// attempt. Used to scope `lawSaveError` to the pane that actually triggered
// the save - without this, a save initiated from the machine pane that
// fails would surface the "Opslaan mislukt" dialog inside the text-pane
// body too, blaming a pane the user didn't touch.
const lastSaveTouchedText = ref(false);
const lastSaveTouchedMachine = ref(false);

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
// In welk artikel het voorstel hoort te landen (`payload.article`, of - bij een
// voorstel over de hele wet - het artikel dat `proposalDivergence` uitkoos), en
// welk artikel het op dit moment daadwerkelijk vasthoudt. Die twee lopen uiteen
// zodra je naar een ander artikel kijkt: `watch(selectedArticle)` zet de panes
// dan terug naar de opgeslagen wet. Het verschil is precies het signaal om bij
// terugkomst opnieuw te seeden.
const reviewSeedTarget = ref<string | null>(null);
// Waar het voorstel op dit moment daadwerkelijk in de panes staat, als
// `wet::artikel`. De wet hoort in die sleutel: twee wetten in hetzelfde traject
// kunnen dezelfde artikelnummers hebben ("B 5", "1", "2" zijn niet uniek), en
// bij een wetwissel verandert `selectedArticleNumber` dan niet terwijl
// `watch(selectedArticle)` de panes wél heeft teruggezet naar de opgeslagen wet.
// Op het nummer alleen zou het voorstel er bij terugkomst niet opnieuw in gaan.
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
  // Meteen (niet pas in de nextTick hieronder): de watch die bij een
  // artikelwissel opruimt en de her-seed-watch draaien in de flush ertussen, en
  // moeten dit al als "geseed" zien - anders seeden ze er direct nog eens
  // overheen.
  reviewSeededAt.value = reviewSeedKey(String(target.number));
  selectedArticleNumber.value = String(target.number);
  // `watch(selectedArticle)` (above) resets editedText/machineReadable to
  // the (still-saved) newly selected article; wait a tick so the seed
  // below lands after that reset instead of being clobbered by it.
  nextTick(() => {
    const mr = target.machine_readable ?? null;
    machineReadable.value = mr ? structuredClone(mr) : null;
    yamlSource.value = mr ? yaml.dump(mr, dumpOpts) : '';
    editedText.value = target.text ?? '';
  });
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
// that's the initial load or a tab-restore switchLaw), so it works
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
    // naar de wetsroute zouden op een 404 landen, dus terug naar het traject.
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

async function enrichLaw() {
  if (!activeTrajectRef.value || !lawId.value) return;
  try {
    const { alreadyRunning, tooMany } = await requestEnrich();
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
  const lawYaml = reviewSavesWholeLaw.value ? reviewProposedContent.value : currentLawYaml.value;
  if (!lawYaml) return;
  // Snapshot the law id before the await. saveLaw itself guards its own
  // reactive writes with the same check, but the post-save cleanup below
  // runs in the EditorApp scope and would happily overwrite the new law's
  // in-progress edits with its pristine article data if the user switched
  // laws mid-flight.
  const savedLawId = lawId.value;
  // In review-modus the saved YAML is the full proposal, not a splice of
  // the visible pane's edits - always treat it as touching text (notes
  // re-anchor safely; skipping would risk leaving a note's positions stale
  // against text the proposal actually changed elsewhere in the law).
  lastSaveTouchedText.value = reviewSavesWholeLaw.value ? true : isArticleTextDirty.value;
  lastSaveTouchedMachine.value = reviewSavesWholeLaw.value ? true : isMachineReadableDirty.value;
  try {
    // law_create: de wet bestaat nog niet in het traject, dus het voorstel
    // gaat door het create-pad (POST, pad server-side afgeleid) - daarna
    // bestaat de wet en nemen vervolg-saves de normale PUT.
    if (reviewActive.value && reviewIsLawCreate.value) {
      await createLaw(lawYaml);
    } else {
      await saveLaw(lawYaml);
    }
    if (lawId.value !== savedLawId) return; // law switched mid-PUT
    // points at the re-parsed article. The `watch(selectedArticle)` above
    // fires on the next microtask - leaving a window where the dirty
    // computeds still see the pre-save values and the save button stays
    // enabled, enabling a double-save click. Reset local state explicitly
    // from the freshly-parsed article so both dirty flags clear
    // synchronously with the save.
    const fresh = selectedArticle.value;
    const freshMr = fresh?.machine_readable ?? null;
    machineReadable.value = freshMr ? structuredClone(freshMr) : null;
    yamlSource.value = freshMr ? yaml.dump(freshMr, dumpOpts) : '';
    editedText.value = fresh?.text ?? '';
    // The article text changed, so notes must re-resolve against the saved copy:
    // their quote-based selectors re-anchor via the engine, so a note whose
    // quoted text merely moved comes back at its new place, while one whose own
    // text was edited orphans (surfaced in the issues list) rather than silently
    // re-attaching. Fire-and-forget.
    if (lastSaveTouchedText.value) void reloadNotes();
    // Successful save - the dialog flags drop back to false.
    lastSaveTouchedText.value = false;
    lastSaveTouchedMachine.value = false;
    // Review-modus: a successful save IS the approval (spec §5.3 - save
    // first, then resolve). Runs after the dirty-state reset above so the
    // Wijzigingenbalk has already cleared before the task disappears.
    if (reviewActive.value) {
      await approveAfterSave();
      clearReviewQuery();
    }
  } catch (e) {
    // saveError is surfaced via lawSaveError; log for dev visibility.
    console.warn('saveLaw failed:', e);
  }
}

// Whole-law save failures surface as a single modal over the editor (not an
// inline dialog buried in one pane). `saveErrorText` drives it: a new failure
// (a fresh Error) re-opens it via the watch, a successful save (null) closes
// it. Het verwerken van een verrijking hangt er ook aan - dat is voor de
// gebruiker dezelfde gebeurtenis (mijn wijziging kwam er niet in) en verdient
// dezelfde melding, inclusief de 412-uitleg bij een conflict.
const saveFailure = computed(() => reviewProcessError.value ?? lawSaveError.value);
const saveErrorText = computed(() =>
  saveFailure.value ? saveFailure.value.message || String(saveFailure.value) : '',
);
const saveErrorModalEl = ref<NLDDModalDialog | null>(null);
watch(saveFailure, (err) => {
  const el = saveErrorModalEl.value;
  if (!el) return;
  if (err && typeof el.show === 'function') el.show();
  else if (!err && typeof el.hide === 'function') el.hide();
});
function dismissSaveError() {
  reviewProcessError.value = null;
  saveErrorModalEl.value?.hide?.();
}

// Alias kept to minimise template churn; both panes ultimately call the
// same whole-law save.
const handleMachineReadableSave = handleLawSave;

// --- Wijzigingenbalk (article-level pending changes) -----------------------
// Tekst + Machine edits share one whole-law save, so "dirty" is article-wide.
// The AppShell renders the bar; the editor publishes the state and the
// Opslaan / Wijzigingen-ongedaan / undo / redo actions via useAppChrome.
const articleDirty = computed(
  () => isArticleTextDirty.value || isMachineReadableDirty.value,
);

// Throw away every in-memory edit for the selected article - the same reset
// the post-save cleanup performs, minus the PUT.
function discardArticle() {
  const fresh = selectedArticle.value;
  if (!fresh) return;
  const freshMr = fresh.machine_readable ?? null;
  machineReadable.value = freshMr ? structuredClone(freshMr) : null;
  yamlSource.value = freshMr ? yaml.dump(freshMr, dumpOpts) : '';
  editedText.value = fresh.text ?? '';
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
  yamlSource.value = text;
  try {
    const parsed = yaml.load(text);
    // Unchecked YAML, like every machine_readable the editor reads.
    machineReadable.value = parsed != null && typeof parsed === 'object' ? (parsed as MachineReadableSection) : null;
    parseError.value = null;
  } catch (e) {
    // js-yaml throws a YAMLException.
    parseError.value = (e as Error).message;
  }
}

// The payload is read field by field (not destructured) so each branch sees
// its own section's fields.
function handleSave(payload: MrSavePayload) {
  // JSON round-trip clone: Vue reactive proxies aren't structuredClone-able
  // in all envs. Safe because law YAML is JSON-plain - but Date/undefined
  // are NOT preserved, which only matters if a future field becomes
  // date-typed (use an explicit serialiser then).
  const mr: MachineReadableSection = machineReadable.value
    ? JSON.parse(JSON.stringify(machineReadable.value))
    : {};

  if (!mr.definitions) mr.definitions = {};
  if (!mr.execution) mr.execution = {};
  if (!mr.execution.parameters) mr.execution.parameters = [];
  if (!mr.execution.input) mr.execution.input = [];
  if (!mr.execution.output) mr.execution.output = [];

  // EditSheet types `key` and `index` as optional because its item type is
  // shared by every section; an existing definition or row always arrives with
  // them (MachineReadable's open-edit payload), hence the casts below.
  if (payload.section === 'definition') {
    if (payload.newKey && payload.newKey !== payload.key) delete mr.definitions[payload.key as string];
    mr.definitions[(payload.newKey || payload.key) as string] = payload.data;
  } else if (payload.section === 'add-definition') {
    mr.definitions[payload.key] = payload.data;
  } else if (payload.section === 'parameter') {
    mr.execution.parameters[payload.index as number] = payload.data;
  } else if (payload.section === 'add-parameter') {
    mr.execution.parameters.push(payload.data);
  } else if (payload.section === 'input') {
    mr.execution.input[payload.index as number] = payload.data;
  } else if (payload.section === 'add-input') {
    mr.execution.input.push(payload.data);
  } else if (payload.section === 'output') {
    mr.execution.output[payload.index as number] = payload.data;
  } else if (payload.section === 'add-output') {
    mr.execution.output.push(payload.data);
  } else if (payload.section === 'produces') {
    const produces = withDecisionField(mr.execution.produces, payload.key, payload.data);
    if (produces === undefined) delete mr.execution.produces;
    else mr.execution.produces = produces;
  }

  machineReadable.value = mr;
  yamlSource.value = yaml.dump(machineReadable.value, dumpOpts);
  parseError.value = null;
}

// Delete an item from the machine_readable. Mirrors handleSave's section
// dispatch but removes the entry instead of replacing it. Definitions are
// keyed by name; parameters / inputs / outputs / actions are keyed by
// array index. Out-of-range indices and missing keys are no-ops so a
// stale event from the UI can never crash.
// Read field by field, like handleSave, so each branch sees its own fields.
function handleDelete(payload: MrDeletePayload) {
  // JSON clone - see handleSave: Date/undefined not preserved (fine for
  // JSON-plain law data).
  const mr: MachineReadableSection | null = machineReadable.value
    ? JSON.parse(JSON.stringify(machineReadable.value))
    : null;
  if (!mr) return;

  if (payload.section === 'definition') {
    if (mr.definitions && payload.key != null && payload.key in mr.definitions) {
      delete mr.definitions[payload.key];
    }
  } else if (payload.section === 'parameter') {
    if (mr.execution?.parameters && payload.index >= 0 && payload.index < mr.execution.parameters.length) {
      mr.execution.parameters.splice(payload.index, 1);
    }
  } else if (payload.section === 'input') {
    if (mr.execution?.input && payload.index >= 0 && payload.index < mr.execution.input.length) {
      mr.execution.input.splice(payload.index, 1);
    }
  } else if (payload.section === 'output') {
    if (mr.execution?.output && payload.index >= 0 && payload.index < mr.execution.output.length) {
      mr.execution.output.splice(payload.index, 1);
    }
  } else if (payload.section === 'action') {
    if (mr.execution?.actions && payload.index >= 0 && payload.index < mr.execution.actions.length) {
      mr.execution.actions.splice(payload.index, 1);
    }
  }

  machineReadable.value = mr;
  yamlSource.value = yaml.dump(machineReadable.value, dumpOpts);
  parseError.value = null;
}

// Initialize empty machine_readable scaffold
function handleInitMr() {
  machineReadable.value = {
    definitions: {},
    execution: {
      parameters: [],
      input: [],
      output: [],
      actions: [],
    },
  };
  yamlSource.value = yaml.dump(machineReadable.value, dumpOpts);
  parseError.value = null;
}

// Add a new action and open ActionSheet
let actionSnapshot: string | null = null;

function handleAddAction() {
  // Snapshot BEFORE any mutations so cancel restores the exact original state
  actionSnapshot = JSON.stringify(machineReadable.value);
  const mr: MachineReadableSection = machineReadable.value || {};
  if (!mr.execution) mr.execution = {};
  if (!mr.execution.actions) mr.execution.actions = [];
  // Seed the new action with an EQUALS stub instead of an empty literal so
  // OperationSettings has an operation tree to render and the user can
  // immediately reach the type dropdown to switch to AGE / AND / etc.
  // The findIncompleteOperation guard rejects unfilled stubs on save, so
  // a half-configured action still can't be persisted.
  const newAction = {
    output: '',
    value: { operation: 'EQUALS', subject: '', value: '' },
  };
  mr.execution.actions.push(newAction);
  machineReadable.value = { ...mr };
  yamlSource.value = yaml.dump(machineReadable.value, dumpOpts);
  parseError.value = null;
  activeActionIsNew.value = true;
  activeActionPath.value = null;
  activeAction.value = newAction;
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
  actionSnapshot = JSON.stringify(machineReadable.value);
  activeActionIsNew.value = false;
  activeActionPath.value = Array.isArray(path) ? path : null;
  activeAction.value = action;
  // Clear any stale parse error from a previous failed save
  parseError.value = null;
}

// Restore model from snapshot when ActionSheet is cancelled
function handleActionClose() {
  if (actionSnapshot) {
    machineReadable.value = JSON.parse(actionSnapshot);
    yamlSource.value = yaml.dump(machineReadable.value, dumpOpts);
    actionSnapshot = null;
  }
  activeAction.value = null;
  // Clear any stale parse error from a failed save attempt
  parseError.value = null;
}

const COMPARISON_OPS_SET = new Set([
  'EQUALS', 'NOT_EQUALS', 'GREATER_THAN', 'GREATER_THAN_OR_EQUAL',
  'LESS_THAN', 'LESS_THAN_OR_EQUAL', 'NOT_NULL', 'IN', 'NOT_IN',
]);

// Walk a value tree and report the first incomplete operation (e.g. a stub
// `{ operation: 'ADD', values: [] }` that the user inserted via "Voeg operatie
// toe" but never filled in). Returns null when the tree is structurally valid.
function findIncompleteOperation(raw: unknown): string | null {
  if (raw == null || typeof raw !== 'object') return null;
  // An operation node from unchecked YAML; only the slots below are read.
  const value = raw as OperationLike;
  if (!value.operation) return null;
  const op = value.operation;
  // Arithmetic / logical ops need a non-empty values or conditions array
  if (Array.isArray(value.values) && value.values.length === 0) return op;
  if (Array.isArray(value.conditions) && value.conditions.length === 0) return op;
  // IF/SWITCH need at least one case
  if ((op === 'IF' || op === 'SWITCH') && (!Array.isArray(value.cases) || value.cases.length === 0)) return op;
  // Comparison ops need a non-empty subject (and value, except for NOT_NULL).
  // changeOperationType / addNestedOperation seed these as empty strings, so
  // we must reject the stub before persisting. IN/NOT_IN accept either a
  // variable reference (e.g. "$list") or a literal non-empty array; both
  // are non-empty by the same value !== '' / array.length > 0 check.
  if (COMPARISON_OPS_SET.has(op)) {
    if ((value.subject ?? '') === '') return op;
    if (op !== 'NOT_NULL') {
      const v = value.value;
      if (v == null || v === '') return op;
      if (Array.isArray(v) && v.length === 0) return op;
    }
  }
  // NOT wraps a single value/operation; reject the empty-string stub created
  // when transitioning from arithmetic ops via changeOperationType.
  if (op === 'NOT' && (value.value ?? '') === '') return op;
  // AGE has two structural slots - both must be filled. Empty strings are
  // the seed values from changeOperationType('AGE'); reject them so the
  // user can't save a stub.
  if (op === 'AGE') {
    if ((value.date_of_birth ?? '') === '') return op;
    if ((value.reference_date ?? '') === '') return op;
  }
  // Recurse into structural slots
  for (const child of [value.subject, value.value, value.default, value.date_of_birth, value.reference_date]) {
    const inner = findIncompleteOperation(child);
    if (inner) return inner;
  }
  if (Array.isArray(value.values)) {
    for (const v of value.values) {
      const inner = findIncompleteOperation(v);
      if (inner) return inner;
    }
  }
  if (Array.isArray(value.conditions)) {
    for (const c of value.conditions) {
      const inner = findIncompleteOperation(c);
      if (inner) return inner;
    }
  }
  if (Array.isArray(value.cases)) {
    for (const c of value.cases) {
      const inner = findIncompleteOperation(c?.when) || findIncompleteOperation(c?.then);
      if (inner) return inner;
    }
  }
  return null;
}

// Sync YAML when ActionSheet saves (mutations happened in-place)
async function handleActionSave() {
  const action = activeAction.value;
  if (action) {
    // Output is required by the schema and the engine cannot load a law
    // with an action that has an empty output name.
    if (action.output == null || String(action.output).trim() === '') {
      parseError.value = 'Output mag niet leeg zijn';
      return;
    }
    // Reject incomplete nested operations (e.g. ADD with empty values[]) that
    // the user inserted via "Voeg operatie toe" but never filled in.
    // Note: a literal empty-string `value` is permitted at this layer - the
    // schema validator on save handles type-specific validation; rejecting it
    // here would block the legitimate "set output now, fill value via YAML
    // pane later" workflow used by the test suite and the editor's manual
    // YAML escape hatch.
    const incomplete = findIncompleteOperation(action.value);
    if (incomplete) {
      parseError.value = `Operatie '${incomplete}' is nog niet ingevuld`;
      return;
    }
  }
  // Commit the in-place mutations, then actually persist the law. The
  // sheet's "Opslaan" is a real save, so the Machine pane won't show a
  // separate dirty/save affordance for sheet edits. On a failed PUT the
  // edits stay in the model and the Machine pane's normal dirty/save
  // affordance is the fallback - no data loss either way.
  // JSON clone - see handleSave: reactive proxy not structuredClone-able;
  // Date/undefined not preserved (fine for JSON-plain law YAML).
  machineReadable.value = JSON.parse(JSON.stringify(machineReadable.value));
  yamlSource.value = yaml.dump(machineReadable.value, dumpOpts);
  parseError.value = null;
  // Close the sheet unconditionally: the mutations are already committed
  // above, so even if handleLawSave() throws the sheet must not stay open
  // showing a now-clean (isDirty === false) state. A failed PUT falls back
  // to the Machine pane's normal dirty/save affordance - no data loss.
  try {
    await handleLawSave();
  } finally {
    actionSnapshot = null;
    activeAction.value = null;
  }
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
             Both show the same "pick a tab or go to the traject" prompt, so this is NOT
             keyed on `openTabs.length`. Deliberately NOT keyed on `!activeTab`
             alone either - that stays null for the WHOLE load of an explicit
             article URL (the add-tab watch only sets it once the article
             resolves), so keying on it would hide the panes behind a full-page
             state exactly when we want them up; `!loading && !selectedArticle`
             guards that. `!error` keeps a failed explicit open on its own error
             branch below. -->
        <nldd-page v-else-if="!activeTab && !loading && !selectedArticle && !error">
          <nldd-simple-section width="full">
            <nldd-inline-dialog text="Open een artikel vanuit de tabbalk of het traject om te bewerken.">
              <nldd-button slot="actions" appearance="secondary" text="Naar traject" :href="libraryTabHref" @click.prevent="router.push(libraryTabTarget)"></nldd-button>
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
              <nldd-button slot="actions" appearance="primary" text="Naar traject" :href="libraryTabHref" @click.prevent="router.push(libraryTabTarget)"></nldd-button>
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
          <!-- Review-modus (job_review-taak) heeft geen eigen blok meer in de
               content. De melding gaat als `slot="review-notice"` naar de
               bar-split-view in AppShell (onder de document-tab-bar, boven
               `main`) en de beslissing staat in de Wijzigingenbalk eronder.
               Allebei via useAppChrome, net als de rest van de shell-chrome,
               zodat geen van beide met de panes meescrollt. -->

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
                    <nldd-badge color="critical" size="sm" text="YAML parse error"></nldd-badge>
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
                  :traject-ref="activeTrajectRef"
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
                    variant="warning"
                    :text="`${noteIssues.length} notitie(s) niet verankerd`"
                    :supporting-text="noteIssues.map(i => i.reason).join('; ')"
                  ></nldd-inline-dialog>
                  <nldd-inline-dialog
                    v-if="draftCount > 0 && !canEdit"
                    variant="warning"
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
    <ActionSheet regelrijm :action="activeAction" :article="editedArticle" :editable="canEdit" :is-new="activeActionIsNew" :law="law" :law-id="lawId" :traject-ref="activeTrajectRef" :focus-path="activeActionPath" @close="handleActionClose" @save="handleActionSave" @navigate="handleSheetNavigate" @open-output="handleSheetOpenOutput" />
    <EditSheet :item="activeEditItem" :article="editedArticle" :traject-ref="activeTrajectRef" @save="handleSave" @close="activeEditItem = null" />
    <SearchPopover
      ref="searchPopoverRef"
      @select-law="onSearchSelectLaw"
      @harvest-available="onSearchHarvestAvailable"
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

  <!-- Whole-law save failure, shown over the whole editor rather than inline in
       one pane. -->
  <nldd-modal-dialog
    ref="saveErrorModalEl"
    variant="alert"
    text="Opslaan mislukt"
    :supporting-text="saveErrorText"
    data-testid="save-error-modal"
    @close="dismissSaveError"
  >
    <nldd-button slot="actions" appearance="primary" text="Sluiten" @click="dismissSaveError"></nldd-button>
  </nldd-modal-dialog>

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
        :traject-ref="activeTrajectRef"
      />
    </nldd-page>
  </nldd-sheet>
</template>
