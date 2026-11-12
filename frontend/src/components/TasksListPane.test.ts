import { describe, it, expect, vi, beforeAll, beforeEach, afterEach } from 'vitest';
import { mount } from '@vue/test-utils';
import type { DOMWrapper, VueWrapper } from '@vue/test-utils';
import type { RouteLocationRaw } from 'vue-router';
// De echte app-router, alleen voor `resolve` in de router-stub verderop: de
// href van het Beoordelen-item hoort uit de route-definities van de app te
// komen, niet uit een handgemaakte url.
import appRouter from '../router';

// nldd-sheet compileert tot een kaal custom element; de details-sheet stuurt
// het imperatief aan met show()/hide(). Zelfde stub als MobileTrajectSheet.test.js.
let sheetShown = false;
beforeAll(() => {
  if (typeof customElements !== 'undefined' && !customElements.get('nldd-sheet')) {
    class NddSheetTestStub extends HTMLElement {
      show() { sheetShown = true; }
      hide() { sheetShown = false; }
    }
    customElements.define('nldd-sheet', NddSheetTestStub);
  }
});

// Route useTasks.js's network leg through a controllable spy - same pattern
// as useTasks.test.js. Only apiFetch is replaced; the rest of the package
// stays real, because the app router (imported above) pulls useAuth from it
// through AppShell.
const apiFetch = vi.fn();
vi.mock('@regelrecht/frontend-shared', async (importOriginal) => ({
  ...(await importOriginal()),
  apiFetch: (...a: unknown[]) => apiFetch(...a),
}));

vi.mock('../lib/apiFetch', async (importOriginal) => ({
  ...(await importOriginal()),
  apiFetch: (...a: unknown[]) => apiFetch(...a),
}));

// The component navigates on "Beoordelen" via vue-router; stub `push` so the
// pane mounts without a real router (route-building itself is verified
// against the real router in ../lib/taskReview.test.js). `resolve` is the real
// one, so the item's href is the url the app would produce.
const pushMock = vi.fn();
vi.mock('vue-router', async (importOriginal) => ({
  ...(await importOriginal()),
  useRouter: () => ({ push: pushMock, resolve: (target: RouteLocationRaw) => appRouter.resolve(target) }),
}));

const DOC_TASK = {
  id: 't5',
  task_type: 'job_review',
  title: 'Werkdocument beoordelen: bijv-rapport.md',
  payload: { kind: 'document', traject_ref: 'traject-abcd1234', target_path: 'bijv-rapport.md' },
};
const LAW_TASK = {
  id: 't2',
  task_type: 'job_review',
  title: 'Verrijking beoordelen: test_wet',
  payload: { law_id: 'test_wet', traject_ref: 'traject-abcd1234' },
};

// Index into a list and fail loudly instead of reading through undefined.
function at<T>(list: readonly T[], i: number): T {
  const item = list[i];
  if (item === undefined) throw new Error(`no item at index ${i}`);
  return item;
}

// Narrow a `find()` result; throws (failing the test) when nothing matched.
function found<T>(value: T | undefined): T {
  if (value === undefined) throw new Error('expected a match');
  return value;
}

