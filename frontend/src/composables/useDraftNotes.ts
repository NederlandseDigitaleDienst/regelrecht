/**
 * useDraftNotes - local-only note authoring store (RFC-018 write path, MVP).
 *
 * Notes a user creates in the editor live in `localStorage`, keyed per law,
 * until they are written back. Two ways out: `exportYaml` produces a YAML
 * document for a manual commit (the original RFC-018 §10 MVP, still here for
 * the offline case), and `saveToRepo` PUTs that same document to editor-api,
 * which validates it and commits it to the traject branch - the same path
 * law and scenario edits use. No pull request is opened; the `pr` field in
 * the save response stays empty for traject saves.
 *
 * Draft notes are merged into the resolved-notes list by the caller so they
 * highlight live, exactly like committed notes; they just carry an extra
 * `__draft` marker so the UI can show them as unsaved and offer delete/export.
 */
import { ref, computed, watch } from 'vue';
import type { Ref } from 'vue';
import * as yaml from 'js-yaml';
import { lastSavedPr, sanitizeSavedPr } from './useSavedPr';
import { annotationsUrl, requireTraject } from './corpusUrls';
import { apiFetch, apiFetchText } from '../lib/apiFetch';
import type { Note } from './useNotes';
import type { SavedPr } from './useSavedPr';

/** What a save to the repo reports back to the caller. */
export interface NotesSaveResult {
  pr: SavedPr | null;
  noChange: boolean;
}

const STORAGE_PREFIX = 'regelrecht-draft-notes:';

function storageKey(lawId: string | null | undefined): string {
  return `${STORAGE_PREFIX}${lawId}`;
}

function loadFor(lawId: string | null | undefined): Note[] {
  if (!lawId) return [];
  try {
    const raw = localStorage.getItem(storageKey(lawId));
    const parsed: unknown = raw ? JSON.parse(raw) : [];
    return Array.isArray(parsed) ? parsed : [];
  } catch {
    return [];
  }
}

function persist(lawId: string | null | undefined, list: Note[]): void {
  try {
    localStorage.setItem(storageKey(lawId), JSON.stringify(list));
  } catch {
    /* quota/full or disabled - drafts are best-effort in the MVP */
  }
}

/**
 * @param {import('vue').Ref<string>} lawId reactive law $id
 * @param {import('vue').Ref<string|null>=} trajectRef active traject
 *   reference. Required for `saveToRepo`; drafts themselves live in
 *   localStorage and don't care about the scope.
 */
