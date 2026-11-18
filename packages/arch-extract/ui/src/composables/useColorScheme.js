/**
 * useColorScheme — dark/light/auto picker for the architecture explorer.
 *
 * A standalone port of the shared frontend composable
 * (`packages/frontend-shared/src/useColorScheme.ts`): it owns applying the
 * chosen theme to `<html data-scheme>` and persists the choice in
 * localStorage. 'auto' is encoded as the *absence* of the attribute so the
 * OS-level `prefers-color-scheme` media query takes over. The explorer is a
 * separate npm project with no shared package, so the pattern is reproduced
 * here rather than imported — keep the two in step.
 */
import { ref, readonly, watch } from 'vue';

export const VALID_THEMES = ['auto', 'light', 'dark'];

const STORAGE_KEY = 'arch-explorer-theme';

/** Apply a theme to `<html>`. 'auto' removes the attribute so the OS decides. */
export function applyColorScheme(theme) {
  if (typeof document === 'undefined') return;
  const root = document.documentElement;
  if (theme === 'auto' || !VALID_THEMES.includes(theme)) {
    root.removeAttribute('data-scheme');
  } else {
    root.setAttribute('data-scheme', theme);
  }
}

function read() {
  try {
    const v = window.localStorage?.getItem(STORAGE_KEY);
    return VALID_THEMES.includes(v) ? v : null;
  } catch {
    return null;
  }
}

function write(value) {
  try {
    window.localStorage?.setItem(STORAGE_KEY, value);
  } catch {
    // Ignore storage errors (private mode, quota); the scheme just resets to
    // 'auto' on the next load.
  }
}

// Module-level singleton so every call shares one theme ref and one applier.
const theme = ref((typeof window !== 'undefined' && read()) || 'auto');
let applierInstalled = false;

export function useColorScheme() {
  if (!applierInstalled) {
    applierInstalled = true;
    watch(theme, applyColorScheme, { immediate: true });
  }

  function setColorScheme(value) {
    if (!VALID_THEMES.includes(value)) return;
    theme.value = value;
    write(value);
  }

  /** Cycle auto → light → dark → auto, for a single toggle button. */
  function cycleColorScheme() {
    const idx = VALID_THEMES.indexOf(theme.value);
    setColorScheme(VALID_THEMES[(idx + 1) % VALID_THEMES.length]);
  }

  return { colorScheme: readonly(theme), setColorScheme, cycleColorScheme };
}
