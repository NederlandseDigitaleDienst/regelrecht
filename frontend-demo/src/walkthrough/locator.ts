/**
 * Finding an element again: the part of the replay that has to survive a
 * different window.
 *
 * A recorded click is not a coordinate. The viewer's window is another size,
 * the tab bar may have collapsed into its overflow menu, and the design
 * system's components keep their inner buttons and inputs in shadow roots.
 * So the recorder describes the element it hit: per scope (the document, then
 * each shadow root on the way down) the element that leads to the target,
 * by what identifies it to a person (its text, its label, its link) and not by
 * its place in the DOM, which is what changes between layouts.
 *
 * Properties, not attributes: Vue sets `:text`, `:href` and the like on a
 * custom element as a property when the element defines one, so the
 * attribute is often absent (measured on nldd-search-field's placeholder).
 *
 * A recording is made in one language and may play in another. The texts it
 * stores are then compared through a translator (translateText.ts), which
 * takes each one back to the translation key it came from: the element is
 * identified by what it says, in whatever language it says it.
 */
import { matchesTranslated, textMatchesTranslated, type Translator } from './translateText';

/**
 * What identifies an element within its own scope: its `tag`, and per
 * property (`text`, `href`, ...) or attribute (`@aria-label`, `@data-…`) the
 * value it had, as a string. `loosen` marks the signature it made with
 * `__masked`. Recordings store these as JSON.
 */
export interface Signature {
  tag: string;
  __masked?: true;
  textContent?: string;
  [key: string]: string | number | true | undefined;
}

/** One step of a description: a signature and its place (`nth`) among the elements that fit it in its scope. */
export interface DescriptionStep extends Signature {
  nth?: number;
}

/** What `describe` makes of an element and `resolve` finds again: one step per scope, from the document down. */
export type ElementDescription = DescriptionStep[];

export interface ResolveOptions {
  loose?: boolean;
  translate?: Translator | null;
}

/** The element in the middle of a scroller, as `scrollAnchor` records it. */
export interface ScrollAnchor {
  tag: string;
  text: string;
  index: number;
  /** Where it sat in the visible box, as a fraction of its height. */
  at: number;
}

/** The parts of a recorded scroll event `scrollTarget` reads. */
export interface RecordedScroll {
  top?: number;
  max?: number;
  anchor?: ScrollAnchor | null;
}

/** The visible box of a scroller. */
export type ViewBox = Pick<DOMRect, 'top' | 'left' | 'width' | 'height'>;

/** Whether `n` (anything on an event path) is an element. */
function isElement(n: unknown): n is Element {
  return (n as Node | null)?.nodeType === 1;
}

/** What counts as the thing that was clicked, walking up from the target. */
const ACTIONABLE = [
  'button',
  'a[href]',
  'input',
  'textarea',
  'select',
  'summary',
  'label',
  '[role="button"]',
  '[role="tab"]',
  '[role="menuitem"]',
  '[role="menuitemradio"]',
  '[role="menuitemcheckbox"]',
  '[role="option"]',
  '[role="treeitem"]',
  '[role="link"]',
  '[role="checkbox"]',
  '[role="switch"]',
  '[role="radio"]',
].join(',');

/** Properties that name an element, in order of how much they say. */
const PROPS = ['id', 'text', 'accessibleLabel', 'label', 'href', 'name', 'placeholder', 'value', 'icon', 'startIcon', 'type'];
const ATTRS = ['aria-label', 'title', 'role', 'slot'];

/**
 * An id or name a component made up on render (`nldd-field-input-3ec46f7f-…`,
 * `nldd-segmented-2`): it differs per load or per version of the design
 * system, so it names nothing a replay can find again.
 */
export function generatedId(id: unknown) {
  return /[0-9a-f]{8}-[0-9a-f]{4}-|\d{5,}|^nldd-[a-z-]+-\d+$/i.test(String(id ?? ''));
}

/** Properties whose value can be made up by the component that renders it. */
const MADE_UP = new Set(['id', 'name']);

