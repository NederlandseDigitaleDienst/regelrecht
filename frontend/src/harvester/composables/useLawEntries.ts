import { ref, reactive, computed } from 'vue';
import { usePollingFetch } from './usePollingFetch';
import { LAW_ENTRY_SORT_KEYS, type LawStatus, type SortOrder } from '../constants';

/** A law row (`LawEntry` in packages/pipeline/src/models.rs). */
export type LawEntry = {
  law_id: string;
  law_name: string | null;
  slug: string | null;
  status: LawStatus;
  harvest_job_id: string | null;
  enrich_job_id: string | null;
  /** Fraction (0-1) of articles that received a `machine_readable` section. */
  coverage_score: number | null;
  harvest_fail_count: number;
  enrich_fail_count: number;
  created_at: string;
  updated_at: string;
};

export function useLawEntries() {
  const sort = ref<string>('updated_at');
  const order = ref<SortOrder>('desc');
  const limit = ref(50);
  const offset = ref(0);
  const filters = reactive<Record<string, string>>({});

  function buildUrl() {
    const params = new URLSearchParams();
    if (LAW_ENTRY_SORT_KEYS.has(sort.value)) {
      params.set('sort', sort.value);
    }
    params.set('order', order.value === 'asc' ? 'asc' : 'desc');
    params.set('limit', String(limit.value));
    params.set('offset', String(offset.value));
    for (const [key, value] of Object.entries(filters)) {
      if (value) params.set(key, value);
    }
    return `/api/harvest-admin/law_entries?${params.toString()}`;
  }

  const { data, totalCount, loading, error, refresh, startPolling, stopPolling } =
    usePollingFetch<LawEntry>(buildUrl);

  function setSort(key: string, newOrder?: string) {
    if (!LAW_ENTRY_SORT_KEYS.has(key)) return;
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
