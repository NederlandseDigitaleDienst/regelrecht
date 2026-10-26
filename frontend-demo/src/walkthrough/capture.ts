/**
 * What the recorder writes down while the presenter works: the actions the
 * player will do again in the live demo.
 *
 * Only trusted events (a person, not a script) and only outside the recorder
 * panel and the deck: slide changes are logged by the deck itself, as an
 * index, because "next slide" is what the player has to do and not "click
 * the arrow at x, y".
 *
 * - click: the element acted on (`locator.describe`) and where in it;
 * - input: the field and its value after every keystroke, so text appears in
 *   the replay at the pace it was typed;
 * - change: a select, checkbox or radio;
 * - key: Enter and Escape, which submit and close things;
 * - scroll: a scrolled element and its position, at most a few times a second;
 * - viewport: where the graph looks (graphBridge.ts), while it is dragged or
 *   zoomed, ten times a second and once more when it comes to rest.
 *
 * What is typed is recorded: the replay has to type it again. It is demo
 * input in a demo with fictitious personas, said aloud in the same take.
 */
import { actionTarget, describe, offsetIn, scrollAnchor, type ElementDescription, type ScrollAnchor } from './locator';
import { graphView, type GraphViewPoint } from './graphBridge';

/** An action as `captureActions` hands it to its `log`. */
export type CapturedAction =
  | { type: 'click'; target: ElementDescription; fx: number; fy: number }
  | { type: 'input'; target: ElementDescription; value: string }
  | { type: 'change'; target: ElementDescription; checked: boolean }
  | { type: 'change'; target: ElementDescription; value: string }
  | { type: 'key'; key: string; target: ElementDescription | null }
  | { type: 'scroll'; target: ElementDescription | null; top: number; left: number; max: number; anchor: ScrollAnchor | null }
  | ({ type: 'viewport' } & GraphViewPoint)
  // `ago`: the drag began that many ms before it was logged.
  | { type: 'node'; id: string; dx: number; dy: number; ms: number; ago: number };

type FormField = HTMLInputElement | HTMLTextAreaElement | HTMLSelectElement;

const IGNORE = '.recorder, .deck';
const KEYS = new Set(['Enter', 'Escape']);

function ignored(event: Event) {
  // The path also holds the document and the window, which have no `matches`.
  return (event.composedPath?.() ?? []).some((n) => (n as Partial<Element> | null)?.matches?.(IGNORE));
}

function field(event: Event): FormField | null {
  const el = event.composedPath?.()[0] as Node | undefined;
  return el?.nodeType === 1 && /^(INPUT|TEXTAREA|SELECT)$/.test((el as Element).tagName) ? (el as FormField) : null;
}

/**
 * Start listening; `log(event)` receives each action. Returns the function
 * that stops listening.
 */