/**
 * A data attribute that says nothing lasting about an element: the state it
 * happens to be in (`data-focused` on the menu item under the pointer), or an
 * id the app made up at run time (`data-run="run-mv3t513c"`, a case id). A
 * made-up id shows as a part of six characters or more that mixes letters and
 * digits; a name like `vergelijking` or `complete-data` does not.
 */
const STATE_DATA = new Set(['data-focused', 'data-highlighted', 'data-hovered', 'data-pressed', 'data-active']);
export function passingData(attr: string, value: unknown) {
  if (STATE_DATA.has(attr)) return true;
  return String(value ?? '')
    .split(/[-_:]/)
    .some((part) => part.length >= 6 && /\d/.test(part) && /[a-z]/i.test(part));
}

/**
 * Text that names a whole page rather than one control: anything this long
 * is a container's content, and it changes whenever the content does (a law
 * added to the corpus). A loose match drops it and goes by the element.
 */
const CONTAINER_TEXT = 60;

function scopeOf(el: Element): Node & ParentNode {
  const root = el.getRootNode?.();
  // The root of an element is a document, a shadow root or a detached element: a parent node.
  return root && root !== el ? (root as Node & ParentNode) : document;
}

function norm(s: unknown) {
  return String(s ?? '').replace(/\s+/g, ' ').trim();
}

function textOf(el: Node) {
  return norm(el.textContent).slice(0, 80);
}

function isFormField(el: Element) {
  return /^(INPUT|TEXTAREA|SELECT)$/.test(el.tagName);
}

/**
 * A link without its origin: recorded on localhost, replayed on the demo's
 * own domain, the path is what both share.
 */
function sameOriginPath(href: string) {
  try {
    const u = new URL(href, window.location.href);
    return u.origin === window.location.origin ? `${u.pathname}${u.search}${u.hash}` : u.href;
  } catch {
    return href;
  }
}

/** The value of `key` on `el`, normalised the way signatures store it. */
function read(el: Element, key: string): string | null {
  if (key === 'textContent') return textOf(el);
  const raw = key.startsWith('@') ? el.getAttribute?.(key.slice(1)) : (el as unknown as Record<string, unknown>)[key];
  if (typeof raw !== 'string' || !raw) return null;
  return key === 'href' ? sameOriginPath(raw) : norm(raw);
}

/**
 * Properties the design system renamed. A take recorded before the rename
 * names the element by the old property, which the element no longer has:
 * nldd-toolbar's `label` became `accessibleLabel` in 0.8.100. Read the new
 * one instead, only when the old one is absent, so an element that still
 * carries the old property is judged by it.
 */
const RENAMED: Readonly<Record<string, string>> = { label: 'accessibleLabel' };

function readRecorded(el: Element, key: string) {
  const have = read(el, key);
  const renamed = RENAMED[key];
  return have == null && renamed ? read(el, renamed) : have;
}

/** What identifies `el` within its own scope. */
export function signature(el: Element): Signature {
  const sig: Signature = { tag: el.tagName.toLowerCase() };
  for (const p of PROPS) {
    // A field's value is what the user typed, not who the field is.
    if (p === 'value' && isFormField(el)) continue;
    const v = read(el, p);
    if (MADE_UP.has(p) && generatedId(v)) continue;
    if (v && v.length <= 200) sig[p] = v;
  }
  for (const a of ATTRS) {
    const v = read(el, `@${a}`);
    if (v) sig[`@${a}`] = v;
  }
  for (const a of el.getAttributeNames?.() ?? []) {
    // A name from getAttributeNames: the attribute is there.
    if (a.startsWith('data-') && !a.startsWith('data-v-') && !passingData(a, el.getAttribute(a))) sig[`@${a}`] = el.getAttribute(a)!;
  }
  // Text only when nothing else names it: a list item with a law's name, a
  // plain button. Long or empty text says little.
  const named = Object.keys(sig).some((k) => k !== 'tag' && k !== '@role' && k !== '@slot' && k !== 'type');
  if (!named) {
    const text = textOf(el);
    if (text) sig.textContent = text;
  }
  return sig;
}

