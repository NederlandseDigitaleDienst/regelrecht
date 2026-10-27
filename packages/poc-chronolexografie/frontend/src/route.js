/**
 * The address of the page: `<base>/<case>/<scenario>`, or `<base>/<case>`,
 * or `<base>` on the choice screen. The case is its id (the name of its file
 * under `casussen/`); a scenario has no id of its own, so its slug follows
 * from its `naam`. The app has no router: App.vue writes the address after
 * each choice and reads it on opening and on the browser's back and forward.
 * The query string (`?presentatie`) stays as it is.
 */

/** `Twee uitgevers, één antwoord` → `twee-uitgevers-een-antwoord`. */
export const scenarioSlug = (naam) =>
  String(naam)
    .normalize('NFD')
    .replace(/[̀-ͯ]/g, '')
    .toLowerCase()
    .replace(/[^a-z0-9]+/g, '-')
    .replace(/^-|-$/g, '');

/** The path of a case and scenario under `base`, which ends in `/`. */
export function routePath(base, casus = null, scenario = null) {
  if (!casus) return base;
  const delen = [casus, scenario && scenarioSlug(scenario)].filter(Boolean);
  return base + delen.map(encodeURIComponent).join('/');
}

/**
 * The case and scenario slug a path names, `{ casus, scenario }`, each null
 * when absent. A path outside `base`, or deeper than two parts, names
 * nothing the app knows: `{ casus: null, scenario: null, onbekend: true }`.
 */
export function parseRoute(pathname, base) {
  const leeg = { casus: null, scenario: null, onbekend: false };
  // `/chronolexografie` without its slash is the base too.
  if (`${pathname}/` === base) return leeg;
  if (!pathname.startsWith(base)) return { ...leeg, onbekend: true };
  const delen = pathname.slice(base.length).split('/').filter(Boolean).map(decodeURIComponent);
  if (delen.length > 2) return { ...leeg, onbekend: true };
  return { casus: delen[0] ?? null, scenario: delen[1] ?? null, onbekend: false };
}

/** The scenario of `casus` that `slug` names, or undefined. */
export const findScenario = (casus, slug) =>
  casus?.scenarios?.find((s) => scenarioSlug(s.naam) === slug);
