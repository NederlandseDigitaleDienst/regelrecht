// What the search popover shows, decided without a component around it.
//
// Search runs through the sources in order and shows the first one that has
// matches, saying which source that is. The backend already returns every
// match with its source and priority (lower = searched first: the traject's
// own repo, then the example rulework, then the central corpus).

import type { CorpusLaw } from '../composables/useCorpusLaws';

export interface SourceGroup {
  sourceId: string;
  sourceName: string;
  laws: CorpusLaw[];
}

/** The matches of the highest-priority source that has any, or null. */
export function firstSourceGroup(laws: readonly CorpusLaw[]): SourceGroup | null {
  if (laws.length === 0) return null;
  // Stable: laws keep the backend's order within their source.
  const best = laws.reduce((a, b) =>
    (b.source_priority ?? Number.MAX_SAFE_INTEGER) < (a.source_priority ?? Number.MAX_SAFE_INTEGER) ? b : a,
  );
  const sourceId = best.source_id ?? '';
  return {
    sourceId,
    sourceName: best.source_name || sourceId,
    laws: laws.filter((l) => (l.source_id ?? '') === sourceId),
  };
}

/**
 * `count` distinct ids picked at random from `ids`. `random` is injectable so
 * tests can make the pick deterministic.
 */
export function pickSuggestions(
  ids: readonly string[],
  count: number,
  random: () => number = Math.random,
): string[] {
  const pool = [...new Set(ids)];
  // Partial Fisher-Yates: shuffle only as far as we need.
  for (let i = 0; i < Math.min(count, pool.length); i++) {
    const j = i + Math.floor(random() * (pool.length - i));
    [pool[i], pool[j]] = [pool[j] as string, pool[i] as string];
  }
  return pool.slice(0, count);
}

/** The official text of a regulation on wetten.overheid.nl. */
export function officialTextUrl(bwbId: string): string {
  return `https://wetten.overheid.nl/${encodeURIComponent(bwbId)}`;
}
