/**
 * The runs of the traject the Verwerkingen view shows, as a store: the list in
 * the secondary sidebar and the details in main read the same runs, and only
 * one of them should fetch.
 *
 * The list pane drives it: `follow(ref)` loads the runs and keeps them current
 * while it is mounted. That pane is rendered whenever the view is, also on a
 * narrow screen where only main is visible, so the details never go stale.
 *
 * Polling is cheap but not free: every 10 seconds while something runs, every
 * minute otherwise, because a member can start a run elsewhere and it should
 * show up without a reload.
 */
import { computed, ref } from 'vue';
import { defineStore } from 'pinia';
import { apiFetchJson } from '../lib/apiFetch';
import { isActive } from '../lib/trajectRuns';
import type { TrajectRun, TrajectRunList } from '../lib/trajectRuns';

const ACTIVE_POLL_MS = 10_000;
const IDLE_POLL_MS = 60_000;

export function runsUrl(trajectRef: string): string {
  return `/api/trajects/${encodeURIComponent(trajectRef)}/runs`;
}

export const useTrajectRunsStore = defineStore('trajectRuns', () => {
  const trajectRef = ref<string | null>(null);
  const runs = ref<TrajectRun[]>([]);
  const loaded = ref(false);
  const error = ref<string | null>(null);

  const hasActive = computed(() => runs.value.some(isActive));

  // Bumped on every load; a response for an older load (another traject, or
  // overtaken by a newer poll) is dropped.
  let generation = 0;
  let timer: ReturnType<typeof setTimeout> | null = null;
  // Whether a pane is following. A load still in flight when the pane goes
  // away must not schedule the next poll: nobody would stop that one.
  let following = false;

  async function refresh(): Promise<void> {
    const current = trajectRef.value;
    if (!current) return;
    const mine = ++generation;
    try {
      const body = await apiFetchJson<TrajectRunList>(runsUrl(current));
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
    if (!following || !trajectRef.value) return;
    timer = setTimeout(async () => {
      await refresh();
      schedule();
    }, hasActive.value ? ACTIVE_POLL_MS : IDLE_POLL_MS);
  }

  function stopPolling(): void {
    if (timer) clearTimeout(timer);
    timer = null;
  }

  /** Show the runs of `ref` and keep them current until `stop()`. */
  async function follow(ref_: string | null): Promise<void> {
    if (ref_ !== trajectRef.value) {
      trajectRef.value = ref_;
      runs.value = [];
      loaded.value = false;
      error.value = null;
    }
    following = true;
    await refresh();
    schedule();
  }

  function stop(): void {
    following = false;
    stopPolling();
    generation++;
  }

  function byId(jobId: string | null | undefined): TrajectRun | null {
    return (jobId && runs.value.find((r) => r.job_id === jobId)) || null;
  }

  return { trajectRef, runs, loaded, error, hasActive, refresh, follow, stop, byId };
});
