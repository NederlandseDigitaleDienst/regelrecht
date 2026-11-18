/*
 * The scenario runner on /concepts/scenarios.
 *
 * A reader can change the scenario the corpus ships and watch the engine agree
 * or disagree with them. That is the whole claim the page makes, and a page
 * that only asserts it is weaker than one where the reader can break the
 * assertion themselves: raise the expected amount by a euro and the step turns
 * red, in the browser, against the real law.
 *
 * The engine, the laws and the step vocabulary are the ones CI uses. Nothing
 * here decides what a step means: every step is handed to the shared canonical-
 * grammar dispatch (~/lib/gherkin), the same function the editor and the demo
 * call. A step this page appears to understand differently from `just bdd`
 * would make the page a lie, so it does not get the chance.
 *
 * Progressive enhancement: without this script the page shows the scenario as
 * a read-only code block and says that running it needs JavaScript. The button
 * that opens the runner is hidden until the script has claimed it.
 */
import {
  parseFeature,
  dispatch,
  matchStep,
  typedArgs,
  ExecutionContext,
} from '~/lib/gherkin';
import { prepare } from './engine';

type Status = 'passed' | 'failed' | 'skipped';

interface StepResult {
  keyword: string;
  text: string;
  status: Status;
  message?: string;
}

interface RunResult {
  steps: StepResult[];
  outputs: Record<string, unknown> | null;
  failed: boolean;
}

/**
 * Execute one scenario and report every step.
 *
 * Cucumber's rule, kept: the first failure ends the run and the steps after it
 * are reported as not run rather than as passing. A scenario whose assertion
 * failed has told you what you needed to know, and running on from a broken
 * state produces errors about the breakage rather than about the law.
 */
async function runFeature(text: string, base: string): Promise<RunResult> {
  const { engine } = await prepare(base);

  const parsed = parseFeature(text);
  const scenario = parsed.scenarios[0];
  if (!scenario) throw new Error('No Scenario in this feature. Add one, starting with `Scenario:`.');

  // A fresh context and no leftover data sources: two runs of the same text
  // have to give the same answer, whatever the previous edit registered.
  engine.clearDataSources();
  const ctx = new ExecutionContext();

  const steps: StepResult[] = [];
  const all = [...(parsed.background ?? []), ...scenario.steps];
  let failed = false;

  for (const step of all) {
    const base_: StepResult = { keyword: step.keyword, text: step.text, status: 'skipped' };
    if (failed) {
      steps.push(base_);
      continue;
    }

    const match = matchStep(step.text);
    if (!match) {
      failed = true;
      steps.push({
        ...base_,
        status: 'failed',
        message:
          'This phrasing is not in the canonical grammar, so no engine knows what it means.',
      });
      continue;
    }

    const { entry, args } = match;
    const typed = typedArgs(entry, args);

    try {
      await dispatch(ctx, engine, entry.action, [...typed, ...entry.literals], step.dataTable ?? null, {
        // Every law the corpus scenario names is already in the engine. A law
        // this page cannot reach is a deliberate limit, not a fetch to make:
        // the runner ships the zorgtoeslag chain and nothing else.
        loadDependency: async (lawId: string) => {
          throw new Error(
            `Law "${lawId}" is not loaded here. This runner ships the laws the zorgtoeslag scenario needs; run the full corpus with \`just bdd\`.`,
          );
        },
      });
      steps.push({ ...base_, status: 'passed' });
    } catch (err: any) {
      failed = true;
      steps.push({ ...base_, status: 'failed', message: String(err?.message ?? err) });
    }
  }

  return { steps, outputs: (ctx.result as any)?.outputs ?? null, failed };
}

/**
 * An output as the engine produced it.
 *
 * Deliberately unconverted: the law declares `hoogte_zorgtoeslag` in eurocent
 * and the scenario asserts eurocent, so an amount rewritten as euro here would
 * not match the number the reader is editing two panels away.
 */
function formatOutput(value: unknown): string {
  if (value === null) return 'absent';
  if (typeof value === 'boolean') return value ? 'true' : 'false';
  if (typeof value === 'number') return String(value);
  return JSON.stringify(value);
}

