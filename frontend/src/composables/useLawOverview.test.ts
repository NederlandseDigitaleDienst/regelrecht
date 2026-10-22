import { describe, it, expect, vi, beforeEach } from 'vitest';
import { nextTick, ref } from 'vue';

const apiFetch = vi.fn();
vi.mock('../lib/apiFetch', () => ({ apiFetch: (...args: unknown[]) => apiFetch(...args) }));

import {
  enrichedOnly,
  overviewQuery,
  pageCount,
  useLawOverview,
  OVERVIEW_PAGE_SIZE,
} from './useLawOverview';

function response(list: unknown[], total: number | null) {
  return {
    json: async () => list,
    headers: { get: (name: string) => (name === 'x-total-count' && total !== null ? String(total) : null) },
  };
}

async function settle() {
  for (let i = 0; i < 5; i++) await nextTick();
}

beforeEach(() => {
  apiFetch.mockReset();
  enrichedOnly.value = false;
});

describe('overviewQuery', () => {
  it('pages by offset and adds the enriched filter only when asked', () => {
    expect(overviewQuery(1, false)).toBe(`offset=0&limit=${OVERVIEW_PAGE_SIZE}`);
    expect(overviewQuery(3, true)).toBe(`offset=${2 * OVERVIEW_PAGE_SIZE}&limit=${OVERVIEW_PAGE_SIZE}&enriched=true`);
  });

  it('treats a page below 1 as the first page', () => {
    expect(overviewQuery(0, false)).toBe(`offset=0&limit=${OVERVIEW_PAGE_SIZE}`);
  });
});

describe('pageCount', () => {
  it('rounds up, and an empty list is still one page', () => {
    expect(pageCount(0)).toBe(1);
    expect(pageCount(OVERVIEW_PAGE_SIZE)).toBe(1);
    expect(pageCount(OVERVIEW_PAGE_SIZE + 1)).toBe(2);
  });
});

describe('useLawOverview', () => {
  it('loads a page with its total from the X-Total-Count header', async () => {
    apiFetch.mockResolvedValue(response([{ law_id: 'wet_a' }], 40));
    const { laws, total } = useLawOverview(ref(1));
    await settle();
    expect(apiFetch).toHaveBeenCalledWith(`/api/corpus/laws?offset=0&limit=${OVERVIEW_PAGE_SIZE}`);
    expect(laws.value.map((l) => l.law_id)).toEqual(['wet_a']);
    expect(total.value).toBe(40);
  });

  it('refetches with the enriched filter when it is switched on', async () => {
    apiFetch.mockResolvedValue(response([], 0));
    useLawOverview(ref(1));
    await settle();
    enrichedOnly.value = true;
    await settle();
    expect(apiFetch).toHaveBeenLastCalledWith(
      `/api/corpus/laws?offset=0&limit=${OVERVIEW_PAGE_SIZE}&enriched=true`,
    );
  });

  it('keeps the newest page when an older request answers last', async () => {
    let releaseFirst: (v: unknown) => void = () => {};
    apiFetch
      .mockImplementationOnce(() => new Promise((r) => { releaseFirst = r; }))
      .mockResolvedValueOnce(response([{ law_id: 'page_two' }], 30));
    const page = ref(1);
    const { laws } = useLawOverview(page);
    page.value = 2;
    await settle();
    releaseFirst(response([{ law_id: 'page_one' }], 30));
    await settle();
    expect(laws.value.map((l) => l.law_id)).toEqual(['page_two']);
  });

  it('reports a failed load instead of an empty library', async () => {
    apiFetch.mockRejectedValue(new Error('Failed to load corpus: 502'));
    const { laws, error } = useLawOverview(ref(1));
    await settle();
    expect(laws.value).toEqual([]);
    expect(error.value).toContain('502');
  });
});
