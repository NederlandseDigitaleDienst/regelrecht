import { describe, it, expect } from 'vitest';
import { cpToUtf16, utf16ToCp, buildSelector } from './useTextSelection';
import type { NoteResolver, TextQuoteSelector } from './useTextSelection';

// useTextSelection converts between the resolver's code-point offsets and the
// DOM's UTF-16 offsets, and grows TextQuoteSelector context until the
// resolver finds the quoted span uniquely; these tests pin the offset
// conversion pair and the context-growing loop.

describe('cpToUtf16 / utf16ToCp', () => {
  it('is identity for BMP text', () => {
    expect(cpToUtf16('abcdef', 3)).toBe(3);
    expect(utf16ToCp('abcdef', 3)).toBe(3);
  });

  it('counts an astral code point as two UTF-16 units', () => {
    // "𝕏" (U+1D54F) is one code point but two UTF-16 units.
    const text = 'a𝕏b';
    expect(cpToUtf16(text, 2)).toBe(3); // after "a𝕏"
    expect(utf16ToCp(text, 3)).toBe(2); // inverse
  });
});

describe('buildSelector', () => {
  const raw = 'Indien de normpremie voor een verzekerde hoger is dan de normpremie';

  // The stubs return partial match results ({} or offsets only); the engine
  // contract returns `unknown`, so no cast is needed.
  function engineReturning(bySelector: (selector: TextQuoteSelector) => unknown): NoteResolver {
    return {
      resolveNote(_lawId, selector) {
        return bySelector(selector);
      },
    };
  }

  it('rejects a whitespace-only exact without calling the resolver', () => {
    // A selection that maps to just the space between two words: schema
    // minLength:1 and the resolver's non-empty guard would both pass it,
    // but a note quoting " " anchors nothing visible. Must short-circuit.
    const r = 'de  normpremie'; // two spaces at 2..4
    let called = false;
    const engine = engineReturning(() => {
      called = true;
      return { status: 'found', matches: [{}] };
    });
    const out = buildSelector(r, { start: 2, end: 4 }, 'w', engine, '1');
    expect(out.status).toBe('orphaned');
    expect(called).toBe(false);
  });

  it('accepts a bare selector only when the unique match IS our selection', () => {
    const range = { start: 10, end: 20 }; // "normpremie" (first)
    const engine = engineReturning(() => ({
      status: 'found',
      matches: [{ article_number: '1', start: 10, end: 20 }],
    }));
    const out = buildSelector(raw, range, 'w', engine, '1');
    expect(out.status).toBe('found');
    expect(out.selector.exact).toBe('normpremie');
    expect(out.selector.prefix).toBeUndefined();
    expect(out.selector.suffix).toBeUndefined();
  });

  it('hands the viewed version (validFrom) to the resolver', () => {
    // The engine holds every version of the law; without the valid_from of
    // the version on screen the resolver would validate uniqueness against
    // the newest version instead of the text being annotated.
    const range = { start: 10, end: 20 };
    const seen: (string | undefined)[] = [];
    const engine: NoteResolver = {
      resolveNote(_lawId, _selector, validFrom) {
        seen.push(validFrom);
        return {
          status: 'found',
          matches: [{ article_number: '1', start: 10, end: 20 }],
        };
      },
    };
    const out = buildSelector(raw, range, 'w', engine, '1', '2024-01-01');
    expect(out.status).toBe('found');
    expect(seen).toEqual(['2024-01-01']);
    // Without a version context the argument stays undefined (= latest).
    buildSelector(raw, range, 'w', engine, '1');
    expect(seen).toEqual(['2024-01-01', undefined]);
  });

  it('rejects a unique match at the WRONG offsets as a mis-anchor', () => {
    // The resolver found a single match, but at offsets 56..66, not the
    // 10..20 the user selected (the trimmed prefix/suffix check matched the
    // other "normpremie"). This must NOT be accepted: it would silently
    // anchor the note to a different sentence.
    const range = { start: 10, end: 20 };
    const engine = engineReturning(() => ({
      status: 'found',
      matches: [{ article_number: '1', start: 56, end: 66 }],
    }));
    const out = buildSelector(raw, range, 'w', engine, '1');
    expect(out.status).toBe('ambiguous');
  });

  it('rejects a unique match in the WRONG article', () => {
    const range = { start: 10, end: 20 };
    const engine = engineReturning(() => ({
      status: 'found',
      matches: [{ article_number: '3', start: 10, end: 20 }],
    }));
    const out = buildSelector(raw, range, 'w', engine, '1');
    expect(out.status).toBe('ambiguous');
  });

  it('reports reason "too-common" when the bare quote matched many, even if widening degrades to orphaned', () => {
    // The "in"-case: no-context match is ambiguous (occurs everywhere), but
    // with markdown-stripped context the wider attempts can't relocate it and
    // the resolver returns orphaned. The user-facing reason must stay
    // "too-common", not "not-found".
    const range = { start: 10, end: 20 };
    const engine = engineReturning((sel) =>
      !sel.prefix && !sel.suffix
        ? { status: 'ambiguous', matches: [{}, {}, {}] }
        : { status: 'orphaned', matches: [] },
    );
    const out = buildSelector(raw, range, 'w', engine, '1');
    expect(out.reason).toBe('too-common');
  });

  it('reports reason "not-found" when nothing matched at any width', () => {
    const range = { start: 10, end: 20 };
    const engine = engineReturning(() => ({ status: 'orphaned', matches: [] }));
    const out = buildSelector(raw, range, 'w', engine, '1');
    expect(out.status).toBe('orphaned');
    expect(out.reason).toBe('not-found');
  });

  it('grows context until the match lands on our exact selection', () => {
    const range = { start: 10, end: 20 }; // "normpremie", appears twice
    const calls: TextQuoteSelector[] = [];
    const engine = engineReturning((sel) => {
      calls.push(sel);
      if (!sel.prefix && !sel.suffix) {
        return { status: 'ambiguous', matches: [{}, {}] };
      }
      return {
        status: 'found',
        matches: [{ article_number: '1', start: 10, end: 20 }],
      };
    });
    const out = buildSelector(raw, range, 'w', engine, '1');
    expect(out.status).toBe('found');
    expect(out.selector.prefix).toBeTruthy();
    expect(calls.length).toBeGreaterThan(1);
  });

  it('reports still-ambiguous when even wide context does not disambiguate', () => {
    const range = { start: 10, end: 20 };
    const engine = engineReturning(() => ({
      status: 'ambiguous',
      matches: [{}, {}],
    }));
    const out = buildSelector(raw, range, 'w', engine, '1');
    expect(out.status).toBe('ambiguous');
  });

  it('stops immediately on an orphaned result', () => {
    const range = { start: 10, end: 20 };
    let calls = 0;
    const engine = engineReturning(() => {
      calls++;
      return { status: 'orphaned', matches: [] };
    });
    const out = buildSelector(raw, range, 'w', engine, '1');
    expect(out.status).toBe('orphaned');
    expect(calls).toBe(1);
  });

  it('passes a skipped search through as its own status, not as orphaned', () => {
    // 'skipped' = the resolver's fuzzy budget refused the quote (RFC-018
    // scan bounds). Growing prefix/suffix cannot shorten the quote, so
    // retrying the two wider widths would just burn the same budget again.
    // The status must survive to the caller: "not searched" and "not found"
    // are different messages to the user, with different advice.
    const range = { start: 10, end: 20 };
    let calls = 0;
    const engine = engineReturning(() => {
      calls++;
      return { status: 'skipped', matches: [], skip_reason: 'quote_too_long' };
    });
    const out = buildSelector(raw, range, 'w', engine, '1');
    expect(out.status).toBe('skipped');
    expect(out.reason).toBe('not-searched');
    expect(calls).toBe(1);
  });
});
