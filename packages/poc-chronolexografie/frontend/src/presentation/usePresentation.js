/**
 * The introductory deck: which slide shows, and the keys that move it. The
 * same shape as the deck of the demo (frontend-demo/src/presentation/), cut
 * down to a deck that only covers the screen: there is no rail beside the
 * app, and no slide opens a tab. The closing slide starts a scenario
 * instead (PresentationDeck.vue).
 *
 * `createPresentation` makes one deck, for the tests; the app shares one
 * through `usePresentation`.
 */
import { computed, ref } from 'vue';
import { slides as SLIDES } from './slides.js';

/** The URL parameter that opens the deck: `?presentatie`, or `?presentatie=5` at slide 5. */
export const PARAM = 'presentatie';

/**
 * The slide to open from a query string, as an index, or null when the
 * parameter is absent. A number counts from 1, as the counter does; anything
 * else starts at the first slide.
 */
export function startIndex(search, total) {
  const params = new URLSearchParams(search);
  if (!params.has(PARAM)) return null;
  const n = Number.parseInt(params.get(PARAM), 10);
  if (!Number.isFinite(n) || n < 1) return 0;
  return Math.min(n, total) - 1;
}

/**
 * Where a key event came from. The fields of the design system put their
 * `<input>` in a shadow root, so the event's target is the host and only
 * `composedPath()` holds the input (the demo deck found this the hard way).
 */
function pathMatches(e, selector) {
  const path = e.composedPath?.();
  if (Array.isArray(path) && path.length) return path.some((n) => n?.matches?.(selector));
  return !!e.target?.closest?.(selector);
}

/**
 * What a key does on the deck: 'next', 'prev', 'first', 'last', 'stop',
 * 'fullscreen', or null. Typing in a field does nothing, a key with a
 * modifier is the browser's, and Space on a button presses the button
 * rather than turning the page.
 */
export function keyAction(e) {
  // Alt+← is the browser's back, Ctrl+F its search: those stay the browser's.
  if (e.altKey || e.ctrlKey || e.metaKey) return null;
  if (pathMatches(e, 'input, textarea, select, [contenteditable]')) return null;
  // An open dialog or popover takes its own Escape.
  if (e.key === 'Escape' && pathMatches(e, 'dialog[open], [popover]')) return null;
  if (e.key === ' ' && pathMatches(e, 'button, [role="button"], a[href], nldd-button, nldd-icon-button'))
    return null;
  switch (e.key) {
    case 'ArrowRight':
    case 'PageDown':
    case ' ':
      return 'next';
    case 'ArrowLeft':
    case 'PageUp':
      return 'prev';
    case 'Home':
      return 'first';
    case 'End':
      return 'last';
    case 'Escape':
      return 'stop';
    case 'f':
    case 'F':
      return 'fullscreen';
    default:
      return null;
  }
}

export function createPresentation(slides = SLIDES) {
  const active = ref(false);
  const index = ref(0);
  const total = computed(() => slides.length);
  const current = computed(() => slides[index.value] ?? null);
  const isLast = computed(() => index.value === slides.length - 1);
  let listening = false;

  const goTo = (i) => {
    index.value = Math.max(0, Math.min(slides.length - 1, i));
  };
  const next = () => {
    if (!isLast.value) goTo(index.value + 1);
  };
  const prev = () => goTo(index.value - 1);

  function onKey(e) {
    if (!active.value) return;
    const action = keyAction(e);
    if (!action) return;
    if (action !== 'fullscreen') e.preventDefault();
    if (action === 'next') next();
    else if (action === 'prev') prev();
    else if (action === 'first') goTo(0);
    else if (action === 'last') goTo(slides.length - 1);
    else if (action === 'stop') stop();
    else if (document.fullscreenElement) document.exitFullscreen?.();
    else document.documentElement.requestFullscreen?.();
  }

  /** Open the deck at slide `i`; `keys: false` leaves the keyboard alone. */
  function start(i = 0, { keys = true } = {}) {
    goTo(i);
    active.value = true;
    if (keys && !listening && typeof window !== 'undefined') {
      window.addEventListener('keydown', onKey);
      listening = true;
    }
  }

  function stop() {
    active.value = false;
    if (listening) {
      window.removeEventListener('keydown', onKey);
      listening = false;
    }
    if (typeof window === 'undefined') return;
    if (document.fullscreenElement) document.exitFullscreen?.();
    // A reload after closing shows the app, not the deck again.
    const url = new URL(window.location.href);
    if (url.searchParams.has(PARAM)) {
      url.searchParams.delete(PARAM);
      window.history.replaceState(window.history.state, '', url);
    }
  }

  return { active, index, total, current, isLast, start, stop, next, prev, goTo, onKey };
}

let shared = null;

/** The deck of the app. */
export function usePresentation() {
  shared ??= createPresentation();
  return shared;
}
