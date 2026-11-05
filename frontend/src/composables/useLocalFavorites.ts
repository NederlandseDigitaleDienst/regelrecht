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

/** The favorites kept in this browser; empty when there are none or storage is unavailable. */
export function readLocalFavorites(): Set<string> {
  try {
    const parsed: unknown = JSON.parse(localStorage.getItem(LOCAL_FAVORITES_KEY) ?? '[]');
    return new Set(Array.isArray(parsed) ? parsed.filter((id): id is string => typeof id === 'string') : []);
  } catch {
    return new Set();
  }
}

function writeLocalFavorites(ids: Set<string>): void {
  try {
    if (ids.size === 0) localStorage.removeItem(LOCAL_FAVORITES_KEY);
    else localStorage.setItem(LOCAL_FAVORITES_KEY, JSON.stringify([...ids]));
  } catch {
    // Storage disabled or full: the star still toggles for this page view.
  }
}

/** Star or unstar a law in this browser. Returns the new set. */
export function toggleLocalFavorite(lawId: string): Set<string> {
  const ids = readLocalFavorites();
  if (ids.has(lawId)) ids.delete(lawId);
  else ids.add(lawId);
  writeLocalFavorites(ids);
  return ids;
}

/**
 * Send the browser's favorites to the logged-in account, then forget the ones
 * that arrived. A favorite whose save fails stays in the browser and is tried
 * again at the next load, so a network blip loses nothing.
 */
export async function moveLocalFavoritesToAccount(): Promise<void> {
  const ids = readLocalFavorites();
  if (ids.size === 0) return;
  const left = new Set(ids);
  await Promise.all(
    [...ids].map(async (lawId) => {
      try {
        await apiFetch(favoriteUrl(null, lawId), { method: 'PUT' });
        left.delete(lawId);
      } catch {
        // Kept for the next attempt; see above.
      }
    }),
  );
  writeLocalFavorites(left);
}