/** What a person reads on an element, as opposed to what identifies it in code. */
const WORDS = new Set(['text', 'accessibleLabel', 'label', 'placeholder', '@aria-label', '@title', 'textContent']);

/** The text nodes under `el`, in order: what its text content is made of. */
function textPieces(el: Node) {
  const out: string[] = [];
  const walk = document.createTreeWalker(el, NodeFilter.SHOW_TEXT);
  // SHOW_TEXT: every node the walker yields is a Text node.
  for (let n = walk.nextNode(); n; n = walk.nextNode()) out.push((n as Text).data);
  return out;
}

/**
 * Whether `have` on `el` is the recorded `v`, in the language the demo runs
 * in now. Without a translator, the same text; with one, the same text in
 * the other language (a link: the same page in the other language).
 */
function sameValue(el: Element, k: string, have: string | null, v: string, masked: boolean | undefined, translate: Translator | null | undefined) {
  const shown = masked && have != null ? MASK(have) : have;
  if (shown === v) return true;
  if (!translate || have == null) return false;
  if (k === 'href') return translate.path?.(v) === have;
  if (!WORDS.has(k)) return false;
  if (masked) return translate(v).some((c) => MASK(c) === shown);
  if (k === 'textContent') return textMatchesTranslated(textPieces(el), v, translate);
  return matchesTranslated(have, v, translate);
}

/**
 * A field the recording names by its label or placeholder. Its `value` is
 * what was typed into it so far (a design-system field carries it on its
 * host), and in another language that is the translation being typed.
 */
const LABELLED = ['accessibleLabel', 'label', 'placeholder', '@aria-label'];

function matches(el: Element, sig: Signature, translate: Translator | null = null) {
  if (el.tagName.toLowerCase() !== sig.tag) return false;
  const typedInto = !!translate && LABELLED.some((k) => k in sig);
  for (const [k, v] of Object.entries(sig)) {
    if (k === 'tag' || k === '__masked') continue;
    if (k === 'value' && typedInto) continue;
    // Takes recorded before generated ids were left out still carry them.
    if (MADE_UP.has(k) && generatedId(v)) continue;
    if (k.startsWith('@data-') && passingData(k.slice(1), v)) continue;
    // Every key but `tag` and `__masked` holds a recorded string.
    if (!sameValue(el, k, readRecorded(el, k), v as string, sig.__masked, translate)) return false;
  }
  return true;
}

function visible(el: Element) {
  const r = el.getBoundingClientRect?.();
  return !!r && r.width > 0 && r.height > 0;
}

const MASK = (v: unknown) => String(v).replace(/\d+/g, '#');

/**
 * The same signature with the numbers masked: a count in a label ("Wetten,
 * 79") is what drifts between the recording and the corpus of the day, and
 * the words around it are what still names the element. Null when nothing is
 * left to mask, or when the signature says no more than its tag: matching
 * every button is not a looser match but a wrong one.
 */
export function loosen(sig: Signature): Signature | null {
  const out: Signature = { tag: sig.tag, __masked: true };
  let changed = false;
  let named = false;
  for (const [k, v] of Object.entries(sig)) {
    if (k === 'tag' || k === 'nth' || k === '__masked') continue;
    // A page's whole text: go by the element instead. Only for a design-system
    // element (a custom tag), of which a view has one or two; a bare tag of a
    // plain element would match half the page.
    // A recorded text content is a string.
    if (k === 'textContent' && (v as string).length >= CONTAINER_TEXT && sig.tag.includes('-')) {
      changed = true;
      named = true;
      continue;
    }
    const masked = MASK(v);
    if (masked !== v) changed = true;
    if (k !== '@role' && k !== '@slot' && k !== 'type') named = true;
    out[k] = masked;
  }
  return changed && named ? out : null;
}

/** Elements in `scope` that fit `sig`, the visible ones first. */
function candidates(scope: ParentNode, sig: Signature, translate: Translator | null = null) {
  const all = [...scope.querySelectorAll(sig.tag)].filter((el) => matches(el, sig, translate));
  return [...all.filter(visible), ...all.filter((el) => !visible(el))];
}

/**
 * The chain of scopes from the document down to `el`: one step per element
 * that leads into the next shadow root, plus `el` itself.
 */
