// Corpus juris favorites for someone who is not logged in, kept in this
// browser. Logged in, favorites live on the account (`/api/favorites`); at the
// first load after login the browser's set is sent there and cleared, so
// nothing starred before logging in is lost.
//
// Only the Corpus juris set: trajecten need a login, so there is no anonymous
// traject favorite to keep.
//
// The key and the stored shape (a JSON array of law ids) are a stored format:
// renaming the key makes the saved favorites unreachable.

import { apiFetch } from '../lib/apiFetch';
import { favoriteUrl } from './corpusUrls';

export const LOCAL_FAVORITES_KEY = 'regelrecht-favorites:corpus';

/** As many as an account may keep (`MAX_FAVORITES` in the editor-api). */
export const MAX_LOCAL_FAVORITES = 1000;

// The set as last written, for when storage is unavailable (disabled, or a
// private window that refuses it): the star then still works until the page
// is left, instead of flipping back on the next read.
let memory: Set<string> | null = null;

/** The favorites kept in this browser; empty when there are none. */
export function readLocalFavorites(): Set<string> {
  let raw: string | null;
  try {
    raw = localStorage.getItem(LOCAL_FAVORITES_KEY);
  } catch {
    // Storage unavailable: what this page view starred.
    return new Set(memory ?? []);
  }
  try {
    const parsed: unknown = JSON.parse(raw ?? '[]');
    return new Set(Array.isArray(parsed) ? parsed.filter((id): id is string => typeof id === 'string') : []);
  } catch {
    // A stored value that is not ours to read: start over.
    return new Set();
  }
}

function writeLocalFavorites(ids: Set<string>): void {
  memory = new Set(ids);
  try {
    if (ids.size === 0) localStorage.removeItem(LOCAL_FAVORITES_KEY);
    else localStorage.setItem(LOCAL_FAVORITES_KEY, JSON.stringify([...ids]));
  } catch {
    // Storage unavailable or full: `memory` keeps the set for this page view.
  }
}

/**
 * Star or unstar a law in this browser. Returns the new set. A set that is
 * full takes no new law, so it never holds more than the account accepts.
 */
export function toggleLocalFavorite(lawId: string): Set<string> {
  const ids = readLocalFavorites();
  if (ids.has(lawId)) ids.delete(lawId);
  else if (ids.size < MAX_LOCAL_FAVORITES) ids.add(lawId);
  writeLocalFavorites(ids);
  return ids;
}

/**
 * Whether a failed save may succeed on a later try: a network failure, a
 * server error, or a refusal that passes (401 before the session settles, a
 * timeout, rate limiting). Any other refusal, such as 403 for an account
 * without the role or 409 for a full set, will be the same next time.
 */
function worthRetrying(error: unknown): boolean {
  const status = (error as { status?: unknown } | null)?.status;
  if (typeof status !== 'number') return true;
  return status >= 500 || status === 401 || status === 408 || status === 429;
}

/**
 * Send the browser's favorites to the logged-in account, one at a time, then
 * forget them. A favorite whose save may succeed later stays in the browser
 * and is tried again at the next load, so a network blip loses nothing; one
 * the server refuses for good is dropped, so it is not sent on every load.
 */
export async function moveLocalFavoritesToAccount(): Promise<void> {
  const ids = readLocalFavorites();
  if (ids.size === 0) return;
  const left = new Set<string>();
  for (const lawId of ids) {
    try {
      await apiFetch(favoriteUrl(null, lawId), { method: 'PUT' });
    } catch (e) {
      if (worthRetrying(e)) left.add(lawId);
    }
  }
  writeLocalFavorites(left);
}
