/**
 * Variants the user saves themselves, in the browser.
 *
 * Why this exists: "save as variant" used to create a git branch in the case
 * checkout. Hosted, there is no checkout, and since the move to the monorepo
 * the path is wrong locally too. That left no way at all to keep a change: a
 * reloaded page threw everything away. This is that way.
 *
 * What a browser variant is: the same shape as a checked-in variant from
 * `variants.json` (`{ id, title, files }`), so the rest of the app need not
 * know the difference. The only differences are where the text comes from, and
 * `origin: 'browser'`, so the UI can mark it.
 *
 * What it deliberately is not: a shareable variant. It lives in this browser,
 * on this device, and is gone when someone clears their browser data. The text
 * at the button says so too; a variant that silently disappears is worse than
 * one that says up front that it can.
 *
 * About drift: every variant keeps, per file, a fingerprint of the text it was
 * made on. When the corpus later differs, the edit was made on a law that no
 * longer reads that way. `checkDrift` reports that; the app still lets the
 * variant activate, but says it belongs to an older version of the law. This
 * is exactly why edited YAML did not belong in plain persisted state (see the
 * header of usePersistedState), so without this check we would repeat that
 * mistake.
 *
 * About space: localStorage fills up around 5 MB and a law here is ~50 KB, so
 * about forty variants fit. Hence the fingerprint instead of the whole base
 * text, and hence `saveVariant` failing loudly when storage is full: a "saved"
 * that turns out to be gone after a reload is worse than a refusal.
 */
import { ref } from 'vue';
import { persistedRef, protectKey, readPersisted, writePersisted } from './usePersistedState';

/**
 * The key in persisted state. Its value is a stored format: users' variants
 * live under it, so it does not change with the code's language.
 */
const STORAGE_KEY = 'varianten';

/**
 * The prefix of a browser variant id. It keeps them apart from the checked-in
 * variants (a1, nk-2, ...), which never carry it. The ids are stored and
 * appear in the app, so the value stays.
 */
export const BROWSER_PREFIX = 'eigen-';

/** Is this the id of a variant from the browser? */
export function isBrowserVariant(id: unknown): id is string {
  return typeof id === 'string' && id.startsWith(BROWSER_PREFIX);
}

/**
 * A short fingerprint of a law text, to tell whether it changed later. A hash
 * and not the text itself: a law here is ~50 KB, and with the text every
 * variant cost twice as much of a storage that fills up around 5 MB. This
 * halves that, and we need to know no more than "is this still the same text".
 *
 * Not cryptography: this stops no attacker, it notices a change. FNV-1a is
 * enough for that and needs no async, unlike SubtleCrypto.
 */
export function fingerprint(text: unknown): string | null {
  if (typeof text !== 'string') return null;
  let h = 0x811c9dc5;
  for (let i = 0; i < text.length; i++) {
    h ^= text.charCodeAt(i);
    h = Math.imul(h, 0x01000193) >>> 0;
  }
  // The length as well: two texts with the same hash but a different length
  // can then still be told apart, at the cost of four characters.
  return `${h.toString(36)}-${text.length.toString(36)}`;
}

/**
 * "Start over" clears the user's choices, not their work. The banner at that
 * button promises in so many words that saved variants stay, and a variant is
 * the only thing here that cannot simply be made again.
 */
protectKey(STORAGE_KEY);

/**
 * One file of a saved variant. `path` is the index path of the base document
 * (the same as `base` in variants.json) and `originalHash` the fingerprint of
 * the text the edit started from. A variant from before that fingerprint has
 * none.
 */
export interface StoredFile {
  path: string;
  yaml: string;
  originalHash?: string | null;
}

/** A variant as it is stored in the browser. */
export interface StoredVariant {
  id: string;
  title: string;
  created: string;
  updated?: string | null;
  base?: string | null;
  files: StoredFile[];
}

/** A variant in the shape of variants.json, as the lawStore reads it. */
export interface BrowserVariant {
  id: string;
  title: string;
  files: { path: string; base: string }[];
  origin: 'browser';
  created: string;
  updated: string | null;
  base: string | null;
}

/** What the user edited: a file with its new and its original text. */
export interface EditedFile {
  path: string;
  yaml: string;
  /** Without it the file gets no fingerprint, and `checkDrift` reports it as unknown. */
  original?: string;
}

/** The top-level field names a variant had before the code was translated. */
const OLD_FIELDS = ['titel', 'gemaakt', 'bijgewerkt', 'basis', 'bestanden'];

function isRecord(value: unknown): value is Record<string, unknown> {
  return value !== null && typeof value === 'object' && !Array.isArray(value);
}

/** Does any stored variant still carry a field from before the translation? */
function hasOldFormat(raw: unknown): boolean {
  return Array.isArray(raw) && raw.some((v) => isRecord(v) && OLD_FIELDS.some((f) => f in v));
}