/**
 * Scroll a row into the middle of the sheet.
 *
 * `scrollIntoView` is the obvious call and the wrong one here: the sheet is a
 * native dialog in the top layer, and the browser answered it by scrolling the
 * document behind the overlay by a thousand pixels while the sheet itself
 * stayed where it was. The element that scrolls is the dialog in the sheet's
 * shadow root, and walking up from the row never reaches it, because the row is
 * light DOM slotted into it. So it is passed in and moved directly.
 *
 * A frame's delay first: the rows have only just been appended and the cells
 * inside them upgrade asynchronously, so their height is not final yet.
 */
function reveal(row: HTMLElement | undefined, scroller: Element | null | undefined) {
  if (!row || !scroller) return;
  requestAnimationFrame(() => {
    const r = row.getBoundingClientRect();
    const s = scroller.getBoundingClientRect();
    scroller.scrollTo({
      top: scroller.scrollTop + r.top - s.top - (scroller.clientHeight - r.height) / 2,
      behavior: 'smooth',
    });
  });
}

/*
 * The mark per outcome.
 *
 * The colours are nldd-icon-cell's own, which are not nldd-icon's: the cell
 * knows 'content' | 'secondary' | 'accent' | 'success' | 'warning' | 'critical'
 * and draws nothing at all for a name outside that set. `secondary-content`
 * came from the icon's list, matched no selector, and left a skipped step in
 * the default colour with no error anywhere.
 */
const ICONS: Record<Status, { icon: string; color: string }> = {
  passed: { icon: 'success', color: 'success' },
  failed: { icon: 'error', color: 'critical' },
  skipped: { icon: 'remove', color: 'secondary' },
};

class ScenarioRunner extends HTMLElement {
  private sheet: any = null;
  private editor: any = null;
  private initial = '';
  private bound = false;

  connectedCallback() {
    // A re-insertion (a client-side navigation back to this page) must not add
    // a second listener to every button.
    if (this.bound) return;

    this.sheet = this.querySelector('[data-sheet]');
    this.editor = this.querySelector('[data-editor]');
    // From the attribute, not the property: the editor is a custom element and
    // has not necessarily upgraded yet, so `.value` can still be undefined here
    // and Reset would then empty the editor instead of restoring it. The
    // attribute is what the page was built with and never changes.
    this.initial = this.editor?.getAttribute('value') ?? '';

    const open = this.querySelector('[data-open]') as HTMLElement | null;
    if (!open || !this.sheet) return;
    this.bound = true;

    // The sheet is a <dialog>; the design system asks for it at the document
    // root rather than inside a content column, where it would take part in
    // the page's layout. Moving it here rather than in the template keeps the
    // markup next to the button it belongs to.
    document.body.appendChild(this.sheet);

    open.hidden = false;
    open.addEventListener('click', () => {
      this.sheet.show();
      // Start the download while they read the scenario; the run then usually
      // begins at once instead of waiting on a network.
      prepare(import.meta.env.BASE_URL ?? '/').catch(() => {});
    });

    this.sheet.querySelector('[data-close]')?.addEventListener('click', () => this.sheet.hide());
    this.sheet.querySelector('[data-run]')?.addEventListener('click', () => this.run());
    this.sheet.querySelector('[data-reset]')?.addEventListener('click', () => {
      if (this.editor) this.editor.value = this.initial;
      this.clear();
    });
  }

  /** The box inside the sheet that scrolls: the dialog in its shadow root. */
  private scroller(): Element | null {
    return this.sheet?.shadowRoot?.querySelector('dialog') ?? null;
  }

  private part(name: string): HTMLElement | null {
    return this.sheet?.querySelector(`[data-${name}]`) ?? null;
  }

  private clear() {
    const list = this.part('steps');
    if (list) list.innerHTML = '';
    const outputs = this.part('outputs');
    if (outputs) outputs.innerHTML = '';
    const banner = this.part('summary');
    if (banner) banner.hidden = true;
  }

