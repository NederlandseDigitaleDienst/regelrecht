import { describe, it, expect, beforeEach, vi } from 'vitest';
import { ref } from 'vue';
import type { ApiError } from '../lib/apiFetch';
import { useScenarios, isScenarioMismatch } from './useScenarios';
import { res, stubFetch as stubGlobalFetch } from '../test-utils/fetchStubs';

type FetchOptions = { method?: string; headers?: Record<string, string> };
interface FetchCall {
  url: string;
  opts: FetchOptions;
}

// Install a fetch stub with the call shape these tests read.
function stubFetch(impl: (url: string, opts?: FetchOptions) => Promise<unknown>) {
  return stubGlobalFetch(impl);
}

// A recorded PUT, failing the test outright when it is missing, so an
// assertion on its headers cannot pass because the call never happened.
function requirePut(call: FetchCall | undefined): { url: string; headers: Record<string, string> } {
  if (!call?.opts.headers) throw new Error('expected a PUT call with headers');
  return { url: call.url, headers: call.opts.headers };
}

// The list-read failures reject with an ApiError, which carries the status.
function apiError(err: Error | null): ApiError | null {
  return err as ApiError | null;
}

async function waitForScenario(sc: ReturnType<typeof useScenarios>, filename: string) {
  await vi.waitFor(() => {
    expect(sc.selectedScenario.value).toBe(filename);
    expect(sc.featureText.value).not.toBe('');
  });
}

beforeEach(() => {
  vi.restoreAllMocks();
});

describe('useScenarios optimistic concurrency', () => {
  function mockFetch(onPut: (url: string, opts: FetchOptions) => unknown) {
    const calls: FetchCall[] = [];
    stubFetch(async (url, opts) => {
      calls.push({ url, opts: opts ?? {} });
      if (opts?.method === 'PUT') return onPut(url, opts);
      if (url.endsWith('/scenarios')) {
        return res({ json: [{ filename: 'basis.feature' }] });
      }
      return res({ body: 'Feature: v1\n', etag: '"v1"' });
    });
    return calls;
  }

  it('captures the ETag on read and sends it back as If-Match on save', async () => {
    const calls = mockFetch(() =>
      res({ etag: '"new"', json: { pr: null, etag: '"new"' } }),
    );

    const sc = useScenarios(ref('wet_a'), ref('tr-12345678'));
    await waitForScenario(sc, 'basis.feature');

    await sc.saveScenario('basis.feature', 'Feature: v2\n');

    const put = calls.find((c) => c.opts?.method === 'PUT');
    expect(put).toBeTruthy();
    expect(requirePut(put).url).toBe(
      '/api/trajects/tr-12345678/corpus/laws/wet_a/scenarios/basis.feature',
    );
    expect(requirePut(put).headers['If-Match']).toBe('"v1"');

    // The new ETag is chained: a follow-up save sends the updated value.
    await sc.saveScenario('basis.feature', 'Feature: v3\n');
    const puts = calls.filter((c) => c.opts?.method === 'PUT');
    expect(requirePut(puts[1]).headers['If-Match']).toBe('"new"');
  });

  it('saves a brand-new scenario without an If-Match precondition', async () => {
    const calls = mockFetch(() =>
      res({ etag: '"created"', json: { pr: null, etag: '"created"' } }),
    );

    const sc = useScenarios(ref('wet_a'), ref('tr-12345678'));
    await waitForScenario(sc, 'basis.feature');

    await sc.saveScenario('nieuw.feature', 'Feature: nieuw\n');
    const put = calls.find(
      (c) => c.opts?.method === 'PUT' && c.url.endsWith('nieuw.feature'),
    );
    expect(requirePut(put).headers['If-Match']).toBeUndefined();
  });

  it('surfaces a 412 as a Dutch conflict error instead of overwriting silently', async () => {
    mockFetch(() => res({ ok: false, status: 412 }));

    const sc = useScenarios(ref('wet_a'), ref('tr-12345678'));
    await waitForScenario(sc, 'basis.feature');

    await expect(
      sc.saveScenario('basis.feature', 'Feature: v2\n'),
    ).rejects.toThrow(/door iemand anders gewijzigd/i);
    expect(sc.saveError.value?.message).toMatch(/herlaad/i);
  });

  it('does not chain the etag or content across a law switch mid-save', async () => {
    // Scenario filenames recur across laws (`basis.feature` exists for
    // both wet_a and wet_b). An in-flight save of wet_a's copy must not
    // re-insert its ETag into the map after the watch cleared it for
    // wet_b, or wet_b's next save carries a foreign If-Match → 412.
    let resolvePut!: (value?: unknown) => void;
    const putGate = new Promise((resolve) => { resolvePut = resolve; });
    const calls: FetchCall[] = [];
    stubFetch(async (url, opts) => {
      calls.push({ url, opts: opts ?? {} });
      if (opts?.method === 'PUT') {
        await putGate;
        return res({ etag: '"a2"', json: { pr: null, etag: '"a2"' } });
      }
      if (url.endsWith('/scenarios')) {
        return res({ json: [{ filename: 'basis.feature' }] });
      }
      if (url.includes('/laws/wet_b/')) {
        return res({ body: 'Feature: b1\n', etag: '"b1"' });
      }
      return res({ body: 'Feature: a1\n', etag: '"a1"' });
    });

    const lawId = ref('wet_a');
    const sc = useScenarios(lawId, ref('tr-12345678'));
    await waitForScenario(sc, 'basis.feature');
    expect(sc.featureText.value).toBe('Feature: a1\n');

    // Kick off the save, then switch laws while the PUT is in flight.
    const save = sc.saveScenario('basis.feature', 'Feature: a2\n');
    lawId.value = 'wet_b';
    await vi.waitFor(() => {
      expect(sc.featureText.value).toBe('Feature: b1\n');
    });

    resolvePut();
    await save;

    // The stale completion must not overwrite the new law's content…
    expect(sc.featureText.value).toBe('Feature: b1\n');

    // …nor poison the new law's etag map: saving wet_b's same-named
    // scenario sends the ETag wet_b's read returned, not the old
    // save's chained one.
    await sc.saveScenario('basis.feature', 'Feature: b2\n');
    const puts = calls.filter((c) => c.opts?.method === 'PUT');
    expect(requirePut(puts[1]).url).toBe(
      '/api/trajects/tr-12345678/corpus/laws/wet_b/scenarios/basis.feature',
    );
    expect(requirePut(puts[1]).headers['If-Match']).toBe('"b1"');
  });
});

