/**
 * The address follows the game, and the game the address (route.js). A
 * choice on the page (a case, a scenario, back to the choice screen) changes
 * `state`, and a watcher pushes the new address, so the browser's back and
 * forward walk the choices. Opening the page and going back or forward read
 * the address and play it: choose the case it names and begin its scenario,
 * unless the game of this tab (state.js) is already there.
 *
 * Going forward to a scenario begins it again, with the chronicle back on
 * what the cell records before the first scenario: the game is not kept per
 * address. Back from a scenario to its case keeps the chronicle and leaves
 * free play.
 */
import { ref, watch } from 'vue';
import { api } from './api.js';
import { findScenario, parseRoute, routePath } from './route.js';
import { beginScenario, kiezen, naarKeuze, state } from './state.js';

const BASE = import.meta.env.BASE_URL;

/**
 * What the address named that the app does not have, for the notice in
 * App.vue: `{ casus }`, `{ casus, scenario }` or `{}` for a path that names
 * nothing; null otherwise.
 */
export const unknown = ref(null);

// While a navigation runs, its steps leave the address alone; it is written
// once, at the end (`navigate`).
let busy = 0;

function write(replace = false) {
  if (busy || unknown.value) return;
  const path = routePath(BASE, state.casus?.id, state.scenario);
  if (path === window.location.pathname) return;
  const url = path + window.location.search + window.location.hash;
  if (replace) window.history.replaceState(window.history.state, '', url);
  else window.history.pushState(null, '', url);
}

/** Run `fn` as one navigation: the address changes once, after it. */
export async function navigate(fn, { replace = false } = {}) {
  unknown.value = null;
  busy++;
  try {
    return await fn();
  } finally {
    busy--;
    write(replace);
  }
}

/** Back to the choice screen, from the notice about an unknown address. */
export const toChoice = () => navigate(naarKeuze);

/** Play what the address names; `started` opens a scenario that began. */
async function follow(started) {
  const route = parseRoute(window.location.pathname, BASE);
  if (route.onbekend) {
    naarKeuze();
    unknown.value = {};
    return;
  }
  if (!route.casus) {
    if (state.casus) naarKeuze();
    return;
  }
  if (state.casus?.id !== route.casus) {
    const { casussen } = await api.casussen();
    if (!casussen.some((c) => c.id === route.casus)) {
      naarKeuze();
      unknown.value = { casus: route.casus };
      return;
    }
    if (!(await kiezen(route.casus))) return;
  }
  if (!route.scenario) {
    state.scenario = null;
    return;
  }
  const scenario = findScenario(state.casus, route.scenario);
  if (!scenario) {
    const casus = state.casus.titel;
    naarKeuze();
    unknown.value = { casus, scenario: route.scenario };
    return;
  }
  if (state.scenario !== scenario.naam && (await beginScenario(scenario.naam))) await started();
}

/**
 * Start following the address: play the one the page opened on, then every
 * one the browser goes back or forward to, and write a new one after each
 * choice on the page.
 */
export async function followAddress(started) {
  const play = () =>
    navigate(() => follow(started), { replace: true }).catch((e) => {
      state.fout = e.message;
    });
  window.addEventListener('popstate', play);
  watch(
    () => [state.casus?.id, state.scenario],
    () => write(),
  );
  await play();
}
