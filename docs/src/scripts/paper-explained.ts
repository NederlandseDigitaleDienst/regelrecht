/*
 * Behavior for /research/rules-as-executed/uitgelegd and /explained.
 *
 * Progressive enhancement, as on the landing page. The server renders every
 * alternative of every switch, each under a visible label, and this file turns
 * them into one segmented control showing one alternative at a time. The
 * receipt panel only exists with JavaScript, because its decisions are computed
 * here, in the visitor's browser.
 *
 * The page is built from NLDD components only, so this file defines no
 * elements of its own: it finds its parts by data attributes on the
 * components (`data-px-switch`, `data-px-more`, `data-px-receipts`).
 */
import { runReceipts, recompute, type Receipts } from './paper-explained-run';

/** One segmented control, several panels; shows the panel the control selects. */
function initSwitch(root: HTMLElement) {
  const control = root.querySelector<HTMLElement & { value?: string }>('[data-control]');
  const panels = Array.from(root.querySelectorAll<HTMLElement>('[data-panel]'));
  if (!control || panels.length === 0) return;

  // The labels above each panel are for the stacked no-JS view; the control
  // names the visible panel once there is a control.
  root.querySelectorAll<HTMLElement>('[data-panel-label]').forEach((l) => (l.hidden = true));

  const show = (value: string) => {
    for (const p of panels) p.hidden = p.dataset.panel !== value;
  };
  const initial = root.dataset.pxSwitch || panels[0].dataset.panel || '';
  control.setAttribute('value', initial);
  control.hidden = false;
  show(initial);

  control.addEventListener('change', (e) => {
    const value = (e as CustomEvent<{ value: string }>).detail?.value ?? control.value;
    if (value) show(value);
  });
}

/**
 * "Meer uit het paper": a button and the nldd-sheet it opens.
 *
 * The sheet is a <dialog> and the design system asks for it at the document
 * root, so it moves there the first time it opens and stays (the scenario
 * runner on /concepts/scenarios does the same). The sheet closes itself on
 * Escape, the backdrop and the dismiss button of its title bar.
 */
function initMore(root: HTMLElement) {
  const button = root.querySelector<HTMLElement>('[data-px-more-open]');
  const sheet = root.querySelector<HTMLElement & { show?: () => void }>('[data-px-more-sheet]');
  if (!button || !sheet) return;
  button.addEventListener('click', async () => {
    if (sheet.parentElement !== document.body) document.body.appendChild(sheet);
    await customElements.whenDefined('nldd-sheet');
    if (typeof sheet.show === 'function') sheet.show();
    else sheet.setAttribute('open', '');
  });
}

const money = (cents: number, lang: string) =>
  new Intl.NumberFormat(lang === 'en' ? 'en-GB' : 'nl-NL', { style: 'currency', currency: 'EUR' }).format(
    cents / 100,
  );

/** A ratio from the law file ("0.137") as the statute writes it: 13,7 %. */
const percent = (ratio: string, lang: string) =>
  new Intl.NumberFormat(lang === 'en' ? 'en-GB' : 'nl-NL', {
    style: 'percent',
    maximumFractionDigits: 2,
  }).format(Number(ratio));

const longDate = (iso: string, lang: string) =>
  new Intl.DateTimeFormat(lang === 'en' ? 'en-GB' : 'nl-NL', {
    day: 'numeric',
    month: 'long',
    year: 'numeric',
  }).format(new Date(`${iso}T00:00:00`));

/** The version a law path names, by the date its file is named after. */
const versionOf = (path: string, lang: string, template: string) => {
  const from = path.split('/').pop()!.replace(/\.yaml$/, '');
  return template.replace('{date}', longDate(from, lang));
};

function formatFact(value: unknown, unit: string | null, lang: string, t: Strings): string {
  if (typeof value === 'number' && unit === 'eurocent') return money(value, lang);
  if (typeof value === 'number') return new Intl.NumberFormat(lang === 'en' ? 'en-GB' : 'nl-NL').format(value);
  if (typeof value === 'boolean') return value ? t.yes : t.no;
  if (value === null || value === undefined) return '–';
  return String(value);
}

interface Strings {
  ok: string;
  mismatch: string;
  checking: string;
  check: string;
  failed: string;
  revealA: string;
  inForce: string;
  inputs: string;
  yes: string;
  no: string;
}

class ReceiptsPanel {
  private observer?: IntersectionObserver;
  private receipts: Receipts | null = null;

  constructor(private el: HTMLElement) {}

  start() {
    const status = this.el.querySelector<HTMLElement>('[data-status]');
    if (status) status.hidden = false;
    // Fetch and compute once the panel is within a screen or so: a visitor
    // who never scrolls this far downloads neither the engine nor the laws.
    this.observer = new IntersectionObserver(
      (entries) => {
        if (!entries.some((e) => e.isIntersecting)) return;
        this.observer?.disconnect();
        void this.compute(status);
      },
      { rootMargin: '1200px 0px' },
    );
    this.observer.observe(this.el);
  }