describe('isScenarioMismatch', () => {
  it('is false when the file targets the opened law', () => {
    expect(isScenarioMismatch({ target_law_ids: ['wet_a'] }, 'wet_a')).toBe(false);
  });
  it('is false when targets are unknown (no execution step yet)', () => {
    expect(isScenarioMismatch({ target_law_ids: [] }, 'wet_a')).toBe(false);
    expect(isScenarioMismatch({}, 'wet_a')).toBe(false);
  });
  it('is true when the file only targets other laws', () => {
    expect(isScenarioMismatch({ target_law_ids: ['wet_b', 'wet_c'] }, 'wet_a')).toBe(true);
  });
});

describe('useScenarios auto-select', () => {
  function mockList(entries: unknown[]) {
    stubFetch(async (url) => {
      if (String(url).endsWith('/scenarios')) return res({ json: entries });
      return res({ body: 'Feature: x\n', etag: '"v1"' });
    });
  }

  it('prefers the first file that targets the opened law', async () => {
    mockList([
      { filename: 'other.feature', target_law_ids: ['andere_wet'] },
      { filename: 'matching.feature', target_law_ids: ['wet_a'] },
    ]);

    const s = useScenarios(ref('wet_a'));
    await s.fetchScenarios();
    expect(s.selectedScenario.value).toBe('matching.feature');
  });

  it('prefers an explicit match over a file with unknown targets', async () => {
    mockList([
      { filename: 'wip.feature', target_law_ids: [] },
      { filename: 'matching.feature', target_law_ids: ['wet_a'] },
    ]);

    const s = useScenarios(ref('wet_a'));
    await s.fetchScenarios();
    expect(s.selectedScenario.value).toBe('matching.feature');
  });

  it('prefers unknown targets over a known mismatch', async () => {
    mockList([
      { filename: 'other.feature', target_law_ids: ['andere_wet'] },
      { filename: 'wip.feature', target_law_ids: [] },
    ]);

    const s = useScenarios(ref('wet_a'));
    await s.fetchScenarios();
    expect(s.selectedScenario.value).toBe('wip.feature');
  });

  it('falls back to the first file when none match', async () => {
    mockList([
      { filename: 'a.feature', target_law_ids: ['andere_wet'] },
      { filename: 'b.feature', target_law_ids: ['nog_een_wet'] },
    ]);

    const s = useScenarios(ref('wet_a'));
    await s.fetchScenarios();
    expect(s.selectedScenario.value).toBe('a.feature');
  });
});

