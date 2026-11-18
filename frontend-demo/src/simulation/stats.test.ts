import { describe, expect, it } from 'vitest';
import { BUSINESS_DIMENSIONS, CITIZEN_DIMENSIONS, breakdown, describePopulation, flattenResults, groupLabel, mean, median, summariseLaw, toCsv } from './stats';
import { setLocale } from '../i18n/index.js';
import type { BusinessFacts, CitizenFacts, Dimension, SimulationResult } from './stats';

/** The dimension with this id; throws so a missing one fails the test. */
function dimensionById<S>(dimensions: readonly Dimension<S>[], id: string): Dimension<S> {
  const found = dimensions.find((d) => d.id === id);
  if (!found) throw new Error(`no dimension ${id}`);
  return found;
}

type TestCitizen = CitizenFacts & { id: string };

/** A citizen without `student`, which these tests do not read. */
function citizen(facts: Omit<TestCitizen, 'student'>): TestCitizen {
  return facts as TestCitizen;
}

const results: SimulationResult<TestCitizen>[] = [
  { subject: citizen({ id: '1', leeftijd: 25, inkomen: 15000, partner: false, huurder: true, kinderen: 0 }), laws: { a: { ok: true, met: true, amount: 100 } } },
  { subject: citizen({ id: '2', leeftijd: 40, inkomen: 45000, partner: true, huurder: false, kinderen: 2 }), laws: { a: { ok: true, met: false, amount: 0 } } },
  { subject: citizen({ id: '3', leeftijd: 70, inkomen: 25000, partner: true, huurder: false, kinderen: 0 }), laws: { a: { ok: true, met: true, amount: 300 } } },
  { subject: citizen({ id: '4', leeftijd: 90, inkomen: 65000, partner: false, huurder: true, kinderen: 3 }), laws: { a: { ok: false, met: null, amount: null, error: 'boem' } } },
];

describe('mean / median', () => {
  it('ignore non-numbers and return null for nothing', () => {
    expect(mean([1, null, 3, 'x'])).toBe(2);
    expect(median([5, 1, 3])).toBe(3);
    expect(median([4, 1, 3, 2])).toBe(2.5);
    expect(mean([])).toBeNull();
    expect(median([null])).toBeNull();
  });
});

describe('summariseLaw', () => {
  it('counts evaluated, eligible and errors and averages over the eligible', () => {
    const s = summariseLaw(results, 'a');
    expect(s).toMatchObject({ total: 4, evaluated: 3, errors: 1, eligible: 2, withAmount: 2, totalAmount: 400, avgAmount: 200, medianAmount: 200 });
    expect(s.eligiblePct).toBeCloseTo(66.67, 1);
  });

  it('counts an unknown verdict apart, never as eligible', () => {
    const s = summariseLaw([...results, { subject: { id: '5' }, laws: { a: { ok: true, met: 'unknown', amount: null } } }], 'a');
    expect(s).toMatchObject({ evaluated: 4, eligible: 2, undecided: 1, withAmount: 2 });
    expect(s.eligiblePct).toBe(50);
    expect(flattenResults([{ subject: { id: '5' }, laws: { a: { ok: true, met: 'unknown', amount: null } } }], ['a'])[0]).toMatchObject({ a__voldoet: 'onbekend', a__bedrag: null });
  });

  it('treats a law without voldoet_aan_voorwaarden as met', () => {
    const s = summariseLaw([{ subject: {}, laws: { b: { ok: true, met: null, amount: 5 } } }], 'b');
    expect(s.eligible).toBe(1);
  });
});

describe('breakdown', () => {
  it('splits by a dimension in the given order', () => {
    const dim = dimensionById(CITIZEN_DIMENSIONS, 'leeftijd');
    const rows = breakdown(results, 'a', dim.groupOf, dim.order);
    expect(rows.map((r) => r.group)).toEqual(['18-30', '30-45', '67-85', '85+']);
    expect(rows[0]).toMatchObject({ total: 1, eligible: 1, avgAmount: 100 });
  });

  it('has dimensions for both audiences', () => {
    expect(CITIZEN_DIMENSIONS.map((d) => d.id)).toContain('inkomen');
    // Only the field this dimension reads.
    expect(dimensionById(BUSINESS_DIMENSIONS, 'type').groupOf({ type: 'overig' } as BusinessFacts)).toBe('overig');
  });
});

describe('describePopulation / export', () => {
  it('summarises the citizens', () => {
    const d = describePopulation('burgers', results.map((r) => r.subject));
    expect(d.count).toBe(4);
    expect(d.partnerPct).toBe(50);
    expect(d.gemInkomen).toBe(37500);
  });

  it('flattens to one row per subject with per-law columns and writes CSV', () => {
    const rows = flattenResults(results, ['a']);
    expect(rows[0]).toMatchObject({ id: '1', a__voldoet: true, a__bedrag: 100 });
    expect(rows[3]).toMatchObject({ a__voldoet: null, a__fout: 'boem' });
    const csv = toCsv([{ x: 1, y: 'a;b' }, { x: null, y: 'q"r' }]);
    expect(csv.split('\n')).toEqual(['x;y', '1;"a;b"', ';"q""r"']);
  });
});

describe('groupLabel', () => {
  it('shows a group key in the active language, and leaves ranges as they are', () => {
    setLocale('en');
    try {
      expect(groupLabel('Met partner')).toBe('With partner');
      expect(groupLabel('horecabedrijf')).toBe('Hospitality');
      expect(groupLabel('18-30')).toBe('18-30');
    } finally {
      setLocale('nl');
    }
    expect(groupLabel('Met partner')).toBe('Met partner');
  });
});
