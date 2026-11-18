import { describe, it, expect, beforeEach, vi } from 'vitest';
import { ref } from 'vue';
import type { ApiError } from '../lib/apiFetch';
import { useScenarios, isScenarioMismatch, preferredScenarioFile } from './useScenarios';
import { res, stubFetch } from '../test-utils/fetchStubs';

// Reading and saving one feature file is tested in scenario/openScenario.test.ts.

// The list-read failures reject with an ApiError, which carries the status.
function apiError(err: Error | null): ApiError | null {
  return err as ApiError | null;
}

beforeEach(() => {
  vi.restoreAllMocks();
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

describe('preferredScenarioFile', () => {
  const pick = (entries: { filename: string; target_law_ids?: string[] }[]) =>
    preferredScenarioFile(entries, 'wet_a')?.filename ?? null;

  it('prefers the first file that targets the opened law', () => {
    expect(pick([
      { filename: 'other.feature', target_law_ids: ['andere_wet'] },
      { filename: 'matching.feature', target_law_ids: ['wet_a'] },
    ])).toBe('matching.feature');
  });

  it('prefers an explicit match over a file with unknown targets', () => {
    expect(pick([
      { filename: 'wip.feature', target_law_ids: [] },
      { filename: 'matching.feature', target_law_ids: ['wet_a'] },
    ])).toBe('matching.feature');
  });

  it('prefers unknown targets over a known mismatch', () => {
    expect(pick([
      { filename: 'other.feature', target_law_ids: ['andere_wet'] },
      { filename: 'wip.feature', target_law_ids: [] },
    ])).toBe('wip.feature');
  });

  it('falls back to the first file when none match, and to nothing without files', () => {
    expect(pick([
      { filename: 'a.feature', target_law_ids: ['andere_wet'] },
      { filename: 'b.feature', target_law_ids: ['nog_een_wet'] },
    ])).toBe('a.feature');
    expect(pick([])).toBeNull();
  });
});

describe('useScenarios on another regelwerk', () => {
  it('drops the previous list as soon as the regelwerk changes', async () => {
    let answer: (value: unknown) => void = () => {};
    stubFetch(async (url) => {
      if (String(url).includes('wet_b')) return new Promise((resolve) => { answer = resolve; });
      return res({ json: [{ filename: 'a.feature' }] });
    });
    const lawId = ref('wet_a');
    const s = useScenarios(lawId);
    await vi.waitFor(() => expect(s.scenarios.value).toHaveLength(1));

    lawId.value = 'wet_b';
    await vi.waitFor(() => expect(s.loading.value).toBe(true));
    // wet_a's file is not wet_b's, while wet_b's list is still on its way.
    expect(s.scenarios.value).toEqual([]);
    answer(res({ json: [{ filename: 'b.feature' }] }));
    await vi.waitFor(() => expect(s.scenarios.value.map((e) => e.filename)).toEqual(['b.feature']));
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