  private async run() {
    const button = this.part('run') as any;
    const text = this.editor?.value ?? '';
    this.clear();
    if (button) button.loading = true;

    try {
      const result = await runFeature(text, import.meta.env.BASE_URL ?? '/');
      this.render(result);
    } catch (err: any) {
      // A Gherkin syntax error, or the engine failing to load. Either way it is
      // about the text or the network, not about a step.
      this.summary('critical', String(err?.message ?? err));
    } finally {
      if (button) button.loading = false;
    }
  }

  private summary(variant: 'success' | 'critical', text: string) {
    const banner = this.part('summary') as any;
    if (!banner) return;
    banner.variant = variant;
    banner.text = text;
    banner.hidden = false;
  }

  private render(result: RunResult) {
    const list = this.part('steps');
    if (list) {
      for (const step of result.steps) {
        const item = document.createElement('nldd-list-item');
        item.setAttribute('size', 'sm');

        const icon = document.createElement('nldd-icon-cell');
        icon.setAttribute('icon', ICONS[step.status].icon);
        icon.setAttribute('color', ICONS[step.status].color);
        icon.setAttribute('size', '20');
        icon.setAttribute('vertical-alignment', 'top');
        item.appendChild(icon);

        // The cells share one flat slot, so the space between the mark and the
        // step is a cell of its own rather than a margin.
        const gap = document.createElement('nldd-spacer-cell');
        gap.setAttribute('size', '12');
        item.appendChild(gap);

        const cell = document.createElement('nldd-text-cell');
        cell.setAttribute('size', 'sm');
        cell.setAttribute('vertical-alignment', 'top');
        // The step in its own monospace, because its phrasing is the thing the
        // reader is editing; the failure message underneath is prose, and the
        // cell's supporting slot already gives it the secondary colour.
        const code = document.createElement('code');
        code.className = 'rr-play__step';
        code.textContent = `${step.keyword} ${step.text}`;
        cell.appendChild(code);
        if (step.message) {
          const why = document.createElement('span');
          why.slot = 'supporting-text';
          why.textContent = step.message;
          cell.appendChild(why);
        }
        item.appendChild(cell);

        list.appendChild(item);
      }
    }

    const outputs = this.part('outputs');
    if (outputs && result.outputs) {
      for (const [name, value] of Object.entries(result.outputs)) {
        const item = document.createElement('nldd-list-item');
        item.setAttribute('size', 'sm');

        const key = document.createElement('nldd-text-cell');
        key.setAttribute('size', 'sm');
        const keyCode = document.createElement('code');
        keyCode.className = 'rr-play__step';
        keyCode.textContent = name;
        key.appendChild(keyCode);
        item.appendChild(key);

        const gap = document.createElement('nldd-spacer-cell');
        gap.setAttribute('size', '12');
        item.appendChild(gap);

        const val = document.createElement('nldd-text-cell');
        val.setAttribute('size', 'sm');
        val.setAttribute('width', 'fit-content');
        val.setAttribute('horizontal-alignment', 'right');
        const valCode = document.createElement('code');
        valCode.className = 'rr-play__step';
        valCode.textContent = formatOutput(value);
        val.appendChild(valCode);
        item.appendChild(val);

        outputs.appendChild(item);
      }
    }

    const green = result.steps.filter((s) => s.status === 'passed').length;
    if (result.failed) {
      const failing = result.steps.find((s) => s.status === 'failed');
      this.summary('critical', `Failed at: ${failing?.keyword ?? ''} ${failing?.text ?? ''}`);
      // A scenario runs to twenty steps and the failing one is rarely the
      // first, so the sheet opens on a wall of green with the interesting row
      // below the fold. The banner names it; this puts it on screen.
      const at = result.steps.indexOf(failing!);
      reveal(list?.children[at] as HTMLElement | undefined, this.scroller());
    } else {
      this.summary('success', `${green} steps, all green.`);
    }
  }
}

if (!customElements.get('rr-scenario-runner')) {
  customElements.define('rr-scenario-runner', ScenarioRunner);
}
