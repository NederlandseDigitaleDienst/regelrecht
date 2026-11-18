/**
 * useDocumentsManager - the full werkdocumenten editing logic (list + active
 * document + title/save/delete/conflict/draft handling), lifted out of the old
 * TrajectDocuments window so it can back BOTH the in-sheet master-detail and
 * the standalone navigation-split-view page.
 *
 * State is per-call (one instance per consumer tree). The sheet owns one
 * instance shared by its list + editor; the standalone page (a separate
 * browser tab) owns its own. Anything purely presentational (which view is
 * shown, sheet open/close) stays in the components.
 */
import { computed, ref, watch } from 'vue';
import { useTrajectDocuments } from './useTrajectDocuments';
import type { TrajectRefSource } from './useTrajectDocuments';

/** An input event from an nldd field (`detail.value`) or a native input (`target.value`). */
function inputValue(e: Event): string | undefined {
  return (e as CustomEvent<{ value?: string } | null>).detail?.value
    ?? (e.target as HTMLInputElement | null)?.value;
}

export function useDocumentsManager(
  trajectRef: TrajectRefSource,
  reservedPaths: () => readonly string[] = () => [],
) {
  const docs = useTrajectDocuments(trajectRef);
  const {
    documents,
    loading: listLoading,
    listError,
    currentPath,
    currentBody,
    savedBody,
    currentEtag,
    docLoading,
    docError,
    saving,
    saveError,
    conflict,
    deletedRemotely,
    fetchList,
    openDocument,
    saveCurrent,
    reloadCurrent,
    createDocument,
    uploadDocument,
    deleteDocument,
    dropDraft,
  } = docs;

  // The body flows straight to currentBody; the nldd-text-editor is a hybrid
  // Markdown editor (live-styled source), so there is no separate preview mode.
  function onBodyInput(e: Event): void {
    currentBody.value = inputValue(e) ?? currentBody.value;
  }

  // Unsaved changes: the live body differs from the last-saved baseline. Drives
  // the editor's Save button + footer toolbar (shown only when dirty) and the
  // navigate-away guard. A document that is still loading, or that failed to
  // load, can never be dirty. The exception is a 'draft-present' docError: it
  // means a local draft diverges from the server, which IS unsaved changes, so
  // it must stay dirty — mirroring DocumentEditor.blockingError, which likewise
  // does not block on draft-present (otherwise the Save button the draft notice
  // tells the user to click is hidden). While loading, currentPath is already
  // set (so the shell shows) but currentBody still holds the previous document,
  // which would otherwise register as a change and flash the Save button.
  const hasChanges = computed(
    () =>
      !docLoading.value
      && !(docError.value && docError.value.kind !== 'draft-present')
      && currentBody.value !== savedBody.value,
  );

  // --- Titels ---
  // '.md' blijft verborgen voor de gebruiker; '.txt' wijkt af van de default en
  // blijft daarom zichtbaar.
  function displayTitle(path: string | null | undefined): string {
    return path ? path.replace(/\.md$/, '') : '';
  }
  function pathFromTitle(title: string): string {
    const t = title.trim();
    if (!t) return '';
    return /\.(md|txt)$/.test(t) ? t : `${t}.md`;
  }
  // Lightweight client-side validation mirroring the backend rules.
  function validatePath(value: string | null): string | null {
    if (!value) return 'Geef een naam op.';
    if (value.startsWith('/')) return "Naam mag niet beginnen met '/'.";
    if (value.includes('\\')) return 'Naam mag geen backslashes bevatten.';
    const segments = value.split('/');
    for (const seg of segments) {
      if (!seg) return 'Naam bevat lege segmenten.';
      if (seg.startsWith('.')) return "Naam mag geen verborgen segmenten ('.') bevatten.";
      if (!/^[a-z0-9._-]+$/.test(seg)) {
        return "Gebruik alleen kleine letters, cijfers en '._-'.";
      }
    }
    return null;
  }

  // --- Open / nieuw ---
  async function open(path: string): Promise<void> {
    await openDocument(path);
    reconcileAutoBaseline();
  }

  const creating = ref(false);
  // Names that are taken but not (yet) in `documents` - e.g. an uploaded
  // document still converting, whose .md only appears once the job finishes.
  // `reservedPaths()` supplies them, so a create/rename can't collide with an
  // in-flight upload of the same name.
  function isTaken(p: string): boolean {
    return documents.value.some((d) => d.path === p) || reservedPaths().includes(p);
  }
  // First free `${stem}[-N].md`, skipping `selfPath` so a document can keep its
  // own name. Backs both new untitled docs and auto-naming.
  function nextFreePath(stem: string, selfPath: string | null = null): string {
    const taken = (p: string) => p !== selfPath && isTaken(p);
    let path = `${stem}.md`;
    for (let n = 2; taken(path); n++) path = `${stem}-${n}.md`;
    return path;
  }
  function nextUntitledPath(): string {
    return nextFreePath('untitled');
  }
  // Derive a filename stem from the first non-empty line (Apple Notes-style),
  // sanitised to the backend's [a-z0-9._-] and capped at 50. Empty when the line
  // yields nothing usable.
  function deriveNameFromBody(body: string | null | undefined): string {
    const firstLine = (body || '').split('\n').find((l) => l.trim() !== '') ?? '';
    return firstLine
      .replace(/^#+\s*/, '')
      .toLowerCase()
      .replace(/[^a-z0-9._-]+/g, '-')
      .replace(/-+/g, '-')
      .replace(/^[-.]+|[-.]+$/g, '')
      .slice(0, 50)
      .replace(/[-.]+$/g, '');
  }
  // Auto-naming from the first line stays active while the name is still the one
  // we last set automatically. `autoBaseline` holds that path; null = the user
  // renamed it, so leave it alone. Derived from the saved content rather than
  // remembered, so the auto/manual state survives a refresh without being stored
  // anywhere (we only ever name on save).
  //
  // The `untitled` test is what makes a brand-new document auto-name at all: it
  // has no body yet, so `derived` is empty and the name comparison can't match.
  // The price is that it reads intent off the name, and the name doesn't carry
  // any: a document the user deliberately renames to `untitled` re-links to
  // auto-naming, and its next save renames it after its first line. Deliberately
  // not closed - that needs the auto/manual state persisted (server-side or
  // localStorage) to protect the one name our own generator hands out.
  const autoBaseline = ref<string | null>(null);
  function reconcileAutoBaseline(): void {
    const path = currentPath.value;
    if (!path) { autoBaseline.value = null; return; }
    const stem = displayTitle(path);
    const derived = deriveNameFromBody(savedBody.value);
    autoBaseline.value = /^untitled(-\d+)?$/.test(stem) || (derived && derived === stem) ? path : null;
  }
  // Creates an untitled document and loads it; the backend allows only
  // [a-z0-9._-] in paths. Returns the new path so the caller can route to it.
  async function startNew(): Promise<string | null> {
    if (creating.value) return null;
    creating.value = true;
    const path = nextUntitledPath();
    try {
      await createDocument(path);
      reconcileAutoBaseline();
      return path;
    } finally {
      creating.value = false;
    }
  }

  // Close the open document without deleting it - clears the editor back to
  // "nothing selected". Used by the standalone page's back affordance (on a
  // narrow viewport the split view stacks, so "terug" returns to the list).
  function close(): void {
    autoBaseline.value = null;
    currentPath.value = null;
    currentBody.value = '';
    // Reset the saved baseline too — otherwise hasChanges stays true (empty
    // body vs the just-closed document's body) and the next navigation trips
    // the unsaved-changes guard spuriously.
    savedBody.value = '';
    currentEtag.value = null;
  }

  // --- Titel bewerken ---
  const titleDraft = ref('');
  const titleError = ref<string | null>(null);
  watch(currentPath, (p) => {
    titleDraft.value = displayTitle(p);
    titleError.value = null;
  });
  function onTitleInput(e: Event): void {
    const raw = inputValue(e) ?? titleDraft.value;
    // Sanitize to a valid path as the user types instead of rejecting invalid
    // input: lowercase everything and turn any space or other disallowed
    // character into '-'. '/' is kept as a folder separator. This keeps the name
    // always valid, so the "Gebruik alleen kleine letters…" error never appears.
    titleDraft.value = raw.toLowerCase().replace(/[^a-z0-9._/-]+/g, '-');
    // Editing the name clears any stale validation notice (e.g. a duplicate-name
    // error from a prior save attempt).
    titleError.value = null;
  }

  // Synchronous validation of the current rename input, so the rename sheet can
  // close optimistically on a valid name and stay open (with the error) on an
  // invalid/duplicate one, without awaiting the (slow) server round-trip.
  function validateRename(): boolean {
    titleError.value = null;
    const finalPath = pathFromTitle(titleDraft.value);
    const err = validatePath(finalPath);
    if (err) { titleError.value = err; return false; }
    if (finalPath !== currentPath.value && isTaken(finalPath)) {
      titleError.value = 'Een document met deze naam bestaat al.';
      return false;
    }
    return true;
  }

  async function handleSave(): Promise<boolean> {
    // Without an open document there is nothing to save, and a rename would
    // end in a DELETE of the null path.
    if (saving.value || !currentPath.value) return false;
    titleError.value = null;

    // Auto-name (Apple Notes-style): while the name is still auto-managed and the
    // user hasn't touched the name field, derive it from the first line on every
    // save. Otherwise keep the user's typed name (and stop auto-managing).
    const autoManaged = autoBaseline.value !== null && autoBaseline.value === currentPath.value;
    const titleUntouched = titleDraft.value === displayTitle(currentPath.value);
    const wentAuto = autoManaged && titleUntouched;
    let finalPath: string | null;
    if (wentAuto) {
      const derived = deriveNameFromBody(currentBody.value);
      finalPath = derived ? nextFreePath(derived, currentPath.value) : currentPath.value;
    } else {
      finalPath = pathFromTitle(titleDraft.value);
    }

    const err = validatePath(finalPath);
    // validatePath rejects an empty path, so `!finalPath` adds no case; it only
    // tells the compiler finalPath is a string below.
    if (err || !finalPath) {
      titleError.value = err;
      return false;
    }
    if (finalPath === currentPath.value) {
      const result = await saveCurrent();
      if (result?.ok) reconcileAutoBaseline();
      return !!result?.ok;
    }
    // Hernoemen: geen rename-API - schrijf eerst onder het nieuwe pad (blind
    // create) en verwijder daarna het oude pad. In die volgorde raakt een
    // mislukking nooit inhoud kwijt.
    //
    // Multi-user-kanttekening: de bestaat-al-check hieronder kijkt naar de
    // gecachte lijst. Maakt een andere sessie tussen die check en de PUT een
    // bestand op finalPath, dan overschrijft deze blinde write het zonder
    // waarschuwing. Sluitend te maken met `If-None-Match: *` zodra de backend
    // dat ondersteunt (zie useTrajectDocuments.saveCurrent).
    if (isTaken(finalPath)) {
      titleError.value = 'Een document met deze naam bestaat al.';
      return false;
    }
    const oldPath = currentPath.value;
    const oldEtag = currentEtag.value;
    currentPath.value = finalPath;
    currentEtag.value = null;
    const result = await saveCurrent({ ifMatch: null });
    if (!result?.ok) {
      currentPath.value = oldPath;
      currentEtag.value = oldEtag;
      return false;
    }
    // The rename committed: re-derive the auto/manual state from the saved
    // content, so name==first-line re-links and a manual name locks.
    reconcileAutoBaseline();
    const deleted = await deleteDocument(oldPath);
    if (!deleted?.ok) {
      // The new name is saved, so content is never lost, but the old path
      // could not be removed and lingers on the server as an orphan copy.
      // Surface it so the user can delete it by hand instead of silently
      // leaving a duplicate behind.
      //
      // Via deleteNotice, not titleError: this arrives AFTER the save
      // committed, and every caller has closed the rename sheet by then (or
      // never opened it - auto-naming renames from the top-bar Save). A field
      // error under a form nobody is looking at is no warning at all. Name the
      // orphan, too: the document now lives under a different name, so "verwijder
      // het handmatig" without saying which file is not actionable.
      deleteNotice.value =
        `Hernoemd en opgeslagen, maar het oude bestand "${displayTitle(oldPath)}" kon niet worden verwijderd. Verwijder het handmatig.`;
    }
    return true;
  }

  // "Maak alle wijzigingen ongedaan": gooi de lokale draft weg, laad de server-versie.
  function undoChanges(): Promise<void> {
    titleDraft.value = displayTitle(currentPath.value);
    titleError.value = null;
    return reloadCurrent();
  }
  // Resolve a 412 conflict by force-overwriting with `If-Match: *`.
  function overwriteServer() {
    return saveCurrent({ ifMatch: '*' });
  }

  // --- Delete ---
  const pendingDeletePath = ref<string | null>(null);
  const deleteNotice = ref<string | null>(null);
  function askDelete(path: string | null | undefined): void {
    if (path) pendingDeletePath.value = path;
  }
  function cancelDelete(): void {
    if (pendingDeletePath.value === null) return;
    pendingDeletePath.value = null;
  }
  // Resolves to `true` when the currently open document was the one deleted, so
  // the caller can return to the list / navigate away.
  async function confirmDelete(): Promise<boolean> {
    const path = pendingDeletePath.value;
    pendingDeletePath.value = null;
    if (!path) return false;
    deleteNotice.value = null;
    const wasOpenDocument = path === currentPath.value;
    const result = await deleteDocument(path);
    if (result?.ok) {
      return wasOpenDocument;
    }
    if (result?.conflict) {
      deleteNotice.value =
        `"${displayTitle(path)}" is intussen door iemand anders gewijzigd; de lijst is ververst. ` +
        `Open het document om de huidige versie te zien voordat je het verwijdert.`;
    } else {
      deleteNotice.value =
        saveError.value?.message || `Verwijderen van "${displayTitle(path)}" is mislukt.`;
    }
    return false;
  }

  return {
    // state
    documents, listLoading, listError,
    currentPath, currentBody, hasChanges, docLoading, docError,
    saving, saveError, conflict, deletedRemotely,
    creating,
    titleDraft, titleError,
    pendingDeletePath, deleteNotice,
    // derived helpers
    displayTitle,
    // actions
    open, startNew, close, uploadDocument, refreshList: fetchList,
    onBodyInput, onTitleInput,
    validateRename, handleSave, undoChanges, overwriteServer,
    reloadCurrent, dropDraft,
    askDelete, cancelDelete, confirmDelete,
  };
}
