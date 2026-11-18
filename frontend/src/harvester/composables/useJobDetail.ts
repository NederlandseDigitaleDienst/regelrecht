import { ref, onUnmounted } from 'vue';
import { apiFetch } from '@regelrecht/frontend-shared';
import type { Job } from './useJobs';

/**
 * A job as the detail sheet holds it: normally a full `Job`, but the overview
 * falls back to the fields a recent-failure row carries when fetching the full
 * job fails, so only `id` and `status` are guaranteed.
 */
export type DetailJob = Pick<Job, 'id' | 'status'> & Partial<Job>;

export function useJobDetail() {
  const job = ref<DetailJob | null>(null);
  const isOpen = ref(false);

  let progressTimer: ReturnType<typeof setInterval> | null = null;

  function open(initialJob: DetailJob) {
    job.value = { ...initialJob };
    isOpen.value = true;
    stopProgressPoll();
    if (initialJob.status === 'processing') {
      startProgressPoll();
    }
  }

  function close() {
    isOpen.value = false;
    stopProgressPoll();
    setTimeout(() => {
      if (!isOpen.value) job.value = null;
    }, 300);
  }

  function startProgressPoll() {
    stopProgressPoll();
    progressTimer = setInterval(async () => {
      if (!job.value) return;
      try {
        const resp = await apiFetch(
          `/api/harvest-admin/jobs/${encodeURIComponent(job.value.id)}`,
        );
        const updated: Job = await resp.json();
        job.value = updated;
        if (updated.status !== 'processing') {
          stopProgressPoll();
        }
      } catch {
        // ignore fetch errors during polling
      }
    }, 10_000);
  }

  function stopProgressPoll() {
    if (progressTimer) {
      clearInterval(progressTimer);
      progressTimer = null;
    }
  }

  onUnmounted(() => {
    stopProgressPoll();
  });

  return { job, isOpen, open, close };
}
