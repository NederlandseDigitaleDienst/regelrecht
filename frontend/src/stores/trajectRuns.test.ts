import { describe, it, expect, vi, beforeEach, afterEach } from 'vitest';
import { createPinia, setActivePinia } from 'pinia';
import { makeRun } from '../test-utils/runFixtures';

const apiFetchJson = vi.fn();
vi.mock('../lib/apiFetch', () => ({ apiFetchJson: (...a: unknown[]) => apiFetchJson(...a) }));

async function store() {
  const { useTrajectRunsStore } = await import('./trajectRuns');
  return useTrajectRunsStore();
}

describe('trajectRuns store', () => {
  beforeEach(() => {
    vi.useFakeTimers();
    setActivePinia(createPinia());
    apiFetchJson.mockReset();
  });
  afterEach(() => {
    vi.useRealTimers();
  });

  it('loads the runs of the traject it follows', async () => {
    apiFetchJson.mockResolvedValue({ runs: [makeRun()] });
    const s = await store();
    await s.follow('zorgtoeslag-abcd1234');
    expect(apiFetchJson).toHaveBeenCalledWith('/api/trajects/zorgtoeslag-abcd1234/runs');
    expect(s.runs).toHaveLength(1);
    expect(s.loaded).toBe(true);
    expect(s.byId('j1')?.job_id).toBe('j1');
    expect(s.byId('nope')).toBeNull();
    s.stop();
  });

  it('polls every 10 seconds while something runs, and every minute otherwise', async () => {
    apiFetchJson.mockResolvedValue({ runs: [makeRun({ status: 'processing' })] });
    const s = await store();
    await s.follow('t-abcd1234');
    expect(apiFetchJson).toHaveBeenCalledTimes(1);

    apiFetchJson.mockResolvedValue({ runs: [makeRun()] });
    await vi.advanceTimersByTimeAsync(10_000);
    expect(apiFetchJson).toHaveBeenCalledTimes(2);

    // Nothing runs any more: the next poll waits a minute.
    await vi.advanceTimersByTimeAsync(30_000);
    expect(apiFetchJson).toHaveBeenCalledTimes(2);
    await vi.advanceTimersByTimeAsync(30_000);
    expect(apiFetchJson).toHaveBeenCalledTimes(3);
    s.stop();
  });

  it('stops polling on stop()', async () => {
    apiFetchJson.mockResolvedValue({ runs: [makeRun({ status: 'processing' })] });
    const s = await store();
    await s.follow('t-abcd1234');
    s.stop();
    await vi.advanceTimersByTimeAsync(120_000);
    expect(apiFetchJson).toHaveBeenCalledTimes(1);
  });

  // Leaving the view right after opening it: the first load is still in
  // flight when the pane stops following, and must not start a poll nobody
  // will stop.
  it('starts no poll when stopped while a load is in flight', async () => {
    let answer: (v: unknown) => void = () => {};
    apiFetchJson.mockReturnValueOnce(new Promise((r) => (answer = r)));
    const s = await store();
    const loading = s.follow('t-abcd1234');
    s.stop();
    answer({ runs: [makeRun({ status: 'processing' })] });
    await loading;
    await vi.advanceTimersByTimeAsync(120_000);
    expect(apiFetchJson).toHaveBeenCalledTimes(1);
  });

  it('starts empty for another traject and drops a late answer for the old one', async () => {
    let answerOld: (v: unknown) => void = () => {};
    apiFetchJson.mockReturnValueOnce(new Promise((r) => (answerOld = r)));
    const s = await store();
    const first = s.follow('oud-abcd1234');

    apiFetchJson.mockResolvedValueOnce({ runs: [makeRun({ job_id: 'new' })] });
    await s.follow('nieuw-ffff0000');
    answerOld({ runs: [makeRun({ job_id: 'old' })] });
    await first;

    expect(s.runs.map((r) => r.job_id)).toEqual(['new']);
    s.stop();
  });

  it('keeps the last list when a poll fails', async () => {
    apiFetchJson.mockResolvedValueOnce({ runs: [makeRun()] });
    const s = await store();
    await s.follow('t-abcd1234');
    apiFetchJson.mockRejectedValueOnce(new Error('boom'));
    await s.refresh();
    expect(s.runs).toHaveLength(1);
    // Our own words, not the server's or the browser's.
    expect(s.error).toBe('Verwerkingen laden mislukt.');
    s.stop();
  });
});
