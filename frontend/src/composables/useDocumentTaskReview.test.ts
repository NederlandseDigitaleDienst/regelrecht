import { describe, it, expect, vi, beforeEach } from 'vitest';
import { useDocumentTaskReview } from './useDocumentTaskReview';

const fetchTask = vi.fn();
const resolveTask = vi.fn();
vi.mock('./useTasks', () => ({
  useTaskActions: () => ({ fetchTask: (...a: unknown[]) => fetchTask(...a), resolveTask: (...a: unknown[]) => resolveTask(...a) }),
}));

function openTask(overrides = {}) {
  return {
    id: 't1',
    task_type: 'job_review',
    status: 'open',
    payload: {
      kind: 'document',
      traject_ref: 'mijn-traject-1a2b3c4d',
      target_path: 'bijv-rapport.md',
    },
    results: [{ path: 'bijv-rapport.md', content: '# Rapport\n\nVoorstel.' }],
    ...overrides,
  };
}

describe('useDocumentTaskReview', () => {
  beforeEach(() => {
    fetchTask.mockReset();
    resolveTask.mockReset();
  });

  it('laadt het voorstel van een open document-review-taak', async () => {
    fetchTask.mockResolvedValue(openTask());
    const { reviewTask, proposedContent, loadError, loadReview } = useDocumentTaskReview();
    await loadReview('t1');
    expect(fetchTask).toHaveBeenCalledWith('t1');
    expect(reviewTask.value?.id).toBe('t1');
    expect(proposedContent.value).toBe('# Rapport\n\nVoorstel.');
    expect(loadError.value).toBeNull();
  });

  it('kiest het result-blob op payload.target_path', async () => {
    fetchTask.mockResolvedValue(
      openTask({
        results: [
          { path: 'anders.md', content: 'niet dit' },
          { path: 'bijv-rapport.md', content: 'wel dit' },
        ],
      }),
    );
    const { proposedContent, loadReview } = useDocumentTaskReview();
    await loadReview('t1');
    expect(proposedContent.value).toBe('wel dit');
  });

  it('weigert een taak die al afgehandeld is', async () => {
    fetchTask.mockResolvedValue(openTask({ status: 'resolved' }));
    const { reviewTask, proposedContent, loadError, loadReview } = useDocumentTaskReview();
    await loadReview('t1');
    expect(reviewTask.value).toBeNull();
    expect(proposedContent.value).toBeNull();
    expect(loadError.value).toBe('Deze taak is al afgehandeld.');
  });

  it('weigert een taak van het verkeerde type', async () => {
    fetchTask.mockResolvedValue(openTask({ task_type: 'job_failed' }));
    const { reviewTask, loadError, loadReview } = useDocumentTaskReview();
    await loadReview('t1');
    expect(reviewTask.value).toBeNull();
    expect(loadError.value).toBe('Deze taak is al afgehandeld.');
  });

  it('weigert een taak zonder kind: document in de payload (bijv. een wet-review)', async () => {
    fetchTask.mockResolvedValue(
      openTask({ payload: { traject_ref: 'mijn-traject-1a2b3c4d', law_id: 'test_wet' } }),
    );
    const { reviewTask, loadError, loadReview } = useDocumentTaskReview();
    await loadReview('t1');
    expect(reviewTask.value).toBeNull();
    expect(loadError.value).toBe('Geen documentvoorstel gevonden bij deze taak.');
  });

  it('weigert een taak zonder target_path in de payload', async () => {
    fetchTask.mockResolvedValue(
      openTask({ payload: { kind: 'document', traject_ref: 'mijn-traject-1a2b3c4d' } }),
    );
    const { reviewTask, loadError, loadReview } = useDocumentTaskReview();
    await loadReview('t1');
    expect(reviewTask.value).toBeNull();
    expect(loadError.value).toBe('Geen documentvoorstel gevonden bij deze taak.');
  });

  it('zet loadError wanneer er geen result-blob bij de taak zit', async () => {
    fetchTask.mockResolvedValue(openTask({ results: [] }));
    const { reviewTask, loadError, loadReview } = useDocumentTaskReview();
    await loadReview('t1');
    expect(reviewTask.value).toBeNull();
    expect(loadError.value).toBe('Geen resultaat gevonden bij deze taak.');
  });

  it('zet loadError wanneer fetchTask faalt', async () => {
    fetchTask.mockRejectedValue(new Error('netwerk'));
    const { loadError, loadReview } = useDocumentTaskReview();
    await loadReview('t1');
    expect(loadError.value).toBe('Taak laden mislukt.');
  });

  it('approveAfterSave resolvet als approved en reset de review-state', async () => {
    fetchTask.mockResolvedValue(openTask());
    const { reviewTask, proposedContent, loadReview, approveAfterSave } = useDocumentTaskReview();
    await loadReview('t1');
    await approveAfterSave();
    expect(resolveTask).toHaveBeenCalledWith('t1', 'approved');
    expect(reviewTask.value).toBeNull();
    expect(proposedContent.value).toBeNull();
  });

  it('reject resolvet als rejected en reset de review-state', async () => {
    fetchTask.mockResolvedValue(openTask());
    const { reviewTask, proposedContent, loadReview, reject } = useDocumentTaskReview();
    await loadReview('t1');
    await reject();
    expect(resolveTask).toHaveBeenCalledWith('t1', 'rejected');
    expect(reviewTask.value).toBeNull();
    expect(proposedContent.value).toBeNull();
  });

  it('approveAfterSave/reject zijn een no-op zonder actieve review', async () => {
    const { approveAfterSave, reject } = useDocumentTaskReview();
    await approveAfterSave();
    await reject();
    expect(resolveTask).not.toHaveBeenCalled();
  });
});