/**
 * Bring a stored variant into the current format.
 *
 * Variants saved before the code was translated to English carry Dutch field
 * names (`titel`, `gemaakt`, `bijgewerkt`, `basis`, `bestanden` with `pad`
 * and `origineelHash`). They are read under both names and written back in
 * the new one, so nobody loses a variant. Once every browser has loaded the
 * app since, this function can go.
 */
function upgradeStoredVariant(v: Record<string, unknown>): StoredVariant {
  const newFiles = Array.isArray(v.files) ? v.files : [];
  const oldFiles = Array.isArray(v.bestanden) ? v.bestanden : [];
  const files = (newFiles.length ? newFiles : oldFiles).filter(isRecord);
  return {
    id: v.id as string,
    title: (v.title ?? v.titel) as string,
    created: (v.created ?? v.gemaakt) as string,
    updated: (v.updated ?? v.bijgewerkt ?? null) as string | null,
    base: (v.base ?? v.basis ?? null) as string | null,
    files: files.map((f) => ({
      path: (f.path ?? f.pad) as string,
      yaml: f.yaml as string,
      originalHash: (f.originalHash ?? f.origineelHash ?? null) as string | null,
    })),
  };
}

/**
 * The stored list in the current format. Anything that is not a variant (a
 * non-array, a record without an id) is left out rather than allowed to break
 * the app at load.
 */
function upgradeStoredList(raw: unknown): StoredVariant[] {
  if (!Array.isArray(raw)) return [];
  return raw.filter((v): v is Record<string, unknown> => isRecord(v) && typeof v.id === 'string')
    .map(upgradeStoredVariant);
}

/**
 * The saved variants. What comes out of localStorage is not checked beyond
 * the upgrade; the readers below only read fields saveVariant wrote itself.
 */
const saved = persistedRef<StoredVariant[]>(STORAGE_KEY, [], { upgrade: upgradeStoredList });

// Write the upgraded list back once, and only when something was in the old
// format. Directly and not through the ref's watcher: the watcher would run
// after every module has loaded, and if this module loaded before setCase(),
// it would then write the empty fallback over the user's variants. Without a
// case, writePersisted does nothing.
if (hasOldFormat(readPersisted<unknown>(STORAGE_KEY, null))) writePersisted(STORAGE_KEY, saved.value);

/** A counter that bumps on every change, to watch. */
const revision = ref(0);

/** Turn a saved variant into the shape the lawStore knows from variants.json. */
function toVariant(v: StoredVariant): BrowserVariant {
  return {
    id: v.id,
    title: v.title,
    files: v.files.map((f) => ({ path: f.path, base: f.path })),
    origin: 'browser',
    created: v.created,
    // When it was last saved into, or null if that never happened. It has to
    // pass through this mapping: every reader (the picker, the drift notice)
    // reads the variant in this shape, so what drops out here does not exist
    // further on.
    updated: v.updated ?? null,
    base: v.base ?? null,
  };
}

/** All browser variants, in the shape of variants.json, newest first. */
export function browserVariants(): BrowserVariant[] {
  revision.value;
  return [...saved.value]
    .sort((a, b) => String(b.created ?? '').localeCompare(String(a.created ?? '')))
    .map(toVariant);
}

/** The stored form of one variant, or null. */
export function browserVariant(id: string): StoredVariant | null {
  revision.value;
  return saved.value.find((v) => v.id === id) ?? null;
}

/**
 * The YAML of one file of a browser variant, or null when this variant does
 * not edit that file. This is what the lawStore reads instead of a fetch.
 */
export function browserVariantYaml(id: string, path: string): string | null {
  return browserVariant(id)?.files.find((f) => f.path === path)?.yaml ?? null;
}

/**
 * A readable id from a title: lower case, dashes, with the browser prefix in
 * front. When it collides with an existing one, a number goes after it.
 */
export function makeId(title: unknown, takenIds: Iterable<string> = []): string {
  const core = String(title ?? '')
    .toLowerCase()
    .normalize('NFD')
    .replace(/[̀-ͯ]/g, '')
    .replace(/[^a-z0-9]+/g, '-')
    .replace(/^-+|-+$/g, '')
    .slice(0, 40) || 'variant';
  const taken = new Set(takenIds);
  let id = `${BROWSER_PREFIX}${core}`;
  for (let n = 2; taken.has(id); n++) id = `${BROWSER_PREFIX}${core}-${n}`;
  return id;
}

/** A title that is left after trimming and collapsing spaces. */
function validateTitle(title: unknown): string {
  const clean = String(title ?? '').trim().replace(/\s+/g, ' ');
  // The messages in this module are shown to the user, so they stay Dutch.
  if (!clean) throw new Error('Een variant heeft een titel nodig.');
  return clean;
}

/** Files must carry a path and a text; otherwise the variant is broken. */
function validateFiles(files: unknown): asserts files is EditedFile[] {
  if (!Array.isArray(files) || !files.length) {
    throw new Error('Er is niets bewerkt om te bewaren.');
  }
  for (const f of files as Partial<EditedFile>[]) {
    if (typeof f?.path !== 'string' || !f.path || typeof f.yaml !== 'string') {
      throw new Error(`Onbruikbaar bestand in de variant: ${JSON.stringify(f?.path)}`);
    }
  }
}

