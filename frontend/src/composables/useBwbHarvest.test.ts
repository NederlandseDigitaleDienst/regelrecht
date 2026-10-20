import { describe, it, expect, afterEach, vi } from 'vitest';
import { jsonResponse, stubFetch } from '../test-utils/fetchStubs';

const POLL_INTERVAL_MS = 5000;
const POLL_MAX_MS = 10 * 60 * 1000;
const BWB_ID = 'BWBR0001840';

// useBwbHarvest keeps module-level singleton polling state; reset modules per
// test for a clean store.
async function freshHarvest() {
  vi.resetModules();
  const mod = await import('./useBwbHarvest');
  return mod.useBwbHarvest();
}

/** Stub fetch: POST /api/harvest → queued; status polls → `pollStatus()`. */
function stubHarvestFetch(pollStatus: () => string) {
  stubFetch((input: string | Request) => {
    const url = typeof input === 'string' ? input : input.url;
    if (url.includes('/api/harvest/status')) {
      return Promise.resolve(
        jsonResponse({ results: [{ bwb_id: BWB_ID, status: pollStatus() }] }),
      );
    }
    return Promise.resolve(jsonResponse({ status: 'queued' }));
  });
}

describe('useBwbHarvest polling', () => {
  afterEach(() => {
    vi.useRealTimers();
  });

  it('keeps polling past POLL_MAX_MS while a law is enriching (no false timeout)', async () => {
    vi.useFakeTimers();
    stubHarvestFetch(() => 'enriching');
    const { harvestStatus, requestHarvest } = await freshHarvest();

    await requestHarvest(BWB_ID);
    expect(harvestStatus.value[BWB_ID]).toBe('queued');

    // Chunked enrichment of a large law takes far longer than the poll cap;
    // the status must stay 'enriching' and polling must continue.
    await vi.advanceTimersByTimeAsync(POLL_MAX_MS + POLL_INTERVAL_MS);
    expect(harvestStatus.value[BWB_ID]).toBe('enriching');

    const callsBefore = vi.mocked(fetch).mock.calls.length;
    await vi.advanceTimersByTimeAsync(2 * POLL_INTERVAL_MS);
    expect(vi.mocked(fetch).mock.calls.length).toBeGreaterThan(callsBefore);
    expect(harvestStatus.value[BWB_ID]).toBe('enriching');
  });

  it('still times out entries stuck before enrichment (queued/harvesting)', async () => {
    vi.useFakeTimers();
    stubHarvestFetch(() => 'harvesting');
    const { harvestStatus, requestHarvest } = await freshHarvest();

    await requestHarvest(BWB_ID);
    await vi.advanceTimersByTimeAsync(POLL_MAX_MS + POLL_INTERVAL_MS);
    expect(harvestStatus.value[BWB_ID]).toBe('timeout');

    // Polling stopped: no further status fetches.
    const callsBefore = vi.mocked(fetch).mock.calls.length;
    await vi.advanceTimersByTimeAsync(2 * POLL_INTERVAL_MS);
    expect(vi.mocked(fetch).mock.calls.length).toBe(callsBefore);
  });

  it('stops polling when an enriching law reaches a terminal status after the cap', async () => {
    vi.useFakeTimers();
    let status = 'enriching';
    stubHarvestFetch(() => status);
    const { harvestStatus, requestHarvest } = await freshHarvest();

    await requestHarvest(BWB_ID);
    await vi.advanceTimersByTimeAsync(POLL_MAX_MS + POLL_INTERVAL_MS);
    expect(harvestStatus.value[BWB_ID]).toBe('enriching');

    status = 'enriched';
    await vi.advanceTimersByTimeAsync(POLL_INTERVAL_MS);
    expect(harvestStatus.value[BWB_ID]).toBe('enriched');

    const callsBefore = vi.mocked(fetch).mock.calls.length;
    await vi.advanceTimersByTimeAsync(2 * POLL_INTERVAL_MS);
    expect(vi.mocked(fetch).mock.calls.length).toBe(callsBefore);
  });
});
