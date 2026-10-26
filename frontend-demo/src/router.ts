import { createRouter, createWebHistory } from 'vue-router';
import type { RouteLocation, RouteRecordRaw, RouteRecordSingleView } from 'vue-router';
import { DEFAULT_LOCALE, LOCALES, adoptLocale } from './i18n/index.js';

// The meta fields every page record sets. Redirect records set none, hence optional.
declare module 'vue-router' {
  interface RouteMeta {
    /** The locale code the route belongs to. */
    locale?: string;
    /** The shared page name, the same in every language (`wetten`, `portaal`, …). */
    page?: string;
  }
}

/** One page of the demo: its name, its path per locale code, and its component. */
interface DemoPage {
  name: string;
  paths: Record<string, string>;
  formerPaths?: Record<string, string>;
  component: RouteRecordSingleView['component'];
}

/**
 * One route per workspace tab, in every language.
 *
 * Each page is one entry with one path per language, keyed on the language
 * code. The Dutch paths are the originals and keep working forever; a prefixed
 * language is additive. The English slugs are translated (`/en/ruleworks`, not
 * `/en/regelwerken`), the way the landing page does it (`/en/signup`) — a
 * half-translated URL reads as unfinished work.
 *
 * "Keep working forever" also holds for a page whose slug changes: it lists its
 * old addresses under `formerPaths`, and each becomes a redirect to the page in
 * the same language. The rulework tab lived at `/wetten` until the thing it
 * shows got its own name.
 *
 * A page is addressed by name, and that is what makes the rest cheap:
 * `router.resolve({ name: localeRouteName(page, locale), params })` yields the
 * path in the active locale, so the tab bar, the presentation deck and every
 * jump between views can name a page and stay language-agnostic. The docs
 * landing page instead maps counterparts with a hardcoded ternary that has to
 * be edited for every new page; this table is that mapping, derived.
 *
 * The name itself differs per language (`wetten` and `wetten:en`, so
 * vue-router can hold both), but both point at the *same* component, and that
 * is what `<keep-alive>` keys on. A language switch therefore reuses the
 * mounted view: an opened law tab or a running scenario survives it. Two
 * separate `() => import(…)` arrows would be two distinct functions and would
 * remount, which is why a page declares its component once.
 *
 * What the two genuinely share is `meta.page`. Views compare against that, not
 * against the route name.
 */
const PAGES: DemoPage[] = [
  { name: 'home', paths: { nl: '/', en: '/en', fy: '/fy' }, component: () => import('./views/HomeView.vue') },
  { name: 'presentatie', paths: { nl: '/presentatie', en: '/en/presentation', fy: '/fy/presintaasje' }, component: () => import('./views/PresentatieView.vue') },
  {
    name: 'wetten',
    paths: { nl: '/regelwerken/:lawId?', en: '/en/ruleworks/:lawId?', fy: '/fy/regelwurken/:lawId?' },
    formerPaths: { nl: '/wetten/:lawId?', en: '/en/laws/:lawId?', fy: '/fy/wetten/:lawId?' },
    component: () => import('./views/WettenView.vue'),
  },
  { name: 'graaf', paths: { nl: '/graaf', en: '/en/graph', fy: '/fy/graaf' }, component: () => import('./views/GraafView.vue') },
  { name: 'scenarios', paths: { nl: '/scenarios/:featurePath(.*)?', en: '/en/scenarios/:featurePath(.*)?', fy: '/fy/senarios/:featurePath(.*)?' }, component: () => import('./views/ScenariosView.vue') },
  { name: 'simulatie', paths: { nl: '/simulatie', en: '/en/simulation', fy: '/fy/simulaasje' }, component: () => import('./views/SimulatieView.vue') },
  { name: 'portaal', paths: { nl: '/portaal', en: '/en/portal', fy: '/fy/portaal' }, component: () => import('./views/PortaalView.vue') },
  { name: 'zaaksysteem', paths: { nl: '/zaaksysteem/:caseId?', en: '/en/cases/:caseId?', fy: '/fy/saaksysteem/:caseId?' }, component: () => import('./views/ZaaksysteemView.vue') },
  // The recorded walkthrough: this page starts it, and from there it drives
  // the deck and the tabs like a presenter. `:faqId` opens one answer, so a
  // single question can be shared. The Frisian slug drops the circumflex of
  // "rûnlieding" (a path must not need percent-encoding); see REVIEW-fy.md.
  { name: 'rondleiding', paths: { nl: '/rondleiding/:faqId?', en: '/en/tour/:faqId?', fy: '/fy/runlieding/:faqId?' }, component: () => import('./walkthrough/WalkthroughView.vue') },
];

