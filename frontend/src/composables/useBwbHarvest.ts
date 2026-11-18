/**
 * useBwbHarvest - request and track harvesting of external laws.
 *
 * Manages the lifecycle of harvest requests: submitting, polling for status,
 * and determining when a law becomes available. Works with the pipeline-api
 * (proxied through editor-api at /api/harvest/*).
 *
 * State is shared at module level so all callers see the same harvest status.
 */
import { ref, computed } from 'vue';
import { apiFetch, apiFetchJson } from '../lib/apiFetch';

/**
 * One law's harvest state, as the pipeline-api returns it
 * (`HarvestResponse` / `HarvestStatusEntry` in packages/pipeline/src/api/).
 */
export interface HarvestEntry {
  bwb_id: string;
  status: string;
  slug?: string;
}

/** `HarvestBatchResponse` and `HarvestStatusResponse`: a list of entries. */
export interface HarvestResults {
  results: HarvestEntry[];
}

const TERMINAL_STATUSES = new Set<string>([
  'harvest_failed', 'harvest_exhausted', 'enrich_failed', 'enrich_exhausted', 'error', 'timeout', 'unauthorized',
]);
const AVAILABLE_STATUSES = new Set<string>(['harvested', 'enriched']);
const POLLING_STATUSES = new Set<string>(['queued', 'already_queued', 'harvesting', 'enriching']);

const POLL_INTERVAL_MS = 5000;
const POLL_MAX_MS = 10 * 60 * 1000; // 10 minutes

// Module-level shared state - all callers of useBwbHarvest() share these refs.
/** Status per BWB ID */
const harvestStatus = ref<Record<string, string>>({});
/** Resolved slug per BWB ID */
const harvestSlugs = ref<Record<string, string>>({});

let pollInterval: ReturnType<typeof setInterval> | null = null;
let pollStart: number | null = null;

const hasActiveHarvests = computed(() =>
  Object.values(harvestStatus.value).some(s => POLLING_STATUSES.has(s)),
);

// --- Polling (module-level, singleton) ---

function startPolling() {
  if (pollInterval) return;
  pollStart = Date.now();
  pollInterval = setInterval(pollHarvestStatus, POLL_INTERVAL_MS);
}

function stopPolling() {
  if (pollInterval) {
    clearInterval(pollInterval);
    pollInterval = null;
    pollStart = null;
  }
}

async function pollHarvestStatus() {
  // Timeout check. Entries in 'enriching' are exempt from the poll cap:
  // chunked enrichment of a large law legitimately runs far longer than
  // POLL_MAX_MS (the backend keeps the law 'enriching' across chunk runs),
  // so we must not surface a false "timeout" while the backend is working.
  // Entries stuck in the early statuses (queued/harvesting) still time out.
  if (pollStart && Date.now() - pollStart > POLL_MAX_MS) {
    const updated = { ...harvestStatus.value };
    let changed = false;
    for (const [id, status] of Object.entries(updated)) {
      if (POLLING_STATUSES.has(status) && status !== 'enriching') {
        updated[id] = 'timeout';
        changed = true;
      }
    }
    // Only reassign when something actually flipped to 'timeout': past the
    // cap this branch runs on every tick (pollStart never advances), and an
    // 'enriching' entry can legitimately keep polling for hours — rebuilding
    // the reactive object every 5s would churn watchers for nothing.
    if (changed) harvestStatus.value = updated;
    if (!Object.values(updated).some(s => POLLING_STATUSES.has(s))) {
      stopPolling();
      return;
    }
  }

  const activeIds = Object.entries(harvestStatus.value)
    .filter(([, status]) => POLLING_STATUSES.has(status))
    .map(([id]) => id);

  if (activeIds.length === 0) {
    stopPolling();
    return;
  }

  try {
    const data = await apiFetchJson<HarvestResults>(`/api/harvest/status?ids=${activeIds.join(',')}`);

    const updatedStatus = { ...harvestStatus.value };
    const updatedSlugs = { ...harvestSlugs.value };

    for (const entry of data.results) {
      updatedStatus[entry.bwb_id] = entry.status;
      if (entry.slug) updatedSlugs[entry.bwb_id] = entry.slug;
    }

    harvestStatus.value = updatedStatus;
    harvestSlugs.value = updatedSlugs;

    // Stop polling if no active IDs remain
    const stillActive = Object.values(updatedStatus).some(s => POLLING_STATUSES.has(s));
    if (!stillActive) stopPolling();
  } catch {
    // Poll is best-effort - HTTP errors and network failures alike just
    // wait for the next tick.
  }
}

