/**
 * useColorScheme — shared color-scheme picker for the regelrecht frontends.
 *
 * Owns the single source of truth for applying the chosen theme to
 * `<html data-scheme>` (the attribute `@nldd/design-system` keys its dark
 * tokens on) and for validating theme values. 'auto' is encoded as the
 * *absence* of the attribute so the OS-level `prefers-color-scheme` media
 * query takes over.
 *
 * Persistence is injected so each app can supply the right backend without
 * this module knowing about app-specific endpoints:
 *   - the editor injects its server-backed `useUserSettings` adapter
 *     (`{ theme, setTheme }`), so the picker syncs across devices;
 *   - admin and lawmaking have no settings endpoint, so they use the default
 *     localStorage adapter.
 *
 * A persistence adapter is a `ColorSchemePersistence`.
 */
import { ref, readonly, watch, effectScope, type Ref, type DeepReadonly } from 'vue';

export const VALID_THEMES = ['auto', 'light', 'dark'] as const;

/** A theme the picker offers. `auto` follows the operating system. */
export type Theme = (typeof VALID_THEMES)[number];

/**
 * Where an app keeps the chosen theme. `theme` may hold a value that is not a
 * `Theme`, for example a stale value from a server-side settings store, which
 * is why it is typed as a string and checked with `isTheme`.
 */
export interface ColorSchemePersistence {
  theme: Ref<string>;
  setTheme: (value: string) => void;
}

function isTheme(value: unknown): value is Theme {
  return (VALID_THEMES as readonly unknown[]).includes(value);
}

/**
 * Apply a theme to `<html>`. 'auto' (and any invalid value) removes the
 * attribute so `@media (prefers-color-scheme: dark)` drives the palette.
 */
export function applyColorScheme(theme: string): void {
  if (typeof document === 'undefined') return;
  const root = document.documentElement;
  if (theme === 'auto' || !isTheme(theme)) {
    root.removeAttribute('data-scheme');
  } else {
    root.setAttribute('data-scheme', theme);
  }
}

/**
 * localStorage-backed persistence adapter — the default for apps without a
 * server-side settings store. Reads synchronously on creation so the cached
 * palette applies on first paint without a flicker.
 */
export function createLocalStoragePersistence(storageKey = 'rr-theme'): ColorSchemePersistence {
  function read(): Theme | null {
    try {
      const v = window.localStorage?.getItem(storageKey);
      return isTheme(v) ? v : null;
    } catch {
      return null;
    }
  }
  function write(value: string): void {
    try {
      window.localStorage?.setItem(storageKey, value);
    } catch {
      // Ignore storage errors (private mode, quota, disabled) — the only
      // consequence is the scheme resets to 'auto' on next load.
    }
  }
  const theme = ref<string>((typeof window !== 'undefined' && read()) || 'auto');
  return {
    theme,
    setTheme(value) {
      if (!isTheme(value)) return;
      theme.value = value;
      write(value);
    },
  };
}

// The default localStorage adapter is a module-level singleton so every
// `useColorScheme()` call within one app shares one theme ref.
let defaultPersistence: ColorSchemePersistence | null = null;

// Track which persistence backends already have the data-scheme applier wired
// so repeated `useColorScheme` calls don't stack duplicate watchers.
const appliedBackends = new WeakSet<ColorSchemePersistence>();

// Detached scope so the data-scheme watcher lives for the app's lifetime,
// independent of which component's setup() first calls `useColorScheme`. Without
// this the watcher would be owned by that first caller's component scope and
// would be auto-stopped if that component ever unmounts, silently killing the
// applier.
const applierScope = effectScope(true);

/**
 * @param persistence Optional persistence adapter. Defaults to a shared
 *   localStorage adapter.
 */
export function useColorScheme(persistence?: ColorSchemePersistence): {
  colorScheme: DeepReadonly<Ref<string>>;
  setColorScheme: (value: string) => void;
} {
  const backend =
    persistence ||
    (defaultPersistence ??= createLocalStoragePersistence());

  if (!appliedBackends.has(backend)) {
    appliedBackends.add(backend);
    // `immediate` applies the current (cached/default) theme now and on change.
    // Run inside the detached scope so the watcher isn't tied to the caller's
    // component lifetime.
    applierScope.run(() => watch(backend.theme, applyColorScheme, { immediate: true }));
  }

  return {
    // Read-only: callers change the scheme through setColorScheme so the
    // persistence backend stays the single writer.
    colorScheme: readonly(backend.theme),
    setColorScheme: backend.setTheme,
  };
}