  private strings(): Strings {
    return JSON.parse(this.el.dataset.t ?? '{}');
  }

  private async compute(status: HTMLElement | null) {
    const lang = this.el.dataset.lang === 'en' ? 'en' : 'nl';
    const t = this.strings();
    try {
      const r = await runReceipts('/');
      this.receipts = r;
      const fill = (scope: Element, f: string, v: string) => {
        const el = scope.querySelector<HTMLElement>(`[data-f="${f}"]`);
        if (el) el.textContent = v;
      };

      // What both decisions share is shown once, above them. Both record the
      // same version and calculation date; only the digest and the amount
      // differ, and those are what each card shows.
      const shared = this.el.querySelector('[data-shared]');
      if (shared) {
        fill(shared, 'version', versionOf(r.a.version, lang, t.inForce));
        fill(shared, 'date', longDate(r.calculationDate, lang));
        fill(shared, 'inputs', t.inputs.replace('{n}', String(r.facts.length)));
      }

      for (const id of ['a', 'b'] as const) {
        const card = this.el.querySelector<HTMLElement>(`[data-receipt="${id}"]`);
        if (!card) continue;
        const d = r[id];
        fill(card, 'amount', money(d.amount, lang));
        // Shortened on screen: 64 hex characters wrap over three lines on a
        // phone and say nothing more to a reader than their two ends. The full
        // digest stays in the title for whoever wants to compare it.
        fill(card, 'digest', `${d.digest.slice(0, 8)}…${d.digest.slice(-8)}`);
        card.querySelector<HTMLElement>('[data-f="digest"]')?.setAttribute('title', d.digest);
        card.querySelector('[data-check-button]')?.addEventListener('click', () => void this.check(id, card));
      }

      const list = document.querySelector<HTMLElement>('[data-facts-list]');
      if (list) {
        list.replaceChildren(
          ...r.facts.map((f) => {
            const li = document.createElement('li');
            const code = document.createElement('code');
            code.textContent = `${f.provider}.${f.field}`;
            li.append(code, document.createTextNode(` ${formatFact(f.value, f.unit, lang, t)}`));
            return li;
          }),
        );
      }
      document.querySelector<HTMLElement>('[data-facts]')?.removeAttribute('hidden');
      this.el.querySelector<HTMLElement>('[data-receipts]')?.removeAttribute('hidden');
      if (status) status.hidden = true;
    } catch (err) {
      console.error('paper-explained:', err);
      if (status) status.textContent = t.failed;
    }
  }

  private async check(id: 'a' | 'b', card: HTMLElement) {
    const r = this.receipts;
    if (!r) return;
    const lang = this.el.dataset.lang === 'en' ? 'en' : 'nl';
    const t = this.strings();
    const button = card.querySelector<HTMLElement>('[data-check-button]');
    button?.setAttribute('loading', '');
    try {
      const verified = await recompute('/');
      const d = r[id];
      const digestOk = verified.digest === d.digest;
      const amountOk = verified.amount === d.amount;
      const tag = (name: string, ok: boolean) => {
        const el = card.querySelector<HTMLElement>(`[data-check="${name}"]`);
        if (!el) return;
        el.setAttribute('color', ok ? 'success' : 'critical');
        el.setAttribute('text', ok ? t.ok : t.mismatch);
      };
      tag('digest', digestOk);
      tag('amount', amountOk);
      card.querySelector<HTMLElement>('[data-checks]')?.removeAttribute('hidden');

      const reveal = card.querySelector<HTMLElement>('[data-reveal]');
      if (reveal) {
        reveal.textContent =
          id === 'a'
            ? t.revealA
            : (this.el.dataset.revealB ?? '')
                .replace('{published}', percent(r.change.published, lang))
                .replace('{local}', percent(r.change.local, lang))
                .replace('{diff}', money(Math.abs(d.amount - verified.amount), lang));
        reveal.hidden = false;
        // The button is about to go; keep the keyboard where the answer is.
        reveal.focus();
      }
      button?.setAttribute('hidden', '');
    } catch (err) {
      console.error('paper-explained:', err);
      const reveal = card.querySelector<HTMLElement>('[data-reveal]');
      if (reveal) {
        reveal.textContent = t.failed;
        reveal.hidden = false;
      }
    } finally {
      button?.removeAttribute('loading');
    }
  }
}

document.querySelectorAll<HTMLElement>('[data-px-switch]').forEach(initSwitch);
document.querySelectorAll<HTMLElement>('[data-px-more]').forEach(initMore);
document.querySelectorAll<HTMLElement>('[data-px-receipts]').forEach((el) => new ReceiptsPanel(el).start());
