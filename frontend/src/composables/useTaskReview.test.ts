import { describe, it, expect, vi, beforeEach } from 'vitest';
import { useTaskReview } from './useTaskReview';

const fetchTask = vi.fn();
const resolveTask = vi.fn();
const fetchJobTasks = vi.fn();
const applyEnrichment = vi.fn();
vi.mock('./useTasks', () => ({
  useTaskActions: () => ({
    fetchTask: (...a: unknown[]) => fetchTask(...a),
    resolveTask: (...a: unknown[]) => resolveTask(...a),
    fetchJobTasks: (...a: unknown[]) => fetchJobTasks(...a),
    applyEnrichment: (...a: unknown[]) => applyEnrichment(...a),
  }),
}));

const JOB = 'job-1';

function openTask(overrides = {}) {
  return {
    id: 't1',
    task_type: 'job_review',
    status: 'open',
    job_id: JOB,
    title: 'Verrijking beoordelen: test_wet artikel 1',
    payload: { law_id: 'test_wet', article: '1' },
    results: [{ path: 'corpus/regulation/nl/wet/test_wet/2025-01-01.yaml', content: 'proposed: yaml' }],
    ...overrides,
  };
}

function twoParts() {
  return {
    job_id: JOB,
    law_id: 'test_wet',
    tasks: [
      { id: 't1', article: '1', status: 'open', title: 'artikel 1' },
      { id: 't2', article: '2', status: 'open', title: 'artikel 2' },
    ],
  };
}

