import { afterEach, describe, expect, it } from 'vitest';
import { describe as describeEl, loosen, passingData, resolve, signature } from './locator';
import { installClock, uninstallClock } from './clock';
import { centerOf, graphView, showView, type GraphApi } from './graphBridge';

afterEach(() => {
  document.body.innerHTML = '';
});

/** A box for a getBoundingClientRect stub: jsdom has no layout, and the code reads only these sides. */
const rect = (r: Partial<DOMRect>) => r as DOMRect;

/** A design-system-like host: a custom element with a button in its shadow root. */
function host(tag: string, label: string) {
  const el = document.createElement(tag);
  const root = el.attachShadow({ mode: 'open' });
  const button = document.createElement('button');
  button.setAttribute('aria-label', label);
  root.appendChild(button);
  document.body.appendChild(el);
  return { el, button };
}

describe('locator', () => {
  it('names an element by what a person reads, not by its place', () => {
    const a = document.createElement('a');
    a.href = `${window.location.origin}/wetten/zorgtoeslagwet`;
    a.textContent = '  Zorgtoeslag\n  ';
    document.body.appendChild(a);
    // A link without its origin, so a recording from localhost replays on
    // the demo's own domain.
    expect(signature(a).href).toBe('/wetten/zorgtoeslagwet');
  });

  it('scrolls to the line that was in view, not to the same pixels', async () => {
    const { scrollAnchor, findAnchor, scrollTarget } = await import('./locator');
    const box = document.createElement('div');
    for (const text of ['artikel 2', 'parameters:', '-bsn', 'parameters:', 'actions:']) {
      const line = document.createElement('span');
      line.textContent = text;
      box.appendChild(line);
    }
    document.body.appendChild(box);
    const lines = [...box.children];
    // jsdom has no layout: a box 100px high, lines of 20px, the second
    // "parameters:" across its middle.
    box.getBoundingClientRect = () => rect({ top: 0, left: 0, width: 400, height: 100, bottom: 100 });
    lines.forEach((l, i) => (l.getBoundingClientRect = () => rect({ top: i * 15 - 5, bottom: i * 15 + 15, height: 20 })));
    const middle = lines[3]!; // five lines appended above
    middle.getBoundingClientRect = () => rect({ top: 40, bottom: 60, height: 20 });
    const anchor = scrollAnchor(box);
    expect(anchor).toEqual({ tag: 'span', text: 'parameters:', index: 1, at: 0.4 });
    expect(findAnchor(box, anchor)).toBe(middle);
    // In another layout that line sits 300px down: scroll it back to 40%.
    middle.getBoundingClientRect = () => rect({ top: 300 });
    expect(scrollTarget(box, { top: 999, anchor })).toBe(260);
    // An older take without an anchor: the position scaled to this layout.
    Object.defineProperty(box, 'scrollHeight', { value: 1100 });
    Object.defineProperty(box, 'clientHeight', { value: 100 });
    expect(scrollTarget(box, { top: 250, max: 500 })).toBe(500);
    // Scrolled to the end: the end, also when the anchor would land elsewhere.
    expect(scrollTarget(box, { top: 787, max: 786, anchor })).toBe(1000);
    expect(scrollTarget(box, { top: 0, max: 786, anchor })).toBe(0);
  });

  it('finds a page again when its text changed, and a field by a renumbered name', () => {
    const page = document.createElement('rr-test-page');
    page.textContent = 'Accijnsplicht en tarief alcoholhoudende dranken Douane Bronnen Tarief bier Tarief wijn';
    document.body.appendChild(page);
    const recorded = [{ tag: 'rr-test-page', textContent: 'Accijnsplicht en tarief alcoholhoudende drankenDouaneBronnenTarief bierTarief wi', nth: 0 }];
    // A law was added: the page's text changed. Exact finds nothing, loose finds the page.
    page.textContent = 'Precariobelasting ' + page.textContent;
    expect(resolve(recorded)).toBeNull();
    expect(resolve(recorded, { loose: true })).toBe(page);
    const radio = document.createElement('input');
    radio.type = 'radio';
    radio.name = 'nldd-segmented-3';
    document.body.appendChild(radio);
    expect(signature(radio).name).toBeUndefined();
    expect(resolve([{ tag: 'input', name: 'nldd-segmented-2', type: 'radio', nth: 0 }])).toBe(radio);
  });

  it('clicks the component when the part inside it that was clicked is gone', () => {
    const { el } = host('rr-test-item', 'Alles');
    const recorded = [{ tag: 'rr-test-item', nth: 0 }, { tag: 'input', type: 'radio', nth: 0 }];
    expect(resolve(recorded)).toBeNull();
    expect(resolve(recorded, { loose: true })).toBe(el);
  });

  it('leaves out an id a component makes up on every render', () => {
    const input = document.createElement('input');
    input.id = 'nldd-field-input-3ec46f7f-58fa-4d60-9122-9bcc611bab37';
    input.setAttribute('aria-label', 'Standaardpremie 2025');
    document.body.appendChild(input);
    expect(signature(input).id).toBeUndefined();
    // A take recorded with the old id still finds the field under its new one.
    const recorded = [{ tag: 'input', id: 'nldd-field-input-00000000-1111-2222-3333-444444444444', '@aria-label': 'Standaardpremie 2025', nth: 0 }];
    expect(resolve(recorded)).toBe(input);
  });

  it('finds a button inside a shadow root again, through its host', () => {
    host('rr-test-bar', 'Wetten, 79');
    const { button } = host('rr-test-bar', 'Graaf');
    const steps = describeEl(button);
    expect(steps.map((s) => s.tag)).toEqual(['rr-test-bar', 'button']);
    expect(resolve(steps)).toBe(button);
  });

  it('falls back to a looser match when a count in a label changed', () => {
    const { button } = host('rr-test-bar', 'Wetten, 79');
    const steps = describeEl(button);
    button.setAttribute('aria-label', 'Wetten, 81');
    // Only when asked: an exact miss first means "wait, it is still loading".
    expect(resolve(steps)).toBeNull();
    expect(resolve(steps, { loose: true })).toBe(button);
  });

  it('never loosens a description down to a bare tag', () => {
    expect(loosen({ tag: 'button', textContent: 'Aanvragen' })).toBeNull();
    expect(loosen({ tag: 'button', '@role': 'tab' })).toBeNull();
    expect(loosen({ tag: 'button', '@aria-label': 'Wetten, 81' })).toEqual({ tag: 'button', __masked: true, '@aria-label': 'Wetten, #' });
  });

  it('tells two alike elements apart by their order', () => {
    const first = document.createElement('button');
    const second = document.createElement('button');
    first.textContent = 'Aanvragen';
    second.textContent = 'Aanvragen';
    document.body.append(first, second);
    expect(resolve(describeEl(second))).toBe(second);
  });

  it('finds a component by a property the design system renamed since the recording', () => {
    // nldd-toolbar's `label` became `accessibleLabel` in 0.8.100; the take
    // still names the toolbar by `label`.
    const { el, button } = host('rr-test-toolbar', 'Meer');
    Object.assign(el, { accessibleLabel: 'Werkruimte' });
    const recorded = [{ tag: 'rr-test-toolbar', label: 'Werkruimte', nth: 0 }, { tag: 'button', '@aria-label': 'Meer', nth: 0 }];
    expect(resolve(recorded)).toBe(button);
    // An element that still has the old property is judged by it.
    Object.assign(el, { label: 'Iets anders' });
    expect(resolve(recorded)).toBeNull();
  });

  it('returns null when the element is gone', () => {
    const { button } = host('rr-test-bar', 'Simulatie');
    const steps = describeEl(button);
    document.body.innerHTML = '';
    expect(resolve(steps)).toBeNull();
  });
});

