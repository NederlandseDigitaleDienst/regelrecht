import { describe, it, expect, beforeEach, vi } from 'vitest';
import { ref, nextTick } from 'vue';

// The manager orchestrates the lower-level useTrajectDocuments (which does the
// real network I/O). Mock that layer with controllable refs + spies so these
// tests exercise only the manager's own logic: rename-as-create+delete,
// delete-returns-open-flag, close, and untitled-name generation.
// Set in beforeEach before any test reads it.
const h = vi.hoisted(() => ({ api: null as unknown as DocsApi }));
vi.mock('./useTrajectDocuments', () => ({
  useTrajectDocuments: () => h.api,
}));

import { useDocumentsManager } from './useDocumentsManager';
import type {
  DeleteResult,
  DocError,
  DocumentEntry,
  SaveResult,
} from './useTrajectDocuments';

type DocsApi = ReturnType<typeof makeApi>;

// The manager's input handlers read `target.value` or `detail.value`; cast once here.
function inputEvent(init: { target?: { value: string }; detail?: { value: string } }): Event {
  return init as unknown as Event;
}

function makeApi(overrides = {}) {
  const documents = ref<DocumentEntry[]>([]);
  const currentPath = ref<string | null>(null);
  const currentBody = ref('');
  const savedBody = ref('');
  const currentEtag = ref<string | null>(null);
  return {
    documents,
    loading: ref(false),
    listError: ref<unknown>(null),
    currentPath,
    currentBody,
    savedBody,
    currentEtag,
    docLoading: ref(false),
    docError: ref<DocError | null>(null),
    saving: ref(false),
    saveError: ref<Error | null>(null),
    conflict: ref<string | null>(null),
    deletedRemotely: ref<string | null>(null),
    openDocument: vi.fn(async (p: string) => {
      currentPath.value = p;
    }),
    saveCurrent: vi.fn<(opts?: { ifMatch?: string | null }) => Promise<SaveResult>>(async () => {
      savedBody.value = currentBody.value; // mirror the real save: baseline follows content
      return { ok: true };
    }),
    reloadCurrent: vi.fn(async () => {}),
    createDocument: vi.fn(async (p: string): Promise<SaveResult> => {
      currentPath.value = p;
      return { ok: true, created: true };
    }),
    deleteDocument: vi.fn<(path: string) => Promise<DeleteResult>>(async () => ({ ok: true })),
    dropDraft: vi.fn(),
    ...overrides,
  };
}

let m: ReturnType<typeof useDocumentsManager>;
beforeEach(() => {
  h.api = makeApi();
  m = useDocumentsManager(ref('traject-abc12345'));
});

