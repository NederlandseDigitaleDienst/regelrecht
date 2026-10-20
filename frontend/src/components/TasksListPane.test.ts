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

// De pane vertaalt law_ids naar weergavenamen via useCorpusLaws; dat is de
// tweede netwerkpoot (zelfde mock-vorm als useCorpusLaws.test.js).
const apiFetchJson = vi.fn();
vi.mock('../lib/apiFetch', () => ({
  apiFetchJson: (...a: unknown[]) => apiFetchJson(...a),
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
    apiFetchJson.mockReset();
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
    apiFetchJson.mockResolvedValue([]);
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

  it('toont een lege staat zonder open taken', async () => {
    const wrapper = await mountPane([]);
    expect(wrapper.get('nldd-inline-dialog').attributes('text')).toBe('Geen taken');
  });

  // De acties zitten achter één Acties-menu per rij; `select` is het event dat
  // nldd-menu-item vuurt.
  function menuItems(wrapper: VueWrapper) {
    return wrapper.findAll('nldd-menu-item');
  }
  function itemLabels(wrapper: VueWrapper) {
    return menuItems(wrapper).map((i) => i.attributes('text'));
  }
  function findItem(wrapper: VueWrapper, text: string) {
    const item = menuItems(wrapper).find((i) => i.attributes('text') === text);
    if (!item) throw new Error(`Geen menu-item "${text}" - wel: ${itemLabels(wrapper).join(', ')}`);
    return item;
  }
  async function selectItem(wrapper: VueWrapper, text: string) {
    await findItem(wrapper, text).trigger('select');
  }

  // Een echte klik op een nldd-menu-item loopt in deze volgorde: capture-fase
  // op het host-element, dan de click-handler ín het item (die `select` vuurt,
  // composed + bubbles), dan de bubble-fase op het host-element. Hier is het
  // item een onbekend element zonder shadow root (het ontwerpsysteem wordt in
  // deze test niet geladen), dus staat dat "binnenwerk" als tijdelijke
  // kindnode die het select-event vuurt - alleen zo loopt het select-pad écht
  // tussen beide click-fases door.
  function clickItem(item: DOMWrapper<Element>, init: MouseEventInit = {}) {
    const inner = document.createElement('span');
    item.element.appendChild(inner);
    inner.addEventListener('click', () => {
      inner.dispatchEvent(new CustomEvent('select', { bubbles: true, composed: true }));
    });
    const event = new MouseEvent('click', { bubbles: true, cancelable: true, ...init });
    inner.dispatchEvent(event);
    inner.remove();
    return event;
  }

  it('geeft elke rij één Acties-knop in plaats van losse knoppen', async () => {
    const wrapper = await mountPane([LAW_TASK, DOC_TASK]);
    expect(wrapper.findAll('nldd-button')).toHaveLength(0);
    const btns = wrapper.findAll('nldd-icon-button');
    expect(btns).toHaveLength(2);
    expect(at(btns, 0).attributes('text')).toBe('Acties');
    // Geen `expandable`: dat zou nog een chevron naast het more-icoon zetten.
    expect(at(btns, 0).attributes('icon')).toBe('more');
    // Het menu zit in de popup-slot van de knop, die het daarmee zelf ankert
    // en togglet. Sinds design-system 0.8.76 volgt aria-haspopup daaruit, dus
    // die staat niet meer als popup-type in de markup.
    expect(at(btns, 0).find('nldd-menu[slot="popup"]').exists()).toBe(true);
  });

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

  it('toont een job_failed-taak als alert-rij, zonder de foutmelding in de rij', async () => {
    const wrapper = await mountPane([FAILED_TASK]);
    expect(wrapper.get('nldd-icon-cell').attributes('icon')).toBe('exclamation-circle-filled');
    expect(wrapper.get('nldd-icon-cell').attributes('color')).toBe('critical');
    const cell = wrapper.get('nldd-text-cell');
    expect(cell.attributes('color')).toBe('critical');
    expect(cell.attributes('text')).toBe('Verrijking van Test Wet is mislukt');
    // De technische string van ~200 tekens hoort niet in de rij maar in de sheet.
    expect(cell.attributes('supporting-text')).toBeUndefined();
    expect(itemLabels(wrapper)).toEqual(['Toon details', 'Probeer opnieuw', 'Markeer als gedaan']);
  });

  it('zet de foutmelding in de details-sheet, geopend via "Toon details"', async () => {
    const wrapper = await mountPane([FAILED_TASK]);
    // De sheet is naar body geteleporteerd, dus buiten de wrapper zoeken.
    const sheet = () => {
      const el = document.body.querySelector('nldd-sheet');
      if (!el) throw new Error('nldd-sheet not in the document');
      return el;
    };
    expect(sheet().textContent).not.toContain('Foutmelding');
    expect(sheetShown).toBe(false);

    await selectItem(wrapper, 'Toon details');
    await wrapper.vm.$nextTick();

    expect(sheetShown).toBe(true);
    const text = sheet().textContent;
    expect(text).toContain('Foutmelding');
    expect(text).toContain('CORPUS_AUTH_MINBZK_REGELRECHT_CORPUS_TOKEN');
    // De sheet draagt de leesbare titel, niet de rauwe servertitel.
    expect(text).toContain('Verrijking van Test Wet is mislukt');
  });

  it('geeft een review-taak geen details-ingang - daar valt niets technisch te tonen', async () => {
    const wrapper = await mountPane([LAW_TASK]);
    expect(itemLabels(wrapper)).not.toContain('Toon details');
  });

  it('handelt een taak af via "Markeer als gedaan" - dismissed, geen oordeel', async () => {
    const wrapper = await mountPane([LAW_TASK]);
    apiFetch.mockResolvedValue({ status: 200, json: async () => ({ tasks: [], open_count: 0 }) });
    await selectItem(wrapper, 'Markeer als gedaan');
    expect(apiFetch).toHaveBeenCalledWith(
      '/api/tasks/t2/resolve',
      expect.objectContaining({ method: 'POST', body: JSON.stringify({ action: 'dismissed' }) }),
    );
  });

  it('navigeert naar de editor-route met ?task= voor een job_review-taak', async () => {
    const wrapper = await mountPane([LAW_TASK]);
    // Beoordeel-taken dragen het eyeglasses-icoon (de actie: bekijken), niet
    // een onderwerp-icoon.
    expect(wrapper.get('nldd-icon-cell').attributes('icon')).toBe('eyeglasses');
    expect(wrapper.get('nldd-text-cell').attributes('color')).toBeUndefined();
    expect(itemLabels(wrapper)).toEqual(['Beoordelen', 'Markeer als gedaan']);
    await selectItem(wrapper, 'Beoordelen');

    expect(pushMock).toHaveBeenCalledWith({
      name: 'editor-traject',
      params: { trajectRef: 'traject-abcd1234', lawId: 'test_wet' },
      query: { task: 't2' },
    });
  });

  it('navigeert naar de werkdocumenten-route met ?task= voor een document-review-taak', async () => {
    const wrapper = await mountPane([DOC_TASK]);
    // Ook een document-review draagt eyeglasses: het icoon zegt "beoordelen",
    // het onderwerp staat in de titel.
    expect(wrapper.get('nldd-icon-cell').attributes('icon')).toBe('eyeglasses');
    await selectItem(wrapper, 'Beoordelen');
    expect(pushMock).toHaveBeenCalledWith({
      name: 'werkdocumenten-traject',
      params: { trajectRef: 'traject-abcd1234', docPath: 'bijv-rapport.md' },
      query: { task: 't5' },
    });
  });

  it('toont een disabled Beoordelen-item voor een taak zonder traject_ref/law_id', async () => {
    const wrapper = await mountPane([
      { id: 't3', task_type: 'job_review', title: 'Verrijking beoordelen: ???', payload: {} },
    ]);
    const item = findItem(wrapper, 'Beoordelen');
    expect(item.attributes('disabled')).toBeDefined();
    // Geen bruikbare href: zonder bestemming is er niets om naartoe te linken,
    // dus houdt het ontwerpsysteem het item op de <button>-tak en ontstaat er
    // geen kapotte link. Niet `toBeUndefined()`: in deze test is het item een
    // onbekend element en verdwijnt het attribuut, maar in de app reflecteert
    // een opgewaardeerde nldd-menu-item zijn lege standaardwaarde terug als
    // `href=""`. Leeg óf afwezig is wat telt - beide vallen op de button-tak.
    expect(item.attributes('href')).toBeFalsy();
    await item.trigger('select');
    expect(pushMock).not.toHaveBeenCalled();
  });

  // --- Beoordelen is een echte link ---

  it('geeft Beoordelen de href van de editor-route, met het artikel als de taak er een noemt', async () => {
    const wrapper = await mountPane([LAW_TASK]);
    expect(findItem(wrapper, 'Beoordelen').attributes('href')).toBe(
      '/trajecten/traject-abcd1234/editor/test_wet?task=t2',
    );

    const withArticle = await mountPane([
      { ...LAW_TASK, id: 't2a', payload: { ...LAW_TASK.payload, article: '2' } },
    ]);
    expect(findItem(withArticle, 'Beoordelen').attributes('href')).toBe(
      '/trajecten/traject-abcd1234/editor/test_wet/2?task=t2a',
    );
  });

  it('geeft Beoordelen op een document-review de href van de werkdocumenten-route', async () => {
    const wrapper = await mountPane([DOC_TASK]);
    expect(findItem(wrapper, 'Beoordelen').attributes('href')).toBe(
      '/trajecten/traject-abcd1234/werkdocumenten/bijv-rapport.md?task=t5',
    );
  });

  it('houdt een gewone klik op Beoordelen bij de router - geen page load', async () => {
    const wrapper = await mountPane([LAW_TASK]);
    const event = clickItem(findItem(wrapper, 'Beoordelen'));
    // preventDefault: anders volgt de browser de href en herlaadt de hele app.
    expect(event.defaultPrevented).toBe(true);
    expect(pushMock).toHaveBeenCalledTimes(1);
    expect(pushMock).toHaveBeenCalledWith({
      name: 'editor-traject',
      params: { trajectRef: 'traject-abcd1234', lawId: 'test_wet' },
      query: { task: 't2' },
    });
  });

  it('laat een ctrl/cmd/shift-klik en een middenklik aan de browser - navigeert niet zelf', async () => {
    const wrapper = await mountPane([LAW_TASK]);
    for (const init of [
      { ctrlKey: true },
      { metaKey: true },
      { shiftKey: true },
      { altKey: true },
      // Middenklik vuurt in de praktijk auxclick, maar een click met button=1
      // hoort net zo goed niet door ons afgevangen te worden.
      { button: 1 },
    ]) {
      const event = clickItem(findItem(wrapper, 'Beoordelen'), init);
      expect(event.defaultPrevented).toBe(false);
    }
    // Ook het select-event dat het menu-item bij zo'n klik meestuurt mag niet
    // alsnog het huidige tabblad meenemen.
    expect(pushMock).not.toHaveBeenCalled();

    // En het overslaan geldt alleen voor die klik: een select zónder klik
    // erachter (het menu activeert een item programmatisch als je op het ene
    // item indrukt en op het andere loslaat) moet daarna gewoon weer
    // navigeren. Bleef het overslaan hangen, dan was dat pad hierna dood.
    await selectItem(wrapper, 'Beoordelen');
    expect(pushMock).toHaveBeenCalledTimes(1);
  });

  // Vue's event invoker skips a handler when the event's timestamp is not
  // later than the moment that handler was attached (`e._vts <=
  // invoker.attached`, millisecond resolution). The first Vue handler an event
  // reaches stamps it and always runs; every later one is subject to the
  // check. So when a click lands in the same millisecond as the render that
  // attached the listeners, only the first of two Vue handlers on that click
  // runs. Pinning Date.now forces that ordering; without the pin it happened
  // in about one run in four, and it failed a merge-queue run.
  it('laat een select na een ctrl-klik ook navigeren als de klik in dezelfde milliseconde valt als de render', async () => {
    const now = vi.spyOn(Date, 'now').mockReturnValue(Date.now());
    try {
      const wrapper = await mountPane([LAW_TASK]);
      const event = clickItem(findItem(wrapper, 'Beoordelen'), { ctrlKey: true });
      expect(event.defaultPrevented).toBe(false);
      expect(pushMock).not.toHaveBeenCalled();

      await selectItem(wrapper, 'Beoordelen');
      expect(pushMock).toHaveBeenCalledTimes(1);
    } finally {
      now.mockRestore();
    }
  });

  // --- Probeer opnieuw: één bedoeling, twee mechanieken ---

  it('opent voor een mislukte conversie de bestandskiezer via de ouder', async () => {
    const wrapper = await mountPane([
      {
        id: 't8',
        task_type: 'job_failed',
        title: 'x',
        payload: { target_path: 'bijlage.md', error: 'boom', traject_ref: 'traject-abcd1234' },
      },
    ]);
    await selectItem(wrapper, 'Probeer opnieuw');
    // De picker + foutmodal wonen in LibraryView, dus dit is een emit.
    expect(wrapper.emitted('upload')).toHaveLength(1);
  });

  it('vraagt voor een mislukte verrijking de verrijking opnieuw aan', async () => {
    const wrapper = await mountPane([
      {
        id: 't9',
        task_type: 'job_failed',
        title: 'x',
        payload: { law_id: 'test_wet', error: 'boom', traject_ref: 'traject-abcd1234' },
      },
    ]);
    await selectItem(wrapper, 'Probeer opnieuw');
    expect(apiFetch).toHaveBeenCalledWith(
      '/api/trajects/traject-abcd1234/corpus/laws/test_wet/enrich',
      expect.objectContaining({ method: 'POST' }),
    );
    expect(wrapper.emitted('upload')).toBeUndefined();
  });

  // --- Categorie-filtering: de lijst toont alleen wat het panel belooft ---

  it('filtert op de werkdocumenten-context', async () => {
    const wrapper = await mountPane([DOC_TASK, LAW_TASK], [], { categorie: 'werkdocumenten' });
    const cells = wrapper.findAll('nldd-text-cell');
    expect(cells).toHaveLength(1);
    expect(at(cells, 0).attributes('text')).toBe('Beoordeel werkdocument bijv-rapport.md');
  });

  it('filtert op de wet-context', async () => {
    const wrapper = await mountPane([DOC_TASK, LAW_TASK], [], { categorie: 'wet' });
    const cells = wrapper.findAll('nldd-text-cell');
    expect(cells).toHaveLength(1);
    expect(at(cells, 0).attributes('text')).toBe('Beoordeel verrijking van Test Wet');
  });

  it('filtert op een individuele wet', async () => {
    const andere = { ...LAW_TASK, id: 't9', payload: { ...LAW_TASK.payload, law_id: 'andere_wet' } };
    const wrapper = await mountPane([LAW_TASK, andere], [], {
      categorie: 'wet',
      lawId: 'andere_wet',
    });
    expect(wrapper.findAll('nldd-list-item')).toHaveLength(1);
  });

  it('zet mislukte taken bovenaan, ongeacht hun plek in de serverlijst', async () => {
    const failed = {
      id: 't7',
      task_type: 'job_failed',
      title: 'Conversie mislukt: bijlage.md',
      payload: { traject_ref: 't-1', target_path: 'bijlage.md', error: 'boom' },
    };
    // De server levert nieuwste eerst; de mislukte staat hier als laatste.
    const wrapper = await mountPane([DOC_TASK, LAW_TASK, failed]);
    const titles = wrapper.findAll('nldd-text-cell').map((c) => c.attributes('text'));
    expect(titles[0]).toBe('Conversie van bijlage.md is mislukt');
  });

  it('laat de onderlinge volgorde van de overige taken staan', async () => {
    const failed = {
      id: 't7',
      task_type: 'job_failed',
      title: 'Conversie mislukt: bijlage.md',
      payload: { traject_ref: 't-1', target_path: 'bijlage.md', error: 'boom' },
    };
    const wrapper = await mountPane([DOC_TASK, failed, LAW_TASK]);
    const titles = wrapper.findAll('nldd-text-cell').map((c) => c.attributes('text'));
    expect(titles).toEqual([
      'Conversie van bijlage.md is mislukt',
      'Beoordeel werkdocument bijv-rapport.md',
      'Beoordeel verrijking van Test Wet',
    ]);
  });

  // --- Wachten op: lopende jobs, geen taken ---

  it('toont onder wachten een activity-indicator per lopende job', async () => {
    const wrapper = await mountPane([], [{ job_id: 'j1', law_id: 'test_wet', status: 'pending' }], {
      categorie: 'wachten',
    });
    const indicators = wrapper.findAll('nldd-activity-indicator');
    expect(indicators).toHaveLength(1);
    expect(at(indicators, 0).attributes('text')).toBe('Wachten op verrijking van Test Wet');
    expect(wrapper.find('nldd-inline-dialog').exists()).toBe(false);
  });

  // Zonder timing="instant" wacht de indicator 1000ms (anti-flash) voordat hij
  // verschijnt - een gat in een rij die er toch al staat.
  it('toont de indicator direct, zonder anti-flash-vertraging', async () => {
    const wrapper = await mountPane([], [{ job_id: 'j1', law_id: 'test_wet' }], {
      categorie: 'wachten',
    });
    expect(wrapper.get('nldd-activity-indicator').attributes('timing')).toBe('instant');
  });

  it('toont een lopende documentconversie met de bestandsnaam uit target_path', async () => {
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
    const indicators = wrapper.findAll('nldd-activity-indicator');
    expect(indicators).toHaveLength(1);
    expect(at(indicators, 0).attributes('text')).toBe('Wachten op conversie van rapport.md');
  });

  it('toont onder wachten een eigen lege staat, niet die van de takenlijst', async () => {
    const wrapper = await mountPane([], [], { categorie: 'wachten' });
    expect(wrapper.get('nldd-inline-dialog').attributes('text')).toBe('Niets loopt op dit moment.');
  });

  it('toont onder wachten geen taken, alleen lopende jobs', async () => {
    const wrapper = await mountPane([LAW_TASK], [{ job_id: 'j1', law_id: 'test_wet' }], {
      categorie: 'wachten',
    });
    // Alleen de running-rij; de taak hoort onder 'alle', niet hier.
    expect(wrapper.findAll('nldd-list-item')).toHaveLength(1);
    expect(wrapper.findAll('nldd-button')).toHaveLength(0);
  });

  // --- "Alle taken" is letterlijk alles: taken én waar je op wacht ---

  it('toont onder alle ook de lopende jobs, onderaan', async () => {
    const wrapper = await mountPane([LAW_TASK], [{ job_id: 'j1', law_id: 'test_wet' }]);
    expect(wrapper.findAll('nldd-list-item')).toHaveLength(2);
    const titles = wrapper.findAll('nldd-text-cell').map((c) => c.attributes('text'));
    expect(titles).toEqual([
      'Beoordeel verrijking van Test Wet',
      'Wachten op verrijking van Test Wet',
    ]);
  });

  it('geeft een lopende job onder alle een indicator en alleen het menu van de job', async () => {
    const wrapper = await mountPane([], [{ job_id: 'j1', law_id: 'test_wet' }]);
    expect(wrapper.findAll('nldd-activity-indicator')).toHaveLength(1);
    // Wachten is geen taak: geen taakactie, alleen kijken naar wat er loopt.
    expect(wrapper.findAll('nldd-icon-button')).toHaveLength(1);
    expect(itemLabels(wrapper)).toEqual(['Bekijk regelwerk']);
  });

  it('toont niet de lege staat onder alle wanneer er alleen iets loopt', async () => {
    const wrapper = await mountPane([], [{ job_id: 'j1', law_id: 'test_wet' }]);
    expect(wrapper.find('nldd-inline-dialog').exists()).toBe(false);
  });

  // --- Lopende jobs horen in hun eigen context, onder je eigen taken ---

  it('toont een lopende conversie onder Werkdocumenten, na de taken', async () => {
    const convertJob = {
      job_id: 'j2',
      job_type: 'document_convert',
      law_id: 'doc:traject-abcd1234/nota.md',
      target_path: 'nota.md',
    };
    const wrapper = await mountPane([DOC_TASK], [convertJob], { categorie: 'werkdocumenten' });
    const titles = wrapper.findAll('nldd-text-cell').map((c) => c.attributes('text'));
    expect(titles).toEqual([
      'Beoordeel werkdocument bijv-rapport.md',
      'Wachten op conversie van nota.md',
    ]);
  });

  it('houdt een lopende verrijking buiten Werkdocumenten', async () => {
    const wrapper = await mountPane(
      [DOC_TASK],
      [{ job_id: 'j1', job_type: 'enrich', law_id: 'test_wet' }],
      { categorie: 'werkdocumenten' },
    );
    expect(wrapper.findAll('nldd-activity-indicator')).toHaveLength(0);
    expect(wrapper.findAll('nldd-list-item')).toHaveLength(1);
  });

  it('toont een lopende verrijking onder de wet waar hij over gaat', async () => {
    const wrapper = await mountPane(
      [LAW_TASK],
      [{ job_id: 'j1', job_type: 'enrich', law_id: 'test_wet' }],
      { categorie: 'wet', lawId: 'test_wet' },
    );
    const titles = wrapper.findAll('nldd-text-cell').map((c) => c.attributes('text'));
    expect(titles).toEqual([
      'Beoordeel verrijking van Test Wet',
      'Wachten op verrijking van Test Wet',
    ]);
  });

  it('houdt een lopende verrijking buiten een andere wet', async () => {
    const wrapper = await mountPane(
      [],
      [{ job_id: 'j1', job_type: 'enrich', law_id: 'test_wet' }],
      { categorie: 'wet', lawId: 'andere_wet' },
    );
    expect(wrapper.findAll('nldd-activity-indicator')).toHaveLength(0);
  });

  // --- Acties op een lopende job ---

  it('biedt bij een lopende conversie Bekijk document en Annuleer conversie', async () => {
    const convertJob = {
      job_id: 'j2',
      job_type: 'document_convert',
      law_id: 'doc:traject-abcd1234/nota.md',
      target_path: 'nota.md',
    };
    const wrapper = await mountPane([], [convertJob], { categorie: 'wachten' });
    expect(itemLabels(wrapper)).toEqual(['Bekijk document', 'Annuleer conversie']);

    await selectItem(wrapper, 'Bekijk document');
    // Emit i.p.v. navigeren: LibraryView zet viewingJobPath vóór de navigatie,
    // zodat de job-weergave deterministisch verschijnt (geen 404/mislukt-race).
    expect(wrapper.emitted('view-job')?.[0]).toEqual(['nota.md']);
    expect(pushMock).not.toHaveBeenCalled();

    // Annuleren gooit de job én de bron-upload weg - dat mag niet lezen als een
    // gewone actie naast "Bekijk document".
    const annuleer = found(menuItems(wrapper).find((i) => i.attributes('text') === 'Annuleer conversie'));
    expect(annuleer.attributes('variant')).toBe('destructive');

    await selectItem(wrapper, 'Annuleer conversie');
    expect(wrapper.emitted('cancel-job')?.[0]).toEqual([convertJob]);
  });

  // Annuleren bestaat alleen voor conversies, dus zonder dit zou een lopende
  // verrijking een leeg menu openen.
  it('biedt bij een lopende verrijking Bekijk regelwerk, en geen annuleren', async () => {
    const wrapper = await mountPane([], [{ job_id: 'j1', job_type: 'enrich', law_id: 'test_wet' }], {
      categorie: 'wachten',
    });
    expect(itemLabels(wrapper)).toEqual(['Bekijk regelwerk']);
    await selectItem(wrapper, 'Bekijk regelwerk');
    expect(pushMock).toHaveBeenCalled();
  });

  it('houdt lopende jobs buiten Prioriteit - er valt niets te doen', async () => {
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
