/**
 * Summaries over simulation results: per law how many subjects meet the
 * conditions and what they get on average, and the same split by a
 * demographic dimension (age, income, partner, business type, size).
 * Pure functions over the run's `results` array.
 */
import type { Verdict } from '../data/format';
import { t } from '../i18n/index.js';

/**
 * The verdict of a law for one subject: true/false when the law decided,
 * 'unknown' when facts were missing (RFC-036), null when the law has no
 * `voldoet_aan_voorwaarden` output.
 */
export type LawVerdict = Verdict;

/** The outcome of one law for one subject, as the runner reduces it. */
export interface LawResult {
  /** the engine produced outputs */
  ok: boolean;
  /** voldoet_aan_voorwaarden, see `LawVerdict` */
  met: LawVerdict;
  /** primary output in euro (or its raw number), null when absent */
  amount: number | null;
  /** the output `amount` was taken from */
  amountName?: string | null;
  error?: string | null;
  /** the raw outputs of the evaluation, when it succeeded */
  outputs?: Record<string, unknown> | null;
}

/** One subject of a run with the outcome of every law that was evaluated for it. */
export interface SimulationResult<S = unknown> {
  subject: S;
  laws: Record<string, LawResult>;
}

/**
 * The citizen fields the dimensions and the population summary read. The keys
 * are those of the subjects `population.ts` generates.
 */
export interface CitizenFacts {
  leeftijd: number;
  inkomen: number;
  partner: boolean;
  huurder: boolean;
  kinderen: number;
  student: boolean;
}

/**
 * The business fields the dimensions and the population summary read. The keys
 * are those of the subjects `population.ts` generates.
 */
export interface BusinessFacts {
  type: string;
  grootte: string;
  voedsel: boolean;
  rechtsvorm: string;
  werknemers: number;
  terras: boolean;
  oppervlakte: number;
}

/** What `summariseLaw` reports for one law. */
export interface LawSummary {
  hasEligibility: boolean;
  total: number;
  evaluated: number;
  errors: number;
  undecided: number;
  eligible: number;
  eligiblePct: number;
  avgAmount: number | null;
  medianAmount: number | null;
  totalAmount: number;
  withAmount: number;
}

/** One group of a `breakdown`: the group key plus the law summary for it. */
export interface LawBreakdownRow extends LawSummary {
  group: string;
}

/** A demographic dimension to split a run by. */
export interface Dimension<S> {
  id: string;
  labelKey: string;
  /** the group keys in display order; null sorts them alphabetically */
  order: readonly string[] | null;
  groupOf: (subject: S) => string;
}

/** A dimension of either audience. */
export type SimulationDimension = Dimension<CitizenFacts> | Dimension<BusinessFacts>;

function isFiniteNumber(v: unknown): v is number {
  return typeof v === 'number' && Number.isFinite(v);
}

export function mean(values: readonly unknown[]): number | null {
  const xs = values.filter(isFiniteNumber);
  return xs.length ? xs.reduce((s, v) => s + v, 0) / xs.length : null;
}

export function median(values: readonly unknown[]): number | null {
  const xs = values.filter(isFiniteNumber).sort((a, b) => a - b);
  if (!xs.length) return null;
  const mid = Math.floor(xs.length / 2);
  // xs is not empty, so mid is a valid index, and mid - 1 is one whenever the length is even.
  return xs.length % 2 ? xs[mid]! : (xs[mid - 1]! + xs[mid]!) / 2;
}

export function summariseLaw(results: readonly SimulationResult[], lawId: string): LawSummary {
  const rows = results.map((r) => r.laws[lawId]).filter((r): r is LawResult => Boolean(r));
  const evaluated = rows.filter((r) => r.ok);
  // An unknown verdict is neither eligible nor ineligible: the law could not
  // decide for lack of facts. It is counted apart, never as a yes.
  const undecided = evaluated.filter((r) => r.met === 'unknown');
  const eligible = evaluated.filter((r) => r.met === true || r.met === null);
  const amounts = eligible.map((r) => r.amount).filter((v) => typeof v === 'number');
  return {
    // A law without `voldoet_aan_voorwaarden` (a tax, a registration) has no
    // notion of qualifying; it is computed for everyone.
    hasEligibility: evaluated.some((r) => r.met !== null),
    total: rows.length,
    evaluated: evaluated.length,
    errors: rows.length - evaluated.length,
    undecided: undecided.length,
    eligible: eligible.length,
    eligiblePct: evaluated.length ? (eligible.length / evaluated.length) * 100 : 0,
    avgAmount: mean(amounts),
    medianAmount: median(amounts),
    totalAmount: amounts.reduce((s, v) => s + v, 0),
    withAmount: amounts.length,
  };
}

