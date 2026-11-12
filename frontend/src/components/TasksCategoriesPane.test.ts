import { describe, it, expect, vi, beforeEach } from 'vitest';
import { flushPromises, mount } from '@vue/test-utils';
import type { VueWrapper } from '@vue/test-utils';
import { installTestCorpus } from '../test-utils/corpusSetup';

// useTasks' network leg - same pattern as TasksListPane.test.js.
const apiFetch = vi.fn();
vi.mock('@regelrecht/frontend-shared', () => ({ apiFetch: (...a: unknown[]) => apiFetch(...a) }));

vi.mock('../lib/apiFetch', async (importOriginal) => ({
  ...(await importOriginal()),
  apiFetch: (...a: unknown[]) => apiFetch(...a),
}));

const LAW_TASK = {
  id: 't1',
  task_type: 'job_review',
  title: 'Verrijking beoordelen: wet_op_de_zorgtoeslag',
  payload: { law_id: 'wet_op_de_zorgtoeslag', traject_ref: 't-1' },
};
const OTHER_LAW_TASK = {
  id: 't2',
  task_type: 'job_failed',
  title: 'Verrijking mislukt: participatiewet',
  payload: { law_id: 'participatiewet', traject_ref: 't-1', error: 'boom' },
};
const DOC_TASK = {
  id: 't3',
  task_type: 'job_review',
  title: 'Werkdocument beoordelen: mvt.md',
  payload: { kind: 'document', traject_ref: 't-1', target_path: 'mvt.md' },
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

describe('TasksCategoriesPane', () => {
  beforeEach(() => {
    vi.resetModules();
    apiFetch.mockReset();
  });

  // Rows go into the mocked JSON response as-is, so they stay untyped here.
  async function mountPane(
    tasks: unknown[],
    running: unknown[] = [],
    props: { categorie?: string; lawId?: string } = {},
  ) {
    apiFetch.mockResolvedValue({
      status: 200,
      json: async () => ({ tasks, open_count: tasks.length, running }),
    });
    // The pane resolves law_ids to display names through the corpus.
    installTestCorpus({
      'traject-abcd1234': {
        list: [
          { law_id: 'wet_op_de_zorgtoeslag', display_name: 'Wet op de zorgtoeslag' },
          { law_id: 'participatiewet', display_name: 'Participatiewet' },
        ],
      },
    }, 'traject-abcd1234');
    const { default: Pane } = await import('./TasksCategoriesPane.vue');
    const wrapper = mount(Pane, { props: { trajectRef: 'traject-abcd1234', ...props }, attachTo: document.body });
    // Flush the task load and the names list.
    await flushPromises();
    await wrapper.vm.$nextTick();
    return wrapper;
  }

  // De labels: alleen de cellen zonder color, want de aantallen zijn óók
  // text-cells (secondary).
  function labelsOf(wrapper: VueWrapper) {
    return wrapper
      .findAll('nldd-text-cell')
      .filter((c) => c.attributes('color') === undefined)
      .map((c) => c.attributes('text'));
  }
  function countsOf(wrapper: VueWrapper) {
    return wrapper
      .findAll('nldd-text-cell')
      .filter((c) => c.attributes('color') === 'secondary')
      .map((c) => c.attributes('text'));
  }

  // Alleen gevulde categorieën: helemaal niets → geen ingangen, alleen een
  // lege staat. Elke categorie die je wél ziet, heeft ook echt taken.
  it('toont zonder taken geen ingangen, alleen een lege staat', async () => {
    const wrapper = await mountPane([]);
    expect(labelsOf(wrapper)).toEqual([]);
    expect(wrapper.findAll('nldd-list')).toHaveLength(0);
    expect(wrapper.get('nldd-inline-dialog').attributes('text')).toBe('Geen taken');
  });

  it('laat het hele contexten-blok weg zonder taken - geen kop', async () => {
    const wrapper = await mountPane([]);
    expect(wrapper.find('nldd-title').exists()).toBe(false);
  });

  it('toont de kop zodra er één context is', async () => {
    const wrapper = await mountPane([DOC_TASK]);
    expect(wrapper.get('nldd-title').text()).toBe('Contexten');
  });

  it('toont alleen gevulde categorieën: één review-taak → Alle taken + de wet', async () => {
    // LAW_TASK is een open review (geen prioriteit) zonder lopende job: geen
    // Prioriteit, geen Wachten op, geen Werkdocumenten.
    const wrapper = await mountPane([LAW_TASK]);
    expect(labelsOf(wrapper)).toEqual(['Alle taken', 'Wet op de zorgtoeslag']);
  });

  it('counts only the tasks and jobs of this traject, also for a law enriched in two', async () => {
    const here = { ...LAW_TASK, id: 'h1', traject_id: 'abcd1234-0000-4000-8000-000000000000' };
    const elsewhere = { ...LAW_TASK, id: 'e1', traject_id: 'ffff0000-0000-4000-8000-000000000000' };
    const runningElsewhere = {
      job_id: 'j9',
      job_type: 'enrich',
      law_id: 'wet_op_de_zorgtoeslag',
      traject_ref: 'ander-ffff0000',
    };
    const wrapper = await mountPane([here, elsewhere], [runningElsewhere]);
    expect(labelsOf(wrapper)).toEqual(['Alle taken', 'Wet op de zorgtoeslag']);
    expect(countsOf(wrapper)).toEqual(['1', '1']);
  });

  it('toont geen aantal bij een categorie zonder taken', async () => {
    const wrapper = await mountPane([]);
    expect(countsOf(wrapper)).toEqual([]);
  });

  it('gebruikt nergens een badge - een rode badge hoort bij urgentie, niet bij een aantal', async () => {
    const wrapper = await mountPane([LAW_TASK, OTHER_LAW_TASK, DOC_TASK], [{ job_id: 'j1' }]);
    expect(wrapper.find('nldd-badge').exists()).toBe(false);
  });

  it('telt per categorie, rechts uitgelijnd als secundaire tekst', async () => {
    const wrapper = await mountPane([LAW_TASK, OTHER_LAW_TASK, DOC_TASK]);
    // Prioriteit 1 (de mislukte), Alle taken 3, Werkdocumenten 1, dan de twee
    // wet-contexten (1 elk). Wachten op heeft geen aantal: niets loopt.
    expect(countsOf(wrapper)).toEqual(['1', '3', '1', '1', '1']);
    const first = found(
      wrapper.findAll('nldd-text-cell').find((c) => c.attributes('color') === 'secondary'),
    );
    expect(first.attributes('horizontal-alignment')).toBe('right');
  });

  // Aflopend naar urgentie; "Alle taken" is bladeren, geen signaal, en staat
  // daarom onderaan.
  it('zet Prioriteit bovenaan en Alle taken onderaan', async () => {
    // Alle drie zichtbaar: een mislukte taak (prioriteit) + een lopende job.
    const wrapper = await mountPane([OTHER_LAW_TASK], [
      { job_id: 'j1', job_type: 'enrich', law_id: 'kieswet' },
    ]);
    const icons = wrapper.findAll('nldd-icon').map((i) => i.attributes('icon'));
    expect(icons.filter((n) => n !== 'chevron-right').slice(0, 3)).toEqual([
      'exclamation-circle',
      'clock',
      'circle-grid-2x2-top-left-check-mark',
    ]);
  });

  it('toont geen aantal bij Prioriteit als er niets mislukt is', async () => {
    // Twee open reviews, geen mislukte: Alle taken telt 2, Prioriteit niets.
    const wrapper = await mountPane([LAW_TASK, DOC_TASK]);
    expect(countsOf(wrapper)).toEqual(['2', '1', '1']);
  });

  it('telt de lopende jobs mee in Alle taken', async () => {
    const wrapper = await mountPane([LAW_TASK], [{ job_id: 'j1' }, { job_id: 'j2' }]);
    // Geen mislukte taak, dus geen Prioriteit-aantal. Dan Wachten op 2, Alle
    // taken 3 (1 taak + 2 lopend), en de wet-context 1.
    expect(countsOf(wrapper)).toEqual(['2', '3', '1']);
  });

  it('zet de wetten plat naast Werkdocumenten, zonder verzamel-ingang', async () => {
    const wrapper = await mountPane([LAW_TASK, OTHER_LAW_TASK, DOC_TASK]);
    // OTHER is mislukt → Prioriteit; niets loopt → geen Wachten op. Geen
    // 'Wetten'-ingang; Participatiewet sorteert voor Wet op de zorgtoeslag.
    expect(labelsOf(wrapper)).toEqual([
      'Prioriteit',
      'Alle taken',
      'Werkdocumenten',
      'Participatiewet',
      'Wet op de zorgtoeslag',
    ]);
  });

  it('geeft elke context hetzelfde label-icoon (het is een filter, geen onderwerp-type)', async () => {
    // Werkdocumenten én de wet-context dragen 'label', niet documents/book.
    const wrapper = await mountPane([LAW_TASK, DOC_TASK]);
    const icons = wrapper.findAll('nldd-icon').map((i) => i.attributes('icon'));
    expect(icons).toContain('label');
    expect(icons).not.toContain('book');
    expect(icons).not.toContain('documents');
  });

  it('toont geen wet-contexten wanneer er alleen werkdocument-taken zijn', async () => {
    const wrapper = await mountPane([DOC_TASK]);
    expect(labelsOf(wrapper)).toEqual(['Alle taken', 'Werkdocumenten']);
  });

  it('toont het aantal lopende jobs bij Wachten op', async () => {
    const wrapper = await mountPane([], [
      { job_id: 'j1', job_type: 'enrich', law_id: 'participatiewet' },
      { job_id: 'j2', job_type: 'enrich', law_id: 'wet_op_de_zorgtoeslag' },
    ]);
    // Alle taken 2 en Wachten op 2 (de jobs), plus de twee wetten waar ze over
    // gaan - die zijn context puur omdat er iets voor loopt.
    expect(countsOf(wrapper)).toEqual(['2', '2', '1', '1']);
  });

  // Een lopende job maakt zelf een context: er staat werk voor die wet open,
  // ook al is er nog geen taak.
  it('maakt een wet met alleen een lopende verrijking toch een context', async () => {
    const wrapper = await mountPane([], [
      { job_id: 'j1', job_type: 'enrich', law_id: 'participatiewet' },
    ]);
    expect(labelsOf(wrapper)).toEqual([
      'Wachten op',
      'Alle taken',
      'Participatiewet',
    ]);
  });

  it('geeft een lopende conversie aan Werkdocumenten, niet aan een doc:-wet', async () => {
    const wrapper = await mountPane([], [
      {
        job_id: 'j2',
        job_type: 'document_convert',
        law_id: 'doc:t-1/nota.md',
        target_path: 'nota.md',
      },
    ]);
    expect(labelsOf(wrapper)).toEqual([
      'Wachten op',
      'Alle taken',
      'Werkdocumenten',
    ]);
  });

  it('markeert de gekozen categorie als huidig', async () => {
    // Mét een werkdocument-taak: zonder werk bestaat die context niet meer.
    const wrapper = await mountPane([DOC_TASK], [], { categorie: 'werkdocumenten' });
    const current = wrapper.findAll('nldd-list-item').filter((i) => i.attributes('current') !== undefined);
    expect(current).toHaveLength(1);
    expect(at(current, 0).get('nldd-text-cell').attributes('text')).toBe('Werkdocumenten');
  });

  it('licht bij een wet-context alleen die ene wet op', async () => {
    const wrapper = await mountPane([LAW_TASK, OTHER_LAW_TASK], [], {
      categorie: 'wet',
      lawId: 'wet_op_de_zorgtoeslag',
    });
    const current = wrapper.findAll('nldd-list-item').filter((i) => i.attributes('current') !== undefined);
    expect(current).toHaveLength(1);
    expect(at(current, 0).get('nldd-text-cell').attributes('text')).toBe('Wet op de zorgtoeslag');
  });

  it('emit de gekozen categorie', async () => {
    // Alle drie top-ingangen zichtbaar: mislukte taak (prioriteit) + lopende job.
    const wrapper = await mountPane([OTHER_LAW_TASK], [
      { job_id: 'j1', job_type: 'enrich', law_id: 'kieswet' },
    ]);
    const items = wrapper.findAll('nldd-list-item');
    await at(items, 0).trigger('click');
    expect(wrapper.emitted('select')?.[0]).toEqual([{ categorie: 'prioriteit', lawId: null }]);
    await at(items, 2).trigger('click');
    expect(wrapper.emitted('select')?.[1]).toEqual([{ categorie: 'alle', lawId: null }]);
  });

  it('emit een wet-context met haar law_id', async () => {
    const wrapper = await mountPane([LAW_TASK]);
    const items = wrapper.findAll('nldd-list-item');
    await at(items, items.length - 1).trigger('click');
    expect(wrapper.emitted('select')?.[0]).toEqual([
      { categorie: 'wet', lawId: 'wet_op_de_zorgtoeslag' },
    ]);
  });
});