describe('useScenarios list-read failures', () => {
  function mockListStatus(status: number) {
    stubFetch(async (url) => {
      if (String(url).endsWith('/scenarios')) {
        return res({ ok: false, status, body: '' });
      }
      return res({ body: 'Feature: x\n', etag: '"v1"' });
    });
  }

  it('shows an empty list only when the backend says so', async () => {
    stubFetch(async () => res({ json: [] }));

    const s = useScenarios(ref('wet_a'));
    await s.fetchScenarios();

    expect(s.scenarios.value).toEqual([]);
    expect(s.error.value).toBeNull();
  });

  it('surfaces a backend failure instead of showing an empty list', async () => {
    mockListStatus(502);

    const s = useScenarios(ref('wet_a'));
    await s.fetchScenarios();

    expect(s.scenarios.value).toEqual([]);
    expect(apiError(s.error.value)?.status).toBe(502);
  });

  it('surfaces a server error on the traject scope too', async () => {
    mockListStatus(500);

    const s = useScenarios(ref('wet_a'), ref('tr-12345678'));
    await s.fetchScenarios();

    expect(apiError(s.error.value)?.status).toBe(500);
  });

  it('does not read 404 as a law without scenarios', async () => {
    mockListStatus(404);

    const s = useScenarios(ref('wet_a'));
    await s.fetchScenarios();

    expect(apiError(s.error.value)?.status).toBe(404);
    expect(s.error.value?.message).toMatch(/niet gevonden/);
  });

  // Every list request stays pending until the test settles it, in the order
  // the requests were made. The composable's own watcher issues them, so each
  // test settles exactly the request it means instead of relying on when Vue
  // flushes that watcher.
  function holdListRequests() {
    const pending: { url: string; settle: (value: unknown) => void }[] = [];
    stubFetch(async (url) => {
      if (!String(url).endsWith('/scenarios')) return res({ body: 'Feature: x\n' });
      const settled = await new Promise((resolve) => {
        pending.push({ url: String(url), settle: resolve });
      });
      return settled;
    });
    return {
      async request(n: number) {
        await vi.waitFor(() => expect(pending.length).toBeGreaterThan(n));
        // The wait above guarantees entry n exists.
        return pending[n]!;
      },
    };
  }

  it('drops a failure that arrives after the user switched law', async () => {
    const lawId = ref('wet_a');
    const requests = holdListRequests();
    const s = useScenarios(lawId);
    const forA = await requests.request(0);

    lawId.value = 'wet_b';
    const forB = await requests.request(1);
    expect(forB.url).toContain('wet_b');

    forA.settle(res({ ok: false, status: 502, body: '' }));
    await new Promise((r) => setTimeout(r, 0));
    expect(s.error.value).toBeNull();

    forB.settle(res({ json: [] }));
    await vi.waitFor(() => expect(s.loading.value).toBe(false));
    expect(s.error.value).toBeNull();
  });

  it('keeps loading while the newer request is still on its way', async () => {
    const lawId = ref('wet_a');
    const requests = holdListRequests();
    const s = useScenarios(lawId);
    const forA = await requests.request(0);

    lawId.value = 'wet_b';
    const forB = await requests.request(1);

    // The stale answer for A lands first. B has not answered, so the screen
    // must still say it is loading rather than "no scenarios".
    forA.settle(res({ json: [{ filename: 'a.feature' }] }));
    await new Promise((r) => setTimeout(r, 0));
    expect(s.loading.value).toBe(true);
    expect(s.scenarios.value).toEqual([]);

    forB.settle(res({ json: [] }));
    await vi.waitFor(() => expect(s.loading.value).toBe(false));
  });

  it('ignores the stale request when the user switches back to the same law', async () => {
    // A, B, A: the first and the third request share a scope, so only the
    // order of the requests can tell them apart.
    const lawId = ref('wet_a');
    const requests = holdListRequests();
    const s = useScenarios(lawId);
    const firstA = await requests.request(0);

    lawId.value = 'wet_b';
    await requests.request(1);
    lawId.value = 'wet_a';
    const secondA = await requests.request(2);

    firstA.settle(res({ ok: false, status: 502, body: '' }));
    await new Promise((r) => setTimeout(r, 0));
    expect(s.error.value).toBeNull();
    expect(s.loading.value).toBe(true);

    secondA.settle(res({ json: [] }));
    await vi.waitFor(() => expect(s.loading.value).toBe(false));
    expect(s.error.value).toBeNull();
  });

  it('clears a previous error when no law is selected', async () => {
    const lawId = ref('wet_a');
    mockListStatus(502);

    const s = useScenarios(lawId);
    await s.fetchScenarios();
    expect(s.error.value).toBeTruthy();

    lawId.value = '';
    await s.fetchScenarios();

    expect(s.error.value).toBeNull();
  });
});
