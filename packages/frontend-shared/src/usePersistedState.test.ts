import { describe, it, expect, beforeEach, vi } from 'vitest';
import { nextTick } from 'vue';

/**
 * Persisted state is the difference between "every reload starts over" and
 * "the demo remembers what you chose". These tests pin what is and is not
 * persisted, and that a broken or outdated value does not take the page down
 * with it. That each case writes under its own prefix is tested by the cases
 * themselves, through their own `setCase`.
 */
function freshStorage() {
  const data = new Map<string, string>();
  return {
    getItem: (k: string) => data.get(k) ?? null,
    setItem: (k: string, v: unknown) => void data.set(k, String(v)),
    removeItem: (k: string) => void data.delete(k),
    get length() { return data.size; },
    key: (i: number) => [...data.keys()][i] ?? null,
    _data: data,
  };
}

const PREFIX = 'test-case:v1:';

async function load() {
  vi.resetModules();
  const mod = await import('./usePersistedState');
  mod.setCase('test-case');
  return mod;
}

describe('persisted state', () => {
  beforeEach(() => {
    const storage = freshStorage();
    // Object.keys() over localStorage works in the browser through the
    // indexer; here real properties stand in for it.
    vi.stubGlobal('localStorage', new Proxy(storage, {
      ownKeys: () => [...storage._data.keys()],
      getOwnPropertyDescriptor: () => ({ enumerable: true, configurable: true }),
    }));
    vi.stubGlobal('window', { location: { reload: vi.fn() } });
  });

  it('returns the fallback when nothing is stored', async () => {
    const { persistedRef } = await load();
    expect(persistedRef('n', 500).value).toBe(500);
  });

  it('reads back a value stored earlier', async () => {
    const { persistedRef } = await load();
    const r = persistedRef('n', 500);
    r.value = 1200;
    await nextTick();

    const again = await load();
    expect(again.persistedRef('n', 500).value).toBe(1200);
  });

  it('also stores lists, such as the chosen columns', async () => {
    const { persistedRef } = await load();
    const r = persistedRef<string[]>('columns', []);
    r.value = ['a1', 'b'];
    await nextTick();

    const again = await load();
    expect(again.persistedRef<string[]>('columns', []).value).toEqual(['a1', 'b']);
  });

  it('falls back when the stored value is no longer valid', async () => {
    const { persistedRef } = await load();
    persistedRef<string | null>('variant', null).value = 'discarded-branch';
    await nextTick();

    // By the next load that variant no longer exists.
    const again = await load();
    const r = again.persistedRef<string | null>('variant', null, {
      isValid: (v) => typeof v === 'string' && ['a1', 'a2'].includes(v),
    });
    expect(r.value).toBe(null);
  });

  it('survives unreadable storage without stumbling', async () => {
    const { persistedRef } = await load();
    localStorage.setItem(`${PREFIX}broken`, '{not-json');
    expect(persistedRef('broken', 'fallback').value).toBe('fallback');
  });

  it('clears everything of this case and reloads', async () => {
    const { persistedRef, resetPersistedState, hasPersistedState } = await load();
    persistedRef('n', 500).value = 900;
    await nextTick();
    expect(hasPersistedState()).toBe(true);

    resetPersistedState();
    expect(hasPersistedState()).toBe(false);
    expect(window.location.reload).toHaveBeenCalled();
  });

  it('leaves keys from outside the case alone', async () => {
    const { persistedRef, resetPersistedState } = await load();
    localStorage.setItem('theme', 'dark');
    persistedRef('n', 500).value = 900;
    await nextTick();

    resetPersistedState();
    expect(localStorage.getItem('theme')).toBe('dark');
  });

  // The "start over" banner promises that saved variants stay. A variant is
  // the only thing here that cannot be made again.
  it('keeps a protected key on start over', async () => {
    const { persistedRef, protectKey, resetPersistedState, hasPersistedState } = await load();
    protectKey('variants');
    persistedRef<{ id: string }[]>('variants', []).value = [{ id: 'browser-own' }];
    persistedRef('n', 500).value = 900;
    await nextTick();

    resetPersistedState();

    expect(localStorage.getItem(`${PREFIX}n`)).toBe(null);
    expect(localStorage.getItem(`${PREFIX}variants`)).toBe('[{"id":"browser-own"}]');
    // And the button goes away, since what is left is no choice to clear.
    expect(hasPersistedState()).toBe(false);
  });
});