/** Group results by a function of the subject and summarise one law per group. */
export function breakdown<S>(
  results: readonly SimulationResult<S>[],
  lawId: string,
  groupOf: (subject: S) => unknown,
  order: readonly string[] | null = null,
): LawBreakdownRow[] {
  const groups = new Map<string, SimulationResult<S>[]>();
  for (const r of results) {
    const g = String(groupOf(r.subject));
    if (!groups.has(g)) groups.set(g, []);
    // Set on the line above when it was missing.
    groups.get(g)!.push(r);
  }
  const keys = order ? order.filter((k) => groups.has(k)) : [...groups.keys()].sort();
  // Every key is in the map: either filtered on `has` or taken from its keys.
  return keys.map((key) => ({ group: key, ...summariseLaw(groups.get(key)!, lawId) }));
}

/**
 * De groepsnamen zijn sleutels, geen labels.
 *
 * `groupOf` levert ze op, `order` sorteert erop en de tabel keyt erop, dus ze
 * moeten in beide talen hetzelfde blijven. Wat je ervan ziet gaat door
 * `groupLabel()`, en dat is de enige plek die er een taal aan geeft. Ze
 * vertalen waar ze staan zou de groepering stilzwijgend breken: `order` zou
 * niet meer matchen en elke tabel viel terug op alfabetische volgorde.
 *
 * De sleutels blijven daarom Nederlands, zoals de veldnamen in het corpus dat
 * ook doen.
 */
const GROUP_LABELS: Record<string, string> = {
  'Met partner': 'group.with_partner',
  'Zonder partner': 'group.without_partner',
  Huur: 'group.rented',
  Koop: 'group.owned',
  Geen: 'group.none',
  'Bereidt voedsel': 'group.prepares_food',
  'Geen voedsel': 'group.no_food',
  horecabedrijf: 'group.hospitality',
  slijtersbedrijf: 'group.off_licence',
  overig: 'group.other',
  small: 'group.small',
  medium: 'group.medium',
  large: 'group.large',
};

/** Hoe een groep heet op het scherm. Leeftijden en bedragen spreken voor zich. */
export function groupLabel(group: string): string {
  const key = GROUP_LABELS[group];
  return key ? t(key) : String(group);
}

export const CITIZEN_DIMENSIONS: Dimension<CitizenFacts>[] = [
  {
    id: 'leeftijd',
    labelKey: 'sim.dimension.age',
    order: ['18-30', '30-45', '45-67', '67-85', '85+'],
    groupOf: (s) => (s.leeftijd < 30 ? '18-30' : s.leeftijd < 45 ? '30-45' : s.leeftijd < 67 ? '45-67' : s.leeftijd < 85 ? '67-85' : '85+'),
  },
  {
    id: 'inkomen',
    labelKey: 'sim.dimension.income',
    order: ['€ 0-20k', '€ 20-40k', '€ 40-60k', '€ 60k+'],
    groupOf: (s) => (s.inkomen < 20000 ? '€ 0-20k' : s.inkomen < 40000 ? '€ 20-40k' : s.inkomen < 60000 ? '€ 40-60k' : '€ 60k+'),
  },
  { id: 'partner', labelKey: 'sim.dimension.partner', order: ['Met partner', 'Zonder partner'], groupOf: (s) => (s.partner ? 'Met partner' : 'Zonder partner') },
  { id: 'wonen', labelKey: 'sim.dimension.housing', order: ['Huur', 'Koop'], groupOf: (s) => (s.huurder ? 'Huur' : 'Koop') },
  { id: 'kinderen', labelKey: 'sim.dimension.children', order: ['Geen', '1', '2', '3+'], groupOf: (s) => (s.kinderen === 0 ? 'Geen' : s.kinderen >= 3 ? '3+' : String(s.kinderen)) },
];

export const BUSINESS_DIMENSIONS: Dimension<BusinessFacts>[] = [
  { id: 'type', labelKey: 'sim.dimension.business_type', order: ['horecabedrijf', 'slijtersbedrijf', 'overig'], groupOf: (s) => s.type },
  { id: 'grootte', labelKey: 'sim.dimension.floor_area', order: ['small', 'medium', 'large'], groupOf: (s) => s.grootte },
  { id: 'voedsel', labelKey: 'sim.dimension.food', order: ['Bereidt voedsel', 'Geen voedsel'], groupOf: (s) => (s.voedsel ? 'Bereidt voedsel' : 'Geen voedsel') },
  { id: 'rechtsvorm', labelKey: 'sim.dimension.legal_form', order: null, groupOf: (s) => s.rechtsvorm },
  { id: 'werknemers', labelKey: 'sim.dimension.employees', order: ['Geen', '1-9', '10-49', '50+'], groupOf: (s) => (s.werknemers === 0 ? 'Geen' : s.werknemers < 10 ? '1-9' : s.werknemers < 50 ? '10-49' : '50+') },
];

