import { describe, it, expect } from 'vitest';
import {
  formatOutputValue,
  formatOutputValueParts,
  formatValue,
  formatMissing,
  matchStatus,
  expectationsFromAssertions,
} from './outputFormat';

const unknownFor = (...facts: [string, string][]) => ({
  __unknown: true,
  missing: facts.map(([name, law]) => ({ law, name, kind: 'no_data' })),
});

// Intl renders the euro sign followed by a non-breaking space; matching on the
// digits keeps the assertions readable and independent of that whitespace.
const EUROS = /1\.500,00/;

describe('formatOutputValueParts', () => {
  it('gives a eurocent output its euro supporting text, whatever it is called', () => {
    expect(formatOutputValueParts(150000, 'eurocent')).toEqual({
      text: '150000',
      supportingText: expect.stringMatching(EUROS),
    });
  });

  it('leaves an integer without a unit unformatted, even when it is called a bedrag', () => {
    expect(formatOutputValueParts(150000, null)).toEqual({
      text: '150000',
      supportingText: '',
    });
  });

  it('formats a euro output without dividing', () => {
    const parts = formatOutputValueParts(1500, 'euro');
    expect(parts.text).toBe('1500');
    expect(parts.supportingText).toMatch(EUROS);
  });

  it('ignores units that are not money', () => {
    expect(formatOutputValueParts(65, 'jaar').supportingText).toBe('');
    expect(formatOutputValueParts(150000, 'percentage').supportingText).toBe('');
  });

  it('never currency-formats a non-numeric value', () => {
    expect(formatOutputValueParts('150000', 'eurocent').supportingText).toBe('');
    expect(formatOutputValueParts(true, 'eurocent')).toEqual({ text: 'ja', supportingText: '' });
    expect(formatOutputValueParts(null, 'eurocent')).toEqual({ text: 'geen', supportingText: '' });
  });
});

describe('formatOutputValue', () => {
  it('appends the euro form for a eurocent value', () => {
    expect(formatOutputValue(150000, 'eurocent')).toMatch(/^150000 \(.*1\.500,00\)$/);
  });

  it('returns the bare value without a monetary unit', () => {
    expect(formatOutputValue(150000, null)).toBe('150000');
    expect(formatOutputValue(150000, 'eurocenten')).toBe('150000');
  });
});

describe('formatValue', () => {
  it('renders a collection as JSON instead of [object Object]', () => {
    expect(formatValue([{ leeftijd: 25 }, { leeftijd: 19 }])).toBe('[{"leeftijd":25},{"leeftijd":19}]');
    expect(formatValue({ leeftijd: 25 })).toBe('{"leeftijd":25}');
    expect(formatValue([])).toBe('[]');
  });

  it('keeps scalars as they were', () => {
    expect(formatValue(true)).toBe('ja');
    expect(formatValue(42)).toBe('42');
  });

  // RFC-036: the user never reads the token `null` or the raw unknown object.
  it('renders absence as geen and an unknown outcome as onbekend', () => {
    expect(formatValue(null)).toBe('geen');
    expect(formatValue(unknownFor(['huur', 'wet_x']))).toBe('onbekend');
    expect(formatValue({ __unknown: true, missing: [] })).toBe('onbekend');
  });

  // An output the engine never produced is not an absence the data stated:
  // "geen" would be the very conflation RFC-036 removes.
  it('renders an output the engine never produced as niet berekend, not as geen', () => {
    expect(formatValue(undefined)).toBe('niet berekend');
    expect(formatOutputValue(undefined, 'eurocent')).toBe('niet berekend');
    expect(formatOutputValueParts(undefined, null)).toEqual({ text: 'niet berekend', supportingText: '' });
  });

  it('still renders an ordinary object as JSON', () => {
    expect(formatValue({ __unknown: false })).toBe('{"__unknown":false}');
    expect(formatValue({ missing: [] })).toBe('{"missing":[]}');
  });
});