// Elke taal uit de tabel moet elke pagina hebben. Een ontbrekend pad zou hier
// een route zonder `path` opleveren, en vue-router maakt daar een match op '/'
// van: de hele app zou dan op de home-pagina uitkomen, zonder foutmelding.
for (const l of LOCALES) {
  const missing = PAGES.filter((p) => !p.paths[l.code]).map((p) => p.name);
  if (missing.length) {
    throw new Error(`router: taal '${l.code}' mist een pad voor ${missing.join(', ')}`);
  }
}

/** The locale a path belongs to, by its prefix. */
export function localeFromPath(path: string): string {
  // De bron heeft een lege prefix en matcht daarom op alles; die slaan we over
  // en gebruiken hem als terugval.
  const hit = LOCALES.find((l) => l.prefix && (path === l.prefix || path.startsWith(`${l.prefix}/`)));
  return hit?.code ?? DEFAULT_LOCALE;
}

// vue-router matches in order and a name may appear once, so the two languages
// are separate records: the Dutch one keeps the bare name (it is the default
// and what `resolve({name})` should find first) and the English one is suffixed.
// `localeRouteName` is the single place that knows about that suffix.
const routes: RouteRecordRaw[] = [
  ...LOCALES.flatMap((l) =>
    PAGES.map((p) => ({
      // Every page has a path for every locale: checked at the top of this module.
      path: p.paths[l.code]!,
      name: localeRouteName(p.name, l.code),
      component: p.component,
      meta: { locale: l.code, page: p.name },
    })),
  ),
  // A former address redirects to the page in the same language, and takes the
  // params, query and hash along: an old link to one law still opens that law.
  ...LOCALES.flatMap((l) =>
    PAGES.filter((p) => p.formerPaths?.[l.code]).map((p) => ({
      // Only pages with a former path for this locale pass the filter.
      path: p.formerPaths![l.code]!,
      redirect: (to: RouteLocation) => ({
        name: localeRouteName(p.name, l.code),
        params: to.params,
        query: to.query,
        hash: to.hash,
      }),
    })),
  ),
  // An unknown path keeps the language it was typed in. Sending an English
  // visitor with a typo to the Dutch home page would be a silent demotion.
  //
  // De volgorde binnen deze lijst maakt hier niet uit, en dat is nagemeten en
  // niet aangenomen: vue-router rangschikt zijn matchers op specificiteit, dus
  // `/en/:pathMatch(.*)*` gaat vóór `/:pathMatch(.*)*` ook als hij eronder
  // staat. `/en/typefout` komt dus hoe dan ook op `/en` uit.
  ...LOCALES.filter((l) => l.prefix).map((l) => ({ path: `${l.prefix}/:pathMatch(.*)*`, redirect: l.prefix })),
  { path: '/:pathMatch(.*)*', redirect: '/' },
];

/** The route name for `page` in `locale`. */
export function localeRouteName(page: string, locale: string): string {
  return locale === DEFAULT_LOCALE ? page : `${page}:${locale}`;
}

/**
 * The page a Dutch path from `demo-config.yaml` refers to.
 *
 * The presentation deck's slides carry literal paths (`route: /regelwerken`), and
 * those are authored content, not code — translating them in the config would
 * put routing knowledge in a file about slides. Instead the path is read back
 * to its page name here, and the caller resolves that name in whatever locale
 * is active.
 */
export function pageForConfigPath(path: string | null | undefined): string | null {
  return splitConfigPath(path)?.page ?? null;
}

/**
 * A Dutch config path taken apart: the page it refers to, and what follows the
 * page's own root (`/zorgtoeslagwet` in `/regelwerken/zorgtoeslagwet`).
 *
 * The rest is cut off against the root that matched, not against the page's
 * current path. A slide written before a page moved still carries the old path,
 * and measured against the new root its law would silently fall away: the tab
 * would open on whatever law happened to be open.
 */
export function splitConfigPath(path: string | null | undefined): { page: string; rest: string } | null {
  const raw = String(path || '');
  // `split` always returns at least one element.
  const clean = raw.split(/[?#]/)[0]!;
  for (const p of PAGES) {
    for (const pattern of [p.paths[DEFAULT_LOCALE], p.formerPaths?.[DEFAULT_LOCALE]]) {
      if (!pattern) continue;
      const root = pattern.split('/:')[0]!;
      if (clean === root || clean.startsWith(`${root}/`)) {
        // `/` is the root of the home page only; it has nothing after it.
        return { page: p.name, rest: root === '/' ? '' : raw.slice(root.length) };
      }
    }
  }
  return null;
}

const router = createRouter({
  history: createWebHistory(),
  routes,
});

// The URL decides the language, not the other way round: opening a shared
// `/en/...` link has to show English. `adoptLocale` rather than `setLocale`,
// because arriving on a link someone sent you is not you choosing a language,
// so it must not overwrite a preference you set yourself.
router.beforeEach((to) => {
  adoptLocale(to.meta?.locale ?? localeFromPath(to.path));
});

export default router;