/** Het label van een dimensie in de taal die aan staat. */
export function dimensionLabel(dimension: { labelKey?: string } | null | undefined): string {
  return dimension?.labelKey ? t(dimension.labelKey) : '';
}

/** Population-level facts of a business run. */
export interface BusinessPopulationSummary {
  count: number;
  horecaPct: number;
  voedselPct: number;
  terrasPct: number;
  gemOppervlakte: number | null;
  metWerknemersPct: number;
}

/** Population-level facts of a citizen run. */
export interface CitizenPopulationSummary {
  count: number;
  gemLeeftijd: number | null;
  partnerPct: number;
  huurderPct: number;
  kinderenPct: number;
  studentPct: number;
  gemInkomen: number | null;
  medInkomen: number | null;
}

export type PopulationSummary = BusinessPopulationSummary | CitizenPopulationSummary;

/** Population-level facts for the header of a run. */
export function describePopulation(kind: 'ondernemers', subjects: readonly BusinessFacts[]): BusinessPopulationSummary;
export function describePopulation(kind: string, subjects: readonly CitizenFacts[]): CitizenPopulationSummary;
export function describePopulation(kind: string, subjects: readonly (BusinessFacts | CitizenFacts)[]): PopulationSummary;
export function describePopulation(kind: string, subjects: readonly (BusinessFacts | CitizenFacts)[]): PopulationSummary {
  if (kind === 'ondernemers') {
    // The kind decides the audience the subjects were generated for.
    const businesses = subjects as readonly BusinessFacts[];
    return {
      count: businesses.length,
      horecaPct: pct(businesses, (s) => s.type !== 'overig'),
      voedselPct: pct(businesses, (s) => s.voedsel),
      terrasPct: pct(businesses, (s) => s.terras),
      gemOppervlakte: mean(businesses.map((s) => s.oppervlakte)),
      metWerknemersPct: pct(businesses, (s) => s.werknemers > 0),
    };
  }
  // Any other kind is a citizen run.
  const citizens = subjects as readonly CitizenFacts[];
  return {
    count: citizens.length,
    gemLeeftijd: mean(citizens.map((s) => s.leeftijd)),
    partnerPct: pct(citizens, (s) => s.partner),
    huurderPct: pct(citizens, (s) => s.huurder),
    kinderenPct: pct(citizens, (s) => s.kinderen > 0),
    studentPct: pct(citizens, (s) => s.student),
    gemInkomen: mean(citizens.map((s) => s.inkomen)),
    medInkomen: median(citizens.map((s) => s.inkomen)),
  };
}

function pct<T>(items: readonly T[], pred: (item: T) => unknown): number {
  return items.length ? (items.filter(pred).length / items.length) * 100 : 0;
}

/** Flat rows for a CSV/JSON export: subject attributes plus per-law outcome. */
/** One exported row: the subject's own fields plus `<law>__voldoet`, `<law>__bedrag` and `<law>__fout` columns. */
export type FlatResultRow = Record<string, unknown>;

export function flattenResults(results: readonly SimulationResult<object>[], lawIds: readonly string[]): FlatResultRow[] {
  return results.map((r) => {
    const row: FlatResultRow = { ...r.subject };
    for (const id of lawIds) {
      const l = r.laws[id];
      const short = id.replaceAll('/', '_');
      row[`${short}__voldoet`] = l ? (l.ok ? (l.met === 'unknown' ? 'onbekend' : l.met !== false) : null) : null;
      row[`${short}__bedrag`] = l?.amount ?? null;
      if (l && !l.ok) row[`${short}__fout`] = l.error;
    }
    return row;
  });
}

export function toCsv(rows: readonly Record<string, unknown>[]): string {
  if (!rows.length) return '';
  const columns = [...new Set(rows.flatMap((r) => Object.keys(r)))];
  const cell = (v: unknown): string => {
    if (v === null || v === undefined) return '';
    const s = typeof v === 'object' ? JSON.stringify(v) : String(v);
    return /[";\n]/.test(s) ? `"${s.replaceAll('"', '""')}"` : s;
  };
  return [columns.join(';'), ...rows.map((r) => columns.map((c) => cell(r[c])).join(';'))].join('\n');
}
