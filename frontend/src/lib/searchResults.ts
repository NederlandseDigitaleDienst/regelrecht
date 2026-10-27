// What the search popover shows, decided without a component around it.
//
// Search runs through the sources in order and shows the first one that has
// matches, saying which source that is; the other sources with matches get a
// row each that switches to them. The backend already returns every
// match with its source and priority (lower = searched first: the traject's
// own repo, then the example rulework, then the central corpus).

import type { CorpusLaw } from '../composables/useCorpusLaws';

export interface SourceGroup {
  sourceId: string;
  sourceName: string;
  laws: CorpusLaw[];
}

/**
 * The matches grouped per source, the highest-priority source first. Laws keep
 * the backend's order within their source.
 */
export function groupBySource(laws: readonly CorpusLaw[]): SourceGroup[] {
  const groups = new Map<string, SourceGroup & { priority: number }>();
  for (const law of laws) {
    const sourceId = law.source_id ?? '';
    let group = groups.get(sourceId);
    if (!group) {
      group = {
        sourceId,
        sourceName: law.source_name || sourceId,
        priority: law.source_priority ?? Number.MAX_SAFE_INTEGER,
        laws: [],
      };
      groups.set(sourceId, group);
    }
    group.laws.push(law);
  }
  // Array.prototype.sort is stable, so equal priorities keep response order.
  return [...groups.values()]
    .sort((a, b) => a.priority - b.priority)
    .map(({ sourceId, sourceName, laws }) => ({ sourceId, sourceName, laws }));
}

/**
 * The row that points at another source's matches: "Ook 3 treffers in X".
 * `atLeast` marks a count the backend's result cap may have cut short.
 */
export function otherSourceLabel(group: SourceGroup, atLeast = false): string {
  const n = group.laws.length;
  const count = atLeast ? `${n}+` : `${n}`;
  return `Ook ${count} ${n === 1 && !atLeast ? 'treffer' : 'treffers'} in ${group.sourceName}`;
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
