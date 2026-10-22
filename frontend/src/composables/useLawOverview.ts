// The "Alle regelwerken" overview in Corpus juris, and the "only enriched"
// filter it shares with the search popover.
//
// The filter is one module-level ref, so ticking it in search also ticks it in
// the overview and the other way round. It is not persisted: a fresh visit
// starts with every law, which is what someone new to the library expects.

import { ref, watch, type Ref } from 'vue';
import { apiFetch } from '../lib/apiFetch';
import { lawsListUrl } from './corpusUrls';
import type { CorpusLaw } from './useCorpusLaws';

export const enrichedOnly = ref(false);

/** Laws per overview page. */
export const OVERVIEW_PAGE_SIZE = 25;

/** The query string for one overview page (1-based). */
export function overviewQuery(page: number, enriched: boolean): string {
  const offset = (Math.max(page, 1) - 1) * OVERVIEW_PAGE_SIZE;
  const parts = [`offset=${offset}`, `limit=${OVERVIEW_PAGE_SIZE}`];
  if (enriched) parts.push('enriched=true');
  return parts.join('&');
}

/** Pages needed for `total` laws; at least 1, so an empty list is page 1 of 1. */
export function pageCount(total: number): number {
  return Math.max(1, Math.ceil(total / OVERVIEW_PAGE_SIZE));
}

/**
 * One page of Corpus juris laws, refetched whenever the page or the filter
 * changes. Only the latest request writes the result, so paging quickly never
 * shows an older page last.
 */
export function useLawOverview(page: Ref<number>) {
  const laws = ref<CorpusLaw[]>([]);
  const total = ref(0);
  const loading = ref(false);
  const error = ref<string | null>(null);
  let generation = 0;

  async function load(): Promise<void> {
    const mine = ++generation;
    loading.value = true;
    error.value = null;
    try {
      const res = await apiFetch(lawsListUrl(null, overviewQuery(page.value, enrichedOnly.value)));
      const list: unknown = await res.json();
      if (mine !== generation) return;
      laws.value = Array.isArray(list) ? (list as CorpusLaw[]) : [];
      const header = Number(res.headers.get('x-total-count'));
      // Without the header (an older backend) the page itself is all we know.
      total.value = Number.isFinite(header) && header > 0 ? header : laws.value.length;
    } catch (e) {
      if (mine !== generation) return;
      laws.value = [];
      total.value = 0;
      error.value = e instanceof Error ? e.message : String(e);
    } finally {
      if (mine === generation) loading.value = false;
    }
  }

  watch([page, enrichedOnly], load, { immediate: true });

  return { laws, total, loading, error, reload: load };
}
