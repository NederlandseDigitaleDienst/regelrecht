/**
 * useNotes - fetch a law's note sidecar and resolve it against the loaded law.
 *
 * Notes are W3C Web Annotations anchored to legal text via a TextQuoteSelector
 * (RFC-005). The Rust resolver runs in WASM (`engine.resolveNotes`) so the
 * editor shows exactly what the engine and CI validate. Match offsets are
 * `char` offsets into the article text, not UTF-16 code units (see the WASM
 * binding docs); consumers convert with cpToUtf16 (useTextSelection) where
 * the DOM needs UTF-16.
 */
import { ref, computed, watch } from 'vue';
import type { Ref } from 'vue';
import { useEngine } from './useEngine';
import { annotationsUrl } from './corpusUrls';
import { apiFetch } from '../lib/apiFetch';
import { useLatest } from '../lib/useLatest';
import type { MatchResult, TextMatch, TextQuoteSelector } from './useTextSelection';

/**
 * A note: a W3C Web Annotation (annotation schema, `definitions/Annotation`).
 * Only the fields this editor reads are typed; the rest passes through as
 * the sidecar or the note creator wrote it. Notes from YAML or localStorage
 * are not validated, so every field is optional here.
 */
export interface Note {
  type?: string;
  motivation?: string;
  created?: string;
  target?: { source?: string; selector?: TextQuoteSelector };
  /** Editor-internal marker on a draft; stripped before export or save. */
  __draft?: boolean;
  [field: string]: unknown;
}

/** One entry of `engine.resolveNotes` (see `resolve_notes` in wasm.rs). */
export interface ResolvedNote {
  note: Note;
  match: MatchResult | null;
  error: string | null;
}

/** A note with its span(s) in the selected article. */
export interface NoteWithSpans {
  note: Note;
  spans: TextMatch[];
}

/** A note that could not be anchored, with the reason shown to the user. */
export interface NoteIssue {
  note: Note;
  reason: string;
}

/** The part of an article this module reads. */
interface ArticleLike {
  number?: string | number | null;
}

type MaybeRef<T> = Readonly<Ref<T>>;

// Cache resolved notes per `${trajectRef}::${lawId}::${validFrom}` for the
// session. The resolver result only changes when the law text or the sidecar
// changes; scoping by trajectRef prevents cross-traject leakage when the user
// switches between trajects, and the version's `valid_from` keeps two
// versions of the same law from sharing resolved offsets.
//
// Caveat (acceptable for the display-only, default-off MVP; revisit in
// the note-editing phase): a save through the editor changes the text
// without changing `$id` or `valid_from`, so the cache is not invalidated
// on save - editing a law in-session and reopening its Notities pane could
// show offsets resolved against the pre-save text. Once notes become
// editable, invalidate on save.
const cache = new Map<string, ResolvedNote[]>();

function cacheKey(trajectRef: string | null, lawId: string, validFrom: string | null): string {
  return `${trajectRef || ''}::${lawId}::${validFrom || ''}`;
}

/**
 * @param {import('vue').Ref<string>} lawId reactive law $id
 * @param {import('vue').Ref<object>} selectedArticle reactive current article
 * @param {import('vue').Ref<string|null>} trajectRef reactive traject ref
 *   (`null` for global / no-traject reads)
 * @param {import('vue').Ref<string|null>=} lawValidFrom `valid_from` of the
 *   law version on screen. Passed to the resolver so notes anchor in the
 *   viewed version's text instead of the newest loaded version
 *   (`loadDependency` loads every version).
 */
