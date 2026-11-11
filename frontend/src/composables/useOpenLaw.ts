/**
 * useOpenLaw - the regelwerk open in the editor: its saved version, the
 * selected article, and the edits to that article that are not saved yet.
 *
 * The edits live in three forms the panes show: the article text, the
 * machine_readable as a structure (Machine pane, sheets) and as YAML text
 * (YAML pane). This module keeps the three in step, so no caller writes one
 * without the others. Every operation that changes what the edits are based
 * on (opening a law, selecting an article, a save, a reload) resets them from
 * the saved article in the same call, never in a later watcher.
 *
 * Only the selected article's edits are kept: selecting another article,
 * law or traject drops them. `hasUnsavedWork` says when that would lose
 * something, so the editor can ask first.
 *
 * Loading reads through an explicit traject, not the route's: during a
 * cross-traject navigation it loads for the traject the route has not
 * committed yet (see `lawTrajectRef`).
 */
import { computed, ref, shallowRef } from 'vue';
import * as yaml from 'js-yaml';
import { lastSavedPr, sanitizeSavedPr } from './useSavedPr';
import { lawCreateUrl, lawUrl } from './corpusUrls';
import { apiFetch } from '../lib/apiFetch';
import type { RequestError } from '../lib/apiFetch';
import { useCorpus, type LawBody } from '../corpus';
import { useLatest } from '../lib/useLatest';
import { resolveLawName } from '../lib/lawName';
import { holdRetryFloor } from '../lib/retryFeedback';
import { findIncompleteOperation } from '../lib/incompleteOperation';
import {
  applyDelete,
  applySave,
  emptyMachineReadable,
  newActionStub,
  type MachineReadableDelete,
  type MachineReadableSave,
} from '../lib/machineReadableEdits';
import type { Path } from '@regelrecht/frontend-shared/regelrijm/index';
import type { ExecutionAction, LawArticle, LawDocument, MachineReadableSection } from '../lib/lawFormat';

/** The JSON body of a law save/create (`{ pr, etag }`). */
type SaveResponse = { pr?: unknown; etag?: string | null } | null;

/** Where `open` goes. Without `trajectRef` it stays in the traject it is in. */
export interface OpenTarget {
  lawId: string;
  articleNumber?: string | number | null;
  trajectRef?: string | null;
  /** On a failed load, keep the loading state at least this long (a retry click). */
  minLoadingMs?: number;
}

/** How a save ended. */
export interface SaveOutcome {
  status: 'saved' | 'failed' | 'switched';
  /** Whether the save wrote article text, so notes need to re-resolve. */
  touchedText: boolean;
}

/** The action being edited in the action sheet: a copy, written back on commit. */
interface ActionEdit {
  /** Its index in `execution.actions`; null for a new action. */
  index: number | null;
  draft: ExecutionAction;
  /** The draft as it started, to tell whether it changed. */
  original: string;
  /** From a RegelRijm reading: the part of the action to open at. */
  path: Path | null;
}

const DUMP_OPTIONS = { lineWidth: 80, noRefs: true };

// yaml.load returns `unknown`; a law body is a mapping (or empty -> null/undefined).
function parseLaw(text: string): LawDocument | null {
  return yaml.load(text) as LawDocument | null;
}

// A deep copy of plain YAML data. JSON rather than structuredClone: Vue's
// reactive proxies are not structured-cloneable, and law data is JSON-plain.
function copy<T>(value: T): T {
  return JSON.parse(JSON.stringify(value)) as T;
}

function sameJson(a: unknown, b: unknown): boolean {
  try {
    return JSON.stringify(a ?? null) === JSON.stringify(b ?? null);
  } catch {
    return false;
  }
}

// apiFetch options for the direct-URL load. The thrown ApiError carries the
// HTTP status so callers can branch on `.status === 404`; the message
// matches the corpus reader's.
const lawFetchInit = Object.freeze({
  errorMessage: (status: number) => `Failed to fetch: ${status}`,
});