/** The files in their stored form, with a fingerprint of the base text. */
function toStoredFiles(files: EditedFile[]): StoredFile[] {
  return files.map((f) => ({
    path: f.path,
    yaml: f.yaml,
    // The fingerprint of the text the edit started from. With it we can later
    // see whether the corpus changed under the variant, without storing that
    // whole text a second time.
    originalHash: fingerprint(f.original),
  }));
}

/** Write the list away; throws when storage is full or disabled. */
function write(next: StoredVariant[]): void {
  // Write right away and check that it worked, instead of leaving it to the
  // ref's watcher. That one swallows a full storage silently, and then the
  // app reports "saved" while the work is gone after a reload.
  if (!writePersisted(STORAGE_KEY, next)) {
    throw new Error(
      'De browseropslag zit vol of staat uit, dus deze variant is niet bewaard. '
      + 'Verwijder een eerdere eigen variant en probeer het opnieuw.',
    );
  }
  saved.value = next;
  revision.value++;
}

/**
 * Save a variant.
 *
 * `title` is what the user typed, `base` the variant id this one builds on,
 * and `takenIds` the ids already in use (the checked-in variants too, so a
 * browser variant never shadows one). Returns the saved variant in the shape
 * of variants.json.
 */
export function saveVariant({
  title,
  files,
  base = null,
  takenIds = [],
}: {
  title: string;
  files: EditedFile[];
  base?: string | null;
  takenIds?: string[];
}): BrowserVariant {
  const clean = validateTitle(title);
  validateFiles(files);
  const id = makeId(clean, [...takenIds, ...saved.value.map((v) => v.id)]);
  const variant: StoredVariant = {
    id,
    title: clean,
    created: new Date().toISOString(),
    base,
    files: toStoredFiles(files),
  };
  write([...saved.value, variant]);
  return toVariant(variant);
}

/**
 * Update an existing browser variant: the same variant, new content.
 *
 * Without this a variant could only grow. Someone who kept working in their
 * own variant and wanted to save got a second one next to it, with the same
 * name plus a number. In the usual loop (make a variant, run it, adjust a
 * parameter, again) that produces a row of nearly identical columns, and
 * browser storage has a limit.
 *
 * `created` stays: it is the same variant, and that date sets its place in the
 * picker. `updated` is added, so it shows that it was worked on since.
 *
 * This overwrites work that was already there, which cannot be undone once
 * written. That is why it returns a `restore` that puts the previous content
 * back: the caller activates the variant afterwards, and when the engine
 * rejects the edited law the old, working content has to be able to return.
 * Without that, a variant stays behind that never loads again.
 *
 * `title` is a new name; left out, the old one stays. Returns the updated
 * variant in the shape of variants.json, plus a function that restores the
 * previous content.
 */
export function updateVariant(
  id: string,
  { files, title }: { files?: EditedFile[]; title?: string } = {},
): { variant: BrowserVariant; restore: () => void } {
  const existing = saved.value.find((v) => v.id === id);
  if (!existing) throw new Error(`Deze variant staat niet in deze browser: ${id}`);
  validateFiles(files);
  const previous = existing;
  const variant: StoredVariant = {
    ...existing,
    title: title === undefined ? existing.title : validateTitle(title),
    updated: new Date().toISOString(),
    files: toStoredFiles(files),
  };
  write(saved.value.map((v) => (v.id === id ? variant : v)));
  return {
    variant: toVariant(variant),
    restore: () => {
      // Restoring must not fail itself: it runs in the error path, and a
      // failure here would hide the error that actually mattered.
      try {
        write(saved.value.map((v) => (v.id === id ? previous : v)));
      } catch { /* storage refuses; the caller reports the real error */ }
    },
  };
}

/** Delete a saved variant. Returns whether anything was removed. */
export function deleteVariant(id: string): boolean {
  const before = saved.value.length;
  saved.value = saved.value.filter((v) => v.id !== id);
  const removed = saved.value.length !== before;
  if (removed) revision.value++;
  return removed;
}

/**
 * Has the corpus changed since this variant was made?
 *
 * `currentBase` gives the base text of a document as it is in the app now.
 * `unknown` is true when the variant predates this check and carries no
 * fingerprint; then there is nothing to compare, and the app would rather say
 * nothing than claim something.
 */
export function checkDrift(
  id: string,
  currentBase: (path: string) => string | undefined,
): { drifted: boolean; paths: string[]; unknown: boolean } {
  const v = browserVariant(id);
  if (!v) return { drifted: false, paths: [], unknown: false };
  const paths: string[] = [];
  let unknown = false;
  for (const f of v.files) {
    if (typeof f.originalHash !== 'string') {
      unknown = true;
      continue;
    }
    const now = currentBase(f.path);
    if (typeof now !== 'string') continue;
    if (fingerprint(now) !== f.originalHash) paths.push(f.path);
  }
  return { drifted: paths.length > 0, paths, unknown };
}
