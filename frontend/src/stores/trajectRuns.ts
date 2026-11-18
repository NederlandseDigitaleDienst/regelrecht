/**
 * The runs (harvests, enrichments, conversions) of a traject, as a store: the
 * Verwerkingen page of a law lists them and its details sheet reads the same
 * runs, and the details of a task look up the run behind it.
 *
 * What is loaded is a scope: a traject, optionally narrowed to one law. The
 * law's page asks for that law's runs only, which the server filters, so the
 * cap of the unfiltered list (the last 200 runs) cannot hide an older run of
 * the law.
 *
 * The page drives it: `follow(scope)` loads the runs and keeps them current
 * while it is mounted. Polling is cheap but not free: every 10 seconds while
 * something runs, every minute otherwise, because a member can start a run
 * elsewhere and it should show up without a reload.
 */
import { computed, ref } from 'vue';
import { defineStore } from 'pinia';
import { apiFetchJson } from '../lib/apiFetch';
import { isActive } from '../lib/trajectRuns';
import type { TrajectRun, TrajectRunList } from '../lib/trajectRuns';

const ACTIVE_POLL_MS = 10_000;
const IDLE_POLL_MS = 60_000;

/** Which runs to load: those of a traject, or of one law in it. */
export interface RunScope {
  trajectRef: string;
  /** The law's `$id`: only its runs. */
  lawId?: string | null;
  /** The law's BWB id, so the harvest that fetched the law is included too. */
  bwbId?: string | null;
}

export function runsUrl(scope: RunScope): string {
  const base = `/api/trajects/${encodeURIComponent(scope.trajectRef)}/runs`;
  if (!scope.lawId) return base;
  const query = new URLSearchParams({ law_id: scope.lawId });
  if (scope.bwbId) query.set('bwb_id', scope.bwbId);
  return `${base}?${query}`;
}

export const useTrajectRunsStore = defineStore('trajectRuns', () => {
  // The scope loaded, as its URL: two scopes are the same exactly when they
  // ask the server the same thing.
  const url = ref<string | null>(null);
  const runs = ref<TrajectRun[]>([]);
  const loaded = ref(false);
  const error = ref<string | null>(null);

  const hasActive = computed(() => runs.value.some(isActive));

  // Bumped on every load; a response for an older load (another scope, or
  // overtaken by a newer poll) is dropped.
  let generation = 0;
  let timer: ReturnType<typeof setTimeout> | null = null;
  // Whether a page is following. A load still in flight when the page goes
  // away must not schedule the next poll: nobody would stop that one.
  let following = false;

  async function refresh(): Promise<void> {
    const current = url.value;
    if (!current) return;
    const mine = ++generation;
    try {
      const body = await apiFetchJson<TrajectRunList>(current);
      if (mine !== generation) return;
      runs.value = Array.isArray(body?.runs) ? body.runs : [];
      error.value = null;
    } catch {
      if (mine !== generation) return;
      // Keep the last list: a failed poll should not empty the screen. The
      // message is ours: the server's (or the browser's) is English and
      // technical.
      error.value = 'Verwerkingen laden mislukt.';
    } finally {
      if (mine === generation) loaded.value = true;
    }
  }

  function schedule(): void {
    stopPolling();
    if (!following || !url.value) return;
    timer = setTimeout(async () => {
      await refresh();
      schedule();
    }, hasActive.value ? ACTIVE_POLL_MS : IDLE_POLL_MS);
  }

  function stopPolling(): void {
    if (timer) clearTimeout(timer);
    timer = null;
  }

  // Switch to another scope: start empty, so the runs of the previous one
  // never show under the new one.
  function enter(scope: RunScope | null): void {
    const next = scope ? runsUrl(scope) : null;
    if (next === url.value) return;
    url.value = next;
    runs.value = [];
    loaded.value = false;
    error.value = null;
  }

  /** Show the runs of `scope` and keep them current until `stop()`. */
  async function follow(scope: RunScope | null): Promise<void> {
    enter(scope);
    following = true;
    await refresh();
    schedule();
  }

  /**
   * Load the runs of `scope` once, without polling: for a caller that needs
   * the details of one run (the task details sheet) and not a list that stays
   * current. A scope a page follows is left alone; it is current already.
   */
  async function load(scope: RunScope): Promise<void> {
    if (following) return;
    enter(scope);
    await refresh();
  }

  function stop(): void {
    following = false;
    stopPolling();
    generation++;
  }

  function byId(jobId: string | null | undefined): TrajectRun | null {
    return (jobId && runs.value.find((r) => r.job_id === jobId)) || null;
  }

  return { url, runs, loaded, error, hasActive, refresh, follow, load, stop, byId };
});
