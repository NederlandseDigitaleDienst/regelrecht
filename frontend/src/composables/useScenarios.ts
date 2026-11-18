/**
 * useScenarios - the feature files in a regelwerk's scenario folder.
 *
 * Reads pick the global or traject-scoped URL based on `trajectRef`. Reading
 * and saving one file is `scenario/openScenario.ts`.
 *
 * @param {import('vue').Ref<string>} lawId
 * @param {import('vue').Ref<string|null>=} trajectRef Active traject
 *   ref. Defaults to `ref(null)` so omitting the second arg gives a
 *   read-only / global-scope view instead of a `TypeError: Cannot read
 *   properties of undefined` deep inside the fetch.
 */
import { ref, watch, type Ref } from 'vue';
import { scenariosListUrl } from './corpusUrls';
import { apiFetch } from '../lib/apiFetch';
import type { RequestError } from '../lib/apiFetch';

/** `ScenarioEntry` in packages/editor-api/src/corpus_handlers.rs. */
export interface ScenarioEntry {
  filename: string;
  target_law_ids?: string[];
}

/**
 * True when the scenario file declares execution targets and none of
 * them is the opened law - running it would evaluate a different law.
 * Files without a parseable execution step (`target_law_ids` empty)
 * count as "unknown", not as a mismatch.
 */
export function isScenarioMismatch(
  entry: Pick<ScenarioEntry, 'target_law_ids'> | null | undefined,
  lawId: string | null | undefined,
): boolean {
  const targets = entry?.target_law_ids || [];
  return targets.length > 0 && !targets.includes(lawId as string);
}

/**
 * The file to show when nobody chose one: the first that targets the open
 * regelwerk, then one whose targets are unknown (no readable execution step),
 * then the first. The file's execution steps bind it to a regelwerk, not the
 * folder it is in.
 */
export function preferredScenarioFile(
  entries: readonly ScenarioEntry[],
  lawId: string | null | undefined,
): ScenarioEntry | null {
  return (
    entries.find((e) => !!lawId && e.target_law_ids?.includes(lawId)) ??
    entries.find((e) => !isScenarioMismatch(e, lawId)) ??
    entries[0] ??
    null
  );
}

export function useScenarios(
  lawId: Readonly<Ref<string | null | undefined>>,
  trajectRef: Readonly<Ref<string | null | undefined>> = ref(null),
) {
  const scenarios = ref<ScenarioEntry[]>([]);
  const loading = ref(false);
  // apiFetch rejects with an ApiError, so the caught values stored here are Errors.
  const error = ref<RequestError | null>(null);

  // Sequence number of the most recent fetchScenarios call. Only that call may
  // write `scenarios`, `error` or `loading`: a slow response for law A must not
  // paint law B's screen, and must not clear `loading` while B is still on its
  // way. Comparing the scope is not enough, because switching A, B, A gives the
  // stale and the fresh request for A the same scope.
  let latestFetch = 0;
  // The regelwerk and traject the list belongs to.
  let listedScope: string | null = null;

  async function fetchScenarios() {
    const fetchId = ++latestFetch;
    const isLatest = () => fetchId === latestFetch;

    // Clear before the early return, not after: navigating from a failed law
    // to "no law selected" would otherwise leave the banner up.
    error.value = null;
    if (!lawId.value) {
      // A request for the previous law may still be in flight; it no longer
      // counts, so nothing is loading any more.
      loading.value = false;
      scenarios.value = [];
      return;
    }

    loading.value = true;
    // Another regelwerk or traject: the old list is not this one's, so it
    // goes now rather than when the new one arrives.
    const scope = `${trajectRef.value ?? ''}::${lawId.value}`;
    if (scope !== listedScope) {
      scenarios.value = [];
      listedScope = scope;
    }

    try {
      // A law without scenarios is already a 200 with `[]` (the backend's
      // `list_scenarios_in_scope` returns that for a missing directory), so no
      // status needs to be read as "empty" here. Every non-ok status is a real
      // failure: 404 means the law is not in this scope at all, 500 a broken
      // traject scan, 502 an unreachable corpus backend. They used to become
      // the same empty list, so "we could not read the scenarios" and "this
      // law has none" were one screen.
      const res = await apiFetch(scenariosListUrl(trajectRef.value, lawId.value), {
        errorMessage: (status) =>
          status === 404
            ? `Dit regelwerk is niet gevonden in deze scope (${status})`
            : `Scenario's konden niet worden geladen: ${status}`,
      });
      const listed = (await res.json()) as ScenarioEntry[];
      if (!isLatest()) return;
      scenarios.value = listed;
    } catch (e) {
      if (!isLatest()) return;
      error.value = e as RequestError;
      scenarios.value = [];
    } finally {
      if (isLatest()) loading.value = false;
    }
  }

  // Re-fetch when the law id OR active traject changes. Switching
  // traject (URL change) re-routes reads through the new traject's
  // backends, so a refresh is needed even if the law id stayed the same.
  watch([lawId, trajectRef], () => {
    fetchScenarios().catch(() => {});
  }, { immediate: true });

  return { scenarios, loading, error, fetchScenarios };
}
