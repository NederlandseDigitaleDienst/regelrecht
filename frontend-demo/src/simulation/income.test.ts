import { describe, expect, it } from 'vitest';
import { disposableIncomeBreakdown, disposableIncomeOf, housingCosts, monthlyValue, summariseDisposableIncome } from './income';
import type { IncomeComponent, IncomeFacts, IncomeResult } from './income';
import type { LawResult } from './stats';

// A tuple, so `components[0]` is known to exist.
const components = [
  { law: 'ib', output: 'belasting', period: 'year', kind: 'tax' },
  { law: 'zt', output: 'toeslag', period: 'year', kind: 'benefit' },
  { law: 'ht', output: 'subsidie', period: 'month', kind: 'benefit' },
] as const satisfies readonly IncomeComponent[];
/** A result whose laws leave out `amount`: the income calculation reads `outputs`. */
const result = (subject: IncomeFacts & { leeftijd?: number }, laws: Record<string, Omit<LawResult, 'amount'>>): IncomeResult => ({ subject, laws: laws as Record<string, LawResult> });
const renter = result(
  { inkomen: 24000, huurder: true, huur: 600, leeftijd: 30 },
  {
    ib: { ok: true, met: null, outputs: { belasting: 240000 } }, // € 2.400 per year
    zt: { ok: true, met: true, outputs: { toeslag: 120000 } }, // € 1.200 per year
    ht: { ok: true, met: true, outputs: { subsidie: 30000 } }, // € 300 per month
  },
);
const owner = result({ inkomen: 60000, huurder: false, leeftijd: 50 }, { ib: { ok: true, met: null, outputs: { belasting: 1200000 } }, zt: { ok: true, met: false, outputs: { toeslag: 0 } } });

describe('monthlyValue', () => {
  it('converts a yearly amount to a month and ignores a law whose conditions are not met', () => {
    expect(monthlyValue(renter, components[0])).toBe(200);
    expect(monthlyValue(renter, components[2])).toBe(300);
    expect(monthlyValue(owner, components[1])).toBe(0);
  });
});

describe('monthlyValue with unknowns', () => {
  const unknown = { __unknown: true, missing: [{ law: 'zt', name: 'spaargeld', kind: 'no_data' }] };
  const undecided = result({ inkomen: 24000, huurder: false }, { ib: { ok: true, met: null, outputs: { belasting: 240000 } }, zt: { ok: true, met: 'unknown', outputs: { toeslag: unknown } }, ht: { ok: false, met: null, outputs: null } });

  it('is null, not 0, for an unknown verdict, an unknown amount or a failed evaluation', () => {
    expect(monthlyValue(undecided, components[1])).toBeNull();
    expect(monthlyValue(undecided, components[2])).toBeNull();
    expect(monthlyValue(undecided, components[0])).toBe(200);
  });

  it('makes the aggregate unknown and leaves the subject out of the averages', () => {
    const d = disposableIncomeOf(undecided, components);
    expect(d).toMatchObject({ income: 2000, taxes: null, benefits: null, disposable: null, afterHousing: null, unknown: true });
    const s = summariseDisposableIncome([renter, undecided], components);
    expect(s.undecided).toBe(1);
    expect(s.avgDisposable).toBe(2200);
    expect(s.components.find((c) => c.component.law === 'zt')).toMatchObject({ withValue: 1, unknown: 1 });
  });
});

describe('disposableIncomeOf', () => {
  it('is income minus taxes plus benefits, per month, and again after housing', () => {
    const d = disposableIncomeOf(renter, components);
    expect(d.income).toBe(2000);
    expect(d.taxes).toBe(200);
    expect(d.benefits).toBe(400);
    expect(d.disposable).toBe(2200);
    expect(d.afterHousing).toBe(1600);
  });

  it('charges an owner 30% of income as housing costs', () => {
    expect(housingCosts(owner.subject)).toBe(1500);
    expect(disposableIncomeOf(owner, components).disposable).toBe(4000);
  });
});

describe('summariseDisposableIncome', () => {
  it('averages over the run and per component', () => {
    const s = summariseDisposableIncome([renter, owner], components);
    expect(s.avgDisposable).toBe(3100);
    expect(s.medianDisposable).toBe(3100);
    expect(s.components.find((c) => c.component.law === 'ht')).toMatchObject({ avg: 150, withValue: 1 });
  });

  it('breaks down by a dimension in the given order', () => {
    const rows = disposableIncomeBreakdown([renter, owner], components, (s) => (s.huurder ? 'Huur' : 'Koop'), ['Huur', 'Koop']);
    expect(rows.map((r) => r.group)).toEqual(['Huur', 'Koop']);
    expect(rows[1]?.avgDisposable).toBe(4000);
  });
});
