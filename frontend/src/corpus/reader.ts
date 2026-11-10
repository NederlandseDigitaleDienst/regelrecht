import { ref, type Ref } from 'vue';
import * as yaml from 'js-yaml';
import { createLruMap } from '../lib/lruMap';
import { humanizeLawId, resolveLawName } from '../lib/lawName';
import type { LawDocument } from '../lib/lawFormat';
import type { CorpusTransport } from './transport';
import type {
  CorpusLaw,
  LawBody,
  LawListPage,
  LawListQuery,
  LawOutput,
  LawVersions,
  TrajectRef,
} from './types';

/**
 * The corpus as one traject (or the global corpus) sees it.
 *
 * Reads are cached per traject unless noted: a body from traject A is never
 * handed out for traject B. Every read rejects with an `ApiError` when the
 * request fails.
 */
export interface TrajectCorpusView {
  readonly trajectRef: TrajectRef;
  /**
   * The current version of a rulework. Cached; `fresh` bypasses the cache,
   * for opening a rulework to edit it (the ETag must be current) and for
   * reloading one the server changed.
   */
  law(lawId: string, options?: { fresh?: boolean }): Promise<LawBody>;
  /** Every version, newest first, for the engine to pick the one in force. Cached when non-empty. */
  versions(lawId: string): Promise<LawVersions>;
  /** The outputs a rulework offers to other ruleworks. Not cached. */
  outputs(lawId: string): Promise<LawOutput[]>;
  /** Ids of the ruleworks that implement an open term of `lawId`. Not cached. */
  implementors(lawId: string): Promise<string[]>;
  /** One page of the law list: a search, an id lookup. Not cached. */
  searchLaws(query: LawListQuery): Promise<LawListPage>;
  /** Up to 1000 entries of the law list, for a picker. Cached until a body is recorded. */
  allLaws(): Promise<CorpusLaw[]>;
  /** Ids of the ruleworks edited on the traject's branch; null outside a traject. Not cached. */
  changedLaws(): Promise<string[] | null>;
  /**
   * Record a body the client already has: one it just saved or created, or
   * loaded from a direct URL. Replaces the cached body, drops the cached
   * versions of that rulework and the cached law list, and refreshes the
   * names (showing the old ones meanwhile). A read of this traject that was
   * still running does not overwrite any of it.
   */
  recordBody(lawId: string, yamlText: string, etag: string | null): LawBody;
}

/** The corpus of the active traject: every read goes to the traject current at call time. */
export interface CorpusReader extends TrajectCorpusView {
  /** A rulework's display name in the active traject; reactive, falls back to the readable id. */
  displayName(lawId: string): string;
  /**
   * The corpus of one fixed traject. Only for a caller that loads for a
   * traject the route has not committed yet: `useLaw` during a cross-traject
   * navigation, and recording a save that finished after a switch.
   */
  forTraject(trajectRef: string | null | undefined): TrajectCorpusView;
}

export interface CorpusReaderOptions {
  transport: CorpusTransport;
  /** The active traject. In the app it follows the route; in tests it is a plain ref. */
  trajectRef: Readonly<Ref<string | null | undefined>>;
}

// 50 bodies cover the global corpus plus a handful of trajects times the
// ruleworks a session opens. Versions are fetched per dependency of a
// scenario, so they get more room. Law lists are per traject.
const BODY_CACHE_MAX = 50;
const VERSIONS_CACHE_MAX = 100;
const LIST_CACHE_MAX = 5;
// The backend caps `limit` at 1000 (MAX_LIMIT in corpus_handlers.rs).
const LIST_LIMIT = 1000;
// A failed names list is retried on the next lookup after this long, so a
// render that asks for names does not fire a request per frame.
const NAMES_RETRY_MS = 10_000;

function normalize(trajectRef: string | null | undefined): TrajectRef {
  return trajectRef || null;
}

function scopeKey(trajectRef: TrajectRef): string {
  return trajectRef ?? '';
}

function lawKey(trajectRef: TrajectRef, lawId: string): string {
  return `${scopeKey(trajectRef)}::${lawId}`;
}

// A rulework body is a mapping, or empty.
function parseLaw(text: string): LawDocument | null {
  return yaml.load(text) as LawDocument | null;
}

function makeBody(text: string, etag: string | null): LawBody {
  const law = parseLaw(text);
  return { law, rawYaml: text, lawName: resolveLawName(law), etag };
}

/** One request per key at a time: a second caller joins the first. */
function singleFlight<T>() {
  const pending = new Map<string, Promise<T>>();
  return (key: string, start: () => Promise<T>): Promise<T> => {
    const running = pending.get(key);
    if (running) return running;
    const p = start();
    pending.set(key, p);
    // A failed read must not pin a rejected promise: the next call retries.
    p.catch(() => {}).finally(() => pending.delete(key));
    return p;
  };
}

interface NamesEntry {
  byId: Map<string, CorpusLaw>;
  failedAt: number | null;
  /** A save may have changed a name: refetch, but keep showing these meanwhile. */
  stale: boolean;
}

