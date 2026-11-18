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
// once, at the end (`navigate`). Only the latest navigation writes: one the
// visitor overtook (back pressed while a case still loads) stops at its next
// step (`current`). A step already under way still lands: a case that was
// loading when the visitor went back opens anyway. That window is only open
// while the cell itself loads; after that, choosing a case does not wait.
let busy = 0;
let latest = 0;
/** Whether navigation `mine` is still the latest one. */
export const current = (mine) => mine === latest;

function write(replace = false) {
  if (busy || unknown.value) return;
  const path = routePath(BASE, state.casus?.id, state.scenario);
  if (path === window.location.pathname) return;
  const url = path + window.location.search + window.location.hash;
  if (replace) window.history.replaceState(window.history.state, '', url);
  else window.history.pushState(null, '', url);
}

/**
 * Run `fn` as one navigation: the address changes once, after it. `fn` gets
 * the navigation's number for `current`; when it returns false, or throws,
 * the address stays as it is, so a reload tries the same address again.
 */
export async function navigate(fn, { replace = false } = {}) {
  unknown.value = null;
  const mine = ++latest;
  busy++;
  let ok = false;
  try {
    ok = (await fn(mine)) !== false;
    return ok;
  } finally {
    busy--;
    if (ok && current(mine)) write(replace);
  }
}

/** Back to the choice screen, from the toolbar or the notice about an unknown address. */
export const toChoice = () => navigate(naarKeuze);

/**
 * Show the notice about an unknown address over an empty page. The saved
 * game of this tab stays (unlike `naarKeuze`): a mistyped link should not
 * cost a game; going on to the choice screen forgets it.
 */
function notFound(what) {
  state.casus = null;
  state.scenario = null;
  unknown.value = what;
  return true;
}

/**
 * Play what the address names; `started` opens a scenario that began.
 * Returns false when the cell refused a step or a later navigation took
 * over.
 */
async function follow(started, mine) {
  const route = parseRoute(window.location.pathname, BASE);
  if (route.unknown) return notFound({});
  if (!route.casus) {
    if (state.casus) naarKeuze();
    return true;
  }
  if (state.casus?.id !== route.casus) {
    const { casussen } = await api.casussen();
    if (!current(mine)) return false;
    if (!casussen.some((c) => c.id === route.casus)) return notFound({ casus: route.casus });
    if (!(await kiezen(route.casus)) || !current(mine)) return false;
  }
  if (!route.scenario) {
    state.scenario = null;
    return true;
  }
  const scenario = findScenario(state.casus, route.scenario);
  if (!scenario) return notFound({ casus: state.casus.titel, scenario: route.scenario });
  if (state.scenario === scenario.naam) return true;
  if (!(await beginScenario(scenario.naam)) || !current(mine)) return false;
  await started();
  return true;
}

/**
 * Start following the address: play every one the browser goes back or
 * forward to, and write a new one after each choice on the page. `first`
 * plays the address the page opened on too; App.vue leaves it out when the
 * saved game of this tab waits for the cell to load, so its banner stays and
 * the game is not overwritten.
 */
export async function followAddress(started, { first = true } = {}) {
  const play = () =>
    navigate((mine) => follow(started, mine), { replace: true }).catch((e) => {
      state.fout = e.message;
    });
  window.addEventListener('popstate', play);
  watch(
    () => [state.casus?.id, state.scenario],
    () => write(),
  );
  if (first) await play();
}
