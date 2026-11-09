import { describe, it, expect, beforeEach, vi } from 'vitest';
import {
  useTrajectDetail,
  writableSource,
  branchTreeUrl,
} from './useTrajectDetail';
import type { TrajectDetail, TrajectSource } from './useTrajectDetail';
import { res, stubFetch } from '../test-utils/fetchStubs';

const OWN_SOURCE: TrajectSource = {
  source_id: 'own',
  name: 'eigen',
  source_type: 'github',
  gh_owner: 'MinBZK',
  gh_repo: 'regelrecht-corpus',
  gh_branch: 'traject/tariefswijziging-2026',
  gh_base_branch: 'development',
  gh_path: 'regulation/nl',
  gh_ref: null,
  local_path: null,
  priority: 0,
  auth_ref: null,
  is_writable_own: true,
};
const READONLY_SOURCE = { ...OWN_SOURCE, source_id: 'ro', is_writable_own: false };

const DETAIL: TrajectDetail = {
  id: '11111111-2222-3333-4444-555566667777',
  name: 'Tariefswijziging 2026',
  description: 'Waarom',
  scope: 'zorgtoeslag',
  status: 'bezig',
  role: 'owner',
  ref: 'tariefswijziging-2026-11111111',
  members: [],
  pending_invites: [],
  sources: [READONLY_SOURCE, OWN_SOURCE],
};

beforeEach(() => {
  vi.restoreAllMocks();
});

describe('writableSource', () => {
  it('returns the source flagged is_writable_own', () => {
    expect(writableSource(DETAIL)?.source_id).toBe('own');
  });

  it('returns null when there is no detail or no writable source', () => {
    expect(writableSource(null)).toBe(null);
    expect(writableSource({ sources: [READONLY_SOURCE] })).toBe(null);
    expect(writableSource({})).toBe(null);
  });
});

describe('branchTreeUrl', () => {
  it('builds a github tree URL for the branch, slashes preserved', () => {
    expect(branchTreeUrl(OWN_SOURCE)).toBe(
      'https://github.com/MinBZK/regelrecht-corpus/tree/traject/tariefswijziging-2026',
    );
  });

  it('returns null for a non-github or incomplete source', () => {
    expect(branchTreeUrl(null)).toBe(null);
    expect(branchTreeUrl({ gh_owner: 'x', gh_repo: null, gh_branch: 'b' })).toBe(null);
    expect(branchTreeUrl({ gh_owner: 'x', gh_repo: 'y', gh_branch: null })).toBe(null);
  });
});

describe('useTrajectDetail', () => {
  it('fetches the detail by id and exposes it reactively', async () => {
    const fetchSpy = vi.fn().mockResolvedValue(res({ json: DETAIL }));
    stubFetch(fetchSpy);

    const { detail, loading, error, load } = useTrajectDetail();
    expect(loading.value).toBe(false);
    await load(DETAIL.id);

    expect(fetchSpy).toHaveBeenCalledWith(
      '/api/trajects/11111111-2222-3333-4444-555566667777',
      {},
    );
    expect(detail.value).not.toBe(null);
    expect(detail.value?.name).toBe('Tariefswijziging 2026');
    expect(error.value).toBe(null);
    expect(loading.value).toBe(false);
  });

  it('resets state before loading so a reopen cannot flash stale data', async () => {
    stubFetch(vi.fn().mockResolvedValue(res({ json: DETAIL })));
    const { detail, load } = useTrajectDetail();
    await load(DETAIL.id);
    expect(detail.value).not.toBe(null);

    // Second load against a slow/failed fetch must have cleared detail first.
    let resolveFetch: (value: Response) => void = () => {};
    stubFetch(vi.fn().mockReturnValue(
      new Promise<Response>((r) => {
        resolveFetch = r;
      }),
    ));
    const p = load(DETAIL.id);
    expect(detail.value).toBe(null); // cleared synchronously before await
    resolveFetch(res({ json: DETAIL }));
    await p;
  });

  it('records an error on a non-ok response and leaves detail null', async () => {
    stubFetch(vi.fn().mockResolvedValue(res({ ok: false, status: 404 })));
    const { detail, error, load } = useTrajectDetail();
    await load(DETAIL.id);
    expect(detail.value).toBe(null);
    expect(error.value).toBeInstanceOf(Error);
    expect((error.value as Error).message).toMatch(/404/);
  });
});
