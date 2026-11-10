import { computed, ref, shallowRef } from 'vue';
import * as yaml from 'js-yaml';
import { lastSavedPr, sanitizeSavedPr } from './useSavedPr';
import { lawCreateUrl, lawUrl } from './corpusUrls';
import { apiFetch } from '../lib/apiFetch';
import type { RequestError } from '../lib/apiFetch';
import { useCorpus, type LawBody } from '../corpus';
import { useLatest } from '../lib/useLatest';
import { resolveLawName } from '../lib/lawName';
export { resolveLawName } from '../lib/lawName';
import { holdRetryFloor } from '../lib/retryFeedback';
import type { LawArticle, LawDocument } from '../lib/lawFormat';
export type { LawArticle, LawDocument } from '../lib/lawFormat';

/** A rulework body as the corpus reader hands it out. */
export type LawCacheEntry = LawBody;

type TrajectRef = string | null | undefined;

/** The JSON body of a law save/create (`{ pr, etag }`). */
type SaveResponse = { pr?: unknown; etag?: string | null } | null;

// yaml.load returns `unknown`; a law body is a mapping (or empty -> null/undefined).
function parseLaw(text: string): LawDocument | null {
  return yaml.load(text) as LawDocument | null;
}

// Shared apiFetch options for law GETs. The thrown ApiError carries the
// HTTP status so callers can branch on `.status === 404`; the message
// keeps its historical shape.
export const lawFetchInit = Object.freeze({
  errorMessage: (status: number) => `Failed to fetch: ${status}`,
});

/**
 * Fetch a law's YAML, possibly from cache, through the given traject.
 *
 * @param {string|null} trajectRef - traject to read (null = global read)
 * @param {string} lawId
 */
export async function fetchLaw(trajectRef: TrajectRef, lawId: string): Promise<LawCacheEntry> {
  return useCorpus().forTraject(trajectRef).law(lawId);
}

