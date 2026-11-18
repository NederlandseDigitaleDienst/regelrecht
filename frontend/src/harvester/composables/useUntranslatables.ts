import { ref, reactive, computed } from 'vue';
import { usePollingFetch } from './usePollingFetch';
import { UNTRANSLATABLE_SORT_KEYS, type SortOrder } from '../constants';

/** An untranslatable as the API returns it (`Untranslatable` in packages/admin/src/models.rs). */
export type Untranslatable = {
  id: string;
  law_id: string;
  /** From a LEFT JOIN on `law_entries`, so it may be missing. */
  law_name: string | null;
  enrich_job_id: string;
  provider: string;
  article: string;
  construct: string;
  reason: string;
  suggestion: string | null;
  legal_text_excerpt: string | null;
  accepted: boolean;
  created_at: string;
};

// Mirrors useLawEntries: server-side paginated/sorted/filtered list, polled.
// Reads GET /api/harvest-admin/untranslatables (editor-api proxies to the
// harvester-admin service).
export function useUntranslatables() {
  const sort = ref<string>('created_at');
  const order = ref<SortOrder>('desc');
  const limit = ref(50);
  const offset = ref(0);
  const filters = reactive<Record<string, string>>({});

  function buildUrl() {
    const params = new URLSearchParams();
    if (UNTRANSLATABLE_SORT_KEYS.has(sort.value)) {
      params.set('sort', sort.value);
    }
    params.set('order', order.value === 'asc' ? 'asc' : 'desc');
    params.set('limit', String(limit.value));
    params.set('offset', String(offset.value));
    for (const [key, value] of Object.entries(filters)) {
      if (value) params.set(key, value);
    }
    return `/api/harvest-admin/untranslatables?${params.toString()}`;
  }

  const { data, totalCount, loading, error, refresh, startPolling, stopPolling } =
    usePollingFetch<Untranslatable>(buildUrl);

  function setSort(key: string, newOrder?: string) {
    if (!UNTRANSLATABLE_SORT_KEYS.has(key)) return;
    if (newOrder === 'asc' || newOrder === 'desc') {
      sort.value = key;
      order.value = newOrder;
    } else if (sort.value === key) {
      order.value = order.value === 'asc' ? 'desc' : 'asc';
    } else {
      sort.value = key;
      order.value = 'desc';
    }
    offset.value = 0;
    refresh();
  }

  function setFilter(key: string, value: string) {
    if (value) {
      filters[key] = value;
    } else {
      delete filters[key];
    }
    offset.value = 0;
    refresh();
  }

  function goToPage(page: number) {
    const maxPage = Math.max(1, Math.ceil(totalCount.value / limit.value));
    const clamped = Math.max(1, Math.min(page, maxPage));
    offset.value = (clamped - 1) * limit.value;
    refresh();
  }

  const currentPage = computed(() => Math.floor(offset.value / limit.value) + 1);
  const totalPages = computed(() => Math.max(1, Math.ceil(totalCount.value / limit.value)));

  // Initial load + start polling
  refresh();
  startPolling();

  return {
    data, totalCount, loading, error,
    sort, order, limit, offset, filters,
    currentPage, totalPages,
    setSort, setFilter, goToPage,
    refresh, startPolling, stopPolling,
  };
}
