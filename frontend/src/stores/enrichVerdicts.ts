/**
 * The verdicts a reviewer has given on the parts of an enrichment, as a store
 * the whole editor shares.
 *
 * An enrichment is reviewed article by article, with navigation in between,
 * and is written in one go once every part has a verdict. So the verdicts
 * outlive any one mount of the editor. sessionStorage keeps them through a
 * reload as well; when it is unavailable (quota, private mode) memory is
 * the only copy, and after a reload the reviewer decides again - nothing has
 * been written yet.
 */
import { computed, ref } from 'vue';
import { defineStore } from 'pinia';
import type { EnrichDecision } from '../composables/useTasks';

/** A recorded verdict on one part of an enrichment. */
export interface Verdict {
  action: EnrichDecision['action'];
  content: string | null;
}

/** Verdicts per task id. */
export type JobVerdicts = Record<string, Verdict>;

const STORAGE_KEY = 'regelrecht.enrich-verdicts';

function loadVerdicts(): Record<string, JobVerdicts> {
  try {
    // Our own sessionStorage entry, written by persist() below.
    return JSON.parse(window.sessionStorage.getItem(STORAGE_KEY) ?? '{}') ?? {};
  } catch {
    return {};
  }
}

export const useEnrichVerdictsStore = defineStore('enrichVerdicts', () => {
  /** `{ [jobId]: { [taskId]: { action, content } } }` */
  const verdicts = ref<Record<string, JobVerdicts>>(loadVerdicts());

  function persist(): void {
    try {
      window.sessionStorage.setItem(STORAGE_KEY, JSON.stringify(verdicts.value));
    } catch {
      // Memory is the source; storage only carries it through a reload.
    }
  }

  /**
   * The verdicts on one enrichment, by task id. A getter that returns a
   * function rather than an action, so a testing Pinia that stubs actions
   * still answers from the state.
   */
  const forJob = computed(() => (jobId: string | null | undefined): JobVerdicts => (jobId && verdicts.value[jobId]) || {});

  function record(
    jobId: string | null | undefined,
    taskId: string | null | undefined,
    action: Verdict['action'],
    content: string | null,
  ): void {
    if (!jobId || !taskId) return;
    verdicts.value = { ...verdicts.value, [jobId]: { ...forJob.value(jobId), [taskId]: { action, content } } };
    persist();
  }

  function clear(jobId: string | null | undefined): void {
    if (!jobId || !verdicts.value[jobId]) return;
    const next = { ...verdicts.value };
    delete next[jobId];
    verdicts.value = next;
    persist();
  }

  return { verdicts, forJob, record, clear };
});