describe('unknown outcomes (RFC-036)', () => {
  it('lists the missing facts as supporting text, comma-separated, with their law', () => {
    expect(formatOutputValueParts(unknownFor(['huur', 'wet_x'], ['partner_bsn', 'wet_y']), 'eurocent')).toEqual({
      text: 'onbekend',
      supportingText: 'ontbreekt: huur (wet_x), partner_bsn (wet_y)',
    });
  });

  it('names a fact without a law by name alone', () => {
    expect(formatMissing({ __unknown: true, missing: [{ name: 'huur' }] })).toBe('ontbreekt: huur');
    expect(formatMissing({ __unknown: true, missing: [] })).toBe('');
    expect(formatMissing(null)).toBe('');
    expect(formatMissing(42)).toBe('');
  });

  it('formats the one-line form with the missing facts in parentheses', () => {
    expect(formatOutputValue(unknownFor(['huur', 'wet_x']), 'eurocent')).toBe('onbekend (ontbreekt: huur (wet_x))');
    expect(formatOutputValue({ __unknown: true, missing: [] }, null)).toBe('onbekend');
    expect(formatOutputValue(null, 'eurocent')).toBe('geen');
  });
});

describe('expectationsFromAssertions', () => {
  it('keeps values as strings and skips assertions without an output', () => {
    expect(expectationsFromAssertions([
      { assertionType: 'succeeds', outputName: null, value: null },
      { assertionType: 'boolean', outputName: 'recht', value: true },
      { assertionType: 'equals', outputName: 'bedrag', value: 1500 },
      { assertionType: 'equalsString', outputName: 'code', value: 'A' },
    ])).toEqual({ recht: 'true', bedrag: '1500', code: 'A' });
  });

  it('turns is-null into a null expectation and the unknown forms into the unknown shape', () => {
    expect(expectationsFromAssertions([
      { assertionType: 'null', outputName: 'partner', value: null },
      { assertionType: 'unknown', outputName: 'huur', value: null },
      { assertionType: 'unknownFor', outputName: 'toeslag', value: 'inkomen' },
    ])).toEqual({
      partner: null,
      huur: { __unknown: true, missing: [] },
      toeslag: { __unknown: true, missing: [{ name: 'inkomen' }] },
    });
  });

  it('accepts a missing list', () => {
    expect(expectationsFromAssertions(undefined)).toEqual({});
  });
});

describe('matchStatus', () => {
  it('is neutral for an output without an expectation', () => {
    expect(matchStatus('x', 5, {})).toBe('neutral');
    expect(matchStatus('x', 5, { x: undefined })).toBe('neutral');
  });

  it('compares ordinary values after typing the expectation', () => {
    expect(matchStatus('x', 5, { x: '5' })).toBe('passed');
    expect(matchStatus('x', true, { x: 'true' })).toBe('passed');
    expect(matchStatus('x', 0.1 + 0.2, { x: '0.3' })).toBe('passed');
    expect(matchStatus('x', 6, { x: '5' })).toBe('failed');
  });

  it('makes is-null a real check: only an absent value passes', () => {
    expect(matchStatus('x', null, { x: null })).toBe('passed');
    expect(matchStatus('x', 0, { x: null })).toBe('failed');
    expect(matchStatus('x', '', { x: null })).toBe('failed');
    expect(matchStatus('x', undefined, { x: null })).toBe('failed');
    expect(matchStatus('x', unknownFor(['huur', 'w']), { x: null })).toBe('failed');
  });

  it('passes an unknown expectation on any unknown outcome', () => {
    const exp = { x: { __unknown: true, missing: [] } };
    expect(matchStatus('x', unknownFor(['huur', 'w']), exp)).toBe('passed');
    expect(matchStatus('x', null, exp)).toBe('failed');
    expect(matchStatus('x', 5, exp)).toBe('failed');
    expect(matchStatus('x', undefined, exp)).toBe('failed');
  });

  it('passes unknown-for only when the named fact is among the missing ones', () => {
    const exp = { x: { __unknown: true, missing: [{ name: 'huur' }] } };
    expect(matchStatus('x', unknownFor(['huur', 'w']), exp)).toBe('passed');
    expect(matchStatus('x', unknownFor(['inkomen', 'w'], ['huur', 'w']), exp)).toBe('passed');
    expect(matchStatus('x', unknownFor(['inkomen', 'w']), exp)).toBe('failed');
    expect(matchStatus('x', null, exp)).toBe('failed');
  });

  it('never lets an unknown outcome satisfy a value expectation', () => {
    expect(matchStatus('x', unknownFor(['huur', 'w']), { x: '5' })).toBe('failed');
    expect(matchStatus('x', unknownFor(['huur', 'w']), { x: 'true' })).toBe('failed');
  });
});
