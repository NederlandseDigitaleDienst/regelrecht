/**
 * Engine value helpers shared by the editor and the demo (RFC-036).
 *
 * The engine knows two kinds of "nothing". `null` is absence: the register is
 * authoritative and says there is none (no partner, no rent). Unknown is a
 * fact that exists but nobody has supplied: a `source: {}` input without data,
 * or an optional parameter the caller did not pass. Unknown reaches JS as the
 * object
 *
 *     { "__unknown": true, "missing": [{ "law": "...", "name": "...", "kind": "no_data" | "not_passed" }] }
 *
 * with a sentinel key like `__untranslatable`. One definition lives here so
 * the runner, the editor and the demo cannot disagree on the shape.
 */

/** Why a fact is missing: no data source supplied it, or the caller did not pass it. */
export type MissingKind = 'no_data' | 'not_passed';

/** One fact an Unknown value is waiting for. */
export interface MissingFact {
  law: string;
  name: string;
  kind: MissingKind;
}

/**
 * The engine's Unknown value. Only the sentinel key is checked at runtime;
 * `missing` is whatever the engine sent, which is why `missingFacts` still
 * checks that it is an array.
 */
export interface UnknownValue {
  __unknown: true;
  missing?: unknown;
}

/** True when `value` is the engine's Unknown value. */
export function isUnknown(value: unknown): value is UnknownValue {
  return (
    value !== null
    && typeof value === 'object'
    && !Array.isArray(value)
    && (value as { __unknown?: unknown }).__unknown === true
  );
}

/**
 * The missing facts an Unknown value carries, each `{ law, name, kind }`.
 * An empty array for any other value, so callers need no `isUnknown` guard.
 * The elements are passed on as the engine sent them, without validation.
 */
export function missingFacts(value: unknown): MissingFact[] {
  if (!isUnknown(value) || !Array.isArray(value.missing)) return [];
  return value.missing as MissingFact[];
}