describe('useTaskReview', () => {
  beforeEach(() => {
    fetchTask.mockReset();
    resolveTask.mockReset();
    fetchJobTasks.mockReset();
    applyEnrichment.mockReset();
    applyEnrichment.mockResolvedValue({ accepted: 1, total: 2 });
    // The global test setup gives every test a fresh Pinia; the verdicts
    // store reads sessionStorage when it is created.
    window.sessionStorage.clear();
  });

  it('laadt het voorstel en de andere onderdelen van dezelfde verrijking', async () => {
    fetchTask.mockResolvedValue(openTask());
    fetchJobTasks.mockResolvedValue(twoParts());
    const { reviewTask, proposedContent, loadError, jobParts, partIndex, partCount, loadReview } =
      useTaskReview();
    await loadReview('t1');
    expect(fetchTask).toHaveBeenCalledWith('t1');
    expect(fetchJobTasks).toHaveBeenCalledWith(JOB);
    expect(reviewTask.value?.id).toBe('t1');
    expect(proposedContent.value).toBe('proposed: yaml');
    expect(jobParts.value).toHaveLength(2);
    expect(partIndex.value).toBe(1);
    expect(partCount.value).toBe(2);
    expect(loadError.value).toBeNull();
  });

  it('valt terug op de taak zelf als de onderdelen niet op te halen zijn', async () => {
    fetchTask.mockResolvedValue(openTask());
    fetchJobTasks.mockRejectedValue(new Error('netwerk'));
    const { jobParts, partCount, loadReview } = useTaskReview();
    await loadReview('t1');
    expect(jobParts.value).toEqual([
      { id: 't1', article: '1', status: 'open', title: 'Verrijking beoordelen: test_wet artikel 1' },
    ]);
    expect(partCount.value).toBe(1);
  });

  it('weigert een taak die al afgehandeld is', async () => {
    fetchTask.mockResolvedValue(openTask({ status: 'resolved' }));
    const { reviewTask, proposedContent, loadError, loadReview } = useTaskReview();
    await loadReview('t1');
    expect(reviewTask.value).toBeNull();
    expect(proposedContent.value).toBeNull();
    expect(loadError.value).toBe('Deze taak is al afgehandeld.');
  });

  it('weigert een taak van het verkeerde type', async () => {
    fetchTask.mockResolvedValue(openTask({ task_type: 'job_failed' }));
    const { reviewTask, loadError, loadReview } = useTaskReview();
    await loadReview('t1');
    expect(reviewTask.value).toBeNull();
    expect(loadError.value).toBe('Deze taak is al afgehandeld.');
  });

  it('zet loadError wanneer er geen law-resultaat bij de taak zit', async () => {
    fetchTask.mockResolvedValue(openTask({ results: [{ path: '.enrichment.yaml', content: 'x' }] }));
    const { reviewTask, loadError, loadReview } = useTaskReview();
    await loadReview('t1');
    expect(reviewTask.value).toBeNull();
    expect(loadError.value).toBe('Geen resultaat gevonden bij deze taak.');
  });

  it('zet loadError wanneer de taak geen law_id in de payload heeft', async () => {
    fetchTask.mockResolvedValue(openTask({ payload: {} }));
    const { reviewTask, loadError, loadReview } = useTaskReview();
    await loadReview('t1');
    expect(reviewTask.value).toBeNull();
    expect(loadError.value).toBe('Geen resultaat gevonden bij deze taak.');
  });

  it('kiest het result-bestand op payload.yaml_path, niet de eerste dot-loze file', async () => {
    fetchTask.mockResolvedValue(
      openTask({
        payload: { law_id: 'test_wet', yaml_path: 'laws/w/law.yaml' },
        results: [
          { path: 'features/x.feature', content: 'Feature: ...' },
          { path: 'laws/w/law.yaml', content: 'wet: ja' },
        ],
      })
    );
    fetchJobTasks.mockResolvedValue(twoParts());
    const { proposedContent, loadReview } = useTaskReview();
    await loadReview('t1');
    expect(proposedContent.value).toBe('wet: ja');
  });

  it('zet loadError wanneer de taak geen payload heeft', async () => {
    fetchTask.mockResolvedValue(openTask({ payload: undefined }));
    const { reviewTask, loadError, loadReview } = useTaskReview();
    await loadReview('t1');
    expect(reviewTask.value).toBeNull();
    expect(loadError.value).toBe('Geen resultaat gevonden bij deze taak.');
  });

  it('zet loadError wanneer fetchTask faalt', async () => {
    fetchTask.mockRejectedValue(new Error('netwerk'));
    const { loadError, loadReview } = useTaskReview();
    await loadReview('t1');
    expect(loadError.value).toBe('Taak laden mislukt.');
  });

  it('een oordeel over één onderdeel schrijft niets en wijst het volgende aan', async () => {
    fetchTask.mockResolvedValue(openTask());
    fetchJobTasks.mockResolvedValue(twoParts());
    const { loadReview, decide, undecidedParts } = useTaskReview();
    await loadReview('t1');

    const { done, next } = decide('approved', 'number: "1"\n');
    expect(done).toBe(false);
    expect(next).not.toBe(null);
    expect(next?.id).toBe('t2');
    expect(applyEnrichment).not.toHaveBeenCalled();
    expect(undecidedParts.value.map((p) => p.id)).toEqual(['t2']);
  });

  it('verwerkt de verrijking in één keer zodra het laatste onderdeel is beoordeeld', async () => {
    fetchTask.mockResolvedValue(openTask());
    fetchJobTasks.mockResolvedValue(twoParts());
    const first = useTaskReview();
    await first.loadReview('t1');
    expect(first.decide('approved', 'number: "1"\ntext: een\n').done).toBe(false);

    // Het tweede onderdeel wordt als eigen review geladen (nieuwe ?task=).
    fetchTask.mockResolvedValue(
      openTask({ id: 't2', payload: { law_id: 'test_wet', article: '2' } })
    );
    const second = useTaskReview();
    await second.loadReview('t2');
    expect(second.decide('rejected').done).toBe(true);

    await second.processEnrichment('"etag-1"');
    expect(applyEnrichment).toHaveBeenCalledWith(
      JOB,
      [
        { task_id: 't1', action: 'approved', content: 'number: "1"\ntext: een\n' },
        { task_id: 't2', action: 'rejected' },
      ],
      '"etag-1"'
    );
    expect(second.reviewTask.value).toBeNull();
  });

  it('rondt af met de rest op niet overnemen', async () => {
    fetchTask.mockResolvedValue(openTask());
    fetchJobTasks.mockResolvedValue(twoParts());
    const { loadReview, decide, processEnrichment } = useTaskReview();
    await loadReview('t1');
    decide('approved', 'number: "1"\n');

    await processEnrichment('"etag-1"', { rejectRemaining: true });
    expect(applyEnrichment).toHaveBeenCalledWith(
      JOB,
      [
        { task_id: 't1', action: 'approved', content: 'number: "1"\n' },
        { task_id: 't2', action: 'rejected' },
      ],
      '"etag-1"'
    );
  });

  it('laat onbeoordeelde onderdelen weg zolang je niet afrondt', async () => {
    fetchTask.mockResolvedValue(openTask());
    fetchJobTasks.mockResolvedValue(twoParts());
    const { loadReview, decide, processEnrichment } = useTaskReview();
    await loadReview('t1');
    decide('rejected');

    await processEnrichment('"etag-1"');
    expect(applyEnrichment).toHaveBeenCalledWith(
      JOB,
      [{ task_id: 't1', action: 'rejected' }],
      '"etag-1"'
    );
  });

  it('houdt de oordelen vast als het verwerken faalt', async () => {
    fetchTask.mockResolvedValue(openTask());
    fetchJobTasks.mockResolvedValue(twoParts());
    applyEnrichment.mockRejectedValue(new Error('412'));
    const { loadReview, decide, processEnrichment, reviewTask, undecidedParts } = useTaskReview();
    await loadReview('t1');
    decide('approved', 'number: "1"\n');

    await expect(processEnrichment('"oud"', { rejectRemaining: true })).rejects.toThrow('412');
    // Niets geschreven, dus de review blijft staan met het vastgelegde oordeel.
    expect(reviewTask.value?.id).toBe('t1');
    expect(undecidedParts.value.map((p) => p.id)).toEqual(['t2']);
  });

  it('approveAfterSave/reject handelen één taak af (het law_create-pad)', async () => {
    fetchTask.mockResolvedValue(
      openTask({ payload: { law_id: 'test_wet', kind: 'law_create' }, job_id: null })
    );
    const a = useTaskReview();
    await a.loadReview('t1');
    await a.approveAfterSave();
    expect(resolveTask).toHaveBeenCalledWith('t1', 'approved');
    expect(a.reviewTask.value).toBeNull();

    const b = useTaskReview();
    await b.loadReview('t1');
    await b.reject();
    expect(resolveTask).toHaveBeenCalledWith('t1', 'rejected');
    expect(b.reviewTask.value).toBeNull();
  });

  it('decide/approveAfterSave/reject zijn een no-op zonder actieve review', async () => {
    const { approveAfterSave, reject, decide, processEnrichment } = useTaskReview();
    expect(decide('approved')).toEqual({ done: false, next: null });
    await approveAfterSave();
    await reject();
    expect(await processEnrichment('"etag"')).toBeNull();
    expect(resolveTask).not.toHaveBeenCalled();
    expect(applyEnrichment).not.toHaveBeenCalled();
  });

  it('does not post an apply for a task without a job, and keeps its verdict', async () => {
    fetchTask.mockResolvedValue(openTask({ job_id: null }));
    const { loadReview, decide, processEnrichment, reviewTask, undecidedParts } = useTaskReview();
    await loadReview('t1');
    decide('approved', 'number: "1"\n');

    await expect(processEnrichment('"etag-1"')).rejects.toThrow(/geen verrijking/);
    expect(applyEnrichment).not.toHaveBeenCalled();
    // Nothing was written, so the review stays open with its recorded verdict.
    expect(reviewTask.value?.id).toBe('t1');
    expect(undecidedParts.value).toEqual([]);
  });

  it('releases a task without a job when it is rejected, without posting an apply', async () => {
    fetchTask.mockResolvedValue(openTask({ job_id: null }));
    const { loadReview, decide, processEnrichment, reviewTask } = useTaskReview();
    await loadReview('t1');
    decide('rejected', null);

    await expect(processEnrichment('"etag-1"')).resolves.toBeNull();
    expect(applyEnrichment).not.toHaveBeenCalled();
    expect(resolveTask).toHaveBeenCalledWith('t1', 'rejected');
    expect(reviewTask.value).toBeNull();
  });

  it('reset ruimt ook loadError op, niet alleen de taak', async () => {
    fetchTask.mockResolvedValue(openTask());
    fetchJobTasks.mockResolvedValue(twoParts());
    const { reviewTask, proposedContent, jobParts, loadReview, reset } = useTaskReview();
    await loadReview('t1');
    expect(reviewTask.value).not.toBeNull();

    reset();

    expect(reviewTask.value).toBeNull();
    expect(proposedContent.value).toBeNull();
    expect(jobParts.value).toEqual([]);
    // Puur lokaal: de taak zelf blijft open, reset is geen afhandeling.
    expect(resolveTask).not.toHaveBeenCalled();
  });

  // Vanuit de editor kun je via de takenlijst rechtstreeks van de ene taak naar
  // de andere - `?task=` gaat dan van A naar B zonder tussenstop op "geen taak",
  // dus zonder dat er iets `reset()` aanroept. Een nieuwe load hoort daarom zelf
  // schoon te beginnen; anders staat de foutmelding van A boven het voorstel
  // van B (en kleurt de balk kritiek terwijl er niets mis is).
  it('begint schoon bij een volgende load, ook zonder reset ertussen', async () => {
    fetchTask.mockResolvedValueOnce(openTask({ status: 'resolved' }));
    const { reviewTask, proposedContent, loadError, loadReview } = useTaskReview();
    await loadReview('t1');
    expect(loadError.value).toBe('Deze taak is al afgehandeld.');

    fetchTask.mockResolvedValueOnce(openTask({ id: 't2' }));
    await loadReview('t2');

    expect(reviewTask.value?.id).toBe('t2');
    expect(proposedContent.value).toBe('proposed: yaml');
    expect(loadError.value).toBeNull();
  });

  it('reset laat een foutmelding van een mislukte load niet blijven staan', async () => {
    fetchTask.mockRejectedValue(new Error('netwerk'));
    const { loadError, loadReview, reset } = useTaskReview();
    await loadReview('t1');
    expect(loadError.value).toBe('Taak laden mislukt.');

    reset();

    expect(loadError.value).toBeNull();
  });
});
