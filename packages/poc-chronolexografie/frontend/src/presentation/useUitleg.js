/**
 * Whether the explanation page (UitlegPagina.vue) shows instead of the
 * choice screen. Its address is
 * the page's own with `?uitleg`, beside `?presentatie` of the deck
 * (usePresentation.js): a query parameter leaves the paths of the cases and
 * scenarios (route.js) as they are. Opening adds the parameter, so the
 * address can be shared and back closes the page. Closing goes back when
 * opening added the entry, so back after closing does not land on the same
 * page twice; otherwise it takes the parameter off. The page shows only on
 * the choice screen (App.vue).
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
  // Whether the choice screen should take the focus back when it returns:
  // its button that opened the page is gone while the page shows.
  const terug = ref(false);
  // Whether the page opened from a click, so it takes the focus; a page
  // opened from a link or a reload starts at the top as any page.
  let geklikt = false;

  // Back and forward open and close it along with the address.
  const onPop = () => {
    const was = active.value;
    active.value = asked(window.location.search);
    if (was && !active.value) terug.value = true;
  };

  let listening = false;
  function listen() {
    if (listening || typeof window === 'undefined') return;
    window.addEventListener('popstate', onPop);
    listening = true;
  }

  /**
   * Open the page; from a click it gets an address of its own, an entry
   * marked as the page's (`uitleg` in its state), so closing can go back over
   * it, also after back and forward or a reload.
   */
  function open({ push = true } = {}) {
    listen();
    active.value = true;
    geklikt = push;
    if (push && typeof window !== 'undefined' && !asked(window.location.search)) {
      window.history.pushState({ ...window.history.state, uitleg: true }, '', withParam(window.location.href, true));
    }
  }

  function close() {
    active.value = false;
    terug.value = true;
    if (typeof window === 'undefined' || !asked(window.location.search)) return;
    if (window.history.state?.uitleg) window.history.back();
    else window.history.replaceState(window.history.state, '', withParam(window.location.href, false));
  }

  /**
   * Leave the page without going back, on an address that names a case: the
   * parameter goes, so the choice screen does not come back as this page.
   */
  function verlaat() {
    active.value = false;
    if (typeof window !== 'undefined' && asked(window.location.search)) {
      window.history.replaceState(window.history.state, '', withParam(window.location.href, false));
    }
  }

  /** Whether the page should take the focus: once, after a click. */
  function neemFocus() {
    const ja = geklikt;
    geklikt = false;
    return ja;
  }

  /**
   * Open the page when the address asks for it, as on a shared link. An
   * address that also names a case (`opCasus`, `/<casus>?uitleg`) shows the
   * case: the page belongs to the choice screen, so the parameter goes.
   */
  function follow({ opCasus = false } = {}) {
    listen();
    if (typeof window === 'undefined' || !asked(window.location.search)) return;
    if (opCasus) verlaat();
    else open({ push: false });
  }

  /**
   * Leave the page for the case `naar` opens, which returns once the case is
   * chosen and its address written (navigation.js). The page's own entry
   * stays in the history, so back returns to it; the case's entry loses the
   * parameter afterwards.
   */
  async function naarCasus(naar) {
    active.value = false;
    await naar();
    verlaat();
  }

  return { active, terug, open, close, verlaat, naarCasus, neemFocus, follow, onPop };
}

let shared = null;

/** The explanation page of the app. */
export function useUitleg() {
  shared ??= createUitleg();
  return shared;
}
