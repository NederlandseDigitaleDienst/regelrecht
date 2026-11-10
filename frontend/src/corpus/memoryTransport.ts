import { ApiError } from '../lib/apiFetch';
import type { CorpusTransport } from './transport';
import type { CorpusLaw, LawListPage, LawListQuery, LawOutput, TrajectRef } from './types';

/**
 * What one corpus (the global one or a traject's) holds in a test.
 *
 * The adapter does not compute what the backend computes: outputs,
 * implementors and changed laws are given, never derived from the YAML.
 */
export interface MemoryCorpus {
  /** Rulework id -> its versions' YAML, newest first; a string is one version. */
  laws?: Record<string, string | string[]>;
  /** ETag per rulework id, returned with its current version. */
  etags?: Record<string, string>;
  outputs?: Record<string, LawOutput[]>;
  implementors?: Record<string, string[]>;
  /** The law list entries; defaults to one bare entry per id in `laws`. */
  list?: CorpusLaw[];
  changedLaws?: string[];
  /** Make a read fail with this HTTP status, keyed like `calls` (`'law wet_x'`, `'list'`). */
  failures?: Record<string, number>;
}

export interface MemoryTransport extends CorpusTransport {
  /** The corpora by traject ref; `''` is the global corpus. Tests may change them. */
  corpora: Record<string, MemoryCorpus>;
  /** Every read in order, as `'<traject or "-"> <read> [<law id>]'`. */
  calls: string[];
}

function scopeKey(trajectRef: TrajectRef): string {
  return trajectRef ?? '';
}

/** Filter a law list the way a test needs it; not the backend's ranking. */
function filterList(laws: readonly CorpusLaw[], query: LawListQuery): LawListPage {
  let found = [...laws];
  if (query.ids) {
    const wanted = new Set(query.ids);
    found = found.filter((l) => wanted.has(l.law_id));
  }
  if (query.q) {
    const words = query.q.toLowerCase().split(/\s+/).filter(Boolean);
    found = found.filter((l) => {
      const haystack = `${l.law_id} ${l.name ?? ''} ${l.display_name ?? ''}`.toLowerCase();
      return words.every((w) => haystack.includes(w));
    });
  }
  const total = found.length;
  return { laws: query.limit === undefined ? found : found.slice(0, query.limit), total };
}

/** An in-memory corpus for tests, per traject. A missing rulework answers 404. */
export function createMemoryTransport(corpora: Record<string, MemoryCorpus> = {}): MemoryTransport {
  const calls: string[] = [];

  // Logs the read and resolves the corpus, or throws the configured failure.
  function enter(trajectRef: TrajectRef, read: string, lawId?: string): MemoryCorpus {
    const key = lawId === undefined ? read : `${read} ${lawId}`;
    calls.push(`${trajectRef ?? '-'} ${key}`);
    const corpus = transport.corpora[scopeKey(trajectRef)];
    if (!corpus) throw new ApiError(`No corpus for traject ${trajectRef}`, { status: 404 });
    const failure = corpus.failures?.[key];
    if (failure) throw new ApiError(`${key} failed: ${failure}`, { status: failure });
    return corpus;
  }

  function versionsOf(corpus: MemoryCorpus, lawId: string): string[] {
    const entry = corpus.laws?.[lawId];
    if (entry === undefined) throw new ApiError(`Failed to fetch: 404`, { status: 404 });
    return Array.isArray(entry) ? entry : [entry];
  }

  const transport: MemoryTransport = {
    corpora,
    calls,

    async law(trajectRef, lawId) {
      const corpus = enter(trajectRef, 'law', lawId);
      const [current = ''] = versionsOf(corpus, lawId);
      return { text: current, etag: corpus.etags?.[lawId] ?? null };
    },

    async versions(trajectRef, lawId) {
      const corpus = enter(trajectRef, 'versions', lawId);
      return corpus.laws?.[lawId] === undefined ? [] : versionsOf(corpus, lawId);
    },

    async outputs(trajectRef, lawId) {
      return enter(trajectRef, 'outputs', lawId).outputs?.[lawId] ?? [];
    },

    async implementors(trajectRef, lawId) {
      return enter(trajectRef, 'implementors', lawId).implementors?.[lawId] ?? [];
    },

    async list(trajectRef, query) {
      const corpus = enter(trajectRef, 'list');
      const entries = corpus.list ?? Object.keys(corpus.laws ?? {}).map((law_id) => ({ law_id }));
      return filterList(entries, query);
    },

    async changedLaws(trajectRef) {
      return enter(trajectRef, 'changedLaws').changedLaws ?? [];
    },
  };
  return transport;
}
