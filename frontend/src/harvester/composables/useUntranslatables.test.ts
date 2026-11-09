import { describe, it, expect, beforeEach, afterEach, vi, type Mock } from 'vitest';
import { useUntranslatables } from './useUntranslatables';
import { pageRes, stubFetch } from '../../test-utils/fetchStubs';

// The composable fires an initial fetch + starts a 20s poll on creation. Fake
// timers keep the poll from firing during the test; we stop it in afterEach.
let stop: (() => void) | null = null;

beforeEach(() => {
  vi.useFakeTimers();
  vi.restoreAllMocks();
});

afterEach(() => {
  if (stop) stop();
  stop = null;
  vi.useRealTimers();
});

/** Create the composable, wait for its initial fetch to settle. */
async function create(fetchSpy: Mock) {
  stubFetch(fetchSpy);
  const comp = useUntranslatables();
  stop = comp.stopPolling;
  await comp.refresh();
  return comp;
}

/** URL string of the most recent fetch call. */
function lastUrl(fetchSpy: Mock): string {
  const call = fetchSpy.mock.calls.at(-1);
  if (!call) throw new Error('fetch was not called');
  return call[0] as string;
}

describe('useUntranslatables', () => {
  it('issues an initial fetch with default sort/order/pagination', async () => {
    const fetchSpy = vi.fn().mockResolvedValue(pageRes({ data: [{ id: '1' }], total: 1 }));
    const comp = await create(fetchSpy);

    const url = lastUrl(fetchSpy);
    expect(url).toContain('/api/harvest-admin/untranslatables?');
    expect(url).toContain('sort=created_at');
    expect(url).toContain('order=desc');
    expect(url).toContain('limit=50');
    expect(url).toContain('offset=0');
    expect(comp.data.value).toEqual([{ id: '1' }]);
    expect(comp.totalCount.value).toBe(1);
  });

  it('applies a valid sort with explicit order and resets offset', async () => {
    const fetchSpy = vi.fn().mockResolvedValue(pageRes());
    const comp = await create(fetchSpy);

    comp.setSort('law_id', 'asc');
    await Promise.resolve();
    const url = lastUrl(fetchSpy);
    expect(url).toContain('sort=law_id');
    expect(url).toContain('order=asc');
    expect(url).toContain('offset=0');
  });

  it('toggles order when re-sorting the same key', async () => {
    const fetchSpy = vi.fn().mockResolvedValue(pageRes());
    const comp = await create(fetchSpy);

    comp.setSort('construct');
    await Promise.resolve();
    expect(lastUrl(fetchSpy)).toContain('order=desc');

    comp.setSort('construct');
    await Promise.resolve();
    expect(lastUrl(fetchSpy)).toContain('order=asc');
  });

  it('ignores a sort key outside the allowlist (no fetch)', async () => {
    const fetchSpy = vi.fn().mockResolvedValue(pageRes());
    const comp = await create(fetchSpy);
    const before = fetchSpy.mock.calls.length;

    comp.setSort('bogus');
    await Promise.resolve();
    expect(fetchSpy.mock.calls.length).toBe(before);
  });

  it('adds and removes filters', async () => {
    const fetchSpy = vi.fn().mockResolvedValue(pageRes());
    const comp = await create(fetchSpy);

    comp.setFilter('provider', 'claude');
    await Promise.resolve();
    expect(lastUrl(fetchSpy)).toContain('provider=claude');

    comp.setFilter('accepted', 'true');
    await Promise.resolve();
    expect(lastUrl(fetchSpy)).toContain('accepted=true');

    comp.setFilter('provider', '');
    await Promise.resolve();
    expect(lastUrl(fetchSpy)).not.toContain('provider=');
  });

  it('paginates by translating a page into an offset', async () => {
    const fetchSpy = vi.fn().mockResolvedValue(pageRes({ data: [], total: 150 }));
    const comp = await create(fetchSpy);

    comp.goToPage(3);
    await Promise.resolve();
    expect(lastUrl(fetchSpy)).toContain('offset=100');
  });
});