export function useNotes(
  lawId: MaybeRef<string | null | undefined>,
  selectedArticle: MaybeRef<ArticleLike | null | undefined>,
  trajectRef?: MaybeRef<string | null | undefined>,
  lawValidFrom?: MaybeRef<string | null | undefined>,
) {
  const { initEngine, loadDependency } = useEngine();
  const resolved = ref<ResolvedNote[]>([]); // [{ note, match, error }]
  const loading = ref(false);
  const error = ref<unknown>(null);

  // Generation guard: each load() call claims a generation; only the latest
  // is allowed to write reactive state. Without this, navigating between laws
  // while a slow annotations fetch is in flight lets the older response
  // overwrite the newer law's notes - and because article numbers collide
  // across laws ('1','2','3' everywhere) the stale offsets would silently
  // highlight wrong spans. useLaw guards the same race the same way.
  const claimLoad = useLatest();

  async function load() {
    const id = lawId.value;
    const tr = trajectRef?.value ?? null;
    const vf = lawValidFrom?.value ?? null;
    const isCurrent = claimLoad();
    const isStale = () => !isCurrent();

    // These early returns resolve synchronously. They must clear `loading`
    // too: if a slow uncached load is in flight and the user navigates to a
    // cached law, that older load is now stale and skips its own
    // `loading = false` reset (gated on !isStale), so without clearing it
    // here the "Notities laden…" spinner stays stuck forever.
    if (!id) {
      resolved.value = [];
      error.value = null;
      loading.value = false;
      return;
    }
    const key = cacheKey(tr, id, vf);
    if (cache.has(key)) {
      // Reset error too: a cached law (e.g. a 404 → []) must not keep showing
      // the previous law's "kon notities niet laden" alert.
      resolved.value = cache.get(key) ?? [];
      error.value = null;
      loading.value = false;
      return;
    }

    loading.value = true;
    error.value = null;
    try {
      // With an active traject the read goes through that traject's
      // backend (where `save_annotations` writes) so a freshly-appended
      // note is visible immediately. Without a traject this falls back
      // to the global annotation route - the central source's main view.
      const res = await apiFetch(annotationsUrl(tr, id), {
        allowStatuses: [404],
        errorMessage: (status: number) => `Kon notities niet laden: ${status}`,
      });
      if (res.status === 404) {
        // A law without a sidecar is normal, not an error.
        cache.set(key, []);
        if (!isStale()) resolved.value = [];
        return;
      }
      const yamlText = await res.text();

      const engine = await initEngine();
      // The resolver needs the law's articles loaded; mirror how the
      // rest of the editor pulls a law into the engine. Call
      // `loadDependency` unconditionally - it short-circuits when the
      // engine already has the law under the same scope, and unloads +
      // refetches when a previous load came from a different traject.
      // A bare `if (!engine.hasLaw(id))` gate here would skip that scope
      // check and resolve notes against stale-scope content.
      await loadDependency(id);
      // `vf` selects the viewed version inside the engine's full version
      // set; `undefined` (no valid_from on the law) means the latest.
      const result = engine.resolveNotes(id, yamlText, vf ?? undefined);
      // resolve_notes (wasm.rs) returns `{ note, match, error }` entries.
      const list: ResolvedNote[] = Array.isArray(result) ? result : [];
      cache.set(key, list);
      if (!isStale()) resolved.value = list;
    } catch (e) {
      if (!isStale()) {
        error.value = e;
        resolved.value = [];
      }
    } finally {
      // Only the latest load owns the loading flag.
      if (!isStale()) loading.value = false;
    }
  }

  // Re-load on the law, the active traject or the viewed version changing -
  // the sidecar lives per traject branch and the resolved offsets are
  // version-specific, so any of the three needs a fresh resolve even if the
  // law id stayed put.
  const trackers = [lawId, trajectRef, lawValidFrom].filter(
    (r): r is NonNullable<typeof r> => Boolean(r),
  );
  watch(trackers, load, { immediate: true });

  /**
   * Force a fresh fetch for the current law: drop its cache entry first
   * so `load()` can't shortcut to the previously-resolved value, then
   * run `load`. Used after `saveToRepo` so a just-committed note shows
   * up immediately instead of waiting for a navigation away and back.
   *
   * `load()` alone won't do - it returns the cached `[]` from the
   * first pre-save fetch and silently leaves the user looking at an
   * empty notes pane right after they hit "Opslaan".
   */
  async function reload() {
    const id = lawId.value;
    if (id) {
      cache.delete(
        cacheKey(trajectRef?.value ?? null, id, lawValidFrom?.value ?? null),
      );
    }
    await load();
  }

  /**
   * Notes whose match falls in the currently-selected article, each with the
   * resolved span(s) for that article. Notes that are orphaned, ambiguous, or
   * failed to parse are surfaced separately via `issues` so the UI can show
   * them without anchoring them in the text.
   */
  const notesForArticle = computed((): NoteWithSpans[] => {
    const articleNr = selectedArticle.value?.number;
    if (articleNr == null || articleNr === '') return [];
    // String() both sides: js-yaml decodes an unquoted `number: 2` to a JS
    // number while the resolver's article_number is always a string. useLaw
    // applies the same defensive coercion for the same reason.
    const target = String(articleNr);
    const out: NoteWithSpans[] = [];
    for (const entry of resolved.value) {
      if (entry.error || !entry.match) continue;
      if (entry.match.status !== 'found') continue;
      const spans = entry.match.matches.filter(
        (m) => String(m.article_number) === target,
      );
      if (spans.length > 0) out.push({ note: entry.note, spans });
    }
    return out;
  });

  /** Orphaned / ambiguous / skipped / parse-failed notes, for a status list. */
  const issues = computed((): NoteIssue[] =>
    resolved.value
      .filter(
        (e) => e.error || (e.match && e.match.status !== 'found'),
      )
      .map((e) => ({
        note: e.note,
        reason: e.error
          ? `parsefout: ${e.error}`
          : e.match!.status === 'orphaned' // the filter kept only entries with an error or a match
            ? 'niet gevonden in de wettekst (orphaned)'
            : e.match!.status === 'skipped'
              ? // The resolver hit a scan bound (config.rs): the law was not
                // (fully) searched. Distinct from orphaned, which asserts
                // the text is absent. skip_reason names the bound that was
                // hit, so the advice matches the cause instead of always
                // blaming the quote length.
                e.match!.skip_reason === 'quote_too_long'
                ? 'niet naar gezocht: citaat te lang voor de zoekbegrenzing - kort het citaat in'
                : 'niet volledig doorzocht: de wettekst overschrijdt de zoekbegrenzing'
              : 'meerdere matches (ambigu) - voeg context toe',
      })),
  );

  return { notesForArticle, issues, loading, error, reload };
}

