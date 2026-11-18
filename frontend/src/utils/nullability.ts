/**
 * Nullability of a scenario value (RFC-036, schema v0.5.8).
 *
 * A field declaration carries `nullable: boolean` (default false): whether
 * the word `null`, a stated absence, is one of the field's values. The
 * scenario editor knows that declaration for a law's parameters and for the
 * external data-source columns of the dependency graph, and lets the author
 * state an absence only where the law allows one. A blank cell (no value
 * stated, unknown) is always allowed; that is a different thing from null.
 */

/** Shown under a field that states an absence the law does not allow. */
export const NOT_NULLABLE_MESSAGE = 'Dit gegeven kan niet afwezig zijn (niet nullable)';

/**
 * Whether a stated absence is allowed for a field with this declaration.
 *
 * Three states of `meta.nullable`: `true` (the law allows null), `false` (the
 * law declares the field as never absent, and the engine rejects a null
 * there), `undefined` (no declaration known: the element fields of a
 * collection, or a column the law never names). Only `false` restricts. An
 * unknown declaration makes no claim, the same rule the engine's type
 * checker follows, so the editor never refuses a null the engine accepts.
 *
 * @param {{ nullable?: boolean } | null | undefined} meta - field declaration
 * @returns {boolean}
 */
export function nullAllowed(meta: { nullable?: boolean } | null | undefined): boolean {
  return meta?.nullable !== false;
}

/** The two spellings of a stated absence in a form value. */
export function isNullText(v: unknown): v is null | 'null' {
  return v === null || v === 'null';
}
