import { describe, it, expect, beforeEach, afterEach, vi, type Mock } from 'vitest';
import { useMarkings } from './useMarkings';
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
  const comp = useMarkings();
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

describe('useMarkings', () => {
  it('asks for the markings endpoint with sensible defaults', async () => {
    const spy = vi.fn().mockResolvedValue(pageRes());
    await create(spy);

    const url = lastUrl(spy);
    expect(url).toContain('/api/harvest-admin/markings?');
    expect(url).toContain('sort=created_at');
    expect(url).toContain('order=desc');
    expect(url).toContain('limit=50');
    expect(url).toContain('offset=0');
  });

  it('sorts on an allowed key and resets to the first page', async () => {
    const spy = vi.fn().mockResolvedValue(pageRes());
    const comp = await create(spy);

    comp.goToPage(3);
    await Promise.resolve();
    comp.setSort('resolved_by', 'asc');
    await Promise.resolve();

    const url = lastUrl(spy);
    expect(url).toContain('sort=resolved_by');
    expect(url).toContain('order=asc');
    expect(url).toContain('offset=0');
  });

  it('toggles direction when the same key is chosen again', async () => {
    const spy = vi.fn().mockResolvedValue(pageRes());
    const comp = await create(spy);

    comp.setSort('about');
    await Promise.resolve();
    expect(lastUrl(spy)).toContain('order=desc');

    comp.setSort('about');
    await Promise.resolve();
    expect(lastUrl(spy)).toContain('order=asc');
  });

  // `law_name` is a joined column, so the backend rejects it. The composable
  // refuses it too rather than sending a request it knows will fail.
  it('ignores a sort key outside the allowlist', async () => {
    const spy = vi.fn().mockResolvedValue(pageRes());
    const comp = await create(spy);
    const before = spy.mock.calls.length;

    comp.setSort('law_name');
    await Promise.resolve();

    expect(spy.mock.calls.length).toBe(before);
  });

  it('adds and removes filters', async () => {
    const spy = vi.fn().mockResolvedValue(pageRes());
    const comp = await create(spy);

    comp.setFilter('resolution', 'operation');
    await Promise.resolve();
    expect(lastUrl(spy)).toContain('resolution=operation');

    comp.setFilter('resolution', '');
    await Promise.resolve();
    expect(lastUrl(spy)).not.toContain('resolution=');
  });

  // The cluster view groups over the filtered set, so it needs the filters
  // without the paging. Handing it the full query string would make the
  // backlog describe one page rather than the whole selection.
  it('exposes filters without paging or sorting, for the cluster view', async () => {
    const spy = vi.fn().mockResolvedValue(pageRes());
    const comp = await create(spy);

    comp.setFilter('provider', 'claude');
    await Promise.resolve();

    const params = comp.filterParams().toString();
    expect(params).toContain('provider=claude');
    expect(params).not.toContain('limit');
    expect(params).not.toContain('offset');
    expect(params).not.toContain('sort');
  });

  // Two `setFilter` calls fire two un-awaited refreshes, and `buildUrl()` runs
  // synchronously at the start of each fetch. The first would go out with only
  // the filter set so far, and the two race for the same `data` ref: there is
  // no sequencing or cancellation in `usePollingFetch`, so a broader first
  // response landing last overwrites the narrower one.
  it('sends one request for several filters at once', async () => {
    const spy = vi.fn().mockResolvedValue(pageRes());
    const comp = await create(spy);
    const before = spy.mock.calls.length;

    comp.setFilters({ resolution: 'operation', resolved_by: 'een WORKING_DAY-bewerking' });
    await Promise.resolve();

    expect(spy.mock.calls.length, 'one request, not two').toBe(before + 1);
    const url = lastUrl(spy);
    expect(url).toContain('resolution=operation');
    expect(url).toContain('resolved_by=');
  });

  it('clears a filter passed as empty in the same update', async () => {
    const spy = vi.fn().mockResolvedValue(pageRes());
    const comp = await create(spy);

    comp.setFilters({ resolution: 'model', resolved_by: 'iets' });
    await Promise.resolve();
    expect(lastUrl(spy)).toContain('resolved_by=iets');

    // A cluster that names no change can only be narrowed by resolution.
    comp.setFilters({ resolution: 'model', resolved_by: '' });
    await Promise.resolve();
    expect(lastUrl(spy)).not.toContain('resolved_by=');
    expect(lastUrl(spy)).toContain('resolution=model');
  });

  it('turns a page number into an offset', async () => {
    const spy = vi.fn().mockResolvedValue(pageRes({ data: [], total: 200 }));
    const comp = await create(spy);

    comp.goToPage(3);
    await Promise.resolve();

    expect(lastUrl(spy)).toContain('offset=100');
  });
});