/**
 * Resolve a list of in-memory draft notes against the loaded law and project
 * them onto the selected article, returning the same `{ note, spans }` shape
 * as `notesForArticle` so the editor highlights drafts exactly like committed
 * notes. Drafts live only in localStorage until exported (RFC-018 write path);
 * they are resolved here per-note via the same WASM resolver, not refetched.
 *
 * @param {import('vue').Ref<Array>} draftNotes reactive list of W3C Annotation
 * @param {import('vue').Ref<string>} lawId
 * @param {import('vue').Ref<object>} selectedArticle
 * @param {import('vue').Ref<string|null>=} trajectRef Active traject ref.
 *   Routes the dependency load through the matching scope so a draft
 *   resolves against the same law copy the editor shows.
 * @param {import('vue').Ref<string|null>=} lawValidFrom `valid_from` of the
 *   viewed law version, so drafts anchor in the text on screen instead of
 *   the newest loaded version (same contract as `useNotes`).
 */
export function useResolvedDraftNotes(
  draftNotes: MaybeRef<Note[] | null | undefined>,
  lawId: MaybeRef<string | null | undefined>,
  selectedArticle: MaybeRef<ArticleLike | null | undefined>,
  trajectRef?: MaybeRef<string | null | undefined>,
  lawValidFrom?: MaybeRef<string | null | undefined>,
) {
  const { initEngine, loadDependency } = useEngine();
  const resolvedDrafts = ref<{ note: Note; match: MatchResult }[]>([]); // [{ note, match }]

  // Generation guard: resolve() awaits initEngine/loadDependency (slow on a
  // law switch). Without this, a resolve started before a law switch can
  // finish after the one started by the switch and overwrite it with stale
  // data - and because draft selectors resolve per-law, that would highlight
  // the previous law's drafts on the new law. useNotes.load() guards the same
  // race the same way.
  const claimResolve = useLatest();

  async function resolve() {
    const id = lawId.value;
    const notes = draftNotes.value;
    const tr = trajectRef?.value ?? null;
    const vf = lawValidFrom?.value ?? null;
    const isCurrent = claimResolve();
    const isStale = () => !isCurrent();
    if (!id || !notes || notes.length === 0) {
      resolvedDrafts.value = [];
      return;
    }
    try {
      const engine = await initEngine();
      // loadDependency checks the scope, so the engine cache detects a
      // stale copy from a previous traject and refetches - without this a
      // switch would keep highlighting drafts against the old law content.
      await loadDependency(id);
      const out: { note: Note; match: MatchResult }[] = [];
      for (const note of notes) {
        const selector = note?.target?.selector;
        if (!selector) continue;
        let match: MatchResult;
        try {
          // The resolver serializes a Rust MatchResult; trusted as such.
          match = engine.resolveNote(id, selector, vf ?? undefined) as MatchResult;
        } catch {
          continue; // a malformed draft selector simply does not highlight
        }
        out.push({ note, match });
      }
      if (!isStale()) resolvedDrafts.value = out;
    } catch {
      if (!isStale()) resolvedDrafts.value = [];
    }
  }

  const trackers = [draftNotes, lawId, trajectRef, lawValidFrom].filter(
    (r): r is NonNullable<typeof r> => Boolean(r),
  );
  watch(trackers, resolve, { immediate: true, deep: true });

  const draftNotesForArticle = computed((): NoteWithSpans[] => {
    const articleNr = selectedArticle.value?.number;
    if (articleNr == null || articleNr === '') return [];
    const target = String(articleNr);
    const out: NoteWithSpans[] = [];
    for (const entry of resolvedDrafts.value) {
      if (!entry.match || entry.match.status !== 'found') continue;
      const spans = entry.match.matches.filter(
        (m) => String(m.article_number) === target,
      );
      if (spans.length > 0) out.push({ note: entry.note, spans });
    }
    return out;
  });

  return { draftNotesForArticle };
}
