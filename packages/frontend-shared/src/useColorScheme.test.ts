import { describe, it, expect, beforeEach, vi } from 'vitest';
import { ref, nextTick } from 'vue';
import {
  VALID_THEMES,
  applyColorScheme,
  createLocalStoragePersistence,
  useColorScheme,
} from './useColorScheme';

beforeEach(() => {
  document.documentElement.removeAttribute('data-scheme');
  window.localStorage.clear();
});

describe('VALID_THEMES', () => {
  it('is exactly auto/light/dark', () => {
    expect(VALID_THEMES).toEqual(['auto', 'light', 'dark']);
  });
});

describe('applyColorScheme', () => {
  it('sets data-scheme for light and dark', () => {
    applyColorScheme('light');
    expect(document.documentElement.getAttribute('data-scheme')).toBe('light');
    applyColorScheme('dark');
    expect(document.documentElement.getAttribute('data-scheme')).toBe('dark');
  });

  it('removes data-scheme for auto (OS preference takes over)', () => {
    document.documentElement.setAttribute('data-scheme', 'dark');
    applyColorScheme('auto');
    expect(document.documentElement.hasAttribute('data-scheme')).toBe(false);
  });

  it('removes data-scheme for an invalid value', () => {
    document.documentElement.setAttribute('data-scheme', 'dark');
    applyColorScheme('chartreuse');
    expect(document.documentElement.hasAttribute('data-scheme')).toBe(false);
  });
});

describe('createLocalStoragePersistence', () => {
  it('defaults to auto when nothing is stored', () => {
    expect(createLocalStoragePersistence().theme.value).toBe('auto');
  });

  it('reads a valid stored theme', () => {
    window.localStorage.setItem('rr-theme', 'dark');
    expect(createLocalStoragePersistence().theme.value).toBe('dark');
  });

  it('ignores an invalid stored theme', () => {
    window.localStorage.setItem('rr-theme', 'bogus');
    expect(createLocalStoragePersistence().theme.value).toBe('auto');
  });

  it('honours a custom storage key', () => {
    window.localStorage.setItem('rr-admin-theme', 'light');
    expect(createLocalStoragePersistence('rr-admin-theme').theme.value).toBe('light');
  });

  it('setTheme updates the ref and persists a valid value', () => {
    const p = createLocalStoragePersistence();
    p.setTheme('dark');
    expect(p.theme.value).toBe('dark');
    expect(window.localStorage.getItem('rr-theme')).toBe('dark');
  });

  it('setTheme rejects an invalid value', () => {
    const p = createLocalStoragePersistence();
    p.setTheme('nope');
    expect(p.theme.value).toBe('auto');
    expect(window.localStorage.getItem('rr-theme')).toBeNull();
  });

  it('survives localStorage throwing on read (falls back to auto)', () => {
    const spy = vi.spyOn(Storage.prototype, 'getItem').mockImplementation(() => {
      throw new Error('blocked');
    });
    expect(createLocalStoragePersistence().theme.value).toBe('auto');
    spy.mockRestore();
  });

  it('survives localStorage throwing on write (ref still updates)', () => {
    const p = createLocalStoragePersistence();
    const spy = vi.spyOn(Storage.prototype, 'setItem').mockImplementation(() => {
      throw new Error('quota');
    });
    expect(() => p.setTheme('dark')).not.toThrow();
    expect(p.theme.value).toBe('dark');
    spy.mockRestore();
  });
});

describe('useColorScheme', () => {
  it('applies the initial theme and reacts to backend changes', async () => {
    const backend = { theme: ref('dark'), setTheme: vi.fn() };
    const { colorScheme, setColorScheme } = useColorScheme(backend);

    expect(document.documentElement.getAttribute('data-scheme')).toBe('dark');
    expect(colorScheme.value).toBe('dark');
    expect(setColorScheme).toBe(backend.setTheme);

    backend.theme.value = 'auto';
    await nextTick();
    expect(document.documentElement.hasAttribute('data-scheme')).toBe(false);
  });

  it('returns a read-only colorScheme (writes go through setColorScheme)', () => {
    const backend = { theme: ref('light'), setTheme: vi.fn() };
    const { colorScheme } = useColorScheme(backend);
    // @ts-expect-error -- writing to the read-only ref is what this test checks
    colorScheme.value = 'dark'; // readonly: ignored (Vue warns)
    expect(colorScheme.value).toBe('light');
  });

  it('does not break when called twice for the same backend (WeakSet dedup)', async () => {
    const backend = { theme: ref('light'), setTheme: vi.fn() };
    useColorScheme(backend);
    useColorScheme(backend);
    backend.theme.value = 'dark';
    await nextTick();
    expect(document.documentElement.getAttribute('data-scheme')).toBe('dark');
  });

  it('uses a localStorage-backed default when no backend is given', async () => {
    // Fresh module so the `defaultPersistence` singleton reads this test's
    // localStorage rather than state left by an earlier test in this file.
    vi.resetModules();
    window.localStorage.setItem('rr-theme', 'dark');
    const fresh = await import('./useColorScheme');
    const { colorScheme } = fresh.useColorScheme();
    expect(colorScheme.value).toBe('dark');
    expect(document.documentElement.getAttribute('data-scheme')).toBe('dark');
  });
});
