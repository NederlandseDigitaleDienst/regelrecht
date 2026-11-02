/**
 * Whether a page of the choice screen shows instead of it: the explanation
 * page (UitlegPagina.vue) here, the short page (KortPagina.vue, useKort.js)
 * with the same code under its own parameter. Its address is
 * the page's own with `?uitleg`, beside `?presentatie` of the deck
 * (usePresentation.js): a query parameter leaves the paths of the cases and
 * scenarios (route.js) as they are. Opening adds the parameter, so the
 * address can be shared and back closes the page. Closing goes back when
 * opening added the entry, so back after closing does not land on the same
 * page twice; otherwise it takes the parameter off. The page shows only on
 * the choice screen (App.vue).
 *
 * `createUitleg` makes one, for the tests; the app shares one through
 * `useUitleg`. `createPagina` makes one for another parameter.
 */
import { ref } from 'vue';

/** The URL parameter that opens the page. */
export const PARAM = 'uitleg';

/** Whether a query string asks for the page. */
export const asked = (search, param = PARAM) => new URLSearchParams(search).has(param);

/** `href` with the parameter on or off. */
export function withParam(href, on, param = PARAM) {
  const url = new URL(href);
  if (on) url.searchParams.set(param, '');
  else url.searchParams.delete(param);
  // `?uitleg=` reads as `?uitleg` in the address bar.
  return url.toString().replace(new RegExp(`([?&]${param})=(?=&|$)`), '$1').replace(/\?$/, '');
}

export function createPagina(param) {
  const asked_ = (search) => asked(search, param);
  const withParam_ = (href, on) => withParam(href, on, param);
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
    active.value = asked_(window.location.search);
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
   * marked as the page's (its parameter in its state), so closing can go back
   * over it, also after back and forward or a reload.
   *
   * `vervangt` names the parameter of another page of the choice screen that
   * this one takes the place of, on that page's entry: back then skips both,
   * and closing goes back only when that page had pushed its entry.
   */
  function open({ push = true, vervangt = null } = {}) {
    listen();
    active.value = true;
    geklikt = push;
    if (!push || typeof window === 'undefined') return;
    if (vervangt) {
      const { [vervangt]: pushed, ...rest } = window.history.state ?? {};
      const href = withParam_(withParam(window.location.href, false, vervangt), true);
      window.history.replaceState({ ...rest, [param]: pushed || undefined }, '', href);
    } else if (!asked_(window.location.search)) {
      window.history.pushState({ ...window.history.state, [param]: true }, '', withParam_(window.location.href, true));
    }
  }

  function close() {
    active.value = false;
    terug.value = true;
    if (typeof window === 'undefined' || !asked_(window.location.search)) return;
    if (window.history.state?.[param]) window.history.back();
    else window.history.replaceState(window.history.state, '', withParam_(window.location.href, false));
  }

  /**
   * Leave the page without going back, on an address that names a case: the
   * parameter goes, so the choice screen does not come back as this page.
   */
  function verlaat() {
    active.value = false;
    if (typeof window !== 'undefined' && asked_(window.location.search)) {
      window.history.replaceState(window.history.state, '', withParam_(window.location.href, false));
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
    if (typeof window === 'undefined' || !asked_(window.location.search)) return;
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

/** The explanation page, for the tests. */
export const createUitleg = () => createPagina(PARAM);

let shared = null;

/** The explanation page of the app. */
export function useUitleg() {
  shared ??= createUitleg();
  return shared;
}
