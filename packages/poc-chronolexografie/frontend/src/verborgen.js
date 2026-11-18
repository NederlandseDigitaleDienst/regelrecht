/**
 * Hidden sources: a source with `verborgen: true` in pocs/registry.yaml
 * (public/bronnen.json, scripts/copy-assets.js) keeps its cases off the
 * choice screen, and its notices too, so a visitor sees neither the source
 * nor an error about it. The address of such a case still opens it
 * (navigation.js asks the cell, not this list).
 *
 * This is visibility, not access control: anyone past the password can
 * fetch `bron/<naam>` directly.
 *
 * `?alle-casussen` shows the hidden cases in this browser and remembers
 * that; `?alle-casussen=uit` stops it. The parameter is taken off the
 * address once read, with the helper of the choice screen's pages
 * (presentation/useUitleg.js).
 *
 * `createAlleCasussen` makes one, for the tests; the app shares one through
 * `useAlleCasussen`.
 */
import { ref } from 'vue';
import { withParam } from './presentation/useUitleg.js';

/** The URL parameter that switches the hidden cases on or off. */
export const PARAM = 'alle-casussen';

/** The `localStorage` key that remembers the choice. */
export const SLEUTEL = 'chronolexografie.alle-casussen';

/** What a query string asks for: true, false (`=uit`), or null when it is silent. */
export function gevraagd(search) {
  const params = new URLSearchParams(search);
  if (!params.has(PARAM)) return null;
  return params.get(PARAM) !== 'uit';
}

/** The names of the hidden sources in a list from `leesBronLijst` (bronnen.js). */
export const verborgenNamen = (lijst = []) => lijst.filter((b) => b.verborgen).map((b) => b.naam);

/** The cases that show: all of them, or those not from a hidden source. */
export const zichtbareCasussen = (casussen = [], verborgen = [], alles = false) =>
  alles ? casussen : casussen.filter((c) => !verborgen.includes(c.bron));

/** The source states (`casussen().bronnen`) that show, by the same rule. */
export const zichtbareBronnen = (bronnen = [], verborgen = [], alles = false) =>
  alles ? bronnen : bronnen.filter((b) => !verborgen.includes(b.naam));

/**
 * The window's `localStorage`, or null. Reading the property alone can throw
 * (blocked site data), so every access goes through here and through a
 * try/catch: without storage the choice lasts as long as the page.
 */
function opslag(win) {
  try {
    return win?.localStorage ?? null;
  } catch {
    return null;
  }
}

function lees(win) {
  try {
    return opslag(win)?.getItem(SLEUTEL) === 'ja';
  } catch {
    return false;
  }
}

function bewaar(win, aan) {
  try {
    const s = opslag(win);
    if (aan) s?.setItem(SLEUTEL, 'ja');
    else s?.removeItem(SLEUTEL);
  } catch {
    // Not remembered; it still holds for this page.
  }
}

export function createAlleCasussen(win = typeof window === 'undefined' ? undefined : window) {
  const alles = ref(lees(win));

  /**
   * Read `?alle-casussen` from the address the page opened on: remember the
   * choice and take the parameter off, so the address that gets shared
   * does not carry it.
   */
  function follow() {
    if (!win) return;
    const aan = gevraagd(win.location.search);
    if (aan === null) return;
    alles.value = aan;
    bewaar(win, aan);
    win.history.replaceState(win.history.state, '', withParam(win.location.href, false, PARAM));
  }

  return { alles, follow };
}

let shared = null;

/** Whether this browser shows the hidden cases. */
export function useAlleCasussen() {
  shared ??= createAlleCasussen();
  return shared;
}
