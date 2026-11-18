import { describe, it, expect, beforeEach, vi } from 'vitest';
import {
  type BaseStatus,
  CENTRAL_REPO,
  findingsByLevel,
  needsSyncPrompt,
  preflightRepo,
  syncBase,
  syncDetail,
  syncSummary,
} from './trajectSync';

function jsonRes(body: unknown, status = 200) {
  return {
    ok: status >= 200 && status < 300,
    status,
    headers: { get: (k: string) => (k.toLowerCase() === 'content-type' ? 'application/json' : null) },
    async text() {
      return typeof body === 'string' ? body : JSON.stringify(body);
    },
    async json() {
      return body;
    },
  };
}

const status = (over: Partial<BaseStatus> = {}): BaseStatus => ({
  repo: 'example-org/regelrecht-corpus-example',
  base_branch: 'werk',
  traject_branch: 'traject/x-1234abcd',
  behind_by: 0,
  ahead_by: 0,
  base_files: [],
  traject_files: [],
  overlap: [],
  ...over,
});

beforeEach(() => vi.restoreAllMocks());

describe('needsSyncPrompt', () => {
  it('stays quiet when both branches hold the same files', () => {
    expect(needsSyncPrompt(status())).toBe(false);
  });
  it('stays quiet on commits that change no file (a merge commit after a sync)', () => {
    expect(needsSyncPrompt(status({ behind_by: 1, ahead_by: 1 }))).toBe(false);
  });
  it('prompts when either side changed files the other lacks', () => {
    expect(needsSyncPrompt(status({ behind_by: 2, base_files: ['a.yaml'] }))).toBe(true);
    expect(needsSyncPrompt(status({ ahead_by: 1, traject_files: ['b.yaml'] }))).toBe(true);
  });
  it('never prompts on the central corpus', () => {
    expect(needsSyncPrompt(status({ repo: CENTRAL_REPO, base_files: ['a.yaml'] }))).toBe(false);
  });
});

describe('banner texts', () => {
  it('summarises both directions with Dutch plurals', () => {
    expect(syncSummary(status({ base_files: ['a.yaml'] }))).toBe(
      'Werkbranch werk heeft wijzigingen in 1 bestand die nog niet in dit traject staan',
    );
    expect(syncSummary(status({ base_files: ['a', 'b'], traject_files: ['c'] }))).toBe(
      'Werkbranch werk heeft wijzigingen in 2 bestanden die nog niet in dit traject staan; dit traject heeft wijzigingen in 1 bestand die nog niet op de werkbranch staan',
    );
  });
  it('names the files that may conflict', () => {
    expect(syncDetail(status({ base_files: ['a.yaml'], overlap: ['a.yaml', 'b.yaml'] }))).toContain(
      'a.yaml, b.yaml',
    );
    expect(syncDetail(status({ base_files: ['a.yaml'] }))).toContain('zonder conflict');
  });
});

describe('findingsByLevel', () => {
  it('groups messages and treats an unknown level as info', () => {
    const out = findingsByLevel({
      findings: [
        { level: 'blocker', message: 'b' },
        { level: 'warning', message: 'w' },
        // A newer backend may send a level this build does not know.
        { level: 'weird' as unknown as 'info', message: 'x' },
      ],
    });
    expect(out).toEqual({ blocker: ['b'], warning: ['w'], info: ['x'] });
  });
});

describe('API calls', () => {
  it('preflight posts only the fields that are set', async () => {
    const fetchMock = vi.fn().mockResolvedValue(jsonRes({ findings: [] }));
    globalThis.fetch = fetchMock as unknown as typeof fetch;
    await preflightRepo({ repo_owner: 'example-org', repo_name: 'corpus', base_branch: '', repo_path: 'regulation/nl' });
    const [url, opts] = fetchMock.mock.calls[0]!;
    expect(url).toBe('/api/trajects/preflight');
    expect(JSON.parse(opts.body)).toEqual({
      repo_owner: 'example-org',
      repo_name: 'corpus',
      repo_path: 'regulation/nl',
    });
  });

  it('surfaces the conflict message of a 409 on sync', async () => {
    const fetchMock = vi.fn().mockResolvedValue(jsonRes('Aan beide kanten gewijzigd: a.yaml', 409));
    globalThis.fetch = fetchMock as unknown as typeof fetch;
    await expect(syncBase('naam-1234abcd')).rejects.toThrow(/a\.yaml/);
    expect(fetchMock.mock.calls[0]![0]).toBe('/api/trajects/naam-1234abcd/sync-base');
  });
});
