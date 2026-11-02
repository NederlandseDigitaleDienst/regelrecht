/**
 * Whether the short page (KortPagina.vue) shows instead of the choice
 * screen. It works as the explanation page does (useUitleg.js), under its own
 * parameter: `?kort`.
 *
 * `createKort` makes one, for the tests; the app shares one through
 * `useKort`.
 */
import { createPagina } from './useUitleg.js';

/** The URL parameter that opens the page. */
export const KORT = 'kort';

export const createKort = () => createPagina(KORT);

let shared = null;

/** The short page of the app. */
export function useKort() {
  shared ??= createKort();
  return shared;
}
