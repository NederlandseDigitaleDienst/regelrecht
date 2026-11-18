/**
 * useTextSelection - selection offsets and the TextQuoteSelector build for
 * notes (RFC-018 write path).
 *
 * The note resolver works in code-point ("raw char") offsets while the DOM
 * and the DS text editor report UTF-16 offsets; cpToUtf16/utf16ToCp convert
 * between the two. buildSelector turns a raw [start,end) range into a W3C
 * TextQuoteSelector.
 *
 * Uniqueness: a bare `exact` often occurs many times ("verzekerde" appears
 * dozens of times in the Zorgtoeslagwet). The selector is only useful if it
 * resolves to exactly one span, so context is grown until the resolver
 * (`engine.resolveNote`) reports a single match, or we run out of article and
 * report it as still ambiguous so the UI can ask the user to select more.
 */

// W3C TextQuoteSelector context length. RFC-018 §write-path says 30-50 chars;
// 40 is the midpoint and enough to disambiguate every repeated phrase in the
// current corpus while staying readable in the note YAML.
const CONTEXT = 40;

/**
 * A W3C TextQuoteSelector as the annotation schema defines it. `type` is
 * required by the schema, but drafts read back from storage are not
 * validated, so it is optional here.
 */
export interface TextQuoteSelector {
  type?: 'TextQuoteSelector';
  exact: string;
  prefix?: string;
  suffix?: string;
}

/** The resolver's `MatchStatus` (Rust `annotation::types`), on the wire. */
export type MatchStatus = 'found' | 'orphaned' | 'ambiguous' | 'skipped';

/** The resolver's `SkipReason`, on the wire. */
export type SkipReason = 'quote_too_long' | 'search_budget';

/**
 * One located span (Rust `TextMatch`). Keys are the wire format; `start` and
 * `end` are code-point offsets, article-relative.
 */
export interface TextMatch {
  article_number: string;
  start: number;
  end: number;
  confidence: number;
  matched_text: string;
}

/** The outcome of resolving a selector (Rust `MatchResult`). */
export interface MatchResult {
  status: MatchStatus;
  matches: TextMatch[];
  skip_reason?: SkipReason;
}

/**
 * The part of the WASM engine that resolves a single selector. The engine
 * returns a serialized Rust `MatchResult`.
 */
export interface NoteResolver {
  resolveNote(lawId: string, selector: TextQuoteSelector, validFrom?: string): unknown;
}

export type SelectorReason = 'ok' | 'too-common' | 'not-found' | 'mis-anchor' | 'not-searched';

export interface BuiltSelector {
  selector: TextQuoteSelector;
  exact: string;
  status: MatchStatus;
  reason: SelectorReason;
  /** Only set when `status` is 'skipped'. */
  skipReason?: SkipReason | null;
}

/** Code-point offset -> UTF-16 offset within `text` (the DOM uses UTF-16). */
export function cpToUtf16(text: string, cpOffset: number): number {
  let cp = 0;
  let u = 0;
  for (const ch of text) {
    if (cp >= cpOffset) break;
    u += ch.length;
    cp++;
  }
  return u;
}

/** UTF-16 offset within `text` -> code-point offset (inverse of cpToUtf16). */
export function utf16ToCp(text: string, u16Offset: number): number {
  let cp = 0;
  let u = 0;
  for (const ch of text) {
    if (u >= u16Offset) break;
    u += ch.length;
    cp++;
  }
  return cp;
}

/**
 * Build a TextQuoteSelector from a raw range, growing prefix/suffix context
 * until the resolver finds exactly the span the user selected.
 *
 * "Found" from the resolver is necessary but not sufficient: the resolver's
 * exact-match prefix/suffix check trims a one-char-slack window
 * (resolver.rs find_exact_matches), so when the same `exact` repeats and a
 * grown context happens to satisfy the trimmed comparison at the *other*
 * occurrence, the resolver returns `found` pointing at the wrong span. For a
 * legal-text product that silent mis-anchor is unacceptable, so a `found`
 * result is only accepted when the single match lands in the article the user
 * selected in, at exactly the offsets they selected. Otherwise context keeps
 * growing; if it never converges the status is reported as ambiguous so the
 * UI asks the author to extend the selection rather than persisting a note on
 * the wrong sentence.
 *
 * @param {string} rawText           the selected article's text
 * @param {{start:number,end:number}} range  raw char offsets into rawText
 * @param {string} lawId
 * @param {{ resolveNote: Function }} engine  loaded WASM engine
 * @param {string|number} articleNumber  the article `range` is in; the
 *        resolver returns article-relative offsets + article_number, so the
 *        match is verified against this
 * @param {string|null=} validFrom  `valid_from` of the law version on screen,
 *        so uniqueness is checked in the text the user is annotating rather
 *        than in the newest loaded version; omit for the latest version
 * @returns {{
 *   selector: object,
 *   exact: string,
 *   status: 'found'|'ambiguous'|'orphaned'|'skipped',
 *   reason: 'ok'|'too-common'|'not-found'|'mis-anchor'|'not-searched',
 * }}
 *   `reason` is for the UI message only; it does not change accept/reject.
 *   'too-common' = the bare quote already matched several places (RFC-018
 *   §5.4 "common word without context") even if widening context later
 *   degrades to orphaned; 'not-found' = genuinely not locatable; 'mis-anchor'
 *   = a unique match landed off the selection; 'not-searched' = the resolver
 *   refused the scan (status 'skipped': the quote exceeds the fuzzy budget
 *   in config.rs), which is a different message than "not found" — nothing
 *   was established about the text at all.
 */