function chainOf(el: Element) {
  const chain = [el];
  let cur = el;
  for (;;) {
    // Below the document, the root of a connected element is a shadow root.
    const root = scopeOf(cur);
    if (root === document || !(root as ShadowRoot).host) break;
    cur = (root as ShadowRoot).host;
    chain.unshift(cur);
  }
  return chain;
}

/**
 * Describe `el` so it can be found again: per step its signature and its
 * place among the visible elements with that signature in its scope.
 */
export function describe(el: Element): ElementDescription {
  return chainOf(el).map((step) => {
    const sig = signature(step);
    const list = candidates(scopeOf(step), sig);
    return { ...sig, nth: Math.max(0, list.indexOf(step)) };
  });
}

/**
 * The element a description points to now, or null. With `loose`, a step
 * that finds nothing exactly may match with its numbers masked; its place
 * (`nth`) then counts only when the looser match finds as many elements.
 * With `translate`, the description's texts are compared in the language
 * the demo runs in (see translateText.ts).
 */
export function resolve(steps: readonly DescriptionStep[] | null | undefined, { loose = false, translate = null }: ResolveOptions = {}): Element | null {
  let scope: ParentNode = document;
  let el: Element | null = null;
  for (const step of steps ?? []) {
    const { nth = 0, ...sig } = step;
    let list = candidates(scope, sig, translate);
    let index = nth;
    if (!list.length && loose) {
      const looser = loosen(sig);
      list = looser ? candidates(scope, looser, translate) : [];
      if (list.length <= nth) index = 0;
    }
    const found = list[index] ?? null;
    // A part inside a component that the component no longer renders (the
    // design system dropped the hidden radio inside a segmented-control
    // item): the component itself takes the click. Only loose, so it is
    // reported, and only inside its own shadow root.
    if (!found && loose && el && scope === el.shadowRoot) return el;
    el = found;
    if (!el) return null;
    scope = el.shadowRoot ?? el;
  }
  return el;
}

/** The element in an event's path that a person meant to act on. */
export function actionTarget(event: Event): Element | null {
  const path = event.composedPath?.() ?? [event.target];
  for (const n of path) {
    if (!isElement(n)) continue;
    if (n.matches?.(ACTIONABLE)) return n;
    // A design-system component is itself the control when its inner button
    // is not exposed through a role: stop at the custom element.
    if (n.tagName?.includes('-') && n.tagName.startsWith('NLDD-') && n.getRootNode?.() === document) return n;
  }
  return path.find((n) => isElement(n)) ?? null;
}

/** Where in the element the click landed, as fractions, so the cursor can go there. */
export function offsetIn(el: Element, clientX: number, clientY: number) {
  const r = el.getBoundingClientRect();
  if (!r.width || !r.height) return { fx: 0.5, fy: 0.5 };
  return { fx: Math.min(1, Math.max(0, (clientX - r.left) / r.width)), fy: Math.min(1, Math.max(0, (clientY - r.top) / r.height)) };
}

// ---- scroll anchors ------------------------------------------------------------------

/**
 * Every element under `root`, shadow roots included, in document order: the
 * same order on every load, whatever the layout.
 */
function* deepElements(root: ParentNode): Generator<Element> {
  for (const el of root.querySelectorAll('*')) {
    yield el;
    if (el.shadowRoot) yield* deepElements(el.shadowRoot);
    // A scroller inside a design-system component holds the page's content
    // through a slot: that content is not its descendant, but it is what
    // scrolls.
    if (el.tagName === 'SLOT') {
      for (const a of (el as HTMLSlotElement).assignedElements({ flatten: true })) {
        yield a;
        if (a.shadowRoot) yield* deepElements(a.shadowRoot);
        yield* deepElements(a);
      }
    }
  }
}

/**
 * The smallest element with text under `container` that crosses the line at
 * `y`: the line of the law or the trace the presenter had in the middle. Not
 * a hit test, which would find whatever lies on top (a dialog, a tooltip).
 */