export function captureActions(log: (action: CapturedAction) => void): () => void {
  const lastScroll = new WeakMap<Element, number>();

  function onClick(e: MouseEvent) {
    if (!e.isTrusted || ignored(e)) return;
    const el = actionTarget(e);
    if (!el) return;
    log({ type: 'click', target: describe(el), ...offsetIn(el, e.clientX, e.clientY) });
  }

  function onInput(e: Event) {
    if (!e.isTrusted || ignored(e)) return;
    const el = field(e);
    if (!el || el.type === 'checkbox' || el.type === 'radio') return;
    log({ type: 'input', target: describe(el), value: el.value });
  }

  function onChange(e: Event) {
    if (!e.isTrusted || ignored(e)) return;
    const el = field(e);
    if (!el) return;
    if (el.type === 'checkbox' || el.type === 'radio') log({ type: 'change', target: describe(el), checked: (el as HTMLInputElement).checked });
    else if (el.tagName === 'SELECT') log({ type: 'change', target: describe(el), value: el.value });
  }

  function onKey(e: KeyboardEvent) {
    if (!e.isTrusted || !KEYS.has(e.key) || ignored(e)) return;
    const el = e.composedPath?.()[0] as Node | undefined;
    log({ type: 'key', key: e.key, target: el?.nodeType === 1 && el !== document.body ? describe(el as Element) : null });
  }

  function logScroll(el: Element) {
    log({
      type: 'scroll',
      target: el === document.scrollingElement ? null : describe(el),
      top: Math.round(el.scrollTop),
      left: Math.round(el.scrollLeft),
      // How far it could scroll, and what was in the middle: the replay
      // finds the same place on a screen where the text wraps differently.
      max: Math.round(el.scrollHeight - el.clientHeight),
      anchor: scrollAnchor(el),
    });
  }

  // Throttled, with the resting position always logged: a fling ends where
  // the presenter stopped, not at the last sample before it.
  const restScroll = new Map<Element, ReturnType<typeof setTimeout>>();
  function onScroll(e: Event) {
    if (!e.isTrusted) return;
    // A scroll event's target is the document or the element that scrolled.
    const el = (e.target === document ? document.scrollingElement : e.target) as Element | null;
    if (!el || el.nodeType !== 1 || el.closest?.(IGNORE)) return;
    clearTimeout(restScroll.get(el));
    restScroll.set(
      el,
      setTimeout(() => {
        lastScroll.set(el, performance.now());
        logScroll(el);
      }, 200),
    );
    const now = performance.now();
    if (now - (lastScroll.get(el) ?? 0) < 150) return;
    lastScroll.set(el, now);
    logScroll(el);
  }

  // A scroll event does not leave a shadow root, so the listener on the
  // document never hears a design-system pane scroll (the law's YAML, a
  // sheet). Whatever the presenter is about to scroll (wheel, a key, a
  // scrollbar) is on the composed path of that gesture: listen there too.
  const watched = new Set<Element>();
  function watchScrollers(e: Event) {
    if (!e.isTrusted) return;
    // Read as an element only past the nodeType check.
    for (const n of (e.composedPath?.() ?? []) as Element[]) {
      if (n?.nodeType !== 1 || watched.has(n) || n.getRootNode() === document) continue;
      if (n.scrollHeight > n.clientHeight + 1 || n.scrollWidth > n.clientWidth + 1) {
        watched.add(n);
        n.addEventListener('scroll', onScroll);
      }
    }
  }

  // The graph's camera: throttled, with the resting position always logged,
  // so a replay ends exactly where the presenter stopped.
  let lastView = 0;
  let pending: ReturnType<typeof setTimeout> | undefined;
  graphView.onView = (view) => {
    clearTimeout(pending);
    const now = performance.now();
    if (now - lastView >= 100) {
      lastView = now;
      log({ type: 'viewport', ...view });
    } else {
      pending = setTimeout(() => {
        lastView = performance.now();
        log({ type: 'viewport', ...view });
      }, 150);
    }
  };

  // A law dragged aside: logged at the moment the drag began, so the replay
  // moves it while the voice talks about it and not after.
  graphView.onNodeMove = ({ ms, ...move }) => log({ type: 'node', ...move, ms, ago: ms });

  document.addEventListener('click', onClick, true);
  document.addEventListener('input', onInput, true);
  document.addEventListener('change', onChange, true);
  document.addEventListener('keydown', onKey, true);
  document.addEventListener('scroll', onScroll, true);
  for (const type of ['wheel', 'keydown', 'pointerdown', 'touchstart']) document.addEventListener(type, watchScrollers, { capture: true, passive: true });
  return () => {
    for (const type of ['wheel', 'keydown', 'pointerdown', 'touchstart']) document.removeEventListener(type, watchScrollers, { capture: true });
    for (const n of watched) n.removeEventListener('scroll', onScroll);
    clearTimeout(pending);
    for (const timer of restScroll.values()) clearTimeout(timer);
    graphView.onView = null;
    graphView.onNodeMove = null;
    document.removeEventListener('click', onClick, true);
    document.removeEventListener('input', onInput, true);
    document.removeEventListener('change', onChange, true);
    document.removeEventListener('keydown', onKey, true);
    document.removeEventListener('scroll', onScroll, true);
  };
}
