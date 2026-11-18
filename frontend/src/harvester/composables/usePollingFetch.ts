import { ref, onUnmounted, type Ref } from 'vue';
import { apiFetch } from '@regelrecht/frontend-shared';

/** The paginated list envelope (`PaginatedResponse<T>` in packages/admin/src/models.rs). */
export type PaginatedResponse<T> = {
  data: T[];
  total: number;
  limit: number;
  offset: number;
};

export interface PollingFetchOptions {
  interval?: number;
}

/** Builds the URL to fetch; a falsy result skips the fetch. */
export type UrlBuilder = () => string | null | undefined;

export function usePollingFetch<T>(buildUrl: UrlBuilder, options: PollingFetchOptions = {}) {
  const { interval = 20_000 } = options;

  // Cast: `ref<T[]>` would unwrap T into `UnwrapRef<T>`; rows are plain API data.
  const data = ref([]) as Ref<T[]>;
  const totalCount = ref(0);
  const loading = ref(true);
  const error = ref<string | null>(null);

  let timer: ReturnType<typeof setInterval> | null = null;
  let initialLoad = true;

  async function fetchData() {
    const url = buildUrl();
    if (!url) return;

    // Only show loading state on initial load, not on poll refreshes
    if (initialLoad) loading.value = true;

    try {
      // 401 is handled by the editor's global apiAuthGuard (redirect to
      // login); return early on it so we don't flash an error state before the
      // redirect - matching the mutation call sites (RowActions etc.). apiFetch
      // throws ApiError on other non-ok statuses, caught below.
      const response = await apiFetch(url, { allowStatuses: [401] });
      if (response.status === 401) return;
      const json: Partial<PaginatedResponse<T>> = await response.json();
      data.value = json.data || [];
      totalCount.value = json.total ?? data.value.length;
      error.value = null;
    } catch (err) {
      console.error('Failed to fetch data:', err);
      // Only clear data on initial load errors; keep stale data on poll failures
      if (initialLoad) {
        data.value = [];
        totalCount.value = 0;
      }
      error.value = (err as Error).message;
    } finally {
      loading.value = false;
      initialLoad = false;
    }
  }

  function startPolling() {
    stopPolling();
    timer = setInterval(fetchData, interval);
  }

  function stopPolling() {
    if (timer) {
      clearInterval(timer);
      timer = null;
    }
  }

  async function refresh() {
    await fetchData();
  }

  function reset() {
    initialLoad = true;
    data.value = [];
    totalCount.value = 0;
    error.value = null;
    loading.value = true;
  }

  onUnmounted(stopPolling);

  return { data, totalCount, loading, error, refresh, startPolling, stopPolling, reset };
}