function lineAt(container: ParentNode, y: number) {
  let best: Element | null = null;
  let bestHeight = Infinity;
  // Nothing with text across the line (a chart, an image): the nearest text.
  let near: Element | null = null;
  let nearDist = Infinity;
  for (const el of deepElements(container)) {
    const r = el.getBoundingClientRect();
    if (!r.height || !textOf(el)) continue;
    if (r.top <= y && r.bottom > y) {
      if (r.height < bestHeight) {
        best = el;
        bestHeight = r.height;
      }
    } else if (!best) {
      const d = Math.min(Math.abs(r.top - y), Math.abs(r.bottom - y));
      if (d < nearDist && r.height < 200) {
        near = el;
        nearDist = d;
      }
    }
  }
  return best ?? near;
}

/** The visible box of a scroller, the page itself included. */
export function viewBox(container: Element): ViewBox {
  if (container === document.scrollingElement || container === document.documentElement) {
    return { top: 0, left: 0, width: window.innerWidth, height: window.innerHeight };
  }
  return container.getBoundingClientRect();
}

const anchorMatches = (el: Element, a: Pick<ScrollAnchor, 'tag' | 'text'>, translate: Translator | null = null) =>
  el.tagName.toLowerCase() === a.tag && (textOf(el) === a.text || (!!translate && textMatchesTranslated(textPieces(el), a.text, translate)));

/**
 * What is in the middle of a scroller: an element with text, which one of
 * its kind it is under the scroller, and where it sits in the visible box.
 *
 * A scroll position in pixels only holds for the layout it was recorded in.
 * On a wider or narrower screen the same text wraps differently and the
 * same number of pixels lands somewhere else in the law. The element in the
 * middle is what the presenter was showing; the replay brings that back to
 * the same place, on any screen.
 */
export function scrollAnchor(container: Element): ScrollAnchor | null {
  const box = viewBox(container);
  if (!box.height) return null;
  const el = lineAt(container, box.top + box.height / 2);
  if (!el) return null;
  const tag = el.tagName.toLowerCase();
  const text = textOf(el);
  let index = 0;
  for (const c of deepElements(container)) {
    if (c === el) break;
    if (anchorMatches(c, { tag, text })) index += 1;
  }
  const r = el.getBoundingClientRect();
  return { tag, text, index, at: Math.round(((r.top - box.top) / box.height) * 1000) / 1000 };
}

/** The element an anchor names under `container` now, or null. */
export function findAnchor(container: ParentNode, anchor: ScrollAnchor | null | undefined, translate: Translator | null = null) {
  if (!anchor) return null;
  let n = 0;
  for (const c of deepElements(container)) {
    if (!anchorMatches(c, anchor, translate)) continue;
    if (n === anchor.index) return c;
    n += 1;
  }
  return null;
}

/**
 * Where to scroll `container` to for a recorded scroll event: its anchor back
 * where it was in the box; without one (older takes), the recorded position
 * scaled to how far this layout can scroll; without that, the pixels.
 */
export function scrollTarget(container: Element, e: RecordedScroll, translate: Translator | null = null) {
  const max = container.scrollHeight - container.clientHeight;
  // Older takes have no `max`; every use below is behind `> 0`, which an
  // absent max failed as 0 does.
  const recordedMax = e.max ?? 0;
  // Scrolled all the way: what the presenter wanted on screen is the end (or
  // the start), wherever the anchor lands. The anchor of such a scroll is
  // often a paragraph taller than the box, whose top says little about what
  // is at the bottom once the text wraps differently.
  if (recordedMax > 0 && (e.top ?? 0) >= recordedMax - 2) return Math.max(0, max);
  if (recordedMax > 0 && (e.top ?? 0) <= 1) return 0;
  const a = findAnchor(container, e.anchor, translate);
  if (a) {
    // findAnchor finds nothing without an anchor.
    const box = viewBox(container);
    return Math.max(0, Math.round(container.scrollTop + (a.getBoundingClientRect().top - box.top) - e.anchor!.at * box.height));
  }
  if (recordedMax > 0 && max > 0) return Math.round(((e.top ?? 0) * max) / recordedMax);
  return e.top ?? 0;
}