export function useBwbHarvest() {
  // --- Harvest requests ---

  /**
   * Request harvest for a single law by BWB ID.
   * @param {string} bwbId - The BWB identifier (e.g. "BWBR0018451")
   */
  async function requestHarvest(bwbId: string): Promise<void> {
    harvestStatus.value = { ...harvestStatus.value, [bwbId]: 'loading' };
    try {
      const res = await apiFetch('/api/harvest', {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify({ bwb_id: bwbId }),
        allowStatuses: [401],
      });
      if (res.status === 401) {
        // The global apiAuthGuard (src/lib/apiAuthGuard.js) already redirects to
        // login on a 401, so this branch rarely renders before navigation; kept
        // as a graceful fallback for the brief window before the page unloads.
        harvestStatus.value = { ...harvestStatus.value, [bwbId]: 'unauthorized' };
        return;
      }
      const data: HarvestEntry = await res.json();
      harvestStatus.value = { ...harvestStatus.value, [bwbId]: data.status };
      if (data.status === 'queued' || data.status === 'already_queued') {
        startPolling();
      }
    } catch {
      // Other HTTP errors and network failures collapse to one 'error'
      // status the UI renders as "Fout bij aanvragen".
      harvestStatus.value = { ...harvestStatus.value, [bwbId]: 'error' };
    }
  }

  /**
   * Request harvest for multiple laws by slug (for dependency walker).
   * @param {string[]} lawIds - Law slugs to harvest
   * @returns {Promise<object|null>} Batch response or null on failure
   */
  async function requestHarvestBatch(lawIds: string[]): Promise<HarvestResults | null> {
    try {
      const data = await apiFetchJson<HarvestResults>('/api/harvest/batch', {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify({ law_ids: lawIds }),
      });

      // Populate shared harvestStatus / harvestSlugs and start polling so the
      // dependency walker picks up completion via the usual mechanism.
      const updatedStatus = { ...harvestStatus.value };
      const updatedSlugs = { ...harvestSlugs.value };
      let anyQueued = false;
      for (const entry of data.results ?? []) {
        updatedStatus[entry.bwb_id] = entry.status;
        if (entry.slug) updatedSlugs[entry.bwb_id] = entry.slug;
        if (entry.status === 'queued' || entry.status === 'already_queued') {
          anyQueued = true;
        }
      }
      harvestStatus.value = updatedStatus;
      harvestSlugs.value = updatedSlugs;
      if (anyQueued) startPolling();

      return data;
    } catch {
      // Batch harvest is best-effort
    }
    return null;
  }

  // --- Status helpers ---

  // Callers pass `harvestStatus[id]`, which is undefined for a law never
  // requested; no set contains a missing status.
  type MaybeStatus = string | null | undefined;
  const isTerminal = (s: MaybeStatus) => s != null && TERMINAL_STATUSES.has(s);
  const isAvailable = (s: MaybeStatus) => s != null && AVAILABLE_STATUSES.has(s);
  const isPolling = (s: MaybeStatus) => s != null && POLLING_STATUSES.has(s);

  function statusText(bwbId: string, fallback?: string): string {
    const s = harvestStatus.value[bwbId];
    if (!s) return fallback || '';
    switch (s) {
      case 'loading': return 'Aanvragen...';
      case 'queued':
      case 'already_queued': return 'Harvest aangevraagd';
      case 'harvesting': return 'Wordt opgehaald...';
      case 'enriching': return 'Wordt verwerkt...';
      case 'harvested':
      case 'enriched': return 'Beschikbaar \u2014 klik om te openen';
      case 'harvest_failed':
      case 'harvest_exhausted':
      case 'enrich_failed':
      case 'enrich_exhausted': return 'Ophalen mislukt';
      case 'timeout': return 'Timeout \u2014 probeer later opnieuw';
      case 'unauthorized': return 'Log in om te harvesten';
      case 'error': return 'Fout bij aanvragen';
      default: return fallback || '';
    }
  }

  function statusIcon(bwbId: string): string {
    const s = harvestStatus.value[bwbId];
    if (!s) return 'arrow-down-to-line';
    if (AVAILABLE_STATUSES.has(s)) return 'arrow-right';
    if (s === 'loading' || POLLING_STATUSES.has(s)) return 'arrow-clockwise';
    if (TERMINAL_STATUSES.has(s)) return 'x-circle';
    return 'arrow-down-to-line';
  }

  return {
    harvestStatus,
    harvestSlugs,
    hasActiveHarvests,
    requestHarvest,
    requestHarvestBatch,
    isTerminal,
    isAvailable,
    isPolling,
    statusText,
    statusIcon,
  };
}
