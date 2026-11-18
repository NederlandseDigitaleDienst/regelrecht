/**
 * Persisted state: choices and settings that survive a page reload.
 *
 * Why this is one place and not loose localStorage calls: a case has about ten
 * settings across its three screens, and all of them have to come back the
 * same way, survive a change of format, and be cleared with one button.
 *
 * What is persisted: what the user chose themselves (which variants show as a
 * column, the working version, the persona, that persona's choices,
 * population settings, session assumptions). What is not: results of
 * calculations. Those are derived, cost nothing to recompute, and would come
 * back silently stale after the corpus changed.
 *
 * Edited YAML does not go through `persistedRef`. A law text has to match the
 * corpus in the repository, and restoring it blindly after a reload can mean
 * editing a change that no longer exists. Such edits belong in a variant,
 * which keeps its starting point and reports drift when it belongs to an
 * older version of the law.
 *
 * The key names callers pass, and the values stored under them, are a stored
 * format: renaming one makes the saved value unreachable.
 */
import { ref, watch, type Ref } from 'vue';

/**
 * Which case this storage serves. It sets the key prefix, so two cases do not
 * read each other's choices. That is not a theoretical case: hosted, they run
 * behind the same portal and so on the same origin, where localStorage is
 * shared. Locally their ports still keep them apart.
 */
let caseName: string | null = null;

/**
 * Version of the storage format. Raise it when stored values no longer match
 * the code; old keys are then ignored instead of read wrongly.
 */
const STORAGE_VERSION = 1;

/** Every key this case uses, so a reset misses nothing. */
const keys = new Set<string>();

/**
 * Set the case. Call it once at app start, before the first `persistedRef`.
 * A second, different name is an error: refs already hang off the old prefix
 * and would silently lose their value.
 */
export function setCase(name: string): void {
  if (!/^[a-z0-9-]+$/.test(String(name ?? ''))) {
    throw new Error(`setCase: invalid case name ${JSON.stringify(name)}`);
  }
  if (caseName !== null && caseName !== name) {
    throw new Error(`setCase: the case is already '${caseName}' and cannot become '${name}'`);
  }
  caseName = name;
}

function prefix(): string {
  if (caseName === null) {
    // Hard, because the alternative is writing silently into the wrong or an
    // unnamed space, which only shows when someone's choices are gone.
    throw new Error('usePersistedState: setCase() has not been called');
  }
  return `${caseName}:v${STORAGE_VERSION}:`;
}

function read<T>(name: string, fallback: T): T {
  try {
    const raw = localStorage.getItem(prefix() + name);
    if (raw === null) return fallback;
    return JSON.parse(raw);
  } catch {
    // A broken or unreadable value: start from the fallback instead of
    // letting the page stumble.
    return fallback;
  }
}

/**
 * Write a value away. Returns whether that worked.
 *
 * For most things here (which column, which persona, a slider's position) a
 * failed write is no disaster: in private mode the app then works without
 * remembering, and the caller ignores the result. For something the user made
 * themselves it is, and that caller checks what comes back.
 */
function write(name: string, value: unknown): boolean {
  try {
    localStorage.setItem(prefix() + name, JSON.stringify(value));
    return true;
  } catch {
    return false;
  }
}

/**
 * A ref that persists itself. Use it instead of ref() for everything the user
 * sets.
 *
 * The value read back is whatever localStorage held, typed as the fallback.
 * Callers that cannot rely on that pass `isValid`, which receives the value
 * unchecked, as `unknown`.
 *
 * @param name unique key within this case
 * @param fallback value when nothing is stored
 * @param options.isValid check on the value read; on false the fallback is
 *   used. Useful for ids that point at a variant that no longer exists.
 * @param options.upgrade turns a stored value in an older format into the
 *   current one. It runs before the ref exists, so it writes nothing itself;
 *   the caller decides whether to write the result back.
 */
export function persistedRef<T>(
  name: string,
  fallback: T,
  { isValid, upgrade }: { isValid?: (value: unknown) => boolean; upgrade?: (value: unknown) => T } = {},
): Ref<T> {
  keys.add(name);
  const raw = read<unknown>(name, fallback);
  const stored = (upgrade && raw !== fallback ? upgrade(raw) : raw) as T;
  const start = isValid && stored !== fallback && !isValid(stored) ? fallback : stored;
  const r = ref(start) as Ref<T>;
  watch(r, (next) => write(name, next), { deep: true });
  return r;
}

/** Read one value without a ref, for code that is already reactive itself. */
export function readPersisted<T>(name: string, fallback: T): T {
  keys.add(name);
  return read(name, fallback);
}

/** Persist one value without a ref. Returns whether it worked. */
export function writePersisted(name: string, value: unknown): boolean {
  keys.add(name);
  return write(name, value);
}

/**
 * Keys that `resetPersistedState` leaves alone. "Start over" clears the user's
 * choices, not their work: the banner at that button promises in so many words
 * that saved variants stay, and a variant is the only thing here that cannot
 * be made again.
 */
const protectedKeys = new Set<string>();

/** Register a key that "start over" must not clear. */
export function protectKey(name: string): void {
  protectedKeys.add(name);
}

/**
 * Clear everything this case remembers and reload, so every composable starts
 * from its own fallback. Reloading is more reliable here than resetting the
 * refs: simulations, engines and workers hang off that state, and winding
 * them all back by hand is exactly where bugs creep in.
 */
export function resetPersistedState(): void {
  try {
    const p = prefix();
    const keep = new Set([...protectedKeys].map((name) => p + name));
    for (const name of [...keys]) {
      if (!keep.has(p + name)) localStorage.removeItem(p + name);
    }
    // Also keys from an earlier session that were not registered this time,
    // for example from a screen that was not opened.
    for (const key of Object.keys(localStorage)) {
      if (key.startsWith(p) && !keep.has(key)) localStorage.removeItem(key);
    }
  } catch {
    // Nothing to clear when storage is not available.
  }
  window.location.reload();
}

/** Is anything persisted? Decides whether the "start over" button makes sense. */
export function hasPersistedState(): boolean {
  try {
    const p = prefix();
    const keep = new Set([...protectedKeys].map((name) => p + name));
    return Object.keys(localStorage).some((k) => k.startsWith(p) && !keep.has(k));
  } catch {
    return false;
  }
}

/** For tests only: forget the case and the registered keys. */
export function _resetForTest(): void {
  caseName = null;
  keys.clear();
  protectedKeys.clear();
}
