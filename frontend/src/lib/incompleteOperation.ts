/**
 * Whether an operation tree is complete enough to save.
 *
 * The action sheet's operation editor (OperationSettings) seeds a new or
 * retyped operation with empty slots: `values: []`, an empty `subject`, an
 * empty `date_of_birth`. Saving such a stub would write an action the engine
 * cannot run, so the save checks for them first.
 *
 * This check only works while it matches the stubs OperationSettings seeds.
 * Nothing enforces that yet; one operation model for both is a separate step.
 */

/** The operation-node slots the check walks; the tree comes from unchecked YAML. */
interface OperationLike {
  operation?: string;
  values?: unknown;
  conditions?: unknown;
  cases?: unknown;
  subject?: unknown;
  value?: unknown;
  default?: unknown;
  date_of_birth?: unknown;
  reference_date?: unknown;
}

const COMPARISON_OPS = new Set([
  'EQUALS', 'NOT_EQUALS', 'GREATER_THAN', 'GREATER_THAN_OR_EQUAL',
  'LESS_THAN', 'LESS_THAN_OR_EQUAL', 'NOT_NULL', 'IN', 'NOT_IN',
]);

/**
 * The name of the first incomplete operation in `raw`, or null when the tree
 * is structurally complete. A literal (not an operation) is always complete.
 */
export function findIncompleteOperation(raw: unknown): string | null {
  if (raw == null || typeof raw !== 'object') return null;
  // An operation node from unchecked YAML; only the slots below are read.
  const value = raw as OperationLike;
  if (!value.operation) return null;
  const op = value.operation;
  // Arithmetic / logical ops need a non-empty values or conditions array
  if (Array.isArray(value.values) && value.values.length === 0) return op;
  if (Array.isArray(value.conditions) && value.conditions.length === 0) return op;
  // IF/SWITCH need at least one case
  if ((op === 'IF' || op === 'SWITCH') && (!Array.isArray(value.cases) || value.cases.length === 0)) return op;
  // Comparison ops need a non-empty subject (and value, except for NOT_NULL).
  // changeOperationType / addNestedOperation seed these as empty strings.
  // IN/NOT_IN accept either a variable reference (e.g. "$list") or a literal
  // non-empty array; both pass the same value !== '' / length > 0 check.
  if (COMPARISON_OPS.has(op)) {
    if ((value.subject ?? '') === '') return op;
    if (op !== 'NOT_NULL') {
      const v = value.value;
      if (v == null || v === '') return op;
      if (Array.isArray(v) && v.length === 0) return op;
    }
  }
  // NOT wraps a single value/operation; reject the empty-string stub created
  // when transitioning from arithmetic ops via changeOperationType.
  if (op === 'NOT' && (value.value ?? '') === '') return op;
  // AGE has two structural slots - both must be filled. Empty strings are
  // the seed values from changeOperationType('AGE').
  if (op === 'AGE') {
    if ((value.date_of_birth ?? '') === '') return op;
    if ((value.reference_date ?? '') === '') return op;
  }
  // Recurse into structural slots
  for (const child of [value.subject, value.value, value.default, value.date_of_birth, value.reference_date]) {
    const inner = findIncompleteOperation(child);
    if (inner) return inner;
  }
  for (const list of [value.values, value.conditions]) {
    if (!Array.isArray(list)) continue;
    for (const item of list) {
      const inner = findIncompleteOperation(item);
      if (inner) return inner;
    }
  }
  if (Array.isArray(value.cases)) {
    for (const c of value.cases) {
      const inner = findIncompleteOperation(c?.when) || findIncompleteOperation(c?.then);
      if (inner) return inner;
    }
  }
  return null;
}