export function buildSelector(
  rawText: string,
  range: { start: number; end: number },
  lawId: string,
  engine: NoteResolver,
  articleNumber: string | number,
  validFrom?: string | null,
): BuiltSelector {
  const chars = Array.from(rawText);
  const exact = chars.slice(range.start, range.end).join('');
  const wantArticle = String(articleNumber);

  // A whitespace-only quote is meaningless as an anchor: it satisfies the
  // schema's minLength:1 and the resolver's non-empty guard, but a note
  // quoting " " points at nothing a reader can see. The callers never pass an
  // empty/zero-length range; this rejects the whitespace-only case (a
  // selection covering just the space between two words). Reported as
  // orphaned so the UI asks for real text.
  if (exact.trim() === '') {
    return {
      selector: { type: 'TextQuoteSelector', exact },
      exact,
      status: 'orphaned',
      reason: 'not-found',
    };
  }

  // The resolver returns article-relative `char` offsets. range is into this
  // article's text, so a correct unique match has exactly these offsets in
  // this article.
  const isExactlyOurSelection = (result: MatchResult | null | undefined): boolean => {
    if (result?.status !== 'found') return false;
    const matches = result.matches ?? [];
    if (matches.length !== 1) return false;
    const m = matches[0]!; // length checked just above
    return (
      String(m.article_number) === wantArticle &&
      m.start === range.start &&
      m.end === range.end
    );
  };

  // Try increasing context: no context, then CONTEXT chars each side, then
  // double. Short-circuit as soon as the match is provably our selection.
  const widths = [0, CONTEXT, CONTEXT * 2];
  let last: BuiltSelector | null = null;
  // The bare quote (no context) matching several places is the RFC-018 §5.4
  // "common word" case. Remember it: even if widening context later degrades
  // to orphaned (markdown-stripped prefix/suffix no longer matches), the
  // user-facing reason is still "too common", not "not found".
  let sawMultiple = false;
  for (const w of widths) {
    const prefix =
      w > 0 ? chars.slice(Math.max(0, range.start - w), range.start).join('') : '';
    const suffix =
      w > 0 ? chars.slice(range.end, range.end + w).join('') : '';
    const selector: TextQuoteSelector = { type: 'TextQuoteSelector', exact };
    if (prefix) selector.prefix = prefix;
    if (suffix) selector.suffix = suffix;

    let result: MatchResult | null | undefined;
    try {
      // The resolver serializes a Rust MatchResult; trusted as such.
      result = engine.resolveNote(lawId, selector, validFrom ?? undefined) as MatchResult | null | undefined;
    } catch {
      // Resolver threw (law not loaded etc.) - caller surfaces it.
      return { selector, exact, status: 'orphaned', reason: 'not-found' };
    }
    if (isExactlyOurSelection(result)) {
      return { selector, exact, status: 'found', reason: 'ok' };
    }
    // 'skipped': the resolver refused the fuzzy scan (quote over the budget
    // in config.rs). Growing context cannot fix a too-long quote, and the
    // exact pass (which is unbudgeted) already failed, so stop here rather
    // than looping two more widths to the same answer. The status stays
    // 'skipped': the UI must say "this was not searched", not "not found" —
    // flattening it to orphaned here would hand the user the wrong message
    // and the wrong advice.
    if (result?.status === 'skipped') {
      // skip_reason travels along: 'quote_too_long' and 'search_budget' need
      // different advice, and the caller cannot recover it from 'not-searched'.
      return {
        selector,
        exact,
        status: 'skipped',
        reason: 'not-searched',
        skipReason: result.skip_reason ?? null,
      };
    }
    if ((result?.matches?.length ?? 0) > 1) sawMultiple = true;
    // A unique `found` that is NOT our selection is a mis-anchor: treat it as
    // ambiguous (more context may still pin the right one) rather than
    // accepting it. Only a genuine orphaned short-circuits.
    const orphaned = result?.status === 'orphaned';
    const status: MatchStatus = orphaned ? 'orphaned' : 'ambiguous';
    const reason: SelectorReason = sawMultiple
      ? 'too-common'
      : orphaned
        ? 'not-found'
        : result?.status === 'found'
          ? 'mis-anchor'
          : 'too-common';
    last = { selector, exact, status, reason };
    if (orphaned) return last;
  }
  // Widest context still did not provably pin our selection.
  // `widths` is non-empty and every iteration either returns or sets `last`.
  return last!;
}