export function useLaw(
  lawParam?: string | null,
  articleParam?: string | null,
  trajectRefParam?: string | null,
) {
  // No law in the route: the `?law=` query may still carry one (the traject
  // chooser passes it through). Otherwise we open NOTHING - deliberately no
  // hardcoded fallback law. A default here silently loaded that law, resolved
  // its first article, and the editor's add-tab watch turned it into a real
  // tab - so `/editor/` always opened "Artikel 1" of the default law and even
  // resurrected that tab after the user closed it. With no law, the caller
  // (EditorView) decides what to open, e.g. by restoring the last active tab.
  if (!lawParam) {
    const params = new URLSearchParams(window.location.search);
    lawParam = params.get('law') || null;
  }
  const initialArticle = articleParam || null;
  // useLaw reads through an explicit traject, not the route's: during a
  // cross-traject navigation it loads for the traject the route has not
  // committed yet (see `lawTrajectRef`).
  const corpus = useCorpus();
  // Current traject id for this composable instance. `switchLaw` may
  // update this when navigation crosses trajects, so URL builders read
  // through the closure instead of capturing a snapshot.
  let currentTrajectRef: string | null = trajectRefParam || null;
  // Reactive mirror of `currentTrajectRef`, kept in sync wherever it is
  // reassigned. Consumers use this to learn which traject the *open law* was
  // loaded through - which is NOT necessarily the route's active traject
  // during a cross-traject navigation: `switchLaw` updates it a tick before
  // the route (and thus `activeTrajectRef`) commits, so a watcher on the law
  // can tell "this law belongs to the traject I'm loading into" apart from
  // "…the traject I'm still leaving".
  const lawTrajectRef = ref(currentTrajectRef);
  // If the parameter looks like a URL, fetch directly; otherwise build
  // the API URL from the current trajectRef.
  const initialDirectUrl =
    lawParam && (lawParam.startsWith('/') || lawParam.startsWith('http'))
      ? lawParam
      : null;
  const law = shallowRef<LawDocument | null>(null);
  const rawYaml = ref('');
  const selectedArticleNumber = ref<string | null>(null);
  const loading = ref(true);
  // apiFetch rejects with an ApiError and the 412 path throws an Error, so
  // the caught values stored here are Errors.
  const error = ref<RequestError | null>(null);
  const saving = ref(false);
  const saveError = ref<RequestError | null>(null);
  // ETag of the last loaded/saved law content; sent as `If-Match` on
  // save so the backend can 412 when someone else edited in between.
  const currentEtag = ref<string | null>(null);

  const articles = computed((): LawArticle[] => law.value?.articles ?? []);

  const lawName = computed(() => resolveLawName(law.value));

  const selectedArticle = computed(() => {
    if (!selectedArticleNumber.value) return null;
    return articles.value.find(
      (a) => String(a.number) === String(selectedArticleNumber.value)
    ) ?? null;
  });

  // Shared guard for `load()` and `switchLaw()`; stale awaits discard their writes.
  const claimSwitch = useLatest();

  async function load(): Promise<void> {
    const isCurrent = claimSwitch();
    // Snapshot before any await: a concurrent `switchLaw` mutates
    // `currentTrajectRef`, and the direct-URL cache write below must key
    // on the traject this load was issued for - not whichever traject the
    // user has navigated to by the time the response body arrives.
    const loadTrajectRef = currentTrajectRef;
    try {
      loading.value = true;
      let entry: LawCacheEntry;
      if (initialDirectUrl) {
        // Direct URL: not a corpus route, so fetch inline and record the
        // body under its resolved `$id`.
        const res = await apiFetch(initialDirectUrl, lawFetchInit);
        if (!isCurrent()) return;
        const text = await res.text();
        // Deliberately no second staleness check between `.text()` and the
        // record: the content is fresh either way and the traject is
        // snapshotted above, so recording it is right even if this load went
        // stale mid-flight; the final `isCurrent()` below still guards all
        // component state.
        // load() only runs while lawParam is set (see the call below).
        const resolvedId = parseLaw(text)?.$id || (lawParam as string);
        entry = corpus.forTraject(loadTrajectRef).recordBody(resolvedId, text, res.headers.get('ETag'));
      } else {
        // Fresh fetch (never the cache): opening a rulework to edit it needs
        // the current ETag. Shared with any concurrent read of the same law,
        // so a mount doesn't fire duplicate GETs.
        // load() only runs while lawParam is set (see the call below).
        entry = await corpus.forTraject(loadTrajectRef).law(lawParam as string, { fresh: true });
      }
      if (!isCurrent()) return;
      rawYaml.value = entry.rawYaml;
      law.value = entry.law;
      currentEtag.value = entry.etag;
      if (articles.value.length > 0 && !selectedArticleNumber.value) {
        if (initialArticle && articles.value.some(a => String(a.number) === initialArticle)) {
          selectedArticleNumber.value = initialArticle;
        } else {
          // length > 0 was checked above.
          selectedArticleNumber.value = String(articles.value[0]!.number);
        }
      }
    } catch (e) {
      if (!isCurrent()) return;
      error.value = e as RequestError;
    } finally {
      if (isCurrent()) {
        loading.value = false;
      }
    }
  }

  // With no law to open there is nothing to fetch: settle `loading` right away
  // so the caller shows its empty state instead of an indicator that never
  // resolves. `switchLaw` still loads on demand once a tab is picked.
  if (lawParam) load();
  else loading.value = false;

  // Derive the law ID from the parsed law or the original param (null when no
  // law is open, which the editor's add-tab watch guards on).
  const lawId = computed(() => law.value?.$id || lawParam);

  /**
   * Re-load the open law's content, optionally switching law id /
   * article / traject in one step. Passing `newTrajectRef` is how a
   * cross-traject URL change drives a fresh fetch (and the cache key
   * keeps the previous traject's copy untouched).
   *
   * `claimSwitch` is a SHARED staleness token across every caller
   * (`selectTab`, `onBeforeRouteUpdate`, `watch(activeTrajectRef)`,
   * `restoreForTraject`, retry): a later call supersedes an earlier one
   * regardless of which site started it. On supersession this returns `false`
   * WITHOUT touching `law.value`/`error.value` (the winning call owns those);
   * on a completed load (success OR its own error) it returns `true`. A caller
   * whose own guard can't tell "someone else's call won the shared race" apart
   * from "my call finished" must check this so it doesn't act on refs that now
   * reflect a different request.
   *
   * @returns {Promise<boolean>} whether THIS call was the one that completed.
   */
  async function switchLaw(
    newLawId: string,
    articleNumber?: string | number | null,
    newTrajectRef?: string | null,
    { minLoadingMs = 0 }: { minLoadingMs?: number } = {},
  ): Promise<boolean> {
    const isCurrent = claimSwitch();
    const startedAt = Date.now();
    let failed = false;
    try {
      loading.value = true;
      error.value = null;
      saveError.value = null;
      saving.value = false;
      if (newTrajectRef !== undefined) {
        currentTrajectRef = newTrajectRef || null;
        lawTrajectRef.value = currentTrajectRef;
      }
      const entry = await corpus.forTraject(currentTrajectRef).law(newLawId);
      if (!isCurrent()) return false;
      law.value = entry.law;
      rawYaml.value = entry.rawYaml;
      currentEtag.value = entry.etag ?? null;
      if (articleNumber) {
        selectedArticleNumber.value = String(articleNumber);
      } else if (articles.value.length > 0) {
        // length > 0 was checked above.
        selectedArticleNumber.value = String(articles.value[0]!.number);
      }
    } catch (e) {
      if (!isCurrent()) return false;
      failed = true;
      error.value = e as RequestError;
    } finally {
      if (isCurrent()) {
        // On a failed retry, hold the spinner briefly so the click reads as
        // feedback instead of the error dialog snapping straight back.
        await holdRetryFloor({ startedAt, minMs: minLoadingMs, failed });
        if (isCurrent()) loading.value = false;
      }
    }
    return true;
  }

  /**
   * Drop the currently open law and return to the "nothing open" state (no
   * law, no article, no error, not loading). Used when a traject switch lands
   * on the section root: the previous traject's law must not linger in the
   * panes, and `lawId` must read `null` so the editor shows its neutral empty
   * state instead of trying to keep the old document up. Clearing `lawParam`
   * too drops the initial-route fallback, so `lawId` really settles on `null`.
   * Invalidates any in-flight load/switch so a late response can't repopulate
   * the cleared state.
   */
  function clearLaw(): void {
    claimSwitch();
    lawParam = null;
    law.value = null;
    rawYaml.value = '';
    selectedArticleNumber.value = null;
    currentEtag.value = null;
    error.value = null;
    loading.value = false;
  }

  /**
   * Persist edited law YAML to the backend via PUT.
   *
   * On success, updates `rawYaml` + `law` locally so downstream consumers
   * (currentLawYaml computed, engine reload, scenario re-run) converge on
   * the saved text and the editor's dirty-state marker clears.
   *
   * Throws on failure so callers can decide how to surface the error; the
   * `saveError` ref is also populated for passive UI display.
   *
   * `yamlText` is sent verbatim as the PUT body - the caller decides what
   * "the whole law" means for this save. The editor's normal save path
   * passes a re-dumped doc with only the selected article patched in
   * (`currentLawYaml`); review-modus approval (EditorView's
   * `handleLawSave`) instead passes a task's full proposed YAML
   * (`useTaskReview`'s `proposedContent`) unchanged, so approving commits
   * every article the proposal touches - not just the one the editor
   * seeded into its panes. Same request/etag/PR-refresh plumbing either
   * way; no second save implementation.
   *
   * @param {string} yamlText - Full law YAML (must contain matching $id)
   */
  async function saveLaw(yamlText: string): Promise<void> {
    if (!lawId.value) {
      throw new Error('Cannot save law: no lawId');
    }
    if (!currentTrajectRef) {
      throw new Error('Cannot save law: no active traject');
    }
    // Snapshot the law we're saving *before* the await. If the user
    // switches laws while the PUT is in flight, `switchLaw` will replace
    // `lawId` / `rawYaml` / `law` with the new law's state; when the stale
    // response eventually arrives, we must not overwrite the new law's
    // reactive state with the old law's YAML.
    const savedLawId = lawId.value;
    const savedTrajectRef = currentTrajectRef;
    saving.value = true;
    saveError.value = null;
    try {
      const headers: Record<string, string> = {
        'Content-Type': 'text/yaml; charset=utf-8',
      };
      // Optimistic concurrency: echo the ETag of the version we loaded
      // so a concurrent edit by another traject member 412s instead of
      // being silently overwritten. Absent (older deployments, direct
      // URLs without ETag) the save stays a permissive blind write.
      if (currentEtag.value) headers['If-Match'] = currentEtag.value;
      const res = await apiFetch(lawUrl(savedTrajectRef, savedLawId), {
        method: 'PUT',
        headers,
        body: yamlText,
        // 412 resolves (handled as a conflict below); other failures throw.
        allowStatuses: [412],
        // Only surface the body when it's our editor-api speaking. The
        // editor-api returns plain `text/plain; charset=utf-8` for its
        // 400/403 bodies (corpus_handlers.rs), so a non-text/plain
        // content-type means a reverse proxy is intercepting (5xx HTML
        // page, etc.) and we should fall back to a generic message
        // rather than render proxy HTML in the save error dialog.
        errorMessage: (status, body, contentType) =>
          contentType.startsWith('text/plain') && body
            ? body
            : `Save failed: ${status}`,
      });
      if (res.status === 412) {
        throw new Error(
          'Het regelwerk is intussen door iemand anders gewijzigd. ' +
          'Herlaad de pagina om de laatste wijzigingen te zien en voer je ' +
          'wijziging daarna opnieuw door.',
        );
      }
      // Backend returns `{ pr: { url, number, branch } | null, etag }` on 200.
      let json: SaveResponse = null;
      try {
        json = await res.json();
        lastSavedPr.value = sanitizeSavedPr(json?.pr);
      } catch {
        // Older deployments return a bare 200 without JSON - keep the
        // existing PR (if any) and treat the save as successful.
      }
      // Chain the new ETag for the next save. The header is
      // authoritative; the body echo is the fallback (mirrors
      // useTrajectDocuments).
      const newEtag = res.headers.get('ETag') ?? json?.etag ?? null;
      // Recorded for the traject the save went to, even if the user has
      // switched since.
      const saved = corpus.forTraject(savedTrajectRef).recordBody(savedLawId, yamlText, newEtag);
      // Bail on the success path if the user navigated away mid-flight.
      if (lawId.value === savedLawId && currentTrajectRef === savedTrajectRef) {
        rawYaml.value = yamlText;
        law.value = saved.law;
        currentEtag.value = newEtag;
      }
    } catch (e) {
      if (lawId.value === savedLawId && currentTrajectRef === savedTrajectRef) {
        saveError.value = e as RequestError;
      }
      throw e;
    } finally {
      if (lawId.value === savedLawId && currentTrajectRef === savedTrajectRef) {
        saving.value = false;
      }
    }
  }

  /**
   * Seed the editor state from raw YAML without any network fetch. Used by
   * the `law_create`-review flow: the law does not exist in the traject yet
   * (the normal `load()` 404'ed), so the task's proposed YAML IS the content
   * to show. Clears the load error and settles `loading` so the editor
   * renders the proposal like a normally loaded law.
   */
  function seedFromYaml(yamlText: string): boolean {
    let parsed: LawDocument | null;
    try {
      parsed = parseLaw(yamlText);
    } catch {
      return false; // malformed proposal - keep the current (error) state
    }
    law.value = parsed;
    rawYaml.value = yamlText;
    currentEtag.value = null;
    error.value = null;
    loading.value = false;
    if (articles.value.length > 0) {
      // length > 0 was checked above.
      selectedArticleNumber.value = String(articles.value[0]!.number);
    }
    return true;
  }

  /**
   * Haal de open wet opnieuw op bij de server, langs de cache heen.
   *
   * Na een gewone opslag weet de client wat er in de wet staat: hij stuurde de
   * YAML zelf mee, dus `saveLaw` legt die body terug in `law`/`rawYaml` en in
   * de cache. Bij het verwerken van een verrijking stuurt de client geen wet,
   * maar alleen oordelen ("artikel 5 wel, artikel 7 niet"); de server voegt de
   * overgenomen artikelen samen tot de nieuwe wet. Die uitkomst heeft de client
   * nooit gezien en kan hij dus niet terugleggen - hij moet hem ophalen.
   *
   * Langs de cache heen, want daarin staat de wet van vóór het verwerken; de
   * gewone loader zou juist die verouderde versie teruggeven.
   *
   * Springt de gebruiker tijdens het ophalen naar een andere wet of een ander
   * traject, dan wordt er niets overschreven - zelfde guard als `saveLaw`.
   *
   * @returns {Promise<boolean>} of deze aanroep de state daadwerkelijk bijwerkte.
   */
  async function reloadLaw(): Promise<boolean> {
    if (!lawId.value) return false;
    const reloadedLawId = lawId.value;
    const reloadedTrajectRef = currentTrajectRef;
    const entry = await corpus.forTraject(reloadedTrajectRef).law(reloadedLawId, { fresh: true });
    if (lawId.value !== reloadedLawId || currentTrajectRef !== reloadedTrajectRef) return false;
    law.value = entry.law;
    rawYaml.value = entry.rawYaml;
    currentEtag.value = entry.etag ?? null;
    return true;
  }

  /**
   * Create a NEW law in the active traject via POST (the approve-step of a
   * `law_create` review task). Same body/etag/PR plumbing as `saveLaw`, but
   * without `If-Match` (there is nothing to be concurrent with yet); the
   * backend derives the corpus path from the validated YAML. After a
   * successful create the law exists in the traject, so follow-up saves go
   * through the normal `saveLaw` PUT.
   */
  async function createLaw(yamlText: string): Promise<void> {
    if (!currentTrajectRef) {
      throw new Error('Cannot create law: no active traject');
    }
    const savedTrajectRef = currentTrajectRef;
    saving.value = true;
    saveError.value = null;
    try {
      const res = await apiFetch(lawCreateUrl(savedTrajectRef), {
        method: 'POST',
        headers: { 'Content-Type': 'text/yaml; charset=utf-8' },
        body: yamlText,
        // Same proxy-guard as saveLaw: only surface text/plain bodies (the
        // editor-api's own 400/409 messages), fall back generic otherwise.
        errorMessage: (status, body, contentType) =>
          contentType.startsWith('text/plain') && body
            ? body
            : `Save failed: ${status}`,
      });
      let json: SaveResponse = null;
      try {
        json = await res.json();
        lastSavedPr.value = sanitizeSavedPr(json?.pr);
      } catch {
        // Bare 200 without JSON - treat as successful.
      }
      const newEtag = res.headers.get('ETag') ?? json?.etag ?? null;
      const parsed = parseLaw(yamlText);
      rawYaml.value = yamlText;
      law.value = parsed;
      currentEtag.value = newEtag;
      const resolvedId = parsed?.$id;
      if (resolvedId) corpus.forTraject(savedTrajectRef).recordBody(resolvedId, yamlText, newEtag);
    } catch (e) {
      saveError.value = e as RequestError;
      throw e;
    } finally {
      saving.value = false;
    }
  }

  return {
    law,
    lawId,
    rawYaml,
    articles,
    lawName,
    selectedArticle,
    selectedArticleNumber,
    switchLaw,
    clearLaw,
    lawTrajectRef,
    loading,
    error,
    saving,
    saveError,
    saveLaw,
    reloadLaw,
    seedFromYaml,
    createLaw,
    currentEtag,
    lastSavedPr,
  };
}
