/**
 * Doing what the presenter did, on the element `locator.ts` found again.
 *
 * Synthetic events, dispatched where a real one would land: on the innermost
 * element, bubbling and composed so they cross the shadow boundaries the way
 * a real click does. The demo's handlers (Vue's @click on a design-system
 * host, the components' own listeners inside) cannot tell the difference,
 * except through `isTrusted`, which the player uses the other way round: a
 * trusted event during playback is the viewer, not the replay.
 */

/** A real form field, as `fieldOf` finds it. */
type FormField = HTMLInputElement | HTMLTextAreaElement | HTMLSelectElement;

/** Where in an element a click lands, as fractions of its box. */
export interface ClickOffset {
  fx?: number;
  fy?: number;
}

function point(el: Element, fx = 0.5, fy = 0.5): { clientX: number; clientY: number } {
  const r = el.getBoundingClientRect();
  return { clientX: r.left + r.width * fx, clientY: r.top + r.height * fy };
}

/** The innermost element a click at the centre of `el` would hit, inside its shadow roots. */
export function innermost(el: Element, fx = 0.5, fy = 0.5): Element {
  const { clientX, clientY } = point(el, fx, fy);
  let cur: Element = el;
  for (let i = 0; i < 8 && cur?.shadowRoot; i += 1) {
    const hit = cur.shadowRoot.elementFromPoint?.(clientX, clientY);
    if (!hit || hit === cur || !cur.contains(hit) && !cur.shadowRoot.contains(hit)) break;
    cur = hit;
  }
  return cur;
}

export function click(el: Element, { fx = 0.5, fy = 0.5 }: ClickOffset = {}): void {
  const target = innermost(el, fx, fy);
  const at = point(el, fx, fy);
  const base = { bubbles: true, composed: true, cancelable: true, view: window, button: 0, ...at };
  target.dispatchEvent(new PointerEvent('pointerdown', { ...base, pointerId: 1, isPrimary: true, buttons: 1 }));
  target.dispatchEvent(new MouseEvent('mousedown', { ...base, buttons: 1 }));
  (target as Partial<HTMLElement>).focus?.({ preventScroll: true });
  target.dispatchEvent(new PointerEvent('pointerup', { ...base, pointerId: 1, isPrimary: true }));
  target.dispatchEvent(new MouseEvent('mouseup', base));
  target.dispatchEvent(new MouseEvent('click', base));
}

/** The real text field behind a design-system field: its <input> or <textarea>. */
export function fieldOf(el: Element): FormField {
  // Not every element is a form field: an element without one inside is
  // returned as it is, and then reads and writes `value`/`checked` as plain
  // properties, as before.
  if (/^(INPUT|TEXTAREA|SELECT)$/.test(el.tagName)) return el as FormField;
  return (el.shadowRoot?.querySelector<FormField>('input, textarea, select') ?? el.querySelector?.<FormField>('input, textarea, select') ?? el) as FormField;
}

/**
 * Put `value` in a field the way typing does. Through the prototype's setter,
 * because a framework may shadow `value` on the instance; then an `input`
 * event, which is what both Vue and the design-system fields listen to.
 */
export function setValue(el: Element, value: string, { change = false }: { change?: boolean } = {}): void {
  const field = fieldOf(el);
  // An element that is no form field gets a plain property (see fieldOf): the
  // <input> setter would throw "Illegal invocation" on it.
  const proto =
    field instanceof HTMLTextAreaElement ? HTMLTextAreaElement.prototype
    : field instanceof HTMLSelectElement ? HTMLSelectElement.prototype
    : field instanceof HTMLInputElement ? HTMLInputElement.prototype
    : null;
  const setter = proto ? Object.getOwnPropertyDescriptor(proto, 'value')?.set : undefined;
  if (setter) setter.call(field, value);
  else field.value = value;
  field.dispatchEvent(new Event('input', { bubbles: true, composed: true }));
  if (change) field.dispatchEvent(new Event('change', { bubbles: true, composed: true }));
}

export function setChecked(el: Element, checked: boolean): void {
  const field = fieldOf(el);
  if ((field as HTMLInputElement).checked !== checked) click(el);
}

export function key(el: Element, keyName: string): void {
  const field = fieldOf(el);
  const init = { key: keyName, bubbles: true, composed: true, cancelable: true };
  const taken = !field.dispatchEvent(new KeyboardEvent('keydown', init));
  field.dispatchEvent(new KeyboardEvent('keyup', init));
  // Escape closes the sheet, dialog or popover it is pressed in. That is the
  // browser's doing, and a made-up key event does not set it off.
  if (keyName === 'Escape' && !taken) closeAround(el);
}

/** Close the innermost open sheet, dialog or popover around `el`, as Escape does. */
/** The host of the shadow root `n` is in, or undefined in the document. */
function hostOf(n: Node): Element | undefined {
  return (n.getRootNode?.() as Partial<ShadowRoot> | undefined)?.host;
}

function closeAround(el: Element): void {
  for (let n: Element | null | undefined = el; n; n = n.parentElement ?? hostOf(n)) {
    if (n.tagName === 'DIALOG' && (n as HTMLDialogElement).open) {
      // A design-system sheet closes through its own method, so it knows.
      const host = hostOf(n) as (Element & { hide?: unknown }) | undefined;
      if (typeof host?.hide === 'function') host.hide();
      else if (n.dispatchEvent(new Event('cancel', { cancelable: true }))) (n as HTMLDialogElement).close();
      return;
    }
    if (n.matches?.('[popover]:popover-open')) {
      (n as HTMLElement).hidePopover();
      return;
    }
  }
}

export function scrollTo(el: Element, top: number, left = 0): void {
  el.scrollTo?.({ top, left, behavior: 'instant' });
}
