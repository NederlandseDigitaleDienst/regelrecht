/**
 * TEMPORARY workaround: a sheet on the left opens next to the deck rail, not
 * over it.
 *
 * Two design-system components open a sheet against the left edge of the
 * window, from inside their shadow root: `nldd-sheet` with
 * `placement="left"`, and `nldd-navigation-split-view` for its primary
 * sidebar when that is a sheet (the law list on the Wetten tab). Neither
 * offers a hook for that edge; the one variable they read
 * (`--semantics-overlays-inset`) moves all four sides. With the deck as a
 * rail on the left (zelfstandig mode, and the recorded walkthrough) the law
 * list therefore opened over the slides instead of in the demo.
 *
 * This adds one rule to each such component's shadow root, reading a
 * variable the page sets while the rail is on screen (`--rr-sheet-offset` in
 * presentation.css). The rules depend on the components' internal class
 * names, so a design-system update can break them silently;
 * `sheetOffset.test.ts` fails when a class is gone. Remove this once the
 * design system offers an inline-start offset for sheets (requested).
 */

import type { Router } from 'vue-router';

const SHIFT = `inset-inline-start: calc(var(--rr-sheet-offset, 0px) + var(--semantics-overlays-inset)) !important;
  max-width: calc(100vw - var(--rr-sheet-offset, 0px) - var(--semantics-overlays-inset) * 2);`;

/*
 * The component slides a left sheet in over its own full width, so moved next
 * to the rail it would still start at the window edge and travel across the
 * slides. These keyframes read two variables: without the rail they are the
 * component's own slide (100%, no fade); with it (presentation.css) a short
 * slide and a fade that start at the edge of the demo. Only above 1024px:
 * below that the deck has no rail and the sheet is a bottom sheet.
 */
const KEYFRAMES = `@keyframes rr-sheet-in {
  from { transform: translateX(calc(-1 * var(--rr-sheet-slide, 100%))); opacity: var(--rr-sheet-fade, 1); }
  to { transform: translateX(0); opacity: 1; }
}
@keyframes rr-sheet-out {
  from { transform: translateX(0); opacity: 1; }
  to { transform: translateX(calc(-1 * var(--rr-sheet-slide, 100%))); opacity: var(--rr-sheet-fade, 1); }
}`;

function rule(selector: string): string {
  return `${selector} { ${SHIFT} }
${KEYFRAMES}
@media (min-width: 1025px) {
  ${selector}[open] { animation-name: rr-sheet-in !important; }
  ${selector}.is-closing { animation-name: rr-sheet-out !important; }
}`;
}

/** Per component, the rule that moves its left sheet. */
export const RULES: Readonly<Record<string, string>> = {
  'nldd-sheet': rule(':host([placement="left"]) .sheet'),
  'nldd-navigation-split-view': rule('.navigation-split-view__primary-sidebar-sheet'),
};

export function addOffsetRule(host: Element | null | undefined, rule = host ? RULES[host.localName] : undefined): void {
  const root = host?.shadowRoot;
  if (!root || !rule || root.querySelector('style[data-rr-sheet-offset]')) return;
  const style = document.createElement('style');
  style.setAttribute('data-rr-sheet-offset', '');
  style.textContent = rule;
  root.appendChild(style);
}

/** Every component in `root` and its shadow roots that has a rule, with the rule added. */
export function addOffsetRules(root: Document | ShadowRoot = document): void {
  for (const el of root.querySelectorAll('*')) {
    if (RULES[el.localName]) addOffsetRule(el);
    if (el.shadowRoot) addOffsetRules(el.shadowRoot);
  }
}

/**
 * Keep the rules in place. Not by patching the components: the browser takes
 * a custom element's lifecycle callbacks when it is defined, so a later patch
 * of its prototype is never called. Instead the page is walked just before a
 * click is handled (capture phase: the sheet opens in the click's own
 * handler, so the rule is there before it does) and after every navigation,
 * which is when new tabs and their components appear.
 */
export function installSheetOffset(router?: Pick<Router, 'afterEach'> | null): () => void {
  if (typeof document === 'undefined') return () => {};
  const scan = () => addOffsetRules(document);
  document.addEventListener('pointerdown', scan, true);
  document.addEventListener('click', scan, true);
  const offRoute = router?.afterEach?.(() => setTimeout(scan, 0));
  setTimeout(scan, 0);
  return () => {
    document.removeEventListener('pointerdown', scan, true);
    document.removeEventListener('click', scan, true);
    offRoute?.();
  };
}
