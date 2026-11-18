/**
 * Which editor a value deserves.
 *
 * Every value a citizen or caseworker sees may be corrected, so every type the
 * corpus declares needs an input that fits it: a date gets a date picker, a
 * boolean a yes/no choice, a table its rows. Guessing from the runtime value
 * alone is not enough (an absent value carries no type), so the declared
 * `type` in the law decides first and the value only fills in what the
 * declaration leaves open.
 *
 * The corpus declares seven value types: boolean, string, amount, number,
 * array, object and date. `amount` is money in cents and is entered in euros;
 * `array` splits by what it holds, because an array of records is a table and
 * an array of plain values is a list.
 */

import { isAmountSpec, isUnknown } from './format';
import type { FieldSpec } from './format';

/** The editor a whole value gets (see `editKind`). */
export type EditKind = 'boolean' | 'amount' | 'number' | 'date' | 'rows' | 'list' | 'record' | 'text';

/** The editor one plain value in a list, record or table cell gets. */
export type ValueKind = 'boolean' | 'number' | 'date' | 'text';

/** One row of a table value: a plain record. */
export type TableRow = Record<string, unknown>;

/** True for a non-empty array whose elements are all plain objects: a table. */
export function isRecordArray(value: unknown): value is TableRow[] {
  return (
    Array.isArray(value) &&
    value.length > 0 &&
    value.every((row) => row && typeof row === 'object' && !Array.isArray(row))
  );
}

/**
 * The editor kind for one value with its declared spec.
 *
 * @param value the current value, possibly null (absent) or unknown
 * @param spec the field's declared `type`/`type_spec`
 */
export function editKind(value: unknown, spec: FieldSpec | null = null): EditKind {
  const declared = spec?.type;
  // An unknown value (RFC-036) is a marker object, not a value: reading its
  // shape would make every unknown field a record editor. The declaration
  // decides, exactly as it does for an absent value.
  if (isUnknown(value)) value = null;
  // The declaration wins: it holds for an absent value too, where the value
  // itself says nothing.
  if (declared === 'boolean' || typeof value === 'boolean') return 'boolean';
  if (isAmountSpec(spec)) return 'amount';
  if (declared === 'number') return 'number';
  if (declared === 'date') return 'date';
  if (declared === 'array') return isRecordArray(value) ? 'rows' : 'list';
  if (declared === 'object') return 'record';
  if (declared === 'string') return 'text';
  // No usable declaration: read the value.
  if (isRecordArray(value)) return 'rows';
  if (Array.isArray(value)) return 'list';
  if (value && typeof value === 'object') return 'record';
  if (typeof value === 'number') return 'number';
  if (typeof value === 'string' && isIsoDate(value)) return 'date';
  return 'text';
}

/** The editor kind for one plain value inside a list or a table cell. */
export function valueKind(value: unknown): ValueKind {
  if (typeof value === 'boolean') return 'boolean';
  if (typeof value === 'number') return 'number';
  if (typeof value === 'string' && isIsoDate(value)) return 'date';
  return 'text';
}

function isIsoDate(s: string): boolean {
  return /^\d{4}-\d{2}-\d{2}$/.test(s);
}

/** The columns of a table: every key, in the order they first appear. */
export function tableColumns(rows: ReadonlyArray<TableRow | null | undefined> | null | undefined): string[] {
  const seen: string[] = [];
  for (const row of rows ?? []) {
    for (const key of Object.keys(row ?? {})) if (!seen.includes(key)) seen.push(key);
  }
  return seen;
}

/**
 * The kind a table column deserves, judged on the values present in it. An
 * empty column (every row absent there) falls back to free text.
 */
export function columnKind(rows: ReadonlyArray<TableRow | null | undefined> | null | undefined, column: string): ValueKind {
  const values = (rows ?? []).map((r) => r?.[column]).filter((v) => v !== null && v !== undefined);
  if (values.some((v) => typeof v === 'boolean')) return 'boolean';
  if (values.length && values.every((v) => typeof v === 'number')) return 'number';
  if (values.length && values.every((v) => typeof v === 'string' && isIsoDate(v))) return 'date';
  return 'text';
}

/** Type one entered cell back to the kind its column holds. */
export function parseCell(raw: unknown, kind: string): unknown {
  if (kind === 'boolean') return raw === 'true' || raw === true;
  if (kind === 'number') return parseDutchNumber(raw);
  return raw === '' || raw === undefined ? null : raw;
}

/**
 * One number as a Dutch person writes it: `1.234,56` is one thousand two
 * hundred thirty-four and 56 hundredths, not a broken `1.234`. The dots are
 * thousands separators, the comma is the decimal mark. Plain `1234.56` still
 * parses, because a value copied from a register looks like that.
 */
export function parseDutchNumber(raw: unknown): number | null {
  if (raw === '' || raw === null || raw === undefined) return null;
  let text = String(raw).trim();
  if (text === '') return null;
  // A comma present means the dots are thousands separators; without one the
  // dot is the decimal mark and must stay.
  if (text.includes(',')) text = text.replace(/\./g, '').replace(',', '.');
  const n = Number(text);
  return Number.isFinite(n) ? n : null;
}

/**
 * How many decimals a field admits. `precision` is the number of decimals the
 * law declares; money is always two, because it is entered in euros. Absent,
 * the field takes whatever the citizen types.
 */
export function decimalsFor(spec: FieldSpec | null | undefined): number | null {
  if (isAmountSpec(spec)) return 2;
  const p = spec?.type_spec?.precision;
  return typeof p === 'number' && p >= 0 ? p : null;
}

/** The step a number field should take, so a float field is not integer-only. */
export function stepFor(spec: FieldSpec | null | undefined): string {
  const d = decimalsFor(spec);
  if (d === null) return 'any';
  return d === 0 ? '1' : String(1 / 10 ** d);
}

/**
 * The unit shown beside a number, in words. Money is labelled in euros because
 * the value travels in cents; the rest names what it counts.
 */
export function unitLabel(spec: FieldSpec | null | undefined): string | null {
  if (isAmountSpec(spec)) return 'in euro';
  const unit = spec?.type_spec?.unit;
  const words: Record<string, string> = { years: 'in jaren', months: 'in maanden', weeks: 'in weken', days: 'in dagen', percentage: 'in procent', euro: 'in euro' };
  return words[String(unit)] ?? null;
}

/** A new row for a table: the same columns, all empty, so the shape holds. */
export function emptyRow(columns: readonly string[] | null | undefined): Record<string, null> {
  return Object.fromEntries((columns ?? []).map((c) => [c, null]));
}
