import { ref, reactive, computed } from 'vue';
import { usePollingFetch } from './usePollingFetch';
import { JOB_SORT_KEYS, GROUPED_SORT_KEYS, type JobStatus, type SortOrder } from '../constants';

/** A job row (`Job` in packages/pipeline/src/models.rs). */
export type Job = {
  id: string;
  /** The `job_type` enum's wire name (see `JOB_TYPES`). */
  job_type: string;
  law_id: string;
  status: JobStatus;
  priority: number;
  /** Free-form JSON (`serde_json::Value`). */
  payload: unknown;
  /** Free-form JSON (`serde_json::Value`); a failed job carries its `error`. */
  result: JobResult | null;
  /** Free-form JSON (`serde_json::Value`). */
  progress: unknown;
  attempts: number;
  max_attempts: number;
  created_at: string;
  updated_at: string;
  started_at: string | null;
  completed_at: string | null;
  scheduled_at: string | null;
};

/** The fields the dashboard reads from a job's free-form `result`. */
export type JobResult = {
  error?: string;
  [key: string]: unknown;
};

/** One row of the law-grouped view (`JobSummary` in packages/admin/src/handlers.rs). */
export type JobSummary = {
  law_id: string;
  total_jobs: number;
  pending: number;
  processing: number;
  completed: number;
  failed: number;
  latest_created_at: string;
};

export type JobsViewMode = 'flat' | 'grouped';

export interface JobsOptions {
  /** Taken from the URL as is; anything other than 'flat' means grouped. */
  initialViewMode?: unknown;
}

export function useJobs(options: JobsOptions = {}) {
  const initialView: JobsViewMode = options.initialViewMode === 'flat' ? 'flat' : 'grouped';
  const sort = ref<string>(initialView === 'flat' ? 'created_at' : 'latest_created_at');
  const order = ref<SortOrder>('desc');
  const limit = ref(50);
  const offset = ref(0);
  const filters = reactive<Record<string, string>>({});
  const viewMode = ref<JobsViewMode>(initialView);

  function buildUrl() {
    const allowedKeys = viewMode.value === 'grouped' ? GROUPED_SORT_KEYS : JOB_SORT_KEYS;
    const params = new URLSearchParams();
    if (allowedKeys.has(sort.value)) {
      params.set('sort', sort.value);
    }
    params.set('order', order.value === 'asc' ? 'asc' : 'desc');
    params.set('limit', String(limit.value));
    params.set('offset', String(offset.value));
    for (const [key, value] of Object.entries(filters)) {
      if (value) params.set(key, value);
    }
    const endpoint =
      viewMode.value === 'grouped'
        ? '/api/harvest-admin/jobs/summary'
        : '/api/harvest-admin/jobs';
    return `${endpoint}?${params.toString()}`;
  }

  const { data, totalCount, loading, error, refresh: rawRefresh, startPolling, stopPolling, reset } =
    // Flat rows are jobs, grouped rows are per-law summaries.
    usePollingFetch<Job | JobSummary>(buildUrl);

  async function refresh() {
    await rawRefresh();
  }

  async function toggleViewMode() {
    viewMode.value = viewMode.value === 'grouped' ? 'flat' : 'grouped';
    offset.value = 0;
    sort.value = viewMode.value === 'grouped' ? 'latest_created_at' : 'created_at';
    order.value = 'desc';
    if (viewMode.value === 'grouped') {
      delete filters.law_id;
    }
    reset();
    await refresh();
  }

  function setSort(key: string, newOrder?: string) {
    const allowedKeys = viewMode.value === 'grouped' ? GROUPED_SORT_KEYS : JOB_SORT_KEYS;
    if (!allowedKeys.has(key)) return;
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

  function setLawIdFilter(lawId: string) {
    for (const key of Object.keys(filters)) delete filters[key];
    if (viewMode.value === 'flat') {
      filters.law_id = lawId;
    }
    offset.value = 0;
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
    viewMode,
    currentPage, totalPages,
    setSort, setFilter, goToPage,
    toggleViewMode, setLawIdFilter,
    refresh, startPolling, stopPolling,
  };
}
