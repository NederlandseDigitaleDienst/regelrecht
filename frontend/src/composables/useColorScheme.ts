/**
 * Color scheme picker - automatic (follows OS), light, or dark.
 *
 * Thin facade over the shared `useColorScheme`: the editor persists the
 * preference server-side (via `useUserSettings`) and falls back to localStorage
 * on 401/network errors, so the picker still works for anonymous users. The
 * `data-scheme` attribute on <html> (which the design system keys its dark
 * tokens off) is applied centrally by the shared `useColorScheme`, which this
 * facade feeds the server-backed persistence adapter into.
 */
import { useColorScheme as useSharedColorScheme } from '@regelrecht/frontend-shared';
import type { ColorSchemePersistence } from '@regelrecht/frontend-shared/useColorScheme';
import { useUserSettings } from './useUserSettings';

// Capture a single persistence adapter at module scope. `useUserSettings()`
// returns a fresh object each call, so without this every call site would hand
// the shared composable a distinct backend identity and register its own
// data-scheme watcher; a stable backend lets the shared composable install the
// applier exactly once (matching the previous single watchEffect).
let backend: ColorSchemePersistence | null = null;

export function useColorScheme() {
  if (!backend) {
    const { theme, setTheme } = useUserSettings();
    backend = { theme, setTheme };
  }
  return useSharedColorScheme(backend);
}
