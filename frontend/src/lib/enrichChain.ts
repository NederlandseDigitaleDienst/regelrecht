/**
 * The chain of an article and the hotspots of a traject, as the editor-api
 * returns them (`packages/editor-api/src/enrich_chain.rs`), and the words for
 * them. Pure functions, so the wording and the grouping are tested without
 * mounting the sheet. Law names come in as a function (useCorpus'
 * displayName), because the API carries only the `$id`.
 */

export type EdgeKind = 'cites' | 'binds' | 'implements';

export interface ChainVia {
  law_id: string;
  article: string;
  kind: EdgeKind;
}

export interface NodeGaps {
  missing_mr: boolean;
  dangling: number;
  unresolved: number;
  open_terms: number;
}

/** One signal behind a score (Reason in enrich_chain.rs). */
export interface HotspotReason {
  /** M, D, U, O, B, R or K. */
  signal: string;
  points: number;
  /** The reason in a few Dutch words, without the points. */
  detail: string;
  /** The articles it is about, `"<law>#<article>"`. */
  refs: string[];
}

export interface HotspotScore {
  score: number;
  reasons: HotspotReason[];
}

export interface ChainNode {
  law_id: string;
  bwb_id: string;
  article: string;
  depth: number;
  has_mr: boolean;
  gaps: NodeGaps;
  via: ChainVia | null;
  hotspot: HotspotScore | null;
}

export interface ChainGap {
  kind: string;
  bwb_id: string;
  occurrences: number;
}

export interface ChainPartial {
  unread_laws: number;
  depth_capped: boolean;
  node_capped: boolean;
}

/** `GET …/articles/{article}/chain`. */
export interface Chain {
  root: { law_id: string; article: string };
  nodes: ChainNode[];
  gaps: ChainGap[];
  partial: ChainPartial;
}

/** One hotspot of the traject (TrajectHotspot). */
export interface TrajectHotspot {
  law_id: string;
  bwb_id: string;
  article: string;
  score: number;
  reasons: HotspotReason[];
  /** Articles elsewhere that bind to or cite it. */
  dependants: number;
}

/** `GET /api/trajects/{ref}/hotspots`. */
export interface Hotspots {
  hotspots: TrajectHotspot[];
  per_article: Record<string, { score: number; count: number }>;
  partial: boolean;
}

/** The depths the chain step offers; "alles" is the server's maximum. */
export const CHAIN_DEPTHS = [
  { value: 1, label: '1' },
  { value: 2, label: '2' },
  { value: 3, label: '3' },
  { value: 6, label: 'Alles' },
] as const;
export const DEFAULT_CHAIN_DEPTH = 3;

type LawName = (lawId: string) => string;

export function chainUrl(trajectRef: string, lawId: string, article: string, depth: number): string {
  return (
    `/api/trajects/${encodeURIComponent(trajectRef)}/corpus/laws/${encodeURIComponent(lawId)}` +
    `/articles/${encodeURIComponent(article)}/chain?depth=${depth}`
  );
}

export function hotspotsUrl(trajectRef: string, limit: number, lawId?: string | null): string {
  const query = new URLSearchParams({ limit: String(limit) });
  if (lawId) query.set('law_id', lawId);
  return `/api/trajects/${encodeURIComponent(trajectRef)}/hotspots?${query}`;
}

/** `"<law>#<article>"`, the key the API and the selection use. */
export function articleKey(lawId: string, article: string): string {
  return `${lawId}#${article}`;
}

/** "Awir art. 8" for a key, or "art. 8" inside the law the reader is in. */
export function refText(key: string, lawName: LawName, inLaw?: string): string {
  const at = key.lastIndexOf('#');
  const lawId = at >= 0 ? key.slice(0, at) : key;
  const article = at >= 0 ? key.slice(at + 1) : '';
  return lawId === inLaw ? `art. ${article}` : `${lawName(lawId)} art. ${article}`;
}

/**
 * The path from the root to a node: "via art. 2 → Awir art. 8". A hop
 * names its law only when it leaves the law of the hop before it. The root
 * itself has no path.
 */
export function pathText(node: ChainNode, chain: Chain, lawName: LawName): string | null {
  if (!node.via) return null;
  const byKey = new Map(chain.nodes.map((n) => [articleKey(n.law_id, n.article), n]));
  const hops: ChainNode[] = [node];
  let current: ChainNode | undefined = node;
  // Bounded by the number of nodes: a via always points at a node taken
  // earlier, but a malformed answer must not loop.
  for (let i = 0; current?.via && i < chain.nodes.length; i += 1) {
    current = byKey.get(articleKey(current.via.law_id, current.via.article));
    if (current) hops.unshift(current);
  }
  const parts: string[] = [];
  let law = chain.root.law_id;
  for (const hop of hops) {
    parts.push(hop.law_id === law ? `art. ${hop.article}` : `${lawName(hop.law_id)} art. ${hop.article}`);
    law = hop.law_id;
  }
  return `via ${parts.join(' → ')}`;
}

