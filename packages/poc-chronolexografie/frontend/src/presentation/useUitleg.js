/**
 * Whether the explanation page (UitlegPagina.vue) is open. Its address is
 * the page's own with `?uitleg`, beside `?presentatie` of the deck
 * (usePresentation.js): a query parameter leaves the paths of the cases and
 * scenarios (route.js) as they are. Opening adds the parameter, so the
 * address can be shared and back closes the page. Closing goes back when
 * opening added an entry to the history, so back after closing does not land
 * on the same page twice; otherwise it takes the parameter off.
 *
 * `createUitleg` makes one, for the tests; the app shares one through
 * `useUitleg`.
 */
import { ref } from 'vue';

/** The URL parameter that opens the page. */
export const PARAM = 'uitleg';

/** Whether a query string asks for the page. */
export const asked = (search) => new URLSearchParams(search).has(PARAM);

/** `href` with the parameter on or off. */
export function withParam(href, on) {
  const url = new URL(href);
  if (on) url.searchParams.set(PARAM, '');
  else url.searchParams.delete(PARAM);
  // `?uitleg=` reads as `?uitleg` in the address bar.
  return url.toString().replace(`${PARAM}=`, PARAM).replace(/\?$/, '');
}

export function createUitleg() {
  const active = ref(false);
  // Whether opening pushed an entry that closing should go back over.
  let pushed = false;

  function onKey(e) {
    if (active.value && e.key === 'Escape' && !e.altKey && !e.ctrlKey && !e.metaKey) {
      e.preventDefault();
      close();
    }
  }
  // Back and forward open and close it along with the address.
  const onPop = () => {
    active.value = asked(window.location.search);
    if (!active.value) pushed = false;
  };

  let listening = false;
  function listen() {
    if (listening || typeof window === 'undefined') return;
    window.addEventListener('keydown', onKey);
    window.addEventListener('popstate', onPop);
    listening = true;
  }

  /** Open the page; from a click it gets an address of its own. */
  function open({ push = true } = {}) {
    listen();
    active.value = true;
    if (push && typeof window !== 'undefined' && !asked(window.location.search)) {
      window.history.pushState(window.history.state, '', withParam(window.location.href, true));
      pushed = true;
    }
  }

  function close() {
    active.value = false;
    if (typeof window === 'undefined' || !asked(window.location.search)) return;
    if (pushed) {
      pushed = false;
      window.history.back();
    } else {
      window.history.replaceState(window.history.state, '', withParam(window.location.href, false));
    }
  }

  /** Open the page when the address asks for it, as on a shared link. */
  function follow() {
    listen();
    if (typeof window !== 'undefined' && asked(window.location.search)) open({ push: false });
  }

  return { active, open, close, follow, onKey };
}

let shared = null;

/** The explanation page of the app. */
export function useUitleg() {
  shared ??= createUitleg();
  return shared;
}