export function createCorpusReader({ transport, trajectRef }: CorpusReaderOptions): CorpusReader {
  const bodies = createLruMap<string, LawBody>(BODY_CACHE_MAX);
  const versionsCache = createLruMap<string, string[]>(VERSIONS_CACHE_MAX);
  const allLawsCache = createLruMap<string, CorpusLaw[]>(LIST_CACHE_MAX);
  const names = createLruMap<string, NamesEntry>(LIST_CACHE_MAX);
  // Bumped when a names list arrives, so `displayName` re-renders.
  const namesVersion = ref(0);

  // Bumped per traject by `recordBody`. A read that started before the bump
  // still answers its caller but does not store into the cache, so it cannot
  // overwrite what the save recorded; flights are keyed by it too, so a read
  // after the save never joins one from before.
  const generations = new Map<string, number>();
  const generation = (scope: TrajectRef) => generations.get(scopeKey(scope)) ?? 0;

  const bodyFlight = singleFlight<LawBody>();
  const versionsFlight = singleFlight<string[]>();
  const allLawsFlight = singleFlight<CorpusLaw[]>();
  const namesFlight = singleFlight<void>();

  // The ids a body is stored under: the one it was opened by, and its `$id`.
  function bodyIds(lawId: string, body: LawBody): string[] {
    const resolvedId = body.law?.$id;
    return resolvedId && resolvedId !== lawId ? [lawId, resolvedId] : [lawId];
  }

  function storeBody(scope: TrajectRef, lawId: string, body: LawBody): void {
    for (const id of bodyIds(lawId, body)) bodies.set(lawKey(scope, id), body);
  }

  function ensureNames(scope: TrajectRef): NamesEntry | undefined {
    const key = scopeKey(scope);
    const entry = names.get(key);
    const retry = entry?.failedAt != null && Date.now() - entry.failedAt > NAMES_RETRY_MS;
    if (entry && !retry && !entry.stale) return entry;
    const gen = generation(scope);
    namesFlight(`${key}#${gen}`, async () => {
      let next: NamesEntry;
      try {
        const { laws } = await transport.list(scope, { limit: LIST_LIMIT, loaded: true });
        if (laws.length >= LIST_LIMIT) {
          console.warn(`The names list hit the ${LIST_LIMIT}-law cap; names beyond it fall back to the id.`);
        }
        next = { byId: new Map(laws.map((l) => [l.law_id, l])), failedAt: null, stale: false };
      } catch {
        // Names are display sugar: keep what we had, or fall back to readable
        // ids, and retry later.
        next = { byId: entry?.byId ?? new Map(), failedAt: Date.now(), stale: false };
      }
      // A save landed meanwhile: drop this list, and re-evaluate displayName
      // so it asks again.
      if (generation(scope) === gen) names.set(key, next);
      namesVersion.value++;
    });
    return entry;
  }

  function forTraject(rawRef: string | null | undefined): TrajectCorpusView {
    const scope = normalize(rawRef);
    return {
      trajectRef: scope,

      async law(lawId, { fresh = false } = {}) {
        const key = lawKey(scope, lawId);
        if (!fresh) {
          const cached = bodies.get(key);
          if (cached) return cached;
        }
        const gen = generation(scope);
        return bodyFlight(`${key}#${gen}`, async () => {
          const { text, etag } = await transport.law(scope, lawId);
          const body = makeBody(text, etag);
          if (generation(scope) === gen) storeBody(scope, lawId, body);
          return body;
        });
      },

      async versions(lawId) {
        const key = lawKey(scope, lawId);
        const cached = versionsCache.get(key);
        if (cached) return { trajectRef: scope, yamls: cached };
        const gen = generation(scope);
        const yamls = await versionsFlight(`${key}#${gen}`, async () => {
          const list = await transport.versions(scope, lawId);
          // An empty list (an unknown or not yet harvested rulework) stays
          // uncached, so a retry after a harvest finds the new versions.
          if (list.length > 0 && generation(scope) === gen) versionsCache.set(key, list);
          return list;
        });
        return { trajectRef: scope, yamls };
      },

      outputs: (lawId) => transport.outputs(scope, lawId),
      implementors: (lawId) => transport.implementors(scope, lawId),
      searchLaws: (query) => transport.list(scope, query),

      async allLaws() {
        const key = scopeKey(scope);
        const cached = allLawsCache.get(key);
        if (cached) return cached;
        const gen = generation(scope);
        return allLawsFlight(`${key}#${gen}`, async () => {
          const { laws } = await transport.list(scope, { limit: LIST_LIMIT });
          if (generation(scope) === gen) allLawsCache.set(key, laws);
          return laws;
        });
      },

      async changedLaws() {
        // Only a traject has a branch to compare against.
        if (!scope) return null;
        return transport.changedLaws(scope);
      },

      recordBody(lawId, yamlText, etag) {
        const body = makeBody(yamlText, etag);
        const key = scopeKey(scope);
        generations.set(key, generation(scope) + 1);
        storeBody(scope, lawId, body);
        // The saved version replaces one of the versions the engine loads.
        for (const id of bodyIds(lawId, body)) versionsCache.delete(lawKey(scope, id));
        allLawsCache.delete(key);
        const namesEntry = names.peek(key);
        if (namesEntry) {
          namesEntry.stale = true;
          // Re-evaluate displayName, which refetches while showing the old names.
          namesVersion.value++;
        }
        return body;
      },
    };
  }

  const active = () => forTraject(trajectRef.value);

  return {
    get trajectRef() {
      return normalize(trajectRef.value);
    },
    law: (lawId, options) => active().law(lawId, options),
    versions: (lawId) => active().versions(lawId),
    outputs: (lawId) => active().outputs(lawId),
    implementors: (lawId) => active().implementors(lawId),
    searchLaws: (query) => active().searchLaws(query),
    allLaws: () => active().allLaws(),
    changedLaws: () => active().changedLaws(),
    recordBody: (lawId, yamlText, etag) => active().recordBody(lawId, yamlText, etag),

    displayName(lawId) {
      // Read for reactivity: re-run when a list arrives or the traject changes.
      void namesVersion.value;
      const law = ensureNames(normalize(trajectRef.value))?.byId.get(lawId);
      return law?.display_name || law?.name || humanizeLawId(lawId);
    },

    forTraject,
  };
}
