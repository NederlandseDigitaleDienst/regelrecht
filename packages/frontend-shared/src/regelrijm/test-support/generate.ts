// Seeded random action trees over every operation, for the property test.
// Deterministic (mulberry32), so a failing seed reproduces.
import { OPERATIONS } from '../normalise';
import type { Glossary, Operation, Unit } from '../types';

export function rng(seed: number): () => number {
  let a = seed >>> 0;
  return () => {
    a = (a + 0x6d2b79f5) >>> 0;
    let t = a;
    t = Math.imul(t ^ (t >>> 15), t | 1);
    t ^= t + Math.imul(t ^ (t >>> 7), t | 61);
    return ((t ^ (t >>> 14)) >>> 0) / 4294967296;
  };
}

const NAMES = ['a', 'b', 'inkomen', 'heeft_partner', 'PERCENTAGE_LID_5', 'percentage_lid_5', 'datum', 'bedrag_cent', 'bedrag_euro', 'pct', 'termijn', 'van', 'is_en_of', 'x1', '_lead', 'trail_'];
const STRINGS = ['HUWELIJK', 'Dienst Toeslagen', 'een “quote” en \\slash', '2025-01-01', '2025-02-30', 'geen', 'waar', '', '[x]', '‹t›', 'a; b', '1,5'];
const NUMBERS = [0, 1, 2, -3, 0.5, 12.25, 1500, 1500.5, 100000, 1e21, 1e-7, 0.1, 2025, 31, 12];

export const UNIT_DECLARATIONS = new Map<string, Unit>([
  ['bedrag_cent', 'eurocent'],
  ['bedrag_euro', 'euro'],
  ['pct', 'percentage'],
  ['termijn', 'days'],
  ['inkomen', 'eurocent'],
  ['a', 'years'],
  ['b', 'months'],
  ['x1', 'weeks'],
  ['datum', 'ratio'],
]);

// A glossary that deliberately uses keywords inside terms.
export const GLOSSARY: Glossary = { heeft_partner: 'de aanvrager heeft een partner én is gehuwd', inkomen: 'het inkomen van de aanvrager', van: 'van' };

export function generateAction(rand: () => number, maxDepth = 4): { output: string; value: unknown } {
  // `rand()` is in [0, 1), so the index is always within the list.
  const pick = <T>(xs: readonly T[]): T => xs[Math.floor(rand() * xs.length)]!;
  const ref = () => (rand() < 0.15 ? `$${pick(NAMES)}.${pick(NAMES)}` : `$${pick(NAMES)}`);
  const literal = (): unknown => {
    const r = rand();
    if (r < 0.15) return null;
    if (r < 0.25) return rand() < 0.5;
    if (r < 0.5) return pick(STRINGS);
    return pick(NUMBERS);
  };
  const leaf = () => (rand() < 0.55 ? ref() : literal());
  const many = (d: number, lo: number, hi: number): unknown[] => Array.from({ length: lo + Math.floor(rand() * (hi - lo + 1)) }, () => value(d));
  // Earlier operations come back now and then, so a tree has repeated parts
  // and the reading a legend.
  const pool: Operation[] = [];
  const value = (d: number): unknown => {
    if (pool.length && rand() < 0.12) return structuredClone(pick(pool));
    if (d <= 0 || rand() < 0.3) return leaf();
    const o = op(d - 1);
    pool.push(o);
    return o;
  };
  const op = (d: number): Operation => {
    const o = pick(OPERATIONS);
    switch (o) {
      case 'ADD': case 'SUBTRACT': case 'MULTIPLY': case 'DIVIDE': case 'MIN': case 'MAX':
        if (o === 'MAX' && rand() < 0.2) return { operation: 'MAX', values: [0, { operation: 'SUBTRACT', values: many(d, 2, 2) }] };
        return { operation: o, values: many(d, 0, 4) };
      case 'AND': case 'OR':
        return { operation: o, conditions: many(d, 0, 4) };
      case 'NOT':
        return { operation: 'NOT', value: rand() < 0.4 ? { operation: 'EQUALS', subject: value(d), value: value(d) } : value(d) };
      case 'EQUALS': case 'GREATER_THAN': case 'LESS_THAN': case 'GREATER_THAN_OR_EQUAL': case 'LESS_THAN_OR_EQUAL':
        return { operation: o, subject: value(d), value: value(d) };
      case 'IF': {
        const n: Operation = { operation: 'IF', cases: Array.from({ length: Math.floor(rand() * 3) }, () => ({ when: value(d), then: value(d) })) };
        if (rand() < 0.7) n.default = value(d);
        return n;
      }
      case 'IN':
        return rand() < 0.5 ? { operation: 'IN', subject: value(d), values: many(d, 0, 3) } : { operation: 'IN', subject: value(d), value: value(d) };
      case 'LIST':
        return { operation: 'LIST', items: many(d, 0, 3) };
      case 'FOREACH': {
        const n: Operation = { operation: 'FOREACH', collection: value(d), as: pick(['item', 'rij', 'van', 'x1']), body: value(d) };
        if (rand() < 0.5) n.filter = value(d);
        const c = pick(['ADD', 'OR', 'AND', 'MIN', 'MAX', 'ONLY', undefined]);
        if (c) n.combine = c;
        return n;
      }
      case 'AGE':
        return { operation: 'AGE', date_of_birth: value(d), reference_date: value(d) };
      case 'DATE_ADD': {
        const n: Operation = { operation: 'DATE_ADD', date: value(d) };
        for (const k of ['years', 'months', 'weeks', 'days']) if (rand() < 0.4) n[k] = rand() < 0.5 ? pick([1, 2, 30]) : value(d);
        return n;
      }
      case 'DATE':
        return rand() < 0.5
          ? { operation: 'DATE', year: value(d), month: 1 + Math.floor(rand() * 12), day: 1 + Math.floor(rand() * 31) }
          : { operation: 'DATE', year: value(d), month: value(d), day: value(d) };
      case 'DAY_OF_WEEK':
        return { operation: 'DAY_OF_WEEK', date: value(d) };
      case 'DATE_DIFF':
        return { operation: 'DATE_DIFF', from: value(d), to: value(d), in: rand() < 0.8 ? pick(['days', 'months', 'years']) : ref() };
      case 'DATE_PART':
        return { operation: 'DATE_PART', date: value(d), in: pick(['year', 'month', 'day']) };
      case 'START_OF':
        return { operation: 'START_OF', date: value(d), in: pick(['year', 'month']) };
      case 'ROUND': case 'CEIL': case 'FLOOR':
        return { operation: o, value: value(d), precision: pick([0, 1, 2, -2, 5]) };
      default:
        throw new Error(`generator does not know ${o}`);
    }
  };
  return { output: pick(NAMES), value: value(maxDepth) };
}
