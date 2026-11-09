/**
 * The routing table in two languages.
 *
 * The properties asserted here are the ones the rest of the app leans on: that
 * a page can be addressed by name and resolve to the active language, that
 * both languages reach the same component (which is what lets `<keep-alive>`
 * survive a switch), and that a wrong path keeps the language it was typed in.
 */
import { describe, expect, it } from 'vitest';
import router, { localeFromPath, localeRouteName, pageForConfigPath, splitConfigPath } from './router';

const PAGE_NAMES = ['home', 'presentatie', 'wetten', 'graaf', 'scenarios', 'simulatie', 'portaal', 'zaaksysteem'];

describe('localeFromPath', () => {
  it.each([
    ['/', 'nl'],
    ['/regelwerken', 'nl'],
    ['/wetten', 'nl'],
    ['/en', 'en'],
    ['/en/', 'en'],
    ['/en/ruleworks', 'en'],
    ['/en/laws', 'en'],
    ['/fr/reglementations', 'fr'],
    ['/fr/visite', 'fr'],
    // Not every path that begins with the letters "en": only the segment.
    ['/energie', 'nl'],
  ])('%s is %s', (path, expected) => {
    expect(localeFromPath(path)).toBe(expected);
  });
});

describe('the route table', () => {
  it('has both languages for every page', () => {
    for (const name of PAGE_NAMES) {
      expect(router.hasRoute(name), name).toBe(true);
      expect(router.hasRoute(`${name}:en`), `${name}:en`).toBe(true);
    }
  });

  it('resolves a page name to the path of its language', () => {
    expect(router.resolve({ name: 'wetten' }).path).toBe('/regelwerken');
    expect(router.resolve({ name: 'wetten:en' }).path).toBe('/en/ruleworks');
    expect(router.resolve({ name: 'home' }).path).toBe('/');
    expect(router.resolve({ name: 'home:en' }).path).toBe('/en');
  });

  it('resolves an optional param to a clean path, with no trailing slash', () => {
    // ScenariosView appends the raw file path to this, so a trailing slash
    // would produce `/scenarios//wet/...`.
    for (const name of ['wetten', 'scenarios', 'zaaksysteem']) {
      expect(router.resolve({ name }).path, name).not.toMatch(/\/$/);
      expect(router.resolve({ name: `${name}:en` }).path, name).not.toMatch(/\/$/);
    }
  });

  it('percent-encodes a slash inside a param, which the scenarios path must avoid', () => {
    // `scenarios` catches the rest of the path (`:featurePath(.*)`), and the
    // value is a file path with slashes in it. Passing it as a param turns
    // those into `%2F`, and ScenariosView then cannot find its own file.
    // The view therefore appends the raw path to `localePath('scenarios')`
    // instead of resolving with a param; this test pins why.
    const viaParam = router.resolve({ name: 'scenarios', params: { featurePath: 'wet/scenarios/x.feature' } }).path;
    expect(viaParam).toContain('%2F');
    expect(`${router.resolve({ name: 'scenarios' }).path}/wet/scenarios/x.feature`).toBe('/scenarios/wet/scenarios/x.feature');
    expect(`${router.resolve({ name: 'scenarios:en' }).path}/wet/scenarios/x.feature`).toBe('/en/scenarios/wet/scenarios/x.feature');
  });

  it('switches language on a scenario path without encoding the slashes', () => {
    // What App.vue's switchLocale does: take the tail of the current path
    // literally and hang it off the counterpart's root. Resolving with params
    // instead would percent-encode the slashes, and because the view is kept
    // alive nothing ever rewrites the address afterwards — the presenter is
    // left with an unreadable URL on screen.
    const current = '/scenarios/zorgtoeslagwet/scenarios/x.feature';
    const here = router.resolve({ name: 'scenarios' }).path;
    const tail = current.slice(here.length);
    const target = `${router.resolve({ name: 'scenarios:en' }).path}${tail}`;
    expect(target).toBe('/en/scenarios/zorgtoeslagwet/scenarios/x.feature');
    expect(target).not.toContain('%2F');
    // And it still resolves to the scenarios page with the path intact.
    const resolved = router.resolve(target);
    expect(resolved.meta.page).toBe('scenarios');
    expect(resolved.params.featurePath).toBe('zorgtoeslagwet/scenarios/x.feature');
  });

  it('keeps route params across languages', () => {
    const nl = router.resolve({ name: 'wetten', params: { lawId: 'zorgtoeslagwet' } });
    const en = router.resolve({ name: 'wetten:en', params: { lawId: 'zorgtoeslagwet' } });
    expect(nl.path).toBe('/regelwerken/zorgtoeslagwet');
    expect(en.path).toBe('/en/ruleworks/zorgtoeslagwet');
  });

  it('points both languages of a page at the same component', () => {
    // This is what makes a language switch keep an opened law tab. App.vue
    // renders `<keep-alive><component :is="Component" /></keep-alive>`, which
    // keys on the resolved component rather than on the path, so as long as
    // both languages resolve to the very same lazy import the mounted view is
    // reused across a switch. Measured in the browser: a marker set on the
    // pane's DOM node before switching from /en/ruleworks/zorgtoeslagwet is still
    // there afterwards, on /regelwerken/zorgtoeslagwet.
    //
    // Two separate `() => import(…)` arrows would be two distinct functions
    // and would remount, which is why PAGES holds one `component` per page.
    for (const name of PAGE_NAMES) {
      const nl = router.getRoutes().find((r) => r.name === name);
      const en = router.getRoutes().find((r) => r.name === `${name}:en`);
      // A missing route or component throws here and fails the test, as before.
      expect(en!.components!.default, name).toBe(nl!.components!.default);
    }
  });

  it('gives both languages of a page the same meta.page', () => {
    // Views watch `meta.page` to decide whether a route is theirs. On
    // `route.name` they would compare against `wetten` while the English route
    // is called `wetten:en`, return early, and leave the tab empty — no error,
    // just a blank pane on every /en/ deep link.
    for (const name of PAGE_NAMES) {
      expect(router.resolve({ name }).meta.page, name).toBe(name);
      expect(router.resolve({ name: `${name}:en` }).meta.page, name).toBe(name);
    }
  });

  it('records the locale on each route', () => {
    expect(router.resolve({ name: 'graaf' }).meta.locale).toBe('nl');
    expect(router.resolve({ name: 'graaf:en' }).meta.locale).toBe('en');
  });

  it('sends an unknown path home in the language it was typed in', () => {
    // `resolve` reports the matched record, and a redirect is applied when
    // navigating, so the redirect target is what to assert here. A typo under
    // /en/ must not silently demote an English visitor to the Dutch demo.
    const nl = router.resolve('/bestaat-niet');
    const en = router.resolve('/en/does-not-exist');
    // Both paths match the catch-all, so `matched` is never empty here.
    expect(nl.matched.at(-1)!.redirect).toBe('/');
    expect(en.matched.at(-1)!.redirect).toBe('/en');
  });
});

