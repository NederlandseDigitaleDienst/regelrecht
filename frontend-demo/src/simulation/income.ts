/**
 * Disposable income per simulated citizen, as the POC summed it up: monthly
 * income, minus taxes, plus the benefits the laws grant, and once more after
 * housing costs. Which law outputs count, and whether a law states them per
 * month or per year, is configuration (`simulation.disposable_income` in
 * demo-config.yaml): the laws themselves do not declare a period.
 */
import { isUnknown } from '../data/format';
import { mean, median } from './stats';
import type { LawResult } from './stats';

/** One entry of `simulation.disposable_income` in demo-config.yaml. */
export interface IncomeComponent {
  /** law id */
  law: string;
  /** output name (an amount in eurocent) */
  output: string;
  period: 'month' | 'year';
  kind: 'benefit' | 'tax';
  /** how to name it; defaults to the law name */
  label?: string;
}

/**
 * The subject fields income and housing costs read. The keys are those of the
 * citizen subjects `population.ts` generates.
 */
export interface IncomeFacts {
  inkomen?: number;
  huurder?: boolean;
  huur?: number;
}

/** The part of a simulation result the income calculation reads. */
export interface IncomeResult<S extends IncomeFacts = IncomeFacts> {
  subject: S;
  laws: Record<string, LawResult>;
}

/** Disposable income of one subject, per month in euro; null where an unknown component makes it unknown. */
export interface DisposableIncome {
  income: number;
  taxes: number | null;
  benefits: number | null;
  /** per `law#output` its monthly value, null when unknown */
  parts: Record<string, number | null>;
  disposable: number | null;
  afterHousing: number | null;
  housing: number;
  unknown: boolean;
}

/** The average of one income component over a run. */
export interface IncomeComponentSummary {
  component: IncomeComponent;
  avg: number | null;
  withValue: number;
  unknown: number;
}

/** Disposable income summarised over a run (or one group of it). */
export interface DisposableIncomeSummary {
  count: number;
  undecided: number;
  avgIncome: number | null;
  avgTaxes: number | null;
  avgBenefits: number | null;
  avgDisposable: number | null;
  medianDisposable: number | null;
  avgAfterHousing: number | null;
  avgHousing: number | null;
  components: IncomeComponentSummary[];
}

/** One group of `disposableIncomeBreakdown`. */
export interface DisposableIncomeBreakdownRow extends DisposableIncomeSummary {
  group: string;
}

/**
 * Monthly euro value of one output for one result. 0 when the law says the
 * person does not qualify, the amount is absent, or the law was not part of
 * the run (switched off on purpose); `null` when the value is unknown (the
 * law could not decide, the amount is unknown, or the evaluation failed): an
 * unknown component makes the aggregate unknown, it is never counted as 0
 * (RFC-036).
 */
export function monthlyValue(result: IncomeResult, component: IncomeComponent): number | null {
  const law = result.laws?.[component.law];
  if (!law) return 0;
  if (!law.ok || law.met === 'unknown') return null;
  if (law.met === false) return 0;
  const raw = law.outputs?.[component.output];
  if (isUnknown(raw)) return null;
  if (typeof raw !== 'number') return 0;
  const euro = raw / 100;
  return component.period === 'year' ? euro / 12 : euro;
}

/** Monthly housing costs: the rent, or 30% of income for an owner (the POC's rule). */
export function housingCosts(subject: IncomeFacts): number {
  if (subject.huurder) return subject.huur ?? 0;
  return ((subject.inkomen ?? 0) / 12) * 0.3;
}

/**
 * Per subject: income, taxes, benefits, disposable income and the same after
 * housing costs, all per month in euro.
 */
export function disposableIncomeOf(result: IncomeResult, components: readonly IncomeComponent[]): DisposableIncome {
  const income = (result.subject.inkomen ?? 0) / 12;
  let taxes = 0;
  let benefits = 0;
  let unknown = false;
  const parts: Record<string, number | null> = {};
  for (const c of components) {
    const v = monthlyValue(result, c);
    parts[`${c.law}#${c.output}`] = v;
    if (v === null) unknown = true;
    else if (c.kind === 'tax') taxes += v;
    else benefits += v;
  }
  // One unknown component and the sums that include it are unknown too.
  const disposable = unknown ? null : income - taxes + benefits;
  const housing = housingCosts(result.subject);
  return {
    income,
    taxes: unknown ? null : taxes,
    benefits: unknown ? null : benefits,
    parts,
    disposable,
    afterHousing: disposable === null ? null : disposable - housing,
    housing,
    unknown,
  };
}

/**
 * Summary over a run: averages and medians, and the average of every component.
 */
export function summariseDisposableIncome(results: readonly IncomeResult[], components: readonly IncomeComponent[]): DisposableIncomeSummary {
  const rows = results.map((r) => disposableIncomeOf(r, components));
  return {
    count: rows.length,
    // Subjects whose disposable income is unknown; they are left out of the averages.
    undecided: rows.filter((r) => r.unknown).length,
    avgIncome: mean(rows.map((r) => r.income)),
    avgTaxes: mean(rows.map((r) => r.taxes)),
    avgBenefits: mean(rows.map((r) => r.benefits)),
    avgDisposable: mean(rows.map((r) => r.disposable)),
    medianDisposable: median(rows.map((r) => r.disposable)),
    avgAfterHousing: mean(rows.map((r) => r.afterHousing)),
    avgHousing: mean(rows.map((r) => r.housing)),
    components: components.map((component) => {
      const key = `${component.law}#${component.output}`;
      const values = rows.map((r) => r.parts[key]);
      return { component, avg: mean(values), withValue: values.filter((v) => typeof v === 'number' && v !== 0).length, unknown: values.filter((v) => v === null).length };
    }),
  };
}

/** The summary per group of a demographic dimension. */
export function disposableIncomeBreakdown<S extends IncomeFacts>(
  results: readonly IncomeResult<S>[],
  components: readonly IncomeComponent[],
  groupOf: (subject: S) => unknown,
  order: readonly string[] | null = null,
): DisposableIncomeBreakdownRow[] {
  const groups = new Map<string, IncomeResult<S>[]>();
  for (const r of results) {
    const g = String(groupOf(r.subject));
    if (!groups.has(g)) groups.set(g, []);
    // Set on the line above when it was missing.
    groups.get(g)!.push(r);
  }
  const keys = order ? order.filter((k) => groups.has(k)) : [...groups.keys()].sort();
  // Every key is in the map: either filtered on `has` or taken from its keys.
  return keys.map((key) => ({ group: key, ...summariseDisposableIncome(groups.get(key)!, components) }));
}