describe('TasksListPane', () => {
  // De sheet teleporteert naar body en blijft daar staan als de wrapper niet
  // wordt opgeruimd - dan vindt de volgende test de sheet van de vorige.
  let mounted: VueWrapper[] = [];
  beforeEach(() => {
    vi.resetModules();
    apiFetch.mockReset();
    pushMock.mockReset();
    sheetShown = false;
  });
  afterEach(() => {
    mounted.forEach((w) => w.unmount());
    mounted = [];
    document.body.innerHTML = '';
  });

  // useTasks is a module singleton; re-import dynamically after
  // resetModules() so each test starts from a clean slate (mirrors
  // useTasks.test.js).
  // Rows go into the mocked JSON response as-is, so they stay untyped here.
  async function mountPane(
    tasks: unknown[],
    running: unknown[] = [],
    props: { categorie?: string; lawId?: string } = { categorie: 'alle' },
  ) {
    apiFetch.mockResolvedValue({
      status: 200,
      json: async () => ({ tasks, open_count: tasks.length, running }),
    });
    // Geen wet in het corpus: displayName valt terug op humanizeLawId, zodat
    // deze tests op het rauwe id kunnen asserten (de naamresolutie zelf is
    // gedekt in ../lib/taskTitle.test.js).
    const { default: TasksListPane } = await import('./TasksListPane.vue');
    const wrapper = mount(TasksListPane, {
      props: { trajectRef: 'traject-abcd1234', ...props },
      attachTo: document.body,
    });
    // Flush useTasks' deferred initial load.
    await wrapper.vm.$nextTick();
    await Promise.resolve();
    await Promise.resolve();
    await wrapper.vm.$nextTick();
    mounted.push(wrapper);
    return wrapper;
  }


  it('shows an empty state without open tasks', async () => {
    const wrapper = await mountPane([]);
    expect(wrapper.get('nldd-inline-dialog').attributes('text')).toBe('Geen taken');
  });

  // --- Helpers: rows are segmented, menus live next to the list ---

  function rowTitles(wrapper: VueWrapper) {
    return wrapper.findAll('nldd-list-item nldd-text-cell').map((c) => c.attributes('text'));
  }
  function headings(wrapper: VueWrapper) {
    return wrapper.findAll('nldd-title h4').map((h) => h.text());
  }
  // The segment that does the row's main thing: the first one, full width.
  function mainSegment(wrapper: VueWrapper, row = 0) {
    return at(wrapper.findAll('nldd-list-item'), row).get('nldd-list-item-segment[width="full"]');
  }
  // The "more" segment of a row and the menu it points at.
  function moreSegment(wrapper: VueWrapper, row = 0) {
    return at(wrapper.findAll('nldd-list-item'), row).find('nldd-list-item-segment[popovertarget]');
  }
  function menuOf(wrapper: VueWrapper, row = 0) {
    const id = moreSegment(wrapper, row).attributes('popovertarget');
    return wrapper.get(`nldd-menu[id="${id}"]`);
  }
  function itemLabels(wrapper: VueWrapper, row = 0) {
    return menuOf(wrapper, row).findAll('nldd-menu-item').map((i) => i.attributes('text'));
  }
  async function selectItem(wrapper: VueWrapper, text: string, row = 0) {
    const item = menuOf(wrapper, row).findAll('nldd-menu-item').find((i) => i.attributes('text') === text);
    if (!item) throw new Error(`no menu item "${text}" - only: ${itemLabels(wrapper, row).join(', ')}`);
    await item.trigger('select');
  }
  // A click as the browser dispatches it from the segment's inner <a>.
  function click(segment: Pick<DOMWrapper<Element>, 'element'>, init: MouseEventInit = {}) {
    const event = new MouseEvent('click', { bubbles: true, cancelable: true, ...init });
    segment.element.dispatchEvent(event);
    return event;
  }
  const sheet = () => {
    const el = document.body.querySelector('nldd-sheet');
    if (!el) throw new Error('nldd-sheet not in the document');
    return el;
  };

  it('lists only the work of this traject, also for a law enriched in two', async () => {
    const here = { ...LAW_TASK, id: 'h1', traject_id: 'abcd1234-0000-4000-8000-000000000000' };
    const reviewElsewhere = { ...LAW_TASK, id: 'e1', traject_id: 'ffff0000-0000-4000-8000-000000000000' };
    // A failed task from the other traject must not show up here: its
    // "Probeer opnieuw" would enrich the law in this traject instead.
    const failedElsewhere = {
      id: 'e2',
      task_type: 'job_failed',
      title: 'Verrijking mislukt: test_wet',
      traject_id: 'ffff0000-0000-4000-8000-000000000000',
      payload: { law_id: 'test_wet', traject_ref: 'ander-ffff0000', error: 'boom' },
    };
    const runningElsewhere = {
      job_id: 'j9',
      job_type: 'enrich',
      law_id: 'test_wet',
      traject_ref: 'ander-ffff0000',
    };
    const wrapper = await mountPane([here, reviewElsewhere, failedElsewhere], [runningElsewhere], {
      categorie: 'wet',
      lawId: 'test_wet',
    });
    expect(wrapper.findAll('nldd-list-item')).toHaveLength(1);
    expect(wrapper.findAll('nldd-menu-item').map((i) => i.attributes('text'))).not.toContain('Probeer opnieuw');
  });

  // --- The main action is the row itself ---

  it('makes a review row a link, with the other actions in a separate more segment', async () => {
    const wrapper = await mountPane([LAW_TASK]);
    // No overflow button inside a cell any more: the row's own segments carry it.
    expect(wrapper.findAll('nldd-icon-button')).toHaveLength(0);
    expect(mainSegment(wrapper).attributes('href')).toBe('/trajecten/traject-abcd1234/editor/test_wet?task=t2');
    // The row shows the short title; the accessible name keeps the law, which
    // the heading carries visually.
    expect(mainSegment(wrapper).attributes('accessible-label')).toBe('Beoordeel het hele regelwerk van Test Wet');
    expect(moreSegment(wrapper).attributes('popovertarget')).toBe('task-menu-t2');
    expect(moreSegment(wrapper).attributes('accessible-label')).toBe(
      'Acties voor Beoordeel het hele regelwerk van Test Wet',
    );
    expect(itemLabels(wrapper)).toEqual(['Markeer als gedaan']);
  });

  it('links a review to the article it names', async () => {
    const wrapper = await mountPane([{ ...LAW_TASK, id: 't2a', payload: { ...LAW_TASK.payload, article: '2' } }]);
    expect(mainSegment(wrapper).attributes('href')).toBe('/trajecten/traject-abcd1234/editor/test_wet/2?task=t2a');
    expect(rowTitles(wrapper)).toEqual(['Beoordeel artikel 2']);
    // The accessible name starts with the words on screen (voice control).
    expect(mainSegment(wrapper).attributes('accessible-label')).toBe('Beoordeel artikel 2 van Test Wet');
  });

  it('links a document review to the werkdocumenten route', async () => {
    const wrapper = await mountPane([DOC_TASK]);
    // Review tasks carry the eyeglasses icon (the action: look at it).
    expect(wrapper.get('nldd-icon-cell').attributes('icon')).toBe('eyeglasses');
    expect(mainSegment(wrapper).attributes('href')).toBe(
      '/trajecten/traject-abcd1234/werkdocumenten/bijv-rapport.md?task=t5',
    );
  });

  it('keeps a plain click on a review row in the router - no page load', async () => {
    const wrapper = await mountPane([LAW_TASK]);
    const event = click(mainSegment(wrapper));
    // preventDefault: otherwise the browser follows the href and reloads the app.
    expect(event.defaultPrevented).toBe(true);
    expect(pushMock).toHaveBeenCalledTimes(1);
    expect(pushMock).toHaveBeenCalledWith({
      name: 'editor-traject',
      params: { trajectRef: 'traject-abcd1234', lawId: 'test_wet' },
      query: { task: 't2' },
    });
  });

  it('leaves a modified click and a middle click to the browser', async () => {
    const wrapper = await mountPane([LAW_TASK]);
    for (const init of [{ ctrlKey: true }, { metaKey: true }, { shiftKey: true }, { altKey: true }, { button: 1 }]) {
      expect(click(mainSegment(wrapper), init).defaultPrevented).toBe(false);
    }
    expect(pushMock).not.toHaveBeenCalled();
  });

  it('disables the row of a task without traject_ref/law_id instead of linking nowhere', async () => {
    const wrapper = await mountPane([
      { id: 't3', task_type: 'job_review', title: 'Verrijking beoordelen: ???', payload: {} },
    ]);
    const segment = mainSegment(wrapper);
    expect(segment.attributes('disabled')).toBeDefined();
    expect(segment.attributes('button')).toBeDefined();
    expect(segment.attributes('href')).toBeFalsy();
    click(segment);
    expect(pushMock).not.toHaveBeenCalled();
  });

  it('dismisses a task through "Markeer als gedaan" - dismissed, no verdict', async () => {
    const wrapper = await mountPane([LAW_TASK]);
    apiFetch.mockResolvedValue({ status: 200, json: async () => ({ tasks: [], open_count: 0 }) });
    await selectItem(wrapper, 'Markeer als gedaan');
    expect(apiFetch).toHaveBeenCalledWith(
      '/api/tasks/t2/resolve',
      expect.objectContaining({ method: 'POST', body: JSON.stringify({ action: 'dismissed' }) }),
    );
  });

  // --- A failure opens its details, with what you can do about it ---

  const FAILED_TASK = {
    id: 't1',
    task_type: 'job_failed',
    title: 'Verrijking mislukt: test_wet',
    payload: {
      error:
        "no push token for traject source 'minbzk-regelrecht-corpus' (expected env CORPUS_AUTH_MINBZK_REGELRECHT_CORPUS_TOKEN); the converted document cannot be persisted to the traject repository",
      law_id: 'test_wet',
      traject_ref: 'traject-abcd1234',
    },
  };

  it('shows a failed task as an alert row, without the error in the row', async () => {
    const wrapper = await mountPane([FAILED_TASK]);
    expect(wrapper.get('nldd-icon-cell').attributes('icon')).toBe('exclamation-circle-filled');
    expect(wrapper.get('nldd-icon-cell').attributes('color')).toBe('critical');
    const cell = wrapper.get('nldd-text-cell');
    expect(cell.attributes('color')).toBe('critical');
    expect(cell.attributes('text')).toBe('Verrijking is mislukt');
    // The technical string of ~200 characters belongs in the sheet.
    expect(cell.attributes('supporting-text')).toBeUndefined();
    expect(itemLabels(wrapper)).toEqual(['Probeer opnieuw', 'Markeer als gedaan']);
    // Every failed enrichment reads the same on screen; the name says which law.
    expect(mainSegment(wrapper).attributes('accessible-label')).toBe(
      'Verrijking is mislukt: Test Wet, toon details',
    );
  });

  it('offers no retry in the sheet for a failure that names no traject', async () => {
    const wrapper = await mountPane([{ ...FAILED_TASK, payload: { law_id: 'test_wet', error: 'boom' } }]);
    await mainSegment(wrapper).trigger('click');
    await wrapper.vm.$nextTick();
    const buttons = [...sheet().querySelectorAll('nldd-button')].map((b) => b.getAttribute('text'));
    expect(buttons).toEqual(['Markeer als gedaan']);
    expect(itemLabels(wrapper)).toEqual(['Markeer als gedaan']);
  });

  it('opens the details sheet with the error when the failed row is clicked', async () => {
    const wrapper = await mountPane([FAILED_TASK]);
    expect(sheet().textContent).not.toContain('Foutmelding');
    expect(sheetShown).toBe(false);

    await mainSegment(wrapper).trigger('click');
    await wrapper.vm.$nextTick();

    expect(sheetShown).toBe(true);
    const text = sheet().textContent;
    expect(text).toContain('Foutmelding');
    expect(text).toContain('CORPUS_AUTH_MINBZK_REGELRECHT_CORPUS_TOKEN');
    // The sheet has no heading above it, so it carries the full title.
    expect(text).toContain('Verrijking van Test Wet is mislukt');
    const buttons = [...sheet().querySelectorAll('nldd-button')].map((b) => b.getAttribute('text'));
    expect(buttons).toEqual(['Probeer opnieuw', 'Markeer als gedaan']);
  });

  it('retries from the details sheet and closes it', async () => {
    const wrapper = await mountPane([FAILED_TASK]);
    await mainSegment(wrapper).trigger('click');
    await wrapper.vm.$nextTick();
    const retry = found([...sheet().querySelectorAll('nldd-button')].find((b) => b.getAttribute('text') === 'Probeer opnieuw'));
    retry.dispatchEvent(new MouseEvent('click', { bubbles: true }));
    await wrapper.vm.$nextTick();
    await wrapper.vm.$nextTick();
    expect(apiFetch).toHaveBeenCalledWith(
      '/api/trajects/traject-abcd1234/corpus/laws/test_wet/enrich',
      expect.objectContaining({ method: 'POST' }),
    );
    expect(sheetShown).toBe(false);
  });

  it('dismisses from the details sheet', async () => {
    const wrapper = await mountPane([FAILED_TASK]);
    await mainSegment(wrapper).trigger('click');
    await wrapper.vm.$nextTick();
    const done = found([...sheet().querySelectorAll('nldd-button')].find((b) => b.getAttribute('text') === 'Markeer als gedaan'));
    done.dispatchEvent(new MouseEvent('click', { bubbles: true }));
    await wrapper.vm.$nextTick();
    await wrapper.vm.$nextTick();
    expect(apiFetch).toHaveBeenCalledWith(
      '/api/tasks/t1/resolve',
      expect.objectContaining({ body: JSON.stringify({ action: 'dismissed' }) }),
    );
    expect(sheetShown).toBe(false);
  });

  it('opens the file picker through the parent for a failed conversion', async () => {
    const wrapper = await mountPane([
      {
        id: 't8',
        task_type: 'job_failed',
        title: 'x',
        payload: { target_path: 'bijlage.md', error: 'boom', traject_ref: 'traject-abcd1234' },
      },
    ]);
    await selectItem(wrapper, 'Probeer opnieuw');
    // The picker and its error modal live in LibraryView, so this is an emit.
    expect(wrapper.emitted('upload')).toHaveLength(1);
  });

  it('requests a failed enrichment again', async () => {
    const wrapper = await mountPane([FAILED_TASK]);
    await selectItem(wrapper, 'Probeer opnieuw');
    expect(apiFetch).toHaveBeenCalledWith(
      '/api/trajects/traject-abcd1234/corpus/laws/test_wet/enrich',
      expect.objectContaining({ method: 'POST' }),
    );
    expect(wrapper.emitted('upload')).toBeUndefined();
  });

  // --- Order: grouped by context, in the order of the regelwerk ---

  const article = (id: string, lawId: string, nr: string) => ({
    id,
    task_type: 'job_review',
    title: 'x',
    payload: { law_id: lawId, traject_ref: 'traject-abcd1234', article: nr },
  });

  it('groups all tasks under a heading per context and sorts the articles', async () => {
    // Server order: newest first, two enrichments interleaved.
    const wrapper = await mountPane([
      article('a10', 'wet_b', '10'),
      article('x3', 'wet_a', '3'),
      article('a2', 'wet_b', '2'),
      DOC_TASK,
      article('a1a', 'wet_b', '1a'),
      article('x1', 'wet_a', '1'),
    ]);
    expect(headings(wrapper)).toEqual(['Werkdocumenten', 'Wet A', 'Wet B']);
    expect(rowTitles(wrapper)).toEqual([
      'Beoordeel bijv-rapport.md',
      'Beoordeel artikel 1',
      'Beoordeel artikel 3',
      'Beoordeel artikel 1a',
      'Beoordeel artikel 2',
      'Beoordeel artikel 10',
    ]);
  });

  it('puts a failure on top, in the first group', async () => {
    const failed = {
      id: 't7',
      task_type: 'job_failed',
      title: 'x',
      payload: { traject_ref: 'traject-abcd1234', law_id: 'wet_z', error: 'boom' },
    };
    // Last in the server list, and its law sorts last by name.
    const wrapper = await mountPane([DOC_TASK, article('x1', 'wet_a', '1'), article('z1', 'wet_z', '1'), failed]);
    expect(headings(wrapper)).toEqual(['Wet Z', 'Werkdocumenten', 'Wet A']);
    expect(at(rowTitles(wrapper), 0)).toBe('Verrijking is mislukt');
  });

  it('puts a failure first within its group, also when it would sort later', async () => {
    const failedConversion = {
      id: 'f1',
      task_type: 'job_failed',
      title: 'x',
      payload: { traject_ref: 'traject-abcd1234', target_path: 'zeta.md', error: 'boom' },
    };
    const review = { ...DOC_TASK, id: 'r1', payload: { ...DOC_TASK.payload, target_path: 'alfa.md' } };
    const wrapper = await mountPane([review, failedConversion], [], { categorie: 'werkdocumenten' });
    expect(rowTitles(wrapper)).toEqual(['Conversie van zeta.md is mislukt', 'Beoordeel alfa.md']);
  });

  it('leaves out the heading when the page title already names the law', async () => {
    const wrapper = await mountPane([LAW_TASK], [], { categorie: 'wet', lawId: 'test_wet' });
    expect(headings(wrapper)).toEqual([]);
    expect(rowTitles(wrapper)).toEqual(['Beoordeel het hele regelwerk']);
  });

  it('filters on the werkdocumenten context, without a heading', async () => {
    const wrapper = await mountPane([DOC_TASK, LAW_TASK], [], { categorie: 'werkdocumenten' });
    expect(headings(wrapper)).toEqual([]);
    expect(rowTitles(wrapper)).toEqual(['Beoordeel bijv-rapport.md']);
  });

  it('filters on the law context', async () => {
    const wrapper = await mountPane([DOC_TASK, LAW_TASK], [], { categorie: 'wet' });
    expect(headings(wrapper)).toEqual(['Test Wet']);
    expect(rowTitles(wrapper)).toEqual(['Beoordeel het hele regelwerk']);
  });

  it('filters on a single law', async () => {
    const andere = { ...LAW_TASK, id: 't9', payload: { ...LAW_TASK.payload, law_id: 'andere_wet' } };
    const wrapper = await mountPane([LAW_TASK, andere], [], { categorie: 'wet', lawId: 'andere_wet' });
    expect(wrapper.findAll('nldd-list-item')).toHaveLength(1);
  });

  // --- Wachten op: running jobs, not tasks ---

  it('shows an activity indicator per running job under wachten', async () => {
    const wrapper = await mountPane([], [{ job_id: 'j1', law_id: 'test_wet', status: 'pending' }], {
      categorie: 'wachten',
    });
    const indicators = wrapper.findAll('nldd-activity-indicator');
    expect(indicators).toHaveLength(1);
    // The indicator's label stands alone, so it names the law.
    expect(at(indicators, 0).attributes('text')).toBe('Wachten op verrijking van Test Wet');
    expect(headings(wrapper)).toEqual(['Test Wet']);
    expect(rowTitles(wrapper)).toEqual(['Wachten op verrijking']);
    expect(wrapper.find('nldd-inline-dialog').exists()).toBe(false);
  });

  // Without timing="instant" the indicator waits 1000ms (anti-flash) before it
  // shows - a gap in a row that is already there.
  it('shows the indicator at once, without the anti-flash delay', async () => {
    const wrapper = await mountPane([], [{ job_id: 'j1', law_id: 'test_wet' }], { categorie: 'wachten' });
    expect(wrapper.get('nldd-activity-indicator').attributes('timing')).toBe('instant');
  });

  it('names a running document conversion by the file name from target_path', async () => {
    const wrapper = await mountPane(
      [],
      [{
        job_id: 'j2',
        job_type: 'document_convert',
        law_id: 'doc:testtraject-abcd1234/analyses/rapport.md',
        target_path: 'analyses/rapport.md',
        status: 'processing',
      }],
      { categorie: 'wachten' },
    );
    expect(wrapper.get('nldd-activity-indicator').attributes('text')).toBe('Wachten op conversie van rapport.md');
    expect(rowTitles(wrapper)).toEqual(['Wachten op conversie van rapport.md']);
  });

  it('has its own empty state under wachten', async () => {
    const wrapper = await mountPane([], [], { categorie: 'wachten' });
    expect(wrapper.get('nldd-inline-dialog').attributes('text')).toBe('Niets loopt op dit moment.');
  });

  it('shows no tasks under wachten, only running jobs', async () => {
    const wrapper = await mountPane([LAW_TASK], [{ job_id: 'j1', law_id: 'test_wet' }], { categorie: 'wachten' });
    expect(wrapper.findAll('nldd-list-item')).toHaveLength(1);
  });

  // --- "Alle taken" is everything: tasks and what you are waiting for ---

  it('shows running jobs under alle too, at the bottom of their group', async () => {
    const wrapper = await mountPane([LAW_TASK], [{ job_id: 'j1', law_id: 'test_wet' }]);
    expect(wrapper.findAll('nldd-list-item')).toHaveLength(2);
    expect(rowTitles(wrapper)).toEqual(['Beoordeel het hele regelwerk', 'Wachten op verrijking']);
  });

  it('does not show the empty state under alle when only something is running', async () => {
    const wrapper = await mountPane([], [{ job_id: 'j1', law_id: 'test_wet' }]);
    expect(wrapper.find('nldd-inline-dialog').exists()).toBe(false);
  });

  it('shows a running conversion under Werkdocumenten, after the tasks', async () => {
    const convertJob = {
      job_id: 'j2',
      job_type: 'document_convert',
      law_id: 'doc:traject-abcd1234/nota.md',
      target_path: 'nota.md',
    };
    const wrapper = await mountPane([DOC_TASK], [convertJob], { categorie: 'werkdocumenten' });
    expect(rowTitles(wrapper)).toEqual(['Beoordeel bijv-rapport.md', 'Wachten op conversie van nota.md']);
  });

  it('keeps a running enrichment out of Werkdocumenten', async () => {
    const wrapper = await mountPane([DOC_TASK], [{ job_id: 'j1', job_type: 'enrich', law_id: 'test_wet' }], {
      categorie: 'werkdocumenten',
    });
    expect(wrapper.findAll('nldd-activity-indicator')).toHaveLength(0);
    expect(wrapper.findAll('nldd-list-item')).toHaveLength(1);
  });

  it('shows a running enrichment under the law it is about', async () => {
    const wrapper = await mountPane([LAW_TASK], [{ job_id: 'j1', job_type: 'enrich', law_id: 'test_wet' }], {
      categorie: 'wet',
      lawId: 'test_wet',
    });
    expect(rowTitles(wrapper)).toEqual(['Beoordeel het hele regelwerk', 'Wachten op verrijking']);
  });

  it('keeps a running enrichment out of another law', async () => {
    const wrapper = await mountPane([], [{ job_id: 'j1', job_type: 'enrich', law_id: 'test_wet' }], {
      categorie: 'wet',
      lawId: 'andere_wet',
    });
    expect(wrapper.findAll('nldd-activity-indicator')).toHaveLength(0);
  });

  // --- Actions on a running job ---

  it('opens a running conversion from its row and cancels it from its menu', async () => {
    const convertJob = {
      job_id: 'j2',
      job_type: 'document_convert',
      law_id: 'doc:traject-abcd1234/nota.md',
      target_path: 'nota.md',
    };
    const wrapper = await mountPane([], [convertJob], { categorie: 'wachten' });

    await mainSegment(wrapper).trigger('click');
    // An emit rather than navigating: LibraryView sets viewingJobPath before
    // the navigation, so the job view appears deterministically.
    expect(wrapper.emitted('view-job')?.[0]).toEqual(['nota.md']);
    expect(pushMock).not.toHaveBeenCalled();

    expect(itemLabels(wrapper)).toEqual(['Annuleer conversie']);
    // Cancelling throws away the job and the uploaded source.
    const cancel = menuOf(wrapper).get('nldd-menu-item');
    expect(cancel.attributes('destructive')).toBeDefined();
    await selectItem(wrapper, 'Annuleer conversie');
    expect(wrapper.emitted('cancel-job')?.[0]).toEqual([convertJob]);
  });

  // Cancelling exists only for conversions, so a running enrichment has one
  // action, and the row itself is the control.
  it('opens the law from a running enrichment, with the row as the control', async () => {
    const wrapper = await mountPane([], [{ job_id: 'j1', job_type: 'enrich', law_id: 'test_wet' }], {
      categorie: 'wachten',
    });
    expect(wrapper.find('nldd-list-item-segment').exists()).toBe(false);
    expect(wrapper.find('nldd-menu').exists()).toBe(false);
    const row = wrapper.get('nldd-list-item');
    expect(row.attributes('button')).toBeDefined();
    await row.trigger('click');
    expect(pushMock).toHaveBeenCalledTimes(1);
    expect(appRouter.resolve(pushMock.mock.calls[0]![0]).params).toMatchObject({
      trajectRef: 'traject-abcd1234',
      lawId: 'test_wet',
    });
  });

  // Both carry a law_id that names no law in the corpus: a synthetic key for a
  // law conversion, a BWB id for a harvest.
  it('files a law conversion and a harvest under Overig, as rows without a destination', async () => {
    const wrapper = await mountPane(
      [],
      [
        { job_id: 'j3', job_type: 'law_convert', law_id: 'lawdoc:traject-abcd1234/voorstel.pdf', target_path: 'voorstel.pdf' },
        { job_id: 'j4', job_type: 'traject_harvest', law_id: 'BWBR0002399' },
      ],
    );
    expect(headings(wrapper)).toEqual(['Overig']);
    // No law heading above them, so they keep their full title.
    expect(rowTitles(wrapper)).toEqual([
      'Wachten op conversie van voorstel.pdf naar een regelwerk',
      'Wachten op wet ophalen van BWBR0002399',
    ]);
    for (const row of wrapper.findAll('nldd-list-item')) {
      expect(row.attributes('button')).toBeUndefined();
      expect(row.find('nldd-icon-cell[icon="chevron-right"]').exists()).toBe(false);
      await row.trigger('click');
    }
    expect(pushMock).not.toHaveBeenCalled();
  });

  it('keeps running jobs out of Prioriteit - there is nothing to do', async () => {
    const failed = {
      id: 't7',
      task_type: 'job_failed',
      title: 'x',
      payload: { target_path: 'bijlage.md', error: 'boom' },
    };
    const wrapper = await mountPane([failed], [{ job_id: 'j1', job_type: 'enrich', law_id: 'x' }], {
      categorie: 'prioriteit',
    });
    expect(wrapper.findAll('nldd-activity-indicator')).toHaveLength(0);
    expect(wrapper.findAll('nldd-list-item')).toHaveLength(1);
  });
});