describe('replay clock', () => {
  afterEach(() => uninstallClock());

  it('answers with the recorded moment, and lets go again', () => {
    const recorded = Date.UTC(2026, 9, 2, 10, 0, 0);
    installClock(() => recorded + 5000);
    expect(Date.now()).toBe(recorded + 5000);
    expect(new Date().toISOString()).toBe('2026-10-02T10:00:05.000Z');
    // A date with an argument is left alone.
    expect(new Date(0).getTime()).toBe(0);
    uninstallClock();
    expect(Date.now()).toBeGreaterThan(Date.UTC(2026, 0, 1) - 10 * 365 * 864e5);
    expect(Math.abs(Date.now() - new Date().getTime())).toBeLessThan(1000);
  });
});

describe('graph camera', () => {
  it('stores where the graph looks as a point in the graph, not in pixels', () => {
    // A pane of 1000x600 at zoom 2, shifted so that graph point (300, 100) is in the middle.
    expect(centerOf({ x: 500 - 600, y: 300 - 200, zoom: 2 }, { width: 1000, height: 600 })).toEqual({ cx: 300, cy: 100, zoom: 2 });
  });

  it('steers the graph when one is mounted, and says so when not', () => {
    const calls: unknown[][] = [];
    graphView.api = null;
    expect(showView({ cx: 1, cy: 2, zoom: 1.5 })).toBe(false);
    // Only the part showView calls.
    graphView.api = { setCenter: (...a: unknown[]) => calls.push(a) } as Partial<GraphApi> as GraphApi;
    expect(showView({ cx: 1, cy: 2, zoom: 1.5 }, 140)).toBe(true);
    expect(calls).toEqual([[1, 2, { zoom: 1.5, duration: 140 }]]);
    graphView.api = null;
  });
});