describe('de paden per taal', () => {
  it('bevatten geen apostrof of ander teken dat gecodeerd moet worden', () => {
    // Een pad moet te typen en door te sturen zijn. "Scenario's" heet in het
    // Fries "senario's", maar een apostrof in een URL wordt `%27`, en dat is
    // precies het adres dat tijdens een presentatie op het scherm komt of in
    // een chat geplakt wordt. Het Nederlands lost dat al zo op: het tabblad
    // heet "Scenario's" en het pad is `/scenarios`.
    //
    // Alleen kleine letters, cijfers, koppelteken en schuine streep, plus wat
    // vue-router zelf nodig heeft voor zijn parameters.
    for (const route of router.getRoutes()) {
      // De parameters eruit, inclusief hun herhaal-achtervoegsel (`*`, `+`, `?`):
      // die zijn syntaxis van vue-router en staan niet in het adres zelf.
      const withoutParams = route.path.replace(/:\w+(\([^)]*\))?[*+?]?/g, '');
      expect(withoutParams, `pad ${route.path}`).toMatch(/^[a-z0-9/-]*$/);
    }
  });

  it('geven elke taal een eigen pad per pagina', () => {
    // Twee talen die hetzelfde pad delen zouden op één route uitkomen, en dan
    // wint er stil één: de andere taal is dan onbereikbaar via haar eigen adres.
    const paths = router.getRoutes().map((r) => r.path);
    expect(new Set(paths).size).toBe(paths.length);
  });
});

