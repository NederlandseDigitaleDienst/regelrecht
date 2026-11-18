/**
 * useBwbSearch - search for laws on external sources (wetten.overheid.nl).
 *
 * Provides debounced search and result management. Results come from the
 * pipeline-api (proxied through editor-api at /api/harvest/search).
 */
import { ref } from 'vue';
import { apiFetchJson } from '../lib/apiFetch';

/** `BwbSearchResult` in packages/pipeline/src/api/bwb_search.rs. */
export interface BwbSearchResult {
  bwb_id: string;
  title: string;
  type: string;
}

const DEBOUNCE_MS = 400;
export const MIN_QUERY_LENGTH = 3;

export function useBwbSearch() {
  const results = ref<BwbSearchResult[]>([]);
  const loading = ref(false);

  let debounceTimer: ReturnType<typeof setTimeout> | undefined;

  /**
   * Search for laws matching the query. Debounced - safe to call on every keystroke.
   * @param {string} query - The search query (minimum 3 characters)
   */
  function search(query: string | null | undefined): void {
    clearTimeout(debounceTimer);
    const q = (query || '').trim();

    if (q.length < MIN_QUERY_LENGTH) {
      results.value = [];
      return;
    }

    debounceTimer = setTimeout(async () => {
      loading.value = true;
      try {
        results.value = await apiFetchJson<BwbSearchResult[]>(
          `/api/harvest/search?q=${encodeURIComponent(q)}`,
        );
      } catch {
        // BWB search is best-effort - clear stale results on any failure
        // (HTTP error or network).
        results.value = [];
      } finally {
        loading.value = false;
      }
    }, DEBOUNCE_MS);
  }

  /** Clear all search results and cancel pending searches. */
  function clear() {
    clearTimeout(debounceTimer);
    results.value = [];
    loading.value = false;
  }

  return { results, loading, search, clear };
}