describe('useDocumentsManager', () => {
  it('displayTitle hides .md but keeps .txt visible', () => {
    expect(m.displayTitle('beleid.md')).toBe('beleid');
    expect(m.displayTitle('nota/bijlage.txt')).toBe('nota/bijlage.txt');
    expect(m.displayTitle(null)).toBe('');
  });

  it('onTitleInput sanitizes the name to a valid path (no error shown)', () => {
    m.onTitleInput(inputEvent({ target: { value: 'Beleid Nota' } }));
    expect(m.titleDraft.value).toBe('beleid-nota'); // uppercase -> lower, space -> '-'
    m.onTitleInput(inputEvent({ target: { value: 'ABC_123.def' } }));
    expect(m.titleDraft.value).toBe('abc_123.def'); // letters/digits/._ kept
    m.onTitleInput(inputEvent({ target: { value: 'map/Sub Doc' } }));
    expect(m.titleDraft.value).toBe('map/sub-doc'); // '/' kept as folder separator
    m.onTitleInput(inputEvent({ detail: { value: 'Rare!!Tekens@hier' } }));
    expect(m.titleDraft.value).toBe('rare-tekens-hier'); // runs of invalid chars -> single '-'
    expect(m.titleError.value).toBe(null);
  });

  it('typing the name clears a stale title error', () => {
    m.titleError.value = 'Een document met deze naam bestaat al.';
    m.onTitleInput(inputEvent({ target: { value: 'nieuwe-naam' } }));
    expect(m.titleError.value).toBe(null);
  });

  it('startNew creates the next free untitled path and returns it', async () => {
    h.api.documents.value = [{ path: 'untitled.md' }, { path: 'untitled-2.md' }];
    const path = await m.startNew();
    expect(path).toBe('untitled-3.md');
    expect(h.api.createDocument).toHaveBeenCalledWith('untitled-3.md');
  });

  it('startNew skips reserved names (e.g. a converting upload not yet in the list)', async () => {
    h.api.documents.value = [{ path: 'untitled.md' }];
    // untitled-2.md is still converting, so it is not in `documents` yet.
    const mr = useDocumentsManager(ref('traject-abc12345'), () => ['untitled-2.md']);
    const path = await mr.startNew();
    expect(path).toBe('untitled-3.md');
    expect(h.api.createDocument).toHaveBeenCalledWith('untitled-3.md');
  });

  it('handleSave rejects a rename to a name that is still converting', async () => {
    h.api.documents.value = [{ path: 'beleid.md' }];
    h.api.currentPath.value = 'beleid.md';
    const mr = useDocumentsManager(ref('traject-abc12345'), () => ['untitled-2.md']);
    mr.titleDraft.value = 'untitled-2';
    const ok = await mr.handleSave();
    expect(ok).toBe(false);
    expect(mr.titleError.value).toBe('Een document met deze naam bestaat al.');
    expect(h.api.saveCurrent).not.toHaveBeenCalled();
  });

  it('handleSave without a rename just saves the current path', async () => {
    h.api.currentPath.value = 'beleid.md';
    await nextTick(); // watch initialises titleDraft from the path
    const ok = await m.handleSave();
    expect(ok).toBe(true);
    expect(h.api.saveCurrent).toHaveBeenCalledTimes(1);
    expect(h.api.deleteDocument).not.toHaveBeenCalled();
  });

  it('handleSave does nothing when no document is open', async () => {
    // A name typed while nothing is open must not create a document and then
    // DELETE the null path.
    m.titleDraft.value = 'nieuw';
    const ok = await m.handleSave();
    expect(ok).toBe(false);
    expect(h.api.saveCurrent).not.toHaveBeenCalled();
    expect(h.api.deleteDocument).not.toHaveBeenCalled();
  });

  it('handleSave with a rename writes the new path then deletes the old one', async () => {
    h.api.currentPath.value = 'oud.md';
    await nextTick();
    m.titleDraft.value = 'nieuw';
    const ok = await m.handleSave();
    expect(ok).toBe(true);
    // New path adopted before the blind create (ifMatch: null).
    expect(h.api.currentPath.value).toBe('nieuw.md');
    expect(h.api.saveCurrent).toHaveBeenCalledWith({ ifMatch: null });
    expect(h.api.deleteDocument).toHaveBeenCalledWith('oud.md');
  });

  it('handleSave keeps the rename but flags the orphan when the old delete fails', async () => {
    h.api.deleteDocument.mockResolvedValueOnce({ ok: false });
    h.api.currentPath.value = 'oud.md';
    await nextTick();
    m.titleDraft.value = 'nieuw';
    const ok = await m.handleSave();
    // The rename itself succeeded - content is saved under the new name.
    expect(ok).toBe(true);
    expect(h.api.currentPath.value).toBe('nieuw.md');
    expect(h.api.deleteDocument).toHaveBeenCalledWith('oud.md');
    // ...but the old file could not be removed, so the orphan is surfaced -
    // through deleteNotice, which has a modal of its own. titleError renders
    // only inside the rename sheet, and this fires after the save committed,
    // by which point every caller has closed it (or never opened it).
    expect(m.deleteNotice.value).toMatch(/oude bestand "oud" kon niet worden verwijderd/i);
    expect(m.titleError.value).toBeNull();
  });

  it('handleSave refuses a rename onto an existing document', async () => {
    h.api.documents.value = [{ path: 'bezet.md' }];
    h.api.currentPath.value = 'oud.md';
    await nextTick();
    m.titleDraft.value = 'bezet';
    const ok = await m.handleSave();
    expect(ok).toBe(false);
    expect(m.titleError.value).toMatch(/bestaat al/i);
    expect(h.api.saveCurrent).not.toHaveBeenCalled();
    expect(h.api.currentPath.value).toBe('oud.md'); // unchanged
  });

  it('handleSave rejects an invalid name without saving', async () => {
    h.api.currentPath.value = 'oud.md';
    await nextTick();
    m.titleDraft.value = 'Bad Name'; // spaces + capitals are not allowed
    const ok = await m.handleSave();
    expect(ok).toBe(false);
    expect(m.titleError.value).toBeTruthy();
    expect(h.api.saveCurrent).not.toHaveBeenCalled();
  });

  it('validateRename accepts a valid name and rejects invalid/duplicate ones', async () => {
    h.api.documents.value = [{ path: 'bestaat.md' }];
    h.api.currentPath.value = 'oud.md';
    await nextTick();
    m.titleDraft.value = 'nieuw';
    expect(m.validateRename()).toBe(true);
    expect(m.titleError.value).toBe(null);
    m.titleDraft.value = 'bestaat';
    expect(m.validateRename()).toBe(false);
    expect(m.titleError.value).toBe('Een document met deze naam bestaat al.');
    m.titleDraft.value = 'Bad Name';
    expect(m.validateRename()).toBe(false);
    expect(m.titleError.value).toBeTruthy();
  });

  // --- Auto-name from the first line (Apple Notes-style) ---
  it('auto-names an untitled document from its first line on save', async () => {
    h.api.documents.value = [];
    await m.startNew(); // creates untitled.md, marks it auto-managed
    await nextTick(); // titleDraft <- 'untitled'
    h.api.currentBody.value = '# Boodschappen\nmelk';
    const ok = await m.handleSave();
    expect(ok).toBe(true);
    expect(h.api.currentPath.value).toBe('boodschappen.md');
    expect(h.api.deleteDocument).toHaveBeenCalledWith('untitled.md');
  });

  it('re-derives the name on later saves while still auto-managed', async () => {
    h.api.documents.value = [];
    await m.startNew();
    await nextTick();
    h.api.currentBody.value = '# Boodschappen';
    await m.handleSave(); // -> boodschappen.md
    await nextTick(); // titleDraft <- 'boodschappen'
    h.api.currentBody.value = '# Weekmenu';
    await m.handleSave(); // -> weekmenu.md
    expect(h.api.currentPath.value).toBe('weekmenu.md');
  });

  it('stops auto-naming once the user renames the document manually', async () => {
    h.api.documents.value = [];
    await m.startNew();
    await nextTick();
    m.titleDraft.value = 'mijn-notitie'; // user edits the name field
    h.api.currentBody.value = '# Boodschappen';
    await m.handleSave(); // manual -> mijn-notitie.md
    expect(h.api.currentPath.value).toBe('mijn-notitie.md');
    await nextTick();
    h.api.currentBody.value = '# Weekmenu';
    await m.handleSave(); // must NOT auto-rename
    expect(h.api.currentPath.value).toBe('mijn-notitie.md');
  });

  it('keeps the current name when the first line yields nothing', async () => {
    h.api.documents.value = [];
    await m.startNew();
    await nextTick();
    h.api.currentBody.value = '#   \n\n';
    const ok = await m.handleSave();
    expect(ok).toBe(true);
    expect(h.api.currentPath.value).toBe('untitled.md');
    expect(h.api.deleteDocument).not.toHaveBeenCalled();
  });

  it('appends -2 when the derived name is already taken', async () => {
    h.api.documents.value = [{ path: 'boodschappen.md' }];
    await m.startNew(); // untitled.md (boodschappen is taken)
    await nextTick();
    h.api.currentBody.value = '# Boodschappen';
    await m.handleSave();
    expect(h.api.currentPath.value).toBe('boodschappen-2.md');
  });

  it('re-links auto-naming when the first line is made to match the name again', async () => {
    h.api.documents.value = [];
    await m.startNew();
    await nextTick();
    // Manual rename -> locked (name no longer matches the first line).
    m.titleDraft.value = 'mijn-lijst';
    h.api.currentBody.value = '# Boodschappen';
    await m.handleSave(); // -> mijn-lijst.md, locked
    expect(h.api.currentPath.value).toBe('mijn-lijst.md');
    // Make the first line match the name + save -> re-linked.
    await nextTick();
    h.api.currentBody.value = '# Mijn lijst';
    await m.handleSave(); // name stays mijn-lijst.md, but now auto-managed again
    expect(h.api.currentPath.value).toBe('mijn-lijst.md');
    // A further first-line change now re-derives the name.
    await nextTick();
    h.api.currentBody.value = '# Weekmenu';
    await m.handleSave();
    expect(h.api.currentPath.value).toBe('weekmenu.md');
  });

  it('confirmDelete reports true only when the open document was removed', async () => {
    h.api.currentPath.value = 'open.md';
    await nextTick();
    m.askDelete('open.md');
    expect(await m.confirmDelete()).toBe(true);

    m.askDelete('ander.md');
    expect(await m.confirmDelete()).toBe(false);
  });

  it('confirmDelete surfaces a notice on a 412 conflict and reports false', async () => {
    h.api.deleteDocument.mockResolvedValueOnce({ ok: false, conflict: true });
    h.api.currentPath.value = 'open.md';
    await nextTick();
    m.askDelete('open.md');
    const removed = await m.confirmDelete();
    expect(removed).toBe(false);
    expect(m.deleteNotice.value).toMatch(/gewijzigd/i);
  });

  it('close clears the open document and resets the dirty baseline', () => {
    h.api.currentPath.value = 'open.md';
    h.api.currentBody.value = '# hi';
    h.api.savedBody.value = '# hi';
    h.api.currentEtag.value = 'etag-1';
    m.close();
    expect(h.api.currentPath.value).toBeNull();
    expect(h.api.currentBody.value).toBe('');
    expect(h.api.savedBody.value).toBe('');
    expect(h.api.currentEtag.value).toBeNull();
    // Without resetting savedBody, hasChanges would stay true and the next
    // navigation would trip the unsaved-changes guard.
    expect(m.hasChanges.value).toBe(false);
  });

  it('hasChanges is true when the body diverges from the saved baseline', () => {
    h.api.currentBody.value = '# edited';
    h.api.savedBody.value = '# original';
    expect(m.hasChanges.value).toBe(true);
  });

  it('hasChanges stays false while loading or on a blocking load error', () => {
    h.api.currentBody.value = '# edited';
    h.api.savedBody.value = '# original';
    h.api.docLoading.value = true;
    expect(m.hasChanges.value).toBe(false);
    h.api.docLoading.value = false;
    h.api.docError.value = { kind: 'load-error', message: 'x' };
    expect(m.hasChanges.value).toBe(false);
  });

  it('hasChanges stays true on a draft-present notice (Save button must show)', () => {
    // A local draft diverging from the server is unsaved changes, not a blocking
    // error — mirrors DocumentEditor.blockingError, which does not block on it.
    h.api.currentBody.value = '# draft';
    h.api.savedBody.value = '# server';
    h.api.docError.value = { kind: 'draft-present', message: 'x', serverBody: '# server' };
    expect(m.hasChanges.value).toBe(true);
  });
});
