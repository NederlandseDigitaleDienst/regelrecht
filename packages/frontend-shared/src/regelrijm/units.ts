// A literal has no unit of its own; it takes one from where it stands
// (RFC-054, "Literals"). The rule depends only on the shape of the tree and
// on what the article declares, never on the literal itself, so the renderer
// and the test parser compute the same unit for the same position.
import { ifCases, isOperation, isReference } from './normalise';
import { isRecord, type Path, type PathStep, type Unit } from './types';

export const UNITS: readonly Unit[] = Object.freeze(['euro', 'eurocent', 'ratio', 'percentage', 'years', 'weeks', 'months', 'days'] as const);

function isUnit(u: unknown): u is Unit {
  return (UNITS as readonly unknown[]).includes(u);
}

export function pathKey(path: Path): string {
  return path.join('/');
}

/**
 * The declared `type_spec.unit` of every name an article can read:
 * parameters, inputs, outputs and structured definitions.
 */
export function declaredUnits(article: unknown): Map<string, Unit> {
  const units = new Map<string, Unit>();
  const mr = isRecord(article) && isRecord(article.machine_readable) ? article.machine_readable : {};
  const ex = isRecord(mr.execution) ? mr.execution : {};
  for (const list of [ex.parameters, ex.input, ex.output]) {
    // Mid-edit a list can be anything; only a list declares units.
    if (!Array.isArray(list)) continue;
    for (const f of list as unknown[]) {
      if (!isRecord(f)) continue;
      const u = isRecord(f.type_spec) ? f.type_spec.unit : undefined;
      if (typeof f.name === 'string' && f.name && isUnit(u)) units.set(f.name, u);
    }
  }
  const defs = isRecord(mr.definitions) ? mr.definitions : {};
  for (const [name, def] of Object.entries(defs)) {
    const u = isRecord(def) && isRecord(def.type_spec) ? def.type_spec.unit : undefined;
    if (isUnit(u)) units.set(name, u);
  }
  return units;
}

/** The unit of an expression, when its shape says: a reference with a declared unit, or a sum, extreme, choice or rounding of one. */
export function exprUnit(node: unknown, declared: ReadonlyMap<string, Unit>): Unit | null {
  if (isReference(node)) {
    const name = node.slice(1);
    return name.includes('.') ? null : (declared.get(name) ?? null);
  }
  if (!isOperation(node)) return null;
  switch (node.operation) {
    case 'ADD': case 'SUBTRACT': case 'MIN': case 'MAX':
      return firstUnit(node.values, declared);
    case 'IF':
      return firstUnit([...ifCases(node).map((c) => c.then), node.default], declared);
    case 'ROUND': case 'CEIL': case 'FLOOR':
      return exprUnit(node.value, declared);
    default:
      return null;
  }
}

function firstUnit(list: unknown, declared: ReadonlyMap<string, Unit>): Unit | null {
  for (const v of Array.isArray(list) ? (list as unknown[]) : []) {
    const u = exprUnit(v, declared);
    if (u) return u;
  }
  return null;
}

function isLiteral(node: unknown): boolean {
  if (isReference(node)) return false;
  return node === null || typeof node !== 'object' || node instanceof Date || (node as { __lit?: unknown }).__lit !== undefined;
}

/**
 * Map from path key to unit for every literal in the tree under `root`
 * (whose own path is `rootPath`). Only literals with a unit are recorded.
 * `opUnits`, when given, receives the unit of the position of every
 * operation as well (the signature of a folded part reads it).
 */
export function literalUnits(
  root: unknown,
  rootUnit: Unit | null,
  declared: ReadonlyMap<string, Unit>,
  rootPath: Path = [],
  opUnits: Map<string, Unit | null> | null = null,
): Map<string, Unit> {
  const out = new Map<string, Unit>();
  const visit = (node: unknown, unit: Unit | null, path: Path): void => {
    if (isLiteral(node)) {
      if (unit) out.set(pathKey(path), unit);
      return;
    }
    if (!isOperation(node)) return;
    if (opUnits) opUnits.set(pathKey(path), unit);
    const at = (key: string, u: Unit | null) => {
      if (node[key] !== undefined) visit(node[key], u, [...path, key]);
    };
    const each = (key: string, u: Unit | null) => {
      const list = node[key];
      if (Array.isArray(list)) list.forEach((v: unknown, i) => visit(v, u, [...path, key, i]));
    };
    switch (node.operation) {
      case 'ADD': case 'SUBTRACT': case 'MIN': case 'MAX':
        each('values', firstUnit(node.values, declared) ?? unit);
        return;
      case 'MULTIPLY': case 'DIVIDE':
        each('values', null);
        return;
      case 'AND': case 'OR':
        each('conditions', null);
        return;
      case 'NOT':
        at('value', null);
        return;
      case 'EQUALS': case 'GREATER_THAN': case 'LESS_THAN': case 'GREATER_THAN_OR_EQUAL': case 'LESS_THAN_OR_EQUAL':
        at('subject', exprUnit(node.value, declared));
        at('value', exprUnit(node.subject, declared));
        return;
      case 'IN':
        at('subject', null);
        at('value', null);
        each('values', exprUnit(node.subject, declared));
        return;
      case 'IF': {
        const u = firstUnit([...ifCases(node).map((c) => c.then), node.default], declared) ?? unit;
        ifCases(node).forEach((c, i) => {
          if (c.when !== undefined) visit(c.when, null, [...path, 'cases', i, 'when']);
          if (c.then !== undefined) visit(c.then, u, [...path, 'cases', i, 'then']);
        });
        at('default', u);
        return;
      }
      case 'LIST':
        each('items', firstUnit(node.items, declared) ?? unit);
        return;
      case 'FOREACH':
        at('collection', null);
        at('filter', null);
        at('body', ['ADD', 'MIN', 'MAX', 'ONLY', undefined].includes(node.combine as string | undefined) ? unit : null);
        return;
      case 'ROUND': case 'CEIL': case 'FLOOR':
        at('value', unit);
        return;
      default:
        // Date operations and anything unknown: arguments carry no unit.
        for (const k of Object.keys(node)) {
          if (k === 'operation') continue;
          const v = node[k];
          if (Array.isArray(v)) v.forEach((x: unknown, i) => visit(x, null, [...path, k, i] as PathStep[]));
          else visit(v, null, [...path, k]);
        }
    }
  };
  visit(root, rootUnit ?? null, rootPath);
  return out;
}