/** The gaps of a node as short tag texts. */
export function gapTags(gaps: NodeGaps): string[] {
  const tags: string[] = [];
  if (gaps.missing_mr) tags.push('Geen machine_readable');
  if (gaps.dangling) tags.push(gaps.dangling === 1 ? '1 losse binding' : `${gaps.dangling} losse bindingen`);
  if (gaps.unresolved) tags.push(gaps.unresolved === 1 ? '1 binding komt niet aan' : `${gaps.unresolved} bindingen komen niet aan`);
  if (gaps.open_terms) tags.push(gaps.open_terms === 1 ? '1 open term zonder invulling' : `${gaps.open_terms} open termen zonder invulling`);
  return tags;
}

/** One reason as a sentence: "4 artikelen elders lezen dit: Awir art. 8, … (+8)". */
export function reasonText(reason: HotspotReason, lawName: LawName): string {
  const named = reason.refs.slice(0, 3).map((key) => refText(key, lawName));
  const more = reason.refs.length > 3 ? ', …' : '';
  const list = named.length ? `: ${named.join(', ')}${more}` : '';
  return `${reason.detail}${list} (+${reason.points})`;
}

/** The first reason of a score, for a row's supporting text. */
export function topReason(reasons: HotspotReason[], lawName: LawName): string {
  return reasons.length ? reasonText(reasons[0]!, lawName) : '';
}

/** The chain grouped per law and depth, in the order the API gave them. */
export interface ChainGroup {
  key: string;
  lawId: string;
  depth: number;
  nodes: ChainNode[];
}

export function groupChain(chain: Chain): ChainGroup[] {
  const groups: ChainGroup[] = [];
  const byKey = new Map<string, ChainGroup>();
  for (const node of chain.nodes) {
    const key = `${node.depth}:${node.law_id}`;
    let group = byKey.get(key);
    if (!group) {
      group = { key, lawId: node.law_id, depth: node.depth, nodes: [] };
      byKey.set(key, group);
      groups.push(group);
    }
    group.nodes.push(node);
  }
  return groups;
}

/** Counts for the summary line of the chain step. */
export function chainCounts(chain: Chain): { articles: number; laws: number; missing: number; hotspots: number } {
  return {
    articles: chain.nodes.length,
    laws: new Set(chain.nodes.map((n) => n.law_id)).size,
    missing: chain.nodes.filter((n) => n.gaps.missing_mr).length,
    hotspots: chain.nodes.filter((n) => n.hotspot).length,
  };
}

/**
 * What the chain step preselects: the root, and every hotspot on the chain.
 * Keys are `"<law>#<article>"`.
 */
export function preselection(chain: Chain): Set<string> {
  const keys = new Set<string>([articleKey(chain.root.law_id, chain.root.article)]);
  for (const node of chain.nodes) {
    if (node.hotspot) keys.add(articleKey(node.law_id, node.article));
  }
  return keys;
}

/**
 * What the article mode suggests besides the start article: the articles on
 * its chain that could use an enrichment, the hotspots first (highest score
 * first), then those without `machine_readable`, nearest first. Articles that
 * are fine as they are are left out: a suggestion is a reason to tick.
 */
export function chainSuggestions(chain: Chain): ChainNode[] {
  const rootKey = articleKey(chain.root.law_id, chain.root.article);
  return chain.nodes
    .map((node, index) => ({ node, index }))
    .filter(({ node }) => articleKey(node.law_id, node.article) !== rootKey)
    .filter(({ node }) => node.hotspot || !node.has_mr)
    .sort(
      (a, b) =>
        (b.node.hotspot?.score ?? -1) - (a.node.hotspot?.score ?? -1) ||
        a.node.depth - b.node.depth ||
        a.index - b.index,
    )
    .map(({ node }) => node);
}

/**
 * A chain of the start article alone, for when the real chain could not be
 * fetched: the flow can still continue with just this article instead of
 * stalling on step 1.
 */
export function rootOnlyChain(lawId: string, article: string, hasMr: boolean): Chain {
  return {
    root: { law_id: lawId, article },
    nodes: [
      {
        law_id: lawId,
        bwb_id: '',
        article,
        depth: 0,
        has_mr: hasMr,
        gaps: { missing_mr: !hasMr, dangling: 0, unresolved: 0, open_terms: 0 },
        via: null,
        hotspot: null,
      },
    ],
    gaps: [],
    partial: { unread_laws: 0, depth_capped: false, node_capped: false },
  };
}

/**
 * What to tell the user when the chain request failed: the server's own
 * reason when it gave one (a 404 says which of law or article it could not
 * find), else a generic line with the status.
 */
export function chainErrorText(error: unknown): string {
  const status = (error as { status?: number } | null)?.status;
  const body = ((error as { body?: string } | null)?.body ?? '').trim();
  if (status && status < 500 && body && body.length <= 300) {
    return `De keten kon niet worden opgehaald: ${body}`;
  }
  return status ? `De keten kon niet worden opgehaald (status ${status}).` : 'De keten kon niet worden opgehaald.';
}