export function useDraftNotes(
  lawId: Readonly<Ref<string | null | undefined>>,
  trajectRef?: Readonly<Ref<string | null | undefined>>,
) {
  const drafts = ref<Note[]>(loadFor(lawId.value));

  // Re-read from storage whenever the law changes. A real watch (not a lazy
  // resync inside every method/computed) keeps `drafts` authoritative for the
  // current law at all times, so useResolvedDraftNotes - which watches lawId
  // independently - never resolves the previous law's selectors against the
  // new law between a law switch and the next store method call.
  watch(lawId, (id) => {
    drafts.value = loadFor(id);
  });

  /**
   * Append a note. `note` is a full W3C Annotation object (already validated
   * shape from NoteCreator). Returns the stored note (with its draft marker).
   */
  function addDraft(note: Note): Note {
    const stored: Note = {
      ...note,
      created: note.created || new Date().toISOString(),
    };
    drafts.value = [...drafts.value, stored];
    persist(lawId.value, drafts.value);
    return stored;
  }

  /** Remove the draft at `index` (index into the current drafts array). */
  function removeDraft(index: number): void {
    if (index < 0 || index >= drafts.value.length) return;
    drafts.value = drafts.value.filter((_, i) => i !== index);
    persist(lawId.value, drafts.value);
  }

  /**
   * Drop every draft for a law (after a successful save/commit).
   *
   * `lawIdOverride` lets a caller clear the *law it actually saved* rather
   * than `lawId.value`, which may have changed during the save's awaits. A
   * `watch(lawId)` resyncs `drafts` to the now-current law, so this also
   * persists the cleared list under the right key and only resets the
   * in-memory `drafts` ref when it still belongs to the saved law (clearing
   * it would otherwise wipe the freshly-switched law's drafts on screen).
   */
  function clearDrafts(lawIdOverride?: string | null): void {
    const id = lawIdOverride ?? lawId.value;
    persist(id, []);
    if (lawId.value === id) {
      drafts.value = [];
    }
  }

  /**
   * The full sidecar YAML for this law: every committed note plus the drafts.
   *
   * The committed notes are read from the *raw sidecar file*, not from the
   * resolver output. The WASM resolver drops notes whose target.source names a
   * different law (federated sidecars may carry notes for several laws - see
   * resolveNotes in wasm.rs); rebuilding the export from resolved notes would
   * silently truncate those on commit. Re-parsing the original file preserves
   * every committed note verbatim and only appends the drafts.
   *
   * Async because it fetches the sidecar; a missing sidecar (404) is normal
   * for a law that has no committed notes yet - the export is then just the
   * drafts.
   */
  async function exportYaml(lawIdOverride?: string | null): Promise<string> {
    // saveToRepo passes its own snapshot so the PUT URL and the exported
    // body provably share one law id even if the law switches in the
    // synchronous gap before this call. Default: read it here.
    const id = lawIdOverride ?? lawId.value;
    // Snapshot drafts BEFORE the await: watch(lawId) swaps drafts.value
    // synchronously on a law switch, so reading it after the fetch could mix
    // law A's committed notes with law B's drafts. Drafts are the only copy
    // of unsaved work - silent loss is the worst outcome here.
    const snapshotDrafts = [...drafts.value];
    let committed: Note[] = [];
    try {
      const text = await apiFetchText(
        `/data/annotations/${encodeURIComponent(String(id))}/annotations.yaml`,
      );
      const doc = yaml.load(text) as { annotations?: unknown } | null | undefined;
      if (Array.isArray(doc?.annotations)) committed = doc.annotations;
    } catch {
      // 404 (no sidecar yet), other HTTP errors, network and parse
      // failures all fall through to drafts-only - the author still gets
      // a valid file to commit, so no work is lost.
    }
    const annotations = [
      ...committed.map(stripDraftMarker),
      ...snapshotDrafts.map(stripDraftMarker),
    ];
    const doc = {
      $schema:
        'https://raw.githubusercontent.com/NederlandseDigitaleDienst/regelrecht/refs/heads/main/schema/v0.5.3/annotation-schema.json',
      annotations,
    };
    // lineWidth -1: never fold long body strings, so the YAML diff stays
    // readable and selectors are not silently wrapped.
    return yaml.dump(doc, { lineWidth: -1, noRefs: true });
  }

  /**
   * Serialise an explicit set of notes to the same sidecar YAML shape as
   * `exportYaml`, for a caller that already holds the notes it wants (e.g. the
   * article-scoped export, where EditorView passes the notes resolved to the
   * open article). Strips the `__draft` marker; committed and draft notes mix
   * freely.
   */
  function exportYamlFromNotes(notes: Note[]): string {
    const doc = {
      $schema:
        'https://raw.githubusercontent.com/NederlandseDigitaleDienst/regelrecht/refs/heads/main/schema/v0.5.3/annotation-schema.json',
      annotations: notes.map(stripDraftMarker),
    };
    return yaml.dump(doc, { lineWidth: -1, noRefs: true });
  }

  /**
   * Append the local drafts to the law's sidecar via editor-api. The save
   * is routed through the session's active traject (same model as law and
   * scenario edits since #632): the notes land in that traject's writable
   * branch, so a note and a law edit made in the same session land on the
   * same branch. No source is chosen here - the traject's own corpus config
   * decides the target. With no active traject the backend returns 403.
   *
   * The request body is **only the new drafts**, not the merged file: the
   * backend reads the current sidecar from the traject branch and appends.
   * That is why there is no `/data` fetch here - rebuilding the file
   * client-side from the stale static mirror was the blind-overwrite bug.
   *
   * On success the saved law's drafts are cleared (they are committed now)
   * and, if the save produced a PR, the shared `lastSavedPr` ref is
   * updated so EditorApp's badge shows it. Throws with the editor-api
   * message on failure; drafts are left untouched so no work is lost.
   */
  // Shared PUT: send W3C annotations to the traject sidecar and read back the
  // PR. Captures the law id synchronously (before the await) so a law switch
  // mid-request can't misroute the caller's follow-up draft cleanup.
  async function putNotesToRepo(
    notesToSend: Note[],
  ): Promise<NotesSaveResult & { id: string | null | undefined }> {
    const id = lawId.value;
    if (!id) throw new Error('Cannot perform notes save without an open law');
    const tr = trajectRef?.value ?? null;
    requireTraject(tr, 'notes save');
    const url = annotationsUrl(tr, id);
    const res = await apiFetch(url, {
      method: 'PUT',
      headers: {
        'Content-Type': 'application/json',
      },
      body: JSON.stringify(notesToSend),
      // Same content-type guard as useOpenLaw.save: only render the body
      // when it's editor-api's own text/plain error, never a proxy's HTML.
      errorMessage: (status: number, body: string, contentType: string) =>
        contentType.startsWith('text/plain') && body
          ? body
          : `Opslaan mislukt: ${status}`,
    });
    let pr: SavedPr | null = null;
    let noChange = false;
    try {
      const json = (await res.json()) as { pr?: unknown; no_change?: unknown } | null;
      pr = sanitizeSavedPr(json?.pr);
      // Backend sets no_change when every submitted note was already
      // committed and it skipped the write/commit/PR entirely.
      noChange = json?.no_change === true;
    } catch {
      // No JSON body - treat as success, pr stays null.
    }
    // Only OVERWRITE the badge when this save produced a PR. A PR-less
    // 200 (local source, or a NoChange re-save) must NOT null an existing
    // badge - same "keep any prior PR" rule as useOpenLaw.save.
    if (pr) {
      lastSavedPr.value = pr;
    }
    return { id, pr, noChange };
  }

  async function saveToRepo(): Promise<NotesSaveResult> {
    // Snapshot drafts BEFORE the await (watch(lawId) swaps drafts.value on a
    // law switch). Strip the internal __draft marker; the backend gets clean
    // W3C Annotation objects.
    const newNotes = drafts.value.map(stripDraftMarker);
    const { id, pr, noChange } = await putNotesToRepo(newNotes);
    // Clear the law we saved, not lawId.value, which may have switched.
    clearDrafts(id);
    // Caller shows an explicit "opgeslagen (PR #N)" vs "waren al opgeslagen".
    return { pr, noChange };
  }

  // Per-note "publiek maken": publish a single draft to the repo. On success
  // it's committed, so drop just that draft - not the whole set.
  async function publishNote(note: Note): Promise<NotesSaveResult> {
    const { pr, noChange } = await putNotesToRepo([stripDraftMarker(note)]);
    const i = drafts.value.indexOf(note);
    if (i >= 0) removeDraft(i);
    return { pr, noChange };
  }

  const draftCount = computed(() => drafts.value.length);

  return {
    drafts,
    draftCount,
    addDraft,
    removeDraft,
    exportYamlFromNotes,
    clearDrafts,
    exportYaml,
    saveToRepo,
    publishNote,
  };
}

function stripDraftMarker(note: Note): Note {
  if (note && typeof note === 'object' && '__draft' in note) {
    const { __draft, ...rest } = note;
    return rest;
  }
  return note;
}
