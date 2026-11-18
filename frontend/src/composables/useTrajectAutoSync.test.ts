import { describe, it, expect, beforeEach, afterEach, vi } from 'vitest';

const api = vi.hoisted(() => ({
  fetchBaseStatus: vi.fn(),
  syncBase: vi.fn(),
  proposeToBase: vi.fn(),
  invalidate: vi.fn(),
}));

vi.mock('../lib/trajectSync', async (importOriginal) => ({
  ...(await importOriginal<typeof import('../lib/trajectSync')>()),
  fetchBaseStatus: api.fetchBaseStatus,
  syncBase: api.syncBase,
  proposeToBase: api.proposeToBase,
}));
vi.mock('../corpus', () => ({
  useCorpus: () => ({ forTraject: () => ({ invalidate: api.invalidate }) }),
}));

import {
  AUTO_SYNC_MS,
  startAutoSync,
  stopAutoSync,
  useTrajectAutoSync,
} from './useTrajectAutoSync';
import type { BaseStatus } from '../lib/trajectSync';

const REF = 'dossier-1234abcd';
const st = (over: Partial<BaseStatus> = {}): BaseStatus => ({
  repo: 'example-org/regelrecht-corpus-example',
  base_branch: 'werk',
  traject_branch: 'traject/dossier-1234abcd',
  behind_by: 0,
  ahead_by: 0,
  base_files: [],
  traject_files: [],
  overlap: [],
  ...over,
});
const httpError = (status: number, message = 'fout') => Object.assign(new Error(message), { status });
const flush = () => vi.advanceTimersByTimeAsync(0);

beforeEach(() => {
  vi.useFakeTimers();
  Object.values(api).forEach((f) => f.mockReset());
});
afterEach(() => {
  stopAutoSync();
  vi.useRealTimers();
});

describe('useTrajectAutoSync', () => {
  it('brings in working-branch changes on start, invalidating cached laws', async () => {
    api.fetchBaseStatus
      .mockResolvedValueOnce(st({ behind_by: 2, base_files: ['a.yaml'] }))
      .mockResolvedValue(st());
    api.syncBase.mockResolvedValue({ outcome: 'merged', sha: 'abc' });
    const s = useTrajectAutoSync();
    const before = s.syncCount.value;

    startAutoSync(REF);
    await flush();

    expect(api.syncBase).toHaveBeenCalledWith(REF, { auto: true });
    expect(api.invalidate).toHaveBeenCalled();
    expect(s.syncCount.value).toBe(before + 1);
    expect(s.notice.value?.auto).toBe(true);
    expect(s.error.value).toBeNull();
  });

  it('does nothing when only the traject has work of its own', async () => {
    api.fetchBaseStatus.mockResolvedValue(st({ ahead_by: 1, traject_files: ['b.yaml'] }));
    startAutoSync(REF);
    await flush();
    expect(api.syncBase).not.toHaveBeenCalled();
  });

  it('checks again every interval', async () => {
    api.fetchBaseStatus.mockResolvedValue(st());
    startAutoSync(REF);
    await flush();
    expect(api.fetchBaseStatus).toHaveBeenCalledTimes(1);
    await vi.advanceTimersByTimeAsync(AUTO_SYNC_MS);
    expect(api.fetchBaseStatus).toHaveBeenCalledTimes(2);
  });

  it('after a conflict, waits for either branch to change before retrying', async () => {
    const diverged = st({ behind_by: 1, ahead_by: 1, base_files: ['a.yaml'], traject_files: ['a.yaml'] });
    api.fetchBaseStatus.mockResolvedValue(diverged);
    api.syncBase.mockRejectedValue(httpError(409, 'Aan beide kanten gewijzigd: a.yaml'));
    const s = useTrajectAutoSync();

    startAutoSync(REF);
    await flush();
    expect(api.syncBase).toHaveBeenCalledTimes(1);
    expect(s.error.value).toContain('a.yaml');

    // Same divergence: no second attempt.
    await vi.advanceTimersByTimeAsync(AUTO_SYNC_MS);
    expect(api.syncBase).toHaveBeenCalledTimes(1);

    // The working branch moved (e.g. the conflict was resolved in a clone).
    api.fetchBaseStatus.mockResolvedValue({ ...diverged, behind_by: 2 });
    api.syncBase.mockResolvedValue({ outcome: 'merged', sha: 'def' });
    await vi.advanceTimersByTimeAsync(AUTO_SYNC_MS);
    expect(api.syncBase).toHaveBeenCalledTimes(2);
    expect(s.error.value).toBeNull();
  });

  it('stops trying automatically for a member without write access', async () => {
    api.fetchBaseStatus.mockResolvedValue(st({ behind_by: 1, base_files: ['a.yaml'] }));
    api.syncBase.mockRejectedValue(httpError(403));
    const s = useTrajectAutoSync();
    startAutoSync(REF);
    await flush();
    await vi.advanceTimersByTimeAsync(AUTO_SYNC_MS);
    expect(api.syncBase).toHaveBeenCalledTimes(1);
    expect(s.error.value).toBeNull();
  });

  it('stays silent where base-status does not apply (central corpus, no link)', async () => {
    api.fetchBaseStatus.mockRejectedValue(httpError(409));
    const s = useTrajectAutoSync();
    startAutoSync(REF);
    await flush();
    expect(api.syncBase).not.toHaveBeenCalled();
    expect(s.status.value).toBeNull();
    expect(s.error.value).toBeNull();
  });

  it('a manual sync reports when there was nothing to do', async () => {
    api.fetchBaseStatus.mockResolvedValue(st());
    api.syncBase.mockResolvedValue({ outcome: 'up_to_date', sha: null });
    const s = useTrajectAutoSync();
    startAutoSync(REF);
    await flush();
    await s.syncNow();
    expect(api.syncBase).toHaveBeenLastCalledWith(REF, { auto: false });
    expect(s.notice.value?.text).toMatch(/al bij/);
  });
});
