/**
 * The pieces of the enrich sheet that need no component: which articles a law
 * has, which of them already carry `machine_readable`, and how a running
 * scoped enrichment is described in a sentence.
 *
 * Entries group under articles by the rule the worker selects them with
 * (`entries_of` in packages/pipeline/src/enrich.rs) and the editor-api
 * validates a selection with (`top_articles` in enrich_scope.rs): an entry
 * belongs to the shortest existing entry it equals or extends with a dot. A
 * different cut here would offer `69` and `69.1` as two articles while the
 * worker enriches `69.1` as part of `69`.
 */
import type { LawDocument } from './lawFormat';

/** The article `number` belongs to, given every entry number of the law. */
export function articleOf(number: string, entries: ReadonlySet<string>): string {
  for (let i = number.indexOf('.'); i >= 0; i = number.indexOf('.', i + 1)) {
    if (entries.has(number.slice(0, i))) return number.slice(0, i);
  }
  return number;
}

export interface TopArticle {
  number: string;
  /** Whether any of its entries has a `machine_readable` section. */
  hasMachineReadable: boolean;
}

/** The law's articles in document order. */
export function topArticles(law: LawDocument | null | undefined): TopArticle[] {
  const entries = (law?.articles ?? []).map((a) => ({ number: String(a.number), hasMr: !!a.machine_readable }));
  const numbers = new Set(entries.map((e) => e.number));
  const out: TopArticle[] = [];
  for (const { number, hasMr } of entries) {
    const top = articleOf(number, numbers);
    const existing = out.find((a) => a.number === top);
    if (existing) existing.hasMachineReadable ||= hasMr;
    else out.push({ number: top, hasMachineReadable: hasMr });
  }
  return out;
}

/**
 * What a running enrichment covers, for the pane that says it is busy:
 * "artikel 3", "4 artikelen", or the whole law (`null` scope).
 */
export function describeScope(scope: { mode: string | null; articles: string[] | null } | null): string {
  if (!scope || !scope.articles?.length) return 'dit regelwerk';
  if (scope.mode === 'article' || scope.articles.length === 1) return `artikel ${scope.articles[0]}`;
  return `${scope.articles.length} artikelen`;
}
