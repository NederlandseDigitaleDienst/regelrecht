import { describe, it, expect, vi, beforeEach } from 'vitest';

const apiFetch = vi.fn();
vi.mock('../lib/apiFetch', () => ({ apiFetch: (...args: unknown[]) => apiFetch(...args) }));

import {
  LOCAL_FAVORITES_KEY,
  MAX_LOCAL_FAVORITES,
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

  it('takes no new law once the set holds as many as an account may keep', () => {
    const full = Array.from({ length: MAX_LOCAL_FAVORITES }, (_, i) => `wet_${i}`);
    localStorage.setItem(LOCAL_FAVORITES_KEY, JSON.stringify(full));
    expect(toggleLocalFavorite('wet_extra').has('wet_extra')).toBe(false);
    // Unstarring still works on a full set.
    expect(toggleLocalFavorite('wet_0').has('wet_0')).toBe(false);
  });

  it('keeps the set for this page view when storage refuses it', () => {
    const setItem = vi.spyOn(Storage.prototype, 'setItem').mockImplementation(() => {
      throw new Error('QuotaExceededError');
    });
    const getItem = vi.spyOn(Storage.prototype, 'getItem').mockImplementation(() => {
      throw new Error('SecurityError');
    });
    try {
      toggleLocalFavorite('wet_a');
      expect([...readLocalFavorites()]).toEqual(['wet_a']);
    } finally {
      setItem.mockRestore();
      getItem.mockRestore();
    }
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

  it('drops a favorite the server refuses for good, and keeps one that may pass later', async () => {
    for (const id of ['wet_403', 'wet_409', 'wet_503', 'wet_429']) toggleLocalFavorite(id);
    apiFetch.mockImplementation(async (url: string) => {
      const status = Number(url.slice(-3));
      throw Object.assign(new Error(`HTTP ${status}`), { status });
    });

    await moveLocalFavoritesToAccount();

    expect([...readLocalFavorites()].sort()).toEqual(['wet_429', 'wet_503']);
  });

  it('saves one favorite at a time', async () => {
    toggleLocalFavorite('wet_a');
    toggleLocalFavorite('wet_b');
    let inFlight = 0;
    let most = 0;
    apiFetch.mockImplementation(async () => {
      inFlight++;
      most = Math.max(most, inFlight);
      await new Promise((r) => setTimeout(r, 0));
      inFlight--;
      return { ok: true };
    });

    await moveLocalFavoritesToAccount();

    expect(most).toBe(1);
  });

  it('makes no request when the browser has no favorites', async () => {
    await moveLocalFavoritesToAccount();
    expect(apiFetch).not.toHaveBeenCalled();
  });
});