describe('the QR code target', () => {
  it('points at the home page of the language that is on', () => {
    // HomeView builds it as origin + the resolved path of the current route.
    // Someone scanning during an English talk has to land in English; the
    // fixed '/' it used to build would put them in the Dutch demo.
    expect(router.resolve({ name: 'home' }).path).toBe('/');
    expect(router.resolve({ name: 'home:en' }).path).toBe('/en');
  });
});

describe('localeRouteName', () => {
  it('suffixes only English', () => {
    expect(localeRouteName('wetten', 'nl')).toBe('wetten');
    expect(localeRouteName('wetten', 'en')).toBe('wetten:en');
  });
});

describe('pageForConfigPath', () => {
  it.each([
    ['/regelwerken', 'wetten'],
    ['/regelwerken/zorgtoeslagwet', 'wetten'],
    // A slide written before the tab moved still carries the old path.
    ['/wetten', 'wetten'],
    ['/wetten/zorgtoeslagwet', 'wetten'],
    ['/graaf', 'graaf'],
    ['/scenarios/nl/wet/x.feature', 'scenarios'],
    ['/zaaksysteem/42', 'zaaksysteem'],
    ['/', 'home'],
  ])('%s is the %s page', (path, expected) => {
    expect(pageForConfigPath(path)).toBe(expected);
  });

  it('does not let the home page swallow every other path', () => {
    // `/` is a prefix of everything, so a naive startsWith would match it
    // first and send every slide to the landing page.
    expect(pageForConfigPath('/simulatie')).toBe('simulatie');
  });

  it('is null for a path no page owns', () => {
    expect(pageForConfigPath('/nergens')).toBe(null);
    expect(pageForConfigPath('')).toBe(null);
  });
});

describe('splitConfigPath', () => {
  it.each([
    ['/regelwerken', 'wetten', ''],
    ['/regelwerken/zorgtoeslagwet', 'wetten', '/zorgtoeslagwet'],
    // The old path keeps its law. Cut against the page's current root the rest
    // would come out empty, and the slide would open on whatever law was left
    // open: the tab right, the law wrong, and nothing to show for it.
    ['/wetten', 'wetten', ''],
    ['/wetten/zorgtoeslagwet', 'wetten', '/zorgtoeslagwet'],
    // A hash is part of what follows, not of the page's own path.
    ['/regelwerken/zorgtoeslagwet#tekst', 'wetten', '/zorgtoeslagwet#tekst'],
    ['/regelwerken#tekst', 'wetten', '#tekst'],
    ['/regelwerken/zorgtoeslagwet?artikel=2', 'wetten', '/zorgtoeslagwet?artikel=2'],
    ['/scenarios/nl/wet/x.feature', 'scenarios', '/nl/wet/x.feature'],
    ['/', 'home', ''],
  ])('%s is the %s page with %j after it', (path, page, rest) => {
    expect(splitConfigPath(path)).toEqual({ page, rest });
  });

  it('is null for a path no page owns', () => {
    expect(splitConfigPath('/nergens')).toBe(null);
  });
});

describe('a former address', () => {
  // The rulework tab lived at /wetten, /en/laws and /fy/wetten. Those addresses
  // are in slides, bookmarks and forwarded links, so each one has to land on
  // the page it used to be, in the language it was typed in, with the law that
  // was open still open.
  it.each([
    ['/wetten', '/regelwerken', 'nl'],
    ['/wetten/zorgtoeslagwet', '/regelwerken/zorgtoeslagwet', 'nl'],
    ['/en/laws', '/en/ruleworks', 'en'],
    ['/en/laws/zorgtoeslagwet', '/en/ruleworks/zorgtoeslagwet', 'en'],
    ['/fy/wetten', '/fy/regelwurken', 'fy'],
    ['/fy/wetten/zorgtoeslagwet', '/fy/regelwurken/zorgtoeslagwet', 'fy'],
  ])('%s redirects to %s', async (from, to, locale) => {
    await router.push(from);
    const route = router.currentRoute.value;
    expect(route.path).toBe(to);
    expect(route.meta.page).toBe('wetten');
    expect(route.meta.locale).toBe(locale);
  });

  it('keeps the query and the hash', async () => {
    await router.push('/wetten/zorgtoeslagwet?artikel=2#tekst');
    expect(router.currentRoute.value.fullPath).toBe('/regelwerken/zorgtoeslagwet?artikel=2#tekst');
  });
});
