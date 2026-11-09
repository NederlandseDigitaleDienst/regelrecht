// The signature of a folded part (RFC-054, "Signatures"): what kind
// of value it gives and which inputs it reads, not how it computes. Like the
// signature an IDE shows for a call, it is derived, never authored: the kind
// from the operation and the unit of its position, the inputs from the free
// references in the subtree. The article's own constants (`definitions`) are
// the how, not the what, and are left out.
import { ifCases, isOperation, isReference, OPERATION_FIELDS } from './normalise';
import { exprUnit } from './units';
import type { Unit } from './types';

/** How many inputs a signature names before it counts the rest. */
export const SIGNATURE_MAX_INPUTS = 3;

const UNIT_KINDS: Record<string, string> = {
  euro: 'bedrag', eurocent: 'bedrag', percentage: 'percentage', ratio: 'verhouding',
  years: 'aantal jaren', months: 'aantal maanden', weeks: 'aantal weken', days: 'aantal dagen',
};
const CONDITION_OPS = new Set(['AND', 'OR', 'NOT', 'EQUALS', 'GREATER_THAN', 'LESS_THAN', 'GREATER_THAN_OR_EQUAL', 'LESS_THAN_OR_EQUAL', 'IN']);
const DIFF_KINDS: Record<string, string> = { days: 'aantal dagen', months: 'aantal maanden', years: 'aantal jaren' };
const PART_KINDS: Record<string, string> = { year: 'jaartal', month: 'maandnummer', day: 'dagnummer' };

/**
 * What kind of value `node` gives, in one word or two. `unit` is the unit of
 * the position it stands in (`literalUnits` with `opUnits`).
 */
export function valueKind(node: unknown, unit: Unit | null, declared: ReadonlyMap<string, Unit>): string {
  if (isReference(node)) {
    const u = exprUnit(node, declared) ?? unit;
    return (u && UNIT_KINDS[u]) ?? 'waarde';
  }
  if (!isOperation(node)) return 'waarde';
  const op = node.operation;
  if (CONDITION_OPS.has(op)) return 'voorwaarde';
  switch (op) {
    case 'DATE': case 'DATE_ADD': case 'START_OF': return 'datum';
    case 'AGE': return 'leeftijd';
    case 'DAY_OF_WEEK': return 'weekdag';
    case 'DATE_PART': return PART_KINDS[String(node.in)] ?? 'datumdeel';
    case 'DATE_DIFF': return DIFF_KINDS[String(node.in)] ?? 'tijdsduur';
    case 'LIST': return 'lijst';
    case 'FOREACH':
      if (node.combine === 'AND' || node.combine === 'OR') return 'voorwaarde';
      if (node.combine === undefined) return 'lijst';
      // A sum, minimum, maximum or only value over rows gives what each row gives.
      return valueKind(node.body, unit, declared);
    default:
  }
  const u = exprUnit(node, declared) ?? unit;
  if (u && UNIT_KINDS[u]) return UNIT_KINDS[u];
  if (op === 'IF') {
    // A choice between conditions is a condition.
    const branches = [...ifCases(node).map((c) => c.then), node.default].filter((b) => b !== undefined);
    if (branches.length && branches.every((b) => typeof b === 'boolean' || (isOperation(b) && valueKind(b, null, declared) === 'voorwaarde'))) return 'voorwaarde';
    return 'waarde';
  }
  return ['ADD', 'SUBTRACT', 'MULTIPLY', 'DIVIDE', 'ROUND', 'CEIL', 'FLOOR'].includes(op) ? 'getal' : 'waarde';
}

/**
 * The free references of `node` in reading order, each once: references to
 * a name the subtree binds itself (a FOREACH `as`) and to the article's
 * constants are not inputs.
 */
export function freeInputs(node: unknown, constants: ReadonlySet<string> = new Set()): string[] {
  const seen: string[] = [];
  const visit = (v: unknown, bound: ReadonlySet<string>): void => {
    if (isReference(v)) {
      const root = v.slice(1).split('.')[0] ?? '';
      if (!bound.has(root) && !constants.has(root) && !seen.includes(v)) seen.push(v);
      return;
    }
    if (Array.isArray(v)) {
      v.forEach((x: unknown) => visit(x, bound));
      return;
    }
    if (!isOperation(v)) return;
    const fields = OPERATION_FIELDS[v.operation] ?? Object.keys(v).filter((k) => k !== 'operation');
    for (const f of fields) {
      if (v[f] === undefined) continue;
      if (v.operation === 'FOREACH' && (f === 'filter' || f === 'body') && typeof v.as === 'string') {
        visit(v[f], new Set([...bound, v.as]));
      } else if (v.operation === 'IF' && f === 'cases') {
        for (const c of ifCases(v)) {
          if (c.when !== undefined) visit(c.when, bound);
          if (c.then !== undefined) visit(c.then, bound);
        }
      } else {
        visit(v[f], bound);
      }
    }
  };
  visit(node, new Set());
  return seen;
}
