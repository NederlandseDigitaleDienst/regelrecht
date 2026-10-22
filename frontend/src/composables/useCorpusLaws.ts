import { ref, computed, watch, isRef, type Ref } from 'vue';
import { lawsListUrl } from './corpusUrls';
import { apiFetchJson } from '../lib/apiFetch';
import { createLruMap } from '../lib/lruMap';
import { humanizeLawId } from '../lib/lawName';

/**
 * Shared corpus-laws list, scoped by the active traject. Each distinct
 * traject (and the global view) gets its own cached payload - switching
 * trajects routes through the corresponding `/api/trajects/{ref}/corpus/laws`
 * endpoint without losing the previous traject's data on rebound.
 *
 * Module-level state - every consumer for the same `trajectRef` shares
 * the same list/promise.
 */

// Backend hard-caps `?limit` at 1000 (see editor-api/corpus_handlers.rs
// MAX_LIMIT). With the current curated Test Corpus this is far above
// what's loaded, but if the corpus ever grows past 1000, MachineReadable
// input rows that reference an overflow law would silently fall back to
// the title-cased snake_case identifier. Warn loudly when we hit the cap
// so the gap is visible rather than silently broken.
const FETCH_LIMIT = 1000;

/** `CorpusLawEntry` in packages/editor-api/src/corpus_handlers.rs. */
export interface CorpusLaw {
  law_id: string;
  name?: string | null;
  display_name?: string | null;
  source_id?: string;
  source_name?: string;
  source_priority?: number;
  /** The current version carries `machine_readable` (see the backend field). */
  enriched?: boolean;
  /** When the current version is not enriched but another is: that version's date. */
  enriched_version?: string | null;
}

type TrajectRef = string | null | undefined;

// LRU cap on the per-scope caches. Without one a long session that
// hops between many trajects accumulates one entry per scope forever.
// 5 is enough to cover the global view + a handful of recently-used
// trajects without thrashing. The global scope (`''`) follows the same
// rules - that's fine because it just gets re-fetched on demand. The
// companion promise map is kept in sync via `onEvict`.
const SCOPE_CACHE_MAX = 5;

const fetchByScope = new Map<string, Promise<CorpusLaw[]>>(); // scopeKey -> Promise
// In-flight `refresh()` per scope. A second refresh while one is running
// joins it instead of deleting the first one's promise out of
// `fetchByScope` mid-flight and firing yet another request.
const refreshByScope = new Map<string, Promise<CorpusLaw[]>>(); // scopeKey -> Promise
const lawsByScope = createLruMap<string, Ref<CorpusLaw[]>>(SCOPE_CACHE_MAX, {
  onEvict: (key) => fetchByScope.delete(key), // scopeKey -> Ref<Array>
});

function scopeKey(trajectRef: TrajectRef): string {
  return trajectRef || '';
}

function ensureFetched(trajectRef: TrajectRef): Promise<CorpusLaw[]> {
  const key = scopeKey(trajectRef);
  if (fetchByScope.has(key)) {
    // Cache hit: `get` touches, so the LRU treats this access as recent.
    // Without this an often-accessed scope can be evicted before a
    // genuinely-stale one that happened to be added more recently -
    // the invariant "most recently used is kept" only holds if the
    // touch runs on every access, not just on miss.
    lawsByScope.get(key);
    // has() just confirmed the entry.
    return fetchByScope.get(key)!;
  }
  if (!lawsByScope.has(key)) lawsByScope.set(key, ref<CorpusLaw[]>([]));
  // Set just above when missing; SCOPE_CACHE_MAX > 0, so it is not evicted.
  const lawsRef = lawsByScope.get(key)!;
  const p = apiFetchJson<unknown>(lawsListUrl(trajectRef, `limit=${FETCH_LIMIT}`))
    .then(list => {
      lawsRef.value = Array.isArray(list) ? (list as CorpusLaw[]) : [];
      if (lawsRef.value.length >= FETCH_LIMIT) {
        console.warn(
          `useCorpusLaws: hit the ${FETCH_LIMIT}-law cap - laws beyond this won't resolve to display names. ` +
          `Pagination needs to be added if the corpus has grown past ${FETCH_LIMIT} entries.`,
        );
      }
      return lawsRef.value;
    })
    .catch(() => {
      // Swallowed deliberately: the laws list is display-sugar (names in
      // dropdowns); callers fall back to title-cased ids on an empty list.
      lawsRef.value = [];
      // Reset so the next consumer mount triggers a fresh fetch.
      fetchByScope.delete(key);
      return [] as CorpusLaw[];
    });
  fetchByScope.set(key, p);
  return p;
}