describe('data attributes that do not last', () => {
  afterEach(() => {
    document.body.innerHTML = '';
  });

  it('tells a made-up id and a passing state from a name', () => {
    expect(passingData('data-run', 'run-mv3t513c')).toBe(true);
    expect(passingData('data-case', 'zaak-mv3t9uim-3j4xe')).toBe(true);
    expect(passingData('data-focused', '')).toBe(true);
    expect(passingData('data-run', 'vergelijking')).toBe(false);
    expect(passingData('data-highlight', 'complete-data')).toBe(false);
  });

  it('leaves them out of a description, and finds a take that still has them', () => {
    document.body.innerHTML = '<button data-run="run-zz9y8x7w" data-highlight="tab">Tweede</button>';
    const button = document.querySelector('button')!; // the markup above has one
    expect(signature(button)).toEqual({ tag: 'button', type: 'submit', '@data-highlight': 'tab' });
    // Recorded on another day: another run id, and focus on the item.
    const recorded = [{ tag: 'button', type: 'submit', '@data-run': 'run-mv3t513c', '@data-focused': '', '@data-highlight': 'tab', nth: 0 }];
    expect(resolve(recorded)).toBe(button);
  });
});

describe('a recording made in another language', () => {
  it('finds a button by its translation, and a field while its translation is typed into it', async () => {
    const { makeTranslator } = await import('./translateText');
    const translate = makeTranslator({ dicts: { source: { run: 'Uitvoeren', why: 'Motivering' }, local: { run: 'Run', why: 'Reasons' } } });
    const button = document.createElement('button');
    button.textContent = 'Run';
    const field = document.createElement('textarea');
    field.setAttribute('aria-label', 'Reasons');
    field.value = 'Th';
    document.body.append(button, field);
    expect(resolve([{ tag: 'button', textContent: 'Uitvoeren', nth: 0 }], { translate })).toBe(button);
    // Recorded with the Dutch typed so far on it; the field holds the English.
    expect(resolve([{ tag: 'textarea', '@aria-label': 'Motivering', value: 'Di', nth: 0 }], { translate })).toBe(field);
    expect(resolve([{ tag: 'button', textContent: 'Uitvoeren', nth: 0 }])).toBe(null);
  });
});