// Only surface an error body when it is the editor-api speaking: it answers
// 400/403 with `text/plain`, so anything else is a reverse proxy (a 5xx HTML
// page) and gets a generic message instead of proxy HTML in the dialog.
function saveErrorMessage(status: number, body: string, contentType: string): string {
  return contentType.startsWith('text/plain') && body ? body : `Save failed: ${status}`;
}

export function useOpenLaw(
  initial: { lawId?: string | null; articleNumber?: string | null; trajectRef?: string | null } = {},
) {
  // No law in the route: the `?law=` query may still carry one (the traject
  // chooser passes it through). Otherwise we open NOTHING - deliberately no
  // hardcoded fallback law; the editor decides what to open, e.g. by
  // restoring the last active tab.
  let lawParam = initial.lawId || new URLSearchParams(window.location.search).get('law') || null;
  const initialArticle = initial.articleNumber || null;
  const corpus = useCorpus();
  // The traject the open law is read from and saved to. `open` may move it
  // ahead of the route during a cross-traject navigation, so URL builders read
  // it through this closure instead of capturing a snapshot.
  let currentTrajectRef: string | null = initial.trajectRef || null;
  // Reactive mirror of `currentTrajectRef`. During a cross-traject navigation
  // it changes a tick before the route commits, so a watcher can tell "this
  // law belongs to the traject I'm loading into" from "the one I'm leaving".
  const lawTrajectRef = ref(currentTrajectRef);
  // A parameter that looks like a URL is fetched directly.
  const initialDirectUrl =
    lawParam && (lawParam.startsWith('/') || lawParam.startsWith('http')) ? lawParam : null;

  // --- The saved version ---------------------------------------------------
  const law = shallowRef<LawDocument | null>(null);
  const rawYaml = ref('');
  const selectedNumber = ref<string | null>(null);
  const loading = ref(true);
  // apiFetch rejects with an ApiError and the 412 path throws an Error.
  const error = ref<RequestError | null>(null);
  const saving = ref(false);
  const saveError = ref<RequestError | null>(null);
  // ETag of the last loaded/saved content; sent as `If-Match` on save so the
  // backend answers 412 when someone else edited in between.
  const currentEtag = ref<string | null>(null);

  const articles = computed((): LawArticle[] => law.value?.articles ?? []);
  const lawName = computed(() => resolveLawName(law.value));
  // The law's own id, or the requested one while it loads; null when no law
  // is open.
  const lawId = computed(() => law.value?.$id || lawParam);
  const selectedArticleNumber = computed(() => selectedNumber.value);
  const selectedArticle = computed(() => {
    if (!selectedNumber.value) return null;
    return articles.value.find((a) => String(a.number) === String(selectedNumber.value)) ?? null;
  });

  // --- The edits to the selected article ----------------------------------
  const machineReadable = ref<MachineReadableSection | null>(null);
  // The YAML pane's text. Kept apart from `machineReadable` because it may be
  // invalid mid-typing; `parseError` then says why.
  const yamlSource = ref('');
  const editedText = ref('');
  const parseError = ref<string | null>(null);
  // What the edits started from when a review proposal was seeded, instead
  // of the saved article. The guard compares against this, so an untouched
  // proposal is not someone's unsaved work.
  const seedBaseline = shallowRef<{ text: string; machineReadable: MachineReadableSection | null } | null>(null);
  const actionEdit = ref<ActionEdit | null>(null);
  // Which edits the last save attempt carried, so a failure is shown in the
  // pane that caused it.
  const lastSaveTouchedText = ref(false);
  const lastSaveTouchedMachine = ref(false);

  function setMachineReadable(mr: MachineReadableSection | null): void {
    machineReadable.value = mr;
    yamlSource.value = mr ? yaml.dump(mr, DUMP_OPTIONS) : '';
    parseError.value = null;
  }

  function setBuffer(article: Pick<LawArticle, 'text' | 'machine_readable'> | null): void {
    const mr = article?.machine_readable ?? null;
    setMachineReadable(mr ? copy(mr) : null);
    editedText.value = article?.text ?? '';
  }

  // Back to the saved article: the one reset every operation below shares.
  function resetEdits(): void {
    setBuffer(selectedArticle.value);
    seedBaseline.value = null;
    actionEdit.value = null;
  }

  // The selected article with the edits applied: what the panes show.
  const editedArticle = computed(() => {
    if (!selectedArticle.value) return null;
    return { ...selectedArticle.value, text: editedText.value, machine_readable: machineReadable.value };
  });

  // The YAML pane edits machine_readable as text. There is something to edit
  // when the article carries one, or one was created this session. Not keyed
  // on the live value alone: mid-typing invalid YAML nulls it, and that must
  // not yank the editor out from under the user.
  const hasMachineReadable = computed(
    () => !!machineReadable.value || !!selectedArticle.value?.machine_readable,
  );

  // Dirty against the saved article. A YAML-pane round trip can reorder keys
  // and flag a semantic no-op as dirty; the worst case is an enabled save
  // button, cheaper than a canonical dump per keystroke.
  const isMachineReadableDirty = computed(() => {
    if (!selectedArticle.value) return false;
    return !sameJson(selectedArticle.value.machine_readable, machineReadable.value);
  });
  const isArticleTextDirty = computed(() => {
    if (!selectedArticle.value) return false;
    return (selectedArticle.value.text ?? '') !== (editedText.value ?? '');
  });
  const articleDirty = computed(() => isArticleTextDirty.value || isMachineReadableDirty.value);

  const isActionEditDirty = computed(() => {
    const edit = actionEdit.value;
    return !!edit && JSON.stringify(edit.draft) !== edit.original;
  });

  /**
   * Whether leaving the article now would lose someone's work: edits beyond
   * what the article started from (the saved version, or a seeded proposal),
   * or an action-sheet edit not committed yet.
   */
  const hasUnsavedWork = computed(() => {
    if (isActionEditDirty.value) return true;
    const base = seedBaseline.value;
    if (!base) return articleDirty.value;
    return editedText.value !== base.text || !sameJson(base.machineReadable, machineReadable.value);
  });

  // Parse rawYaml once per load into a reusable skeleton, so a keystroke in
  // the YAML pane does not re-parse the whole (25-200 KiB) law.
  const parsedRawLaw = computed(() => {
    if (!rawYaml.value) return null;
    try {
      return parseLaw(rawYaml.value);
    } catch {
      return null;
    }
  });

  /**
   * The whole law with the selected article's edits spliced in: what the
   * engine runs and what a save writes. Only the selected article is
   * swapped; edits are not kept across articles.
   *
   * Known limitation: the spliced body is a `yaml.dump` of the reconstructed
   * document, which drops YAML comments and may reorder top-level keys
   * compared to `rawYaml`. Today's corpus is harvester-generated and
   * comment-free, so this has no effect yet.
   */
  const currentLawYaml = computed(() => {
    if (!rawYaml.value) return null;
    if (!selectedArticle.value) return rawYaml.value;
    const base = parsedRawLaw.value;
    if (!base) return rawYaml.value;
    try {
      // Shallow copies so the splice does not mutate the memoized skeleton.
      const doc = { ...base };
      const docArticles = Array.isArray(base.articles) ? [...base.articles] : null;
      if (!docArticles) return rawYaml.value;
      const idx = docArticles.findIndex((a) => String(a.number) === String(selectedNumber.value));
      if (idx < 0) return rawYaml.value;
      // idx >= 0 was checked above.
      const baseArticle = docArticles[idx]!;
      const textPristine = editedText.value === (baseArticle.text ?? '');
      const mrPristine = sameJson(baseArticle.machine_readable, machineReadable.value);
      // Untouched: return the saved text as is. A dump round trip can
      // reformat it, and a save of a pristine article would then commit a
      // reformat-only change.
      if (textPristine && mrPristine) return rawYaml.value;
      const patched = { ...baseArticle };
      if (!textPristine) patched.text = editedText.value;
      if (machineReadable.value != null) {
        patched.machine_readable = machineReadable.value;
      } else if (baseArticle.machine_readable != null) {
        // The YAML pane was cleared: persist the deletion rather than carry
        // the original over from the spread above.
        delete patched.machine_readable;
      }
      docArticles[idx] = patched;
      doc.articles = docArticles;
      return yaml.dump(doc, DUMP_OPTIONS);
    } catch {
      return rawYaml.value;
    }
  });

  // --- Loading ---------------------------------------------------------------
  // One staleness token for every load: a later open supersedes an earlier
  // one, whichever caller started it.
  const claimOpen = useLatest();

  // Take a loaded law as the saved version and pick the article: the
  // requested one, else the first. Resets the edits.
  function adopt(entry: LawBody, pick: () => string | null): void {
    rawYaml.value = entry.rawYaml;
    law.value = entry.law;
    currentEtag.value = entry.etag ?? null;
    selectedNumber.value = pick();
    resetEdits();
  }

  const firstArticle = () => (articles.value[0] ? String(articles.value[0].number) : null);

  async function loadInitial(): Promise<void> {
    const isCurrent = claimOpen();
    // Snapshot before any await: a concurrent `open` moves `currentTrajectRef`.
    const loadTrajectRef = currentTrajectRef;
    try {
      loading.value = true;
      let entry: LawBody;
      if (initialDirectUrl) {
        // Not a corpus route: fetch inline and record the body under its `$id`.
        const res = await apiFetch(initialDirectUrl, lawFetchInit);
        if (!isCurrent()) return;
        const text = await res.text();
        // No staleness check between `.text()` and the record: the content is
        // fresh either way and the traject is snapshotted above.
        const resolvedId = parseLaw(text)?.$id || (lawParam as string);
        entry = corpus.forTraject(loadTrajectRef).recordBody(resolvedId, text, res.headers.get('ETag'));
      } else {
        // Fresh fetch (never the cache): opening a regelwerk to edit it needs
        // the current ETag.
        entry = await corpus.forTraject(loadTrajectRef).law(lawParam as string, { fresh: true });
      }
      if (!isCurrent()) return;
      // The URL's article when the law has it, else the first.
      adopt(entry, () =>
        initialArticle && articles.value.some((a) => String(a.number) === initialArticle)
          ? initialArticle
          : firstArticle() ?? selectedNumber.value,
      );
    } catch (e) {
      if (!isCurrent()) return;
      error.value = e as RequestError;
    } finally {
      if (isCurrent()) loading.value = false;
    }
  }

  // With no law there is nothing to fetch: settle `loading` right away so the
  // editor shows its empty state.
  if (lawParam) loadInitial();
  else loading.value = false;

  /**
   * Open a law (and article), optionally in another traject. A later `open`
   * supersedes this one; then this resolves to `false` without touching any
   * state, which the winning call owns. Otherwise it resolves to `true`, also
   * when the load failed (`error` is set and the previous law stays).
   */
  async function open(target: OpenTarget): Promise<boolean> {
    const isCurrent = claimOpen();
    const startedAt = Date.now();
    let failed = false;
    try {
      loading.value = true;
      error.value = null;
      saveError.value = null;
      saving.value = false;
      if (target.trajectRef !== undefined) {
        currentTrajectRef = target.trajectRef || null;
        lawTrajectRef.value = currentTrajectRef;
      }
      const entry = await corpus.forTraject(currentTrajectRef).law(target.lawId);
      if (!isCurrent()) return false;
      lawParam = target.lawId;
      // A requested article is taken as asked, also when the law lacks it:
      // the editor then says the article is not there.
      adopt(entry, () =>
        target.articleNumber ? String(target.articleNumber) : firstArticle() ?? selectedNumber.value,
      );
    } catch (e) {
      if (!isCurrent()) return false;
      failed = true;
      error.value = e as RequestError;
    } finally {
      if (isCurrent()) {
        // On a failed retry, hold the spinner briefly so the click reads as
        // feedback instead of the error dialog snapping straight back.
        await holdRetryFloor({ startedAt, minMs: target.minLoadingMs ?? 0, failed });
        if (isCurrent()) loading.value = false;
      }
    }
    return true;
  }

  /** Select another article of the open law. Its unsaved edits are dropped. */
  function selectArticle(articleNumber: string | number): void {
    const next = String(articleNumber);
    if (next === selectedNumber.value) return;
    selectedNumber.value = next;
    resetEdits();
  }

  /**
   * Back to "nothing open": used when a traject switch or closing the last tab
   * lands on the editor root. Supersedes any load in flight, so a late
   * response cannot repopulate it.
   */
  function clear(): void {
    claimOpen();
    lawParam = null;
    law.value = null;
    rawYaml.value = '';
    selectedNumber.value = null;
    currentEtag.value = null;
    error.value = null;
    loading.value = false;
    resetEdits();
  }

  /**
   * Fetch the open law again from the server, past the cache. Needed after
   * processing an enrichment: the client sent verdicts, not a law, and the
   * server merged the accepted articles into a version the client has never
   * seen. Does nothing when the user moved to another law or traject during
   * the fetch.
   *
   * @returns whether this call updated the state.
   */
  async function reload(): Promise<boolean> {
    if (!lawId.value) return false;
    const reloadedLawId = lawId.value;
    const reloadedTrajectRef = currentTrajectRef;
    const entry = await corpus.forTraject(reloadedTrajectRef).law(reloadedLawId, { fresh: true });
    if (lawId.value !== reloadedLawId || currentTrajectRef !== reloadedTrajectRef) return false;
    rawYaml.value = entry.rawYaml;
    law.value = entry.law;
    currentEtag.value = entry.etag ?? null;
    resetEdits();
    return true;
  }

  /**
   * Show a law from YAML without fetching it: a `law_create` proposal, whose
   * law does not exist in the traject yet.
   */
  function seedFromYaml(yamlText: string): boolean {
    let parsed: LawDocument | null;
    try {
      parsed = parseLaw(yamlText);
    } catch {
      return false; // malformed proposal - keep the current (error) state
    }
    law.value = parsed;
    rawYaml.value = yamlText;
    currentEtag.value = null;
    error.value = null;
    loading.value = false;
    selectedNumber.value = firstArticle() ?? selectedNumber.value;
    resetEdits();
    return true;
  }

  /**
   * Put a review proposal for one article in the edits, as unsaved changes on
   * top of the saved article. Selects that article first. The proposal is
   * the guard's baseline: until someone edits it, leaving loses nothing, since
   * the review seeds it again on return.
   */
  function seedProposal(article: Pick<LawArticle, 'number' | 'text' | 'machine_readable'>): void {
    selectArticle(article.number);
    setBuffer(article);
    seedBaseline.value = { text: editedText.value, machineReadable: machineReadable.value ? copy(machineReadable.value) : null };
  }

  // --- Editing ---------------------------------------------------------------

  /** The YAML pane's text changed. Valid YAML replaces the structure. */
  function setYamlText(text: string): void {
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

  /** A save from the Machine pane or its edit sheet. */
  function saveItem(payload: MachineReadableSave): void {
    const mr = machineReadable.value ? copy(machineReadable.value) : {};
    applySave(mr, payload);
    setMachineReadable(mr);
  }

  /** A delete from the Machine pane. Does nothing without a machine_readable. */
  function deleteItem(payload: MachineReadableDelete): void {
    if (!machineReadable.value) return;
    const mr = copy(machineReadable.value);
    applyDelete(mr, payload);
    setMachineReadable(mr);
  }

  /** Start a machine_readable for an article without one. */
  function initMachineReadable(): void {
    setMachineReadable(emptyMachineReadable());
  }

  /** Throw away every edit to the selected article. */
  function discard(): void {
    resetEdits();
  }

  // --- The action sheet --------------------------------------------------------
  // The sheet and its operation editor change the action in place. They get a
  // copy, so the edits only reach the article on commit and a cancel needs no
  // snapshot.

  const activeAction = computed(() => actionEdit.value?.draft ?? null);
  const activeActionPath = computed(() => actionEdit.value?.path ?? null);
  const activeActionIsNew = computed(() => !!actionEdit.value && actionEdit.value.index === null);

  function startActionEdit(index: number | null, action: ExecutionAction, path: Path | null): void {
    const draft = copy(action);
    actionEdit.value = { index, draft, original: JSON.stringify(draft), path };
    parseError.value = null;
  }

  /**
   * Edit an action of the article in the sheet. `action` is the object the
   * pane shows; a stale one that is no longer in the article opens nothing.
   */
  function beginActionEdit(action: ExecutionAction, path: Path | null = null): void {
    const actions = machineReadable.value?.execution?.actions ?? [];
    const index = actions.findIndex((a) => a === action || sameJson(a, action));
    if (index < 0) return;
    startActionEdit(index, action, Array.isArray(path) ? path : null);
  }

  /** Edit a new action in the sheet; it joins the article on commit. */
  function beginNewAction(): void {
    startActionEdit(null, newActionStub(), null);
  }

  /** Close the sheet without keeping its edits. */
  function cancelActionEdit(): void {
    actionEdit.value = null;
    parseError.value = null;
  }

  /**
   * Write the sheet's action into the article. Returns why it cannot, in
   * Dutch for the author (also set as `parseError`), or null once written.
   */
  function commitActionEdit(): string | null {
    const edit = actionEdit.value;
    if (!edit) return null;
    const { draft } = edit;
    let problem: string | null = null;
    // The schema requires an output name, and the engine cannot load an
    // action without one.
    if (draft.output == null || String(draft.output).trim() === '') {
      problem = 'Output mag niet leeg zijn';
    } else {
      // A literal empty-string value passes here: the schema validator on
      // save checks types, and "set the output now, fill the value in the
      // YAML pane later" is a workflow people use.
      const incomplete = findIncompleteOperation(draft.value);
      if (incomplete) problem = `Operatie '${incomplete}' is nog niet ingevuld`;
    }
    if (problem) {
      parseError.value = problem;
      return problem;
    }
    const mr = machineReadable.value ? copy(machineReadable.value) : {};
    mr.execution ??= {};
    mr.execution.actions ??= [];
    if (edit.index === null) mr.execution.actions.push(copy(draft));
    else mr.execution.actions[edit.index] = copy(draft);
    setMachineReadable(mr);
    actionEdit.value = null;
    return null;
  }

  // --- Saving ----------------------------------------------------------------

  async function putLaw(yamlText: string, savedLawId: string, savedTrajectRef: string): Promise<void> {
    const headers: Record<string, string> = { 'Content-Type': 'text/yaml; charset=utf-8' };
    // Optimistic concurrency: echo the ETag of the version we loaded so a
    // concurrent edit by another member answers 412 instead of being
    // overwritten. Without one (direct URLs) the save is a blind write.
    if (currentEtag.value) headers['If-Match'] = currentEtag.value;
    const res = await apiFetch(lawUrl(savedTrajectRef, savedLawId), {
      method: 'PUT',
      headers,
      body: yamlText,
      // 412 resolves (handled as a conflict below); other failures throw.
      allowStatuses: [412],
      errorMessage: saveErrorMessage,
    });
    if (res.status === 412) {
      throw new Error(
        'Het regelwerk is intussen door iemand anders gewijzigd. ' +
        'Herlaad de pagina om de laatste wijzigingen te zien en voer je ' +
        'wijziging daarna opnieuw door.',
      );
    }
    let json: SaveResponse = null;
    try {
      json = await res.json();
      lastSavedPr.value = sanitizeSavedPr(json?.pr);
    } catch {
      // Older deployments return a bare 200 without JSON: keep the PR we had.
    }
    // Chain the new ETag for the next save; the header is authoritative.
    const newEtag = res.headers.get('ETag') ?? json?.etag ?? null;
    // Recorded for the traject the save went to, even if the user has moved on.
    const saved = corpus.forTraject(savedTrajectRef).recordBody(savedLawId, yamlText, newEtag);
    if (lawId.value === savedLawId && currentTrajectRef === savedTrajectRef) {
      rawYaml.value = yamlText;
      law.value = saved.law;
      currentEtag.value = newEtag;
    }
  }

  async function postLaw(yamlText: string, savedTrajectRef: string): Promise<void> {
    const res = await apiFetch(lawCreateUrl(savedTrajectRef), {
      method: 'POST',
      headers: { 'Content-Type': 'text/yaml; charset=utf-8' },
      body: yamlText,
      errorMessage: saveErrorMessage,
    });
    let json: SaveResponse = null;
    try {
      json = await res.json();
      lastSavedPr.value = sanitizeSavedPr(json?.pr);
    } catch {
      // Bare 200 without JSON - treat as successful.
    }
    const newEtag = res.headers.get('ETag') ?? json?.etag ?? null;
    const parsed = parseLaw(yamlText);
    rawYaml.value = yamlText;
    law.value = parsed;
    currentEtag.value = newEtag;
    if (parsed?.$id) corpus.forTraject(savedTrajectRef).recordBody(parsed.$id, yamlText, newEtag);
  }

  /**
   * Save the open law. Without options it writes the law with the selected
   * article's edits (`currentLawYaml`). `content` writes that whole-law YAML
   * instead (approving a review proposal), and `create` creates the law
   * (a `law_create` proposal, which has nothing to be concurrent with yet).
   *
   * On success the edits reset to the saved article. A failure keeps them
   * and sets `saveError`.
   */
  async function save(options: { content?: string; create?: boolean } = {}): Promise<SaveOutcome> {
    const content = options.content ?? currentLawYaml.value;
    const wholeLaw = options.content !== undefined;
    // A whole-law save may change text anywhere, so it counts as touching both.
    const touchedText = wholeLaw || isArticleTextDirty.value;
    if (!content) return { status: 'failed', touchedText };
    lastSaveTouchedText.value = touchedText;
    lastSaveTouchedMachine.value = wholeLaw || isMachineReadableDirty.value;
    const savedTrajectRef = currentTrajectRef;
    const savedLawId = lawId.value;
    const isSameLaw = () => lawId.value === savedLawId && currentTrajectRef === savedTrajectRef;
    saving.value = true;
    saveError.value = null;
    try {
      if (!savedTrajectRef) throw new Error('Cannot save law: no active traject');
      if (options.create) {
        await postLaw(content, savedTrajectRef);
      } else {
        if (!savedLawId) throw new Error('Cannot save law: no lawId');
        await putLaw(content, savedLawId, savedTrajectRef);
      }
    } catch (e) {
      if (isSameLaw()) saveError.value = e as RequestError;
      return { status: 'failed', touchedText };
    } finally {
      if (isSameLaw()) saving.value = false;
    }
    // The user switched laws while the request was out: their new edits stay.
    if (!isSameLaw()) return { status: 'switched', touchedText };
    // Reset now, in the same tick, so the dirty flags clear with the save and
    // a second click cannot save again.
    resetEdits();
    lastSaveTouchedText.value = false;
    lastSaveTouchedMachine.value = false;
    return { status: 'saved', touchedText };
  }

  return {
    // The saved version
    law,
    lawId,
    rawYaml,
    articles,
    lawName,
    lawTrajectRef,
    selectedArticle,
    selectedArticleNumber,
    loading,
    error,
    saving,
    saveError,
    currentEtag,
    lastSavedPr,
    // The edits
    editedText,
    machineReadable,
    yamlSource,
    parseError,
    editedArticle,
    hasMachineReadable,
    currentLawYaml,
    isArticleTextDirty,
    isMachineReadableDirty,
    articleDirty,
    hasUnsavedWork,
    lastSaveTouchedText,
    lastSaveTouchedMachine,
    // The action sheet
    activeAction,
    activeActionPath,
    activeActionIsNew,
    // Operations
    open,
    selectArticle,
    clear,
    reload,
    seedFromYaml,
    seedProposal,
    setYamlText,
    saveItem,
    deleteItem,
    initMachineReadable,
    discard,
    beginActionEdit,
    beginNewAction,
    cancelActionEdit,
    commitActionEdit,
    save,
  };
}

export type OpenLaw = ReturnType<typeof useOpenLaw>;