/**
 * @param {import('vue').Ref<string|null>=} trajectRef Optional active
 *   traject reference. Without it the global view is used.
 */
export function useCorpusLaws(trajectRef?: Readonly<Ref<TrajectRef>> | null) {
  // A plain string passed here gets silently wrapped in a static
  // `ref(value)` that never reacts to the caller's scope changes -
  // which would look like "the corpus list is stuck" without any
  // error. All current callers pass a `Ref` (via `toRef(props, ...)`
  // or `computed`); a dev-mode warning catches future misuses before
  // they ship as a silent regression.
  if (trajectRef !== undefined && trajectRef !== null && !isRef(trajectRef)) {
    console.warn(
      'useCorpusLaws: trajectRef should be a Ref, got plain value - list will not react to scope changes',
      trajectRef,
    );
  }
  const refSource = isRef(trajectRef)
    ? trajectRef
    : ref(trajectRef ?? null);
  const laws = ref<CorpusLaw[]>([]);

  watch(
    refSource,
    (current) => {
      const key = scopeKey(current);
      ensureFetched(current);
      // Capture the underlying Ref BEFORE attaching the .then. If
      // the LRU evicts `key` while its fetch is still in flight
      // (6+ distinct scopes hit during the window), a later
      // `lawsByScope.peek(key)?.value` would return `undefined` and
      // we'd silently set `laws.value = []` instead of the
      // fetched list. The closure over `capturedRef` keeps the
      // populated ref reachable even after the map entry is gone.
      // `peek` (not `get`): ensureFetched above already touched this
      // scope; a second read here must not double-bump the LRU order.
      const capturedRef = lawsByScope.peek(key);
      laws.value = capturedRef?.value ?? [];
      const p = fetchByScope.get(key);
      if (p && capturedRef) {
        p.then(() => {
          // Only commit if this is still the active scope by the time
          // the fetch resolves - avoids a cross-scope late write.
          if (scopeKey(refSource.value) === key) {
            laws.value = capturedRef.value;
          }
        });
      }
    },
    { immediate: true },
  );

  const lawsById = computed(() => {
    const map = new Map<string, CorpusLaw>();
    for (const law of laws.value) map.set(law.law_id, law);
    return map;
  });

  function displayName(lawId: string): string {
    const law = lawsById.value.get(lawId);
    if (law?.display_name) return law.display_name;
    if (law?.name) return law.name;
    return humanizeLawId(lawId);
  }

  /**
   * Bust the active scope's cache and re-fetch (e.g. after a corpus
   * reload added a law server-side). Updates this consumer's `laws`;
   * other already-mounted consumers of the same scope keep their
   * current snapshot until they re-scope or remount (the shared scope
   * Ref is updated, so any *new* consumer sees the fresh list).
   */
  async function refresh(): Promise<CorpusLaw[]> {
    // Snapshot the scope at call time; `refSource` may re-scope while the
    // awaits below are in flight, and the bust/re-fetch must stay on the
    // scope the caller asked to refresh.
    const scope = refSource.value;
    const key = scopeKey(scope);
    // Single-flight per scope: concurrent refreshes share one re-fetch
    // instead of each busting the slot and stacking duplicate GETs.
    let p = refreshByScope.get(key);
    if (!p) {
      p = (async () => {
        // Serialize behind whatever fetch currently owns the slot (a
        // settled promise resolves immediately). Busting an *in-flight*
        // fetch would orphan it - firing the duplicate concurrent GET
        // this file exists to avoid - and let its late settle race the
        // refreshed list with stale data.
        const current = fetchByScope.get(key);
        if (current) await current;
        fetchByScope.delete(key);
        return ensureFetched(scope);
      })();
      refreshByScope.set(key, p);
      p.catch(() => {}).finally(() => refreshByScope.delete(key));
    }
    const capturedRef = lawsByScope.peek(key);
    await p;
    // Same stale-scope guard as the watcher: only commit if the caller's
    // scope is still the active one by the time the fetch resolves.
    if (capturedRef && scopeKey(refSource.value) === key) {
      laws.value = capturedRef.value;
    }
    return laws.value;
  }

  return { laws, displayName, refresh };
}
