/**
 * Shared output formatting and comparison utilities used by
 * ScenarioForm, the scenario runs (scenario/run.ts), ExecutionTraceView and the
 * graph trace panes.
 */

import { isUnknown, missingFacts } from '@regelrecht/frontend-shared';

/** One scenario assertion as the form mapper builds it; only the fields read here. */
interface AssertionLike {
  outputName?: string | null;
  assertionType?: string;
  value?: unknown;
}

/**
 * What a result view compares one output against: a string for an ordinary
 * value, `null` for `is null`, or the engine's Unknown shape for `is unknown`.
 */
export type Expectation = string | null | { __unknown: true; missing: { name: unknown }[] };

/** Outcome of comparing one output with its expectation. */
export type MatchStatus = 'passed' | 'failed' | 'neutral';

/**
 * The two kinds of "nothing" the engine produces (RFC-036) each get a Dutch
 * word, so the user never reads the JSON token `null` or the raw
 * `{__unknown: …}` object:
 * - `null` is absence, a value the register vouches for ("no partner"): `geen`;
 * - Unknown is a fact nobody supplied: `onbekend`, with the missing facts as
 *   supporting text (see formatOutputValueParts).
 * An `undefined` output is neither: the engine never produced a value under
 * that name (a failed run, an output of another article), and calling that
 * `geen` would state an absence nobody stated. It reads as `niet berekend`.
 */
export function formatValue(value: unknown): string {
  if (value === undefined) return 'niet berekend';
  if (value === null) return 'geen';
  if (isUnknown(value)) return 'onbekend';
  if (typeof value === 'boolean') return value ? 'ja' : 'nee';
  // A collection (RFC-016) or a record reaches the trace as an array or an
  // object; String() would render every element as "[object Object]".
  if (typeof value === 'object') return JSON.stringify(value);
  return String(value);
}

/**
 * Format YAML/engine identifiers for display: underscores → spaces,
 * all-caps strings → lowercase (`BESCHIKKING` → `beschikking`).
 * Mixed-case identifiers (e.g. `Artikel`) are left untouched.
 */
export function humanize(name: string): string;
export function humanize<T>(name: T): T;
export function humanize(name: unknown): unknown {
  if (typeof name !== 'string') return name;
  const spaced = name.replace(/_/g, ' ');
  return /[A-Z]/.test(spaced) && spaced === spaced.toUpperCase() ? spaced.toLowerCase() : spaced;
}

const EURO_FORMATTER = new Intl.NumberFormat('nl-NL', {
  style: 'currency',
  currency: 'EUR',
});

/**
 * Euro rendering of a value, or '' when the field isn't money.
 *
 * The declared `type_spec.unit` is the only signal, matching the engine
 * (`packages/engine/src/units.rs`): a unit labels the value, it never states a
 * conversion the engine performs. `eurocent` stores cents so display divides by
 * 100; `euro` already stores euros. An unannotated or unknown unit gets no
 * currency form at all — guessing from the field name used to hand
 * `toetsingsinkomen` a raw cent count and a name-matched integer a euro sign it
 * had not earned.
 */
function euroText(value: unknown, unit: string | null | undefined): string {
  if (typeof value !== 'number' || !Number.isFinite(value)) return '';
  if (unit === 'eurocent') return EURO_FORMATTER.format(value / 100);
  if (unit === 'euro') return EURO_FORMATTER.format(value);
  return '';
}

/**
 * `ontbreekt: huur (wet_x), partner_bsn (wet_y)` for an Unknown value, '' for
 * anything else. A fact without a law (an expectation from a scenario names
 * only the input) is listed by name alone.
 */
export function formatMissing(value: unknown): string {
  const facts = missingFacts(value);
  if (facts.length === 0) return '';
  return `ontbreekt: ${facts.map((f) => (f.law ? `${f.name} (${f.law})` : f.name)).join(', ')}`;
}

function supportingText(value: unknown, unit: string | null | undefined): string {
  return isUnknown(value) ? formatMissing(value) : euroText(value, unit);
}

export function formatOutputValue(value: unknown, unit?: string | null): string {
  const raw = formatValue(value);
  const extra = supportingText(value, unit);
  return extra ? `${raw} (${extra})` : raw;
}

/** Returns `{ text, supportingText }` for output rendering. For monetary
 *  outputs the euro-formatted value becomes supporting text; for an unknown
 *  outcome the facts that are missing do. */
export function formatOutputValueParts(
  value: unknown,
  unit?: string | null,
): { text: string; supportingText: string } {
  return { text: formatValue(value), supportingText: supportingText(value, unit) };
}

export function normalizeForCompare(value: unknown): unknown {
  if (value === 'true' || value === true) return true;
  if (value === 'false' || value === false) return false;
  if (value === 'null' || value === null) return null;
  if (typeof value === 'string' && /^-?\d+(\.\d+)?$/.test(value)) return Number(value);
  return value;
}

/**
 * The `{ outputName: expected }` map the result views compare against, built
 * from a scenario's assertions (formMapper shape). Values follow the engine's
 * own vocabulary so one comparison serves both: a string for an ordinary
 * value (typed again by normalizeForCompare), `null` for `is null`, and the
 * Unknown shape for `is unknown` / `is unknown for lack of "x"`, the latter
 * with the wanted fact under `missing` (RFC-036). Assertions without an
 * output (succeeds/fails) have nothing to compare and are left out; a later
 * assertion on the same output wins.
 */
export function expectationsFromAssertions(
  assertions: readonly AssertionLike[] | null | undefined,
): Record<string, Expectation> {
  const out: Record<string, Expectation> = {};
  for (const a of assertions || []) {
    if (!a.outputName) continue;
    switch (a.assertionType) {
      case 'null':
        out[a.outputName] = null;
        break;
      case 'unknown':
        out[a.outputName] = { __unknown: true, missing: [] };
        break;
      case 'unknownFor':
        out[a.outputName] = { __unknown: true, missing: [{ name: a.value }] };
        break;
      default:
        if (a.value !== null && a.value !== undefined) out[a.outputName] = String(a.value);
    }
  }
  return out;
}

/**
 * 'passed' | 'failed' | 'neutral' for one output against the expectations
 * map. `is null` is a real check: it passes only on an absent value. An
 * unknown expectation passes on any unknown outcome, and when it names a
 * fact, only if that fact is among the ones missing. An unknown outcome
 * never satisfies a value or null expectation.
 */
export function matchStatus(
  outputName: string,
  actualValue: unknown,
  expectations: Record<string, unknown>,
): MatchStatus {
  if (!(outputName in expectations)) return 'neutral';
  const expected = expectations[outputName];
  if (expected === undefined) return 'neutral';
  if (isUnknown(expected)) {
    if (!isUnknown(actualValue)) return 'failed';
    const actualNames = missingFacts(actualValue).map((f) => f.name);
    return missingFacts(expected).every((f) => actualNames.includes(f.name)) ? 'passed' : 'failed';
  }
  if (isUnknown(actualValue)) return 'failed';
  if (expected === null) return actualValue === null ? 'passed' : 'failed';
  const actual = normalizeForCompare(actualValue);
  const exp = normalizeForCompare(expected);
  if (actual === exp) return 'passed';
  if (typeof actual === 'number' && typeof exp === 'number' && Math.abs(actual - exp) < 1e-9) return 'passed';
  return 'failed';
}
