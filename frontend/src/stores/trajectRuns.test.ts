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
    await s.follow({ trajectRef: 'zorgtoeslag-abcd1234' });
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
    await s.follow({ trajectRef: 't-abcd1234' });
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
    await s.follow({ trajectRef: 't-abcd1234' });
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
    const loading = s.follow({ trajectRef: 't-abcd1234' });
    s.stop();
    answer({ runs: [makeRun({ status: 'processing' })] });
    await loading;
    await vi.advanceTimersByTimeAsync(120_000);
    expect(apiFetchJson).toHaveBeenCalledTimes(1);
  });

  it('starts empty for another scope and drops a late answer for the old one', async () => {
    let answerOld: (v: unknown) => void = () => {};
    apiFetchJson.mockReturnValueOnce(new Promise((r) => (answerOld = r)));
    const s = await store();
    const first = s.follow({ trajectRef: 'oud-abcd1234' });

    apiFetchJson.mockResolvedValueOnce({ runs: [makeRun({ job_id: 'new' })] });
    await s.follow({ trajectRef: 'nieuw-ffff0000' });
    answerOld({ runs: [makeRun({ job_id: 'old' })] });
    await first;

    expect(s.runs.map((r) => r.job_id)).toEqual(['new']);
    s.stop();
  });

  it('keeps the last list when a poll fails', async () => {
    apiFetchJson.mockResolvedValueOnce({ runs: [makeRun()] });
    const s = await store();
    await s.follow({ trajectRef: 't-abcd1234' });
    apiFetchJson.mockRejectedValueOnce(new Error('boom'));
    await s.refresh();
    expect(s.runs).toHaveLength(1);
    // Our own words, not the server's or the browser's.
    expect(s.error).toBe('Verwerkingen laden mislukt.');
    s.stop();
  });

  it('loads once on load(), without starting a poll', async () => {
    apiFetchJson.mockResolvedValue({ runs: [makeRun({ status: 'processing' })] });
    const s = await store();
    await s.load({ trajectRef: 't-abcd1234' });
    expect(s.byId('j1')?.job_id).toBe('j1');
    await vi.advanceTimersByTimeAsync(120_000);
    expect(apiFetchJson).toHaveBeenCalledTimes(1);
  });

  it('leaves a followed scope alone when load() asks for another', async () => {
    apiFetchJson.mockResolvedValue({ runs: [makeRun()] });
    const s = await store();
    await s.follow({ trajectRef: 't-abcd1234' });
    await s.load({ trajectRef: 'ander-ffff0000' });
    expect(s.url).toBe('/api/trajects/t-abcd1234/runs');
    expect(apiFetchJson).toHaveBeenCalledTimes(1);
    s.stop();
  });

  it('asks the server for one law\'s runs, with the BWB id for its harvest', async () => {
    apiFetchJson.mockResolvedValue({ runs: [] });
    const s = await store();
    await s.follow({ trajectRef: 't-abcd1234', lawId: 'wet_op_de_zorgtoeslag', bwbId: 'BWBR0018451' });
    expect(apiFetchJson).toHaveBeenCalledWith(
      '/api/trajects/t-abcd1234/runs?law_id=wet_op_de_zorgtoeslag&bwb_id=BWBR0018451',
    );
    // Another law is another scope: it starts empty.
    apiFetchJson.mockResolvedValueOnce({ runs: [makeRun({ job_id: 'other' })] });
    await s.follow({ trajectRef: 't-abcd1234', lawId: 'zorgverzekeringswet' });
    expect(apiFetchJson).toHaveBeenLastCalledWith('/api/trajects/t-abcd1234/runs?law_id=zorgverzekeringswet');
    expect(s.runs.map((r) => r.job_id)).toEqual(['other']);
    s.stop();
  });
});
