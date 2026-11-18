/**
 * Het pad van een tabblad, in de taal die aan staat.
 *
 * Every cross-view jump in the demo used to be a literal Dutch path
 * (`router.push('/wetten/' + id)`), and under `/en/` each of those quietly
 * moved the visitor into the Dutch tab: the page looked right, the language
 * just changed underneath them. A view therefore addresses a page by name and
 * lets this decide the path.
 *
 * Separate from `index.js` because it needs the router, and `index.js` is
 * imported *by* the router. It is also the one part of i18n that a plain
 * module cannot use: resolving a route needs a component's router instance.
 */
import { useRouter } from 'vue-router';
import type { RouteParamsRaw } from 'vue-router';
import { localeRouteName } from '../router';
import { useI18n } from './index.js';

export function useLocalePath() {
  const router = useRouter();
  const { locale } = useI18n();

  /**
   * @param page   the shared route name (`wetten`, `portaal`, …)
   * @param params
   */
  function localePath(page: string, params?: RouteParamsRaw): string {
    return router.resolve({ name: localeRouteName(page, locale.value), params }).path;
  }

  /** `localePath`, then navigate there. */
  function goTo(page: string, params?: RouteParamsRaw) {
    return router.push(localePath(page, params));
  }

  return { localePath, goTo };
}
