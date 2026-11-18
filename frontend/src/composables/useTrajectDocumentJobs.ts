/**
 * useTrajectDocumentJobs - poll a traject's document-conversion jobs for the
 * werkdocumenten conversion-status block. The endpoint only returns jobs
 * still running - failures surface as job_failed tasks in the taken-sheet,
 * not here.
 *
 * Mirrors the harvester `usePollingFetch` pattern (initial-load-only `loading`,
 * keep-stale on poll failure, auto-stop on unmount) but for the `{ jobs: [...] }`
 * response shape of `GET .../corpus/documents/jobs`. The consumer starts polling
 * when its surface opens and stops when it closes.
 */
import { ref, onUnmounted } from 'vue';
import { apiFetch } from '@regelrecht/frontend-shared';
import { documentJobsUrl, documentJobUrl } from './corpusUrls';
import type { TrajectRefSource } from './useTrajectDocuments';

/** One conversion job (TrajectJobView in pipeline/src/document_convert.rs). */
export interface DocumentJob {
  id: string;
  target_path: string | null;
  status: string;
  error: string | null;
  created_at: string;
}

export type CancelJobResult = { ok: true } | { ok: false; error: string };

const DEFAULT_INTERVAL_MS = 4000;

export function useTrajectDocumentJobs(
  trajectRef: TrajectRefSource,
  { interval = DEFAULT_INTERVAL_MS }: { interval?: number } = {},
) {
  const jobs = ref<DocumentJob[]>([]);
  const loading = ref(false);
  const error = ref<unknown>(null);

  let timer: ReturnType<typeof setInterval> | null = null;
  let initialLoad = true;
  // Guards the async gap in `startPolling`: if the component unmounts during the
  // awaited first refresh, don't arm an interval that nothing would clear.
  let mounted = true;

  async function refresh(): Promise<void> {
    const ref_ = trajectRef.value;
    if (!ref_) {
      jobs.value = [];
      return;
    }
    if (initialLoad) loading.value = true;
    try {
      // 401 handled by the global apiAuthGuard (redirect); return early so we
      // don't flash an error before it fires.
      const res = await apiFetch(documentJobsUrl(ref_), { allowStatuses: [401] });
      if (res.status === 401) return;
      const json = (await res.json()) as { jobs?: DocumentJob[] } | null;
      jobs.value = Array.isArray(json?.jobs) ? json.jobs : [];
      error.value = null;
    } catch (e) {
      // Keep stale jobs on a poll failure; only clear on the very first load.
      if (initialLoad) jobs.value = [];
      error.value = e;
    } finally {
      loading.value = false;
      initialLoad = false;
    }
  }

  async function startPolling(): Promise<void> {
    stopPolling();
    // Fresh start: drop any jobs from a previous traject and re-arm the
    // initial-load flag so `loading` reflects this (re)start, not a stale run.
    reset();
    // Await the first fetch before arming the interval so a rapid
    // close/reopen can't let a stale in-flight response repopulate the list
    // after `reset()` cleared it.
    await refresh();
    // Unmounted during the await → don't arm an orphan interval.
    if (!mounted) return;
    timer = setInterval(refresh, interval);
  }

  function stopPolling(): void {
    if (timer) {
      clearInterval(timer);
      timer = null;
    }
  }

  function reset(): void {
    initialLoad = true;
    jobs.value = [];
    error.value = null;
  }

  // Cancel (kill) a conversion job — e.g. one stuck for hours. DELETE is
  // idempotent server-side, so a double-click is harmless. Refresh so the row
  // disappears immediately.
  //
  // Result-object style like useTrajectDocuments' own mutations (saveCurrent,
  // uploadDocument): apiFetch throws on any non-ok status, and a failed cancel
  // leaves the job running server-side - something the caller has to be able to
  // say out loud, so it must not travel as an unhandled rejection. Deliberately
  // NOT via the `error` ref above: that one belongs to the poll, whose failures
  // are ridden out on purpose (keep-stale), so surfacing it would pop on every
  // transient blip.
  async function cancelJob(jobId: string): Promise<CancelJobResult> {
    const ref_ = trajectRef.value;
    if (!ref_) return { ok: false, error: 'Geen actief traject.' };
    try {
      await apiFetch(documentJobUrl(ref_, jobId), { method: 'DELETE' });
    } catch (e) {
      // Refresh anyway: the job survived, so put its row back rather than
      // leaving the list claiming otherwise.
      await refresh();
      // apiFetch rejects with an Error (ApiError or a network error).
      return { ok: false, error: (e as Error).message || 'Annuleren mislukt.' };
    }
    await refresh();
    return { ok: true };
  }

  onUnmounted(() => {
    mounted = false;
    stopPolling();
  });

  return { jobs, loading, error, refresh, startPolling, stopPolling, reset, cancelJob };
}
