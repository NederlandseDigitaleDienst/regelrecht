/**
 * useDocumentTaskReview - review-modus voor een job_review-taak met
 * `payload.kind === 'document'` (documentconversie) in de werkdocumenten-
 * sectie.
 *
 * Fetch the task detail, validate it, and provide (a) the proposed markdown
 * and (b) approve/reject actions. Only fetch and resolve: putting the proposed
 * markdown in the documents manager as an unsaved change (and taking it out
 * again on reject) is orchestration that stays in the caller (LibraryView.vue),
 * because this composable does not know the documents manager. The law
 * counterpart, useProposalReview, does both, since it owns the open law.
 */
import { ref } from 'vue';
import { useTaskActions } from './useTasks';
import type { TaskDetail, TaskRecordPayload } from './useTasks';

export function useDocumentTaskReview() {
  // useTaskActions(), not useTasks(): that keeps every (also anonymous)
  // visitor of the werkdocumenten section from starting the shared task
  // list's 30s poll.
  const { fetchTask, resolveTask } = useTaskActions();
  const reviewTask = ref<TaskDetail | null>(null);
  const proposedContent = ref<string | null>(null);
  const loadError = ref<string | null>(null);

  async function loadReview(taskId: string): Promise<void> {
    try {
      const detail = await fetchTask(taskId);
      if (detail.task_type !== 'job_review' || detail.status !== 'open') {
        loadError.value = 'Deze taak is al afgehandeld.';
        return;
      }
      const payload: TaskRecordPayload = detail.payload || {};
      if (payload.kind !== 'document' || !payload.traject_ref || !payload.target_path) {
        loadError.value = 'Geen documentvoorstel gevonden bij deze taak.';
        return;
      }
      const results = detail.results || [];
      const doc = results.find((r) => r.path === payload.target_path) || results[0];
      if (!doc) {
        loadError.value = 'Geen resultaat gevonden bij deze taak.';
        return;
      }
      reviewTask.value = detail;
      proposedContent.value = doc.content;
    } catch (e) {
      loadError.value = 'Taak laden mislukt.';
    }
  }

  async function approveAfterSave(): Promise<void> {
    if (reviewTask.value) await resolveTask(reviewTask.value.id, 'approved');
    reviewTask.value = null;
    proposedContent.value = null;
  }

  async function reject(): Promise<void> {
    if (reviewTask.value) await resolveTask(reviewTask.value.id, 'rejected');
    reviewTask.value = null;
    proposedContent.value = null;
  }

  return { reviewTask, proposedContent, loadError, loadReview, approveAfterSave, reject };
}
