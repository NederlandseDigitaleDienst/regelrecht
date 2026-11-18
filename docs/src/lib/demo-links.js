/*
 * Links from this site into the demo, per language.
 *
 * The demo translates its paths (`/simulatie` is `/en/simulation`), so a link
 * that hardcodes one of them sends every reader to that language. Each link
 * goes through `demoUrl` instead, which picks the path for the page's own
 * language.
 *
 * The paths are a copy of the route table in `frontend-demo/src/router.ts`.
 * `scripts/check-demo-links.mjs` compares the two, so a renamed demo route
 * fails the docs source checks instead of staying a dead link. Those checks
 * run when `docs/` or `packages/` changes, not yet on a change to the demo's
 * router alone. This file is plain JavaScript because that script loads it in
 * Node.
 *
 * The demo also runs in Frisian and French; this site has Dutch and English
 * only, so those are the two columns here.
 */
export const DEMO_ORIGIN = 'https://demo.regelrecht.rijks.app';

export const DEMO_PATHS = {
  home: { nl: '/', en: '/en' },
  simulatie: { nl: '/simulatie', en: '/en/simulation' },
  portaal: { nl: '/portaal', en: '/en/portal' },
  rondleiding: { nl: '/rondleiding', en: '/en/tour' },
};

/**
 * The address of a demo page in the given language.
 *
 * @param {keyof typeof DEMO_PATHS} page
 * @param {'nl' | 'en'} lang
 * @returns {string}
 */
export function demoUrl(page, lang) {
  const paths = DEMO_PATHS[page];
  if (!paths) throw new Error(`demo-links: unknown demo page "${page}"`);
  const path = paths[lang];
  if (!path) throw new Error(`demo-links: no path for "${page}" in "${lang}"`);
  return DEMO_ORIGIN + path;
}
