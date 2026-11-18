import { ref, onUnmounted } from 'vue';
import { apiFetch } from '@regelrecht/frontend-shared';

// The response of GET /api/harvest-admin/dashboard-stats, after the Serialize
// structs in packages/admin/src/handlers.rs (`DashboardStats` and friends).

/** Job counts per status (`StatusCounts`). */
export type StatusCounts = {
  pending: number;
  processing: number;
  completed: number;
  failed: number;
};

/** Per-type counts in a window, flattened next to `total` (`WindowCounts`). */
export type WindowCounts = {
  total: number;
  [jobType: string]: number;
};

/** One failed job for the "recent failures" list (`RecentFailure`). */
export type RecentFailure = {
  id: string;
  law_id: string;
  job_type: string;
  failed_at: string;
  error: string;
};

/** Per-day counts for one job type (`DailyCounts`). */
export type DailyCounts = {
  added: number;
  succeeded: number;
  failed: number;
};

/** One calendar day, with the per-type counts flattened next to `date` (`DailyEntry`). */
export type DailyEntry = {
  date: string;
  [jobType: string]: DailyCounts | string;
};

export type DashboardStats = {
  jobs: {
    total: number;
    by_type: Record<string, number>;
    by_status: StatusCounts;
    by_type_status: Record<string, StatusCounts>;
  };
  executed: {
    today: WindowCounts;
    last_7d: WindowCounts;
  };
  open_untranslatables: number;
  recent_failures: RecentFailure[];
  /** Absent while an older API is deployed. */
  daily?: DailyEntry[];
};

export interface DashboardStatsOptions {
  interval?: number;
}

// Polls the harvester dashboard-stats endpoint. Mirrors usePollingFetch's
// semantics (loading only on first load, keep stale data on poll failures, 401
// handled by the global apiAuthGuard) but returns a single stats object instead
// of the `{ data: [], total }` paginated shape usePollingFetch is built around.
export function useDashboardStats(options: DashboardStatsOptions = {}) {
  const { interval = 20_000 } = options;

  const stats = ref<DashboardStats | null>(null);
  const loading = ref(true);
  const error = ref<string | null>(null);

  let timer: ReturnType<typeof setInterval> | null = null;
  let initialLoad = true;

  async function refresh() {
    if (initialLoad) loading.value = true;
    try {
      const response = await apiFetch('/api/harvest-admin/dashboard-stats', {
        allowStatuses: [401],
      });
      // 401 → redirect handled globally; return before flashing an error.
      if (response.status === 401) return;
      stats.value = (await response.json()) as DashboardStats;
      error.value = null;
    } catch (err) {
      console.error('Failed to fetch dashboard stats:', err);
      error.value = (err as Error).message;
      // Keep the last good stats on poll failures.
    } finally {
      loading.value = false;
      initialLoad = false;
    }
  }

  function startPolling() {
    stopPolling();
    timer = setInterval(refresh, interval);
  }

  function stopPolling() {
    if (timer) {
      clearInterval(timer);
      timer = null;
    }
  }

  onUnmounted(stopPolling);

  // Initial load + start polling.
  refresh();
  startPolling();

  return { stats, loading, error, refresh, startPolling, stopPolling };
}
