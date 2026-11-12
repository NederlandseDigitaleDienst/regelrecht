/**
 * useSavedPr - the "last saved PR" badge state shared across save paths.
 *
 * Successor of `useEditorSession.js`: the per-tab session id and its
 * `X-Editor-Session` header are gone - writes are scoped by the traject
 * in the URL path (`/api/trajects/{ref}/corpus/...`) and the backend
 * never read the header. What remains is the PR bookkeeping that the
 * traject write-back path still returns on every save.
 */

import { ref } from 'vue';

/** `SavePrInfo` in packages/editor-api/src/corpus_handlers.rs. */
export interface SavedPr {
  url: string;
  number: number;
}

/**
 * Module-level shared ref so every composable that performs a save
 * (useOpenLaw, useScenarios, useDraftNotes, …) updates the same value - and
 * the AppShell's "PR #N" badge picks up the most recent PR regardless of
 * which pane triggered the save. Saves in the same traject land on the
 * same upstream branch/PR, so a single shared ref is exactly what the
 * badge needs.
 */
export const lastSavedPr = ref<SavedPr | null>(null);

/**
 * Sanitize a PR payload from the editor-api before storing it in
 * `lastSavedPr`. Only PRs whose `url` is an explicit `https://` link survive
 * - anything else (missing url, `javascript:`, relative paths) is collapsed
 * to `null` so the `<a :href>` binding can never accept an attacker-controlled
 * scheme even if the backend ever misbehaves.
 */
export function sanitizeSavedPr(pr: unknown): SavedPr | null {
  const candidate = pr as { url?: unknown } | null | undefined;
  if (!candidate || typeof candidate.url !== 'string' || !candidate.url.startsWith('https://')) {
    return null;
  }
  // Only `url` is checked; the rest is passed through as the backend sent it.
  return candidate as SavedPr;
}
