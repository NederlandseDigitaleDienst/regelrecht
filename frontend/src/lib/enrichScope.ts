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
import type { EnrichOptions } from '../composables/useTasks';

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

/** What a request across the chain shares: provider, model, rounds. */
export interface CommonEnrichChoices {
  provider?: string;
  model?: string;
  feedbackRounds?: number;
}

/** One enrich request for one law, as the chain flow sends it. */
export interface LawEnrichRequest {
  lawId: string;
  options: EnrichOptions;
}

/**
 * The requests for a selection across the chain: one per law, because the
 * server runs one enrichment per law and traject at a time.
 *
 * The law the engineer started from gets the article mode, with the start
 * article first and any other ticked article of that law after it, the
 * instruction, and the stelselcontext. Every other law gets the law mode over
 * its ticked articles, with the stelselcontext and without the instruction,
 * which the server accepts only in the article mode. Laws go in the order the
 * selection lists them, the start law first; articles keep that order too.
 */
export function planChainRequests(
  start: { lawId: string; article: string },
  selected: ReadonlyMap<string, readonly string[]>,
  common: CommonEnrichChoices,
  instruction?: string,
): LawEnrichRequest[] {
  const shared: EnrichOptions = {
    ...(common.provider ? { provider: common.provider } : {}),
    ...(common.model ? { model: common.model } : {}),
    ...(common.feedbackRounds !== undefined ? { feedbackRounds: common.feedbackRounds } : {}),
  };
  const startExtras = (selected.get(start.lawId) ?? []).filter((a) => a !== start.article);
  const requests: LawEnrichRequest[] = [
    {
      lawId: start.lawId,
      options: {
        ...shared,
        mode: 'article',
        articles: [start.article, ...startExtras],
        instruction: instruction?.trim() || undefined,
        context: true,
      },
    },
  ];
  for (const [lawId, articles] of selected) {
    if (lawId === start.lawId || articles.length === 0) continue;
    requests.push({ lawId, options: { ...shared, mode: 'law', articles: [...articles], context: true } });
  }
  return requests;
}

/** Above these, the confirmation step says how large the run is. */
export const LARGE_RUN_LAWS = 5;
export const LARGE_RUN_ARTICLES = 40;

/** Whether a selection is large enough to warn about (never to block). */
export function isLargeRun(requests: readonly LawEnrichRequest[]): boolean {
  const articles = requests.reduce((n, r) => n + (r.options.articles?.length ?? 0), 0);
  return requests.length > LARGE_RUN_LAWS || articles > LARGE_RUN_ARTICLES;
}

/** "3 artikelen in 2 wetten". */
export function selectionSummary(articles: number, laws: number): string {
  const a = articles === 1 ? '1 artikel' : `${articles} artikelen`;
  const l = laws === 1 ? '1 wet' : `${laws} wetten`;
  return `${a} in ${l}`;
}
