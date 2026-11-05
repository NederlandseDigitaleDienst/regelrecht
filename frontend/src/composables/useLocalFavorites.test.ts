import { describe, it, expect, vi, beforeEach } from 'vitest';

const apiFetch = vi.fn();
vi.mock('../lib/apiFetch', () => ({ apiFetch: (...args: unknown[]) => apiFetch(...args) }));

import {
  LOCAL_FAVORITES_KEY,
  readLocalFavorites,
  toggleLocalFavorite,
  moveLocalFavoritesToAccount,
} from './useLocalFavorites';

beforeEach(() => {
  localStorage.clear();
  apiFetch.mockReset();
});

describe('local favorites', () => {
  it('starts empty and survives a reload through localStorage', () => {
    expect(readLocalFavorites().size).toBe(0);
    toggleLocalFavorite('wet_a');
    toggleLocalFavorite('wet_b');
    expect([...readLocalFavorites()]).toEqual(['wet_a', 'wet_b']);
  });

  it('unstars a law that was starred, and drops the key when the set is empty', () => {
    toggleLocalFavorite('wet_a');
    toggleLocalFavorite('wet_a');
    expect(readLocalFavorites().size).toBe(0);
    expect(localStorage.getItem(LOCAL_FAVORITES_KEY)).toBeNull();
  });

  it('ignores a stored value it cannot read instead of throwing', () => {
    localStorage.setItem(LOCAL_FAVORITES_KEY, '{not json');
    expect(readLocalFavorites().size).toBe(0);
    localStorage.setItem(LOCAL_FAVORITES_KEY, JSON.stringify(['wet_a', 3, null]));
    expect([...readLocalFavorites()]).toEqual(['wet_a']);
  });
});

describe('moveLocalFavoritesToAccount', () => {
  it('saves each browser favorite on the account and then forgets it', async () => {
    toggleLocalFavorite('wet_a');
    toggleLocalFavorite('wet_b');
    apiFetch.mockResolvedValue({ ok: true });

    await moveLocalFavoritesToAccount();

    expect(apiFetch).toHaveBeenCalledWith('/api/favorites/wet_a', { method: 'PUT' });
    expect(apiFetch).toHaveBeenCalledWith('/api/favorites/wet_b', { method: 'PUT' });
    expect(readLocalFavorites().size).toBe(0);
  });

  it('keeps a favorite whose save failed, for the next attempt', async () => {
    toggleLocalFavorite('wet_a');
    toggleLocalFavorite('wet_b');
    apiFetch.mockImplementation(async (url: string) => {
      if (url.endsWith('wet_b')) throw new Error('network');
      return { ok: true };
    });

    await moveLocalFavoritesToAccount();

    expect([...readLocalFavorites()]).toEqual(['wet_b']);
  });

  it('makes no request when the browser has no favorites', async () => {
    await moveLocalFavoritesToAccount();
    expect(apiFetch).not.toHaveBeenCalled();
  });
});
