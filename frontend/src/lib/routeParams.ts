import type { LocationQueryValue } from 'vue-router';

/**
 * One route param as a string.
 *
 * vue-router types every param as `string | string[]`, because a repeatable
 * segment (`:id+`, `:id*`) yields an array. The editor's routes have no
 * repeatable segments (see router.ts), so at runtime a param is a string, or
 * undefined when an optional segment is absent. This passes both through
 * unchanged; the array branch is for the type only and cannot occur with
 * these routes.
 *
 * Not for query values: a repeated `?key=` really does arrive as an array.
 * Use singleQuery for those.
 */
export function singleParam(value: string | string[] | undefined): string | undefined {
  return Array.isArray(value) ? value[0] : value;
}

/**
 * One query value as a string.
 *
 * Unlike a param, a query value really can be an array: `?law=a&law=b` gives
 * `['a', 'b']`. Passed into a route param, that makes vue-router throw
 * ("is an array but it is not repeatable"). This takes the first value, the
 * same rule as singleParam. A valueless key (`?law`) arrives as null and
 * becomes undefined, as does a missing key; an empty string stays empty.
 */
export function singleQuery(
  value: LocationQueryValue | LocationQueryValue[] | undefined,
): string | undefined {
  const first = Array.isArray(value) ? value[0] : value;
  return first ?? undefined;
}
