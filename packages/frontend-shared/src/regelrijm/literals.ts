// Literal values. No Intl, no locale and no float arithmetic: money moves its
// decimal point on the digit string, so the same number always gives the same
// bytes and the test parser can give the number back exactly.

import type { Unit } from './types';

export const MONTHS: readonly string[] = Object.freeze([
  'januari', 'februari', 'maart', 'april', 'mei', 'juni', 'juli',
  'augustus', 'september', 'oktober', 'november', 'december',
]);

export const PERIOD_WORDS: Readonly<Record<string, readonly [string, string]>> = Object.freeze({
  years: ['jaar', 'jaren'],
  months: ['maand', 'maanden'],
  weeks: ['week', 'weken'],
  days: ['dag', 'dagen'],
});

/** `-12.5` → `{ sign: '-', int: '12', frac: '5' }`, never in exponent form. */
function digits(n: number): { sign: string; int: string; frac: string } {
  let s = String(Math.abs(n));
  const sign = n < 0 ? '-' : '';
  const m = /^(\d)(?:\.(\d+))?e([+-]\d+)$/.exec(s);
  if (m) {
    // The regex has matched, so its first group is there.
    const mant = m[1]! + (m[2] ?? '');
    const exp = Number(m[3]);
    if (exp >= 0) {
      s = mant.length - 1 <= exp ? mant + '0'.repeat(exp - (mant.length - 1)) : `${mant.slice(0, exp + 1)}.${mant.slice(exp + 1)}`;
    } else {
      s = `0.${'0'.repeat(-exp - 1)}${mant}`;
    }
  }
  const [int = '', frac = ''] = s.split('.');
  return { sign, int, frac };
}

export function formatNumber(n: number): string {
  const { sign, int, frac } = digits(n);
  return `${sign}${int}${frac ? `,${frac}` : ''}`;
}

function groupThousands(int: string): string {
  return int.replace(/\B(?=(\d{3})+(?!\d))/g, '.');
}

/** `€ 1.500,00`. A eurocent value moves its decimal point two places; at least two decimals, more when the value has them. */
export function formatMoney(n: number, unit: Unit | null): string {
  let { sign, int, frac } = digits(n);
  if (unit === 'eurocent') {
    const all = (int + frac).replace(/^0+(?=\d)/, '');
    const point = all.length - frac.length - 2;
    const padded = point < 1 ? '0'.repeat(1 - point) + all : all;
    const p = Math.max(point, 1);
    int = padded.slice(0, p);
    frac = padded.slice(p);
    int = int.replace(/^0+(?=\d)/, '');
  }
  frac = frac.replace(/0+$/, '');
  if (frac.length < 2) frac = frac.padEnd(2, '0');
  return `€ ${sign}${groupThousands(int)},${frac}`;
}

export function formatQuantity(n: number, unit: string): string {
  const [one, many] = PERIOD_WORDS[unit] ?? ['', ''];
  return `${formatNumber(n)} ${n === 1 ? one : many}`;
}

export function isIsoDate(s: string): boolean {
  const m = /^(\d{4})-(\d{2})-(\d{2})$/.exec(s);
  if (!m) return false;
  const [y, mo, d] = [Number(m[1]), Number(m[2]), Number(m[3])];
  if (mo < 1 || mo > 12 || d < 1) return false;
  const leap = (y % 4 === 0 && y % 100 !== 0) || y % 400 === 0;
  // mo is 1..12 here, so the index is in range.
  const days = [31, leap ? 29 : 28, 31, 30, 31, 30, 31, 31, 30, 31, 30, 31][mo - 1]!;
  return d <= days;
}

/** `2025-01-01` → `1 januari 2025` (the year keeps its four digits). */
export function formatDate(s: string): string {
  const [y, mo, d] = s.split('-');
  return `${Number(d)} ${MONTHS[Number(mo) - 1]} ${y}`;
}

export function formatString(s: string): string {
  return `“${s.replace(/\\/g, '\\\\').replace(/”/g, '\\”')}”`;
}

/**
 * The reading of a literal at a position whose unit is `unit` (or null).
 * Returns null for a value that is no literal in the schema.
 */
export function formatLiteral(v: unknown, unit: Unit | null | undefined): string | null {
  if (v === null) return 'geen';
  if (v === true) return 'waar';
  if (v === false) return 'onwaar';
  if (v instanceof Date) return formatDate(v.toISOString().slice(0, 10));
  if (typeof v === 'string') return isIsoDate(v) ? formatDate(v) : formatString(v);
  if (typeof v === 'number' && Number.isFinite(v)) {
    if (unit === 'euro' || unit === 'eurocent') return formatMoney(v, unit);
    if (unit === 'percentage') return `${formatNumber(v)}%`;
    if (unit && PERIOD_WORDS[unit]) return formatQuantity(v, unit);
    return formatNumber(v);
  }
  return null;
}
