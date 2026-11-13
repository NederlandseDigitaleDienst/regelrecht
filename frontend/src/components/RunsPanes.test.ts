import { describe, it, expect, vi, beforeEach, afterEach } from 'vitest';
import { flushPromises, mount } from '@vue/test-utils';
import { createPinia, setActivePinia } from 'pinia';
import { installTestCorpus } from '../test-utils/corpusSetup';
import { makeRun } from '../test-utils/runFixtures';
import RunsListPane from './RunsListPane.vue';
import RunDetailPane from './RunDetailPane.vue';
import { useTrajectRunsStore } from '../stores/trajectRuns';

const apiFetchJson = vi.fn();
vi.mock('../lib/apiFetch', async (importOriginal) => ({
  ...(await importOriginal()),
  apiFetchJson: (...a: unknown[]) => apiFetchJson(...a),
}));

const RUNS = [
  makeRun({ job_id: 'running', status: 'processing', completed_at: null }),
  makeRun({ job_id: 'failed', status: 'failed', job_type: 'document_convert', target_path: 'nota.md', error: 'conversie mislukt' }),
  makeRun({ job_id: 'done', articles_total: 12, articles_with_machine_readable: 9, coverage_score: 0.75, tasks_open: 2, tasks_open_mine: 2, tasks_approved: 1 }),
  makeRun({ job_id: 'bobs', requested_by_name: 'Bob Voorbeeld', tasks_open: 3, tasks_open_mine: 0 }),
  makeRun({ job_id: 'retry', job_type: 'traject_harvest', law_id: 'BWBR0004044', status: 'pending', attempts: 1, completed_at: null, error: 'wetten.overheid.nl antwoordde niet' }),
];

describe('RunsListPane', () => {
  beforeEach(() => {
    setActivePinia(createPinia());
    installTestCorpus();
    apiFetchJson.mockReset();
    apiFetchJson.mockResolvedValue({ runs: RUNS });
  });
  afterEach(() => useTrajectRunsStore().stop());

  async function mountList(jobId: string | null = null) {
    const wrapper = mount(RunsListPane, { props: { trajectRef: 't-abcd1234', jobId } });
    await flushPromises();
    return wrapper;
  }

  it('lists what is going above what is done', async () => {
    const wrapper = await mountList();
    expect(wrapper.findAll('h4').map((h) => h.text())).toEqual(['Bezig', 'Afgerond']);
    const rows = wrapper.findAll('nldd-list-item');
    expect(rows).toHaveLength(5);
    // A running run shows an activity indicator, a finished one an icon.
    // Order: running and retrying first, then the finished ones.
    expect(rows[0]!.find('nldd-activity-indicator').exists()).toBe(true);
    // The row is a button named by its title; the indicator only says "Bezig".
    expect(rows[0]!.get('nldd-activity-indicator').attributes('text')).toBe('Bezig');
    expect(rows[1]!.get('nldd-text-cell').attributes('text')).toBe('Ophalen van BWBR0004044');
    expect(rows[2]!.get('nldd-icon-cell').attributes('icon')).toBe('exclamation-circle-filled');
    expect(rows[2]!.get('nldd-icon-cell').attributes('color')).toBe('critical');
    expect(rows[2]!.get('nldd-text-cell').attributes('text')).toBe('Conversie van nota.md');
    expect(rows[3]!.get('nldd-icon-cell').attributes('icon')).toBe('done');
  });

  it('marks the open run and emits the one clicked', async () => {
    const wrapper = await mountList('done');
    const rows = wrapper.findAll('nldd-list-item');
    expect(rows[3]!.attributes('current')).toBeDefined();
    expect(rows[0]!.attributes('current')).toBeUndefined();
    await rows[2]!.trigger('click');
    expect(wrapper.emitted('select')?.[0]).toEqual(['failed']);
  });

  it('says so when the traject has no runs yet', async () => {
    apiFetchJson.mockResolvedValue({ runs: [] });
    const wrapper = await mountList();
    expect(wrapper.get('nldd-inline-dialog').attributes('text')).toBe('Nog geen verwerkingen');
  });
});

describe('RunDetailPane', () => {
  beforeEach(() => {
    setActivePinia(createPinia());
    // The law of the enrichments is in the traject's corpus, so it can be opened.
    installTestCorpus({ 't-abcd1234': { list: [{ law_id: 'wet_op_de_zorgtoeslag' }] } }, 't-abcd1234');
    const store = useTrajectRunsStore();
    store.runs = RUNS;
    store.loaded = true;
  });

  function rows(wrapper: ReturnType<typeof mount>) {
    return Object.fromEntries(
      wrapper.findAll('nldd-list-item').map((row) => {
        const [label, value] = row.findAll('nldd-text-cell').map((c) => c.attributes('text'));
        return [label, value];
      }),
    );
  }

  it('shows the outcome of an enrichment and links to its open proposals', async () => {
    const wrapper = mount(RunDetailPane, { props: { jobId: 'done' } });
    await flushPromises();
    expect(rows(wrapper)).toMatchObject({
      Status: 'Klaar',
      'Aangevraagd door': 'Alice Voorbeeld',
      Duur: '12 min',
      'Uitvoerbare artikelen': '9 van 12',
      'Nieuw verrijkt': '75% van de artikelen die nog niet uitvoerbaar waren',
      Beoordeling: '1 overgenomen, 2 open',
    });
    const buttons = wrapper.findAll('nldd-button');
    expect(buttons.map((b) => b.attributes('text'))).toEqual(['Beoordeel 2 open voorstellen', 'Open regelwerk']);
    await buttons[0]!.trigger('click');
    await buttons[1]!.trigger('click');
    expect(wrapper.emitted('open-tasks')?.[0]).toEqual(['wet_op_de_zorgtoeslag']);
    expect(wrapper.emitted('open-law')?.[0]).toEqual(['wet_op_de_zorgtoeslag']);
  });

  // Review tasks go to the requester, and Taken lists only your own: a button
  // would lead someone else to an empty list.
  it('says whose open proposals they are when they are not yours', async () => {
    const wrapper = mount(RunDetailPane, { props: { jobId: 'bobs' } });
    await flushPromises();
    expect(wrapper.get('nldd-rich-text').text()).toContain('3 voorstellen wachten op beoordeling door Bob Voorbeeld');
    expect(wrapper.findAll('nldd-button').map((b) => b.attributes('text'))).toEqual(['Open regelwerk']);
  });

  it('offers no law to open that is not in the corpus yet', async () => {
    installTestCorpus({ 't-abcd1234': { list: [] } }, 't-abcd1234');
    const wrapper = mount(RunDetailPane, { props: { jobId: 'done' } });
    await flushPromises();
    expect(wrapper.findAll('nldd-button').map((b) => b.attributes('text'))).toEqual(['Beoordeel 2 open voorstellen']);
  });

  it('shows why a run waits for its next attempt, without the old attempt\'s times', () => {
    const wrapper = mount(RunDetailPane, { props: { jobId: 'retry' } });
    expect(rows(wrapper)).toMatchObject({ Status: 'Nieuwe poging (2 van 3)' });
    expect(rows(wrapper)).not.toHaveProperty('Gestart');
    expect(rows(wrapper)).not.toHaveProperty('Loopt al');
    expect(wrapper.get('nldd-rich-text').text()).toContain('wetten.overheid.nl antwoordde niet');
  });

  it('shows the error of a failed run, and no law to open for a conversion', () => {
    const wrapper = mount(RunDetailPane, { props: { jobId: 'failed' } });
    expect(rows(wrapper)).toMatchObject({ Status: 'Mislukt' });
    expect(wrapper.get('nldd-rich-text').text()).toContain('conversie mislukt');
    expect(wrapper.find('nldd-button').exists()).toBe(false);
  });

  it('says so for a run that is not in the list', () => {
    const wrapper = mount(RunDetailPane, { props: { jobId: 'gone' } });
    expect(wrapper.get('nldd-inline-dialog').attributes('text')).toBe('Deze verwerking staat niet (meer) in de lijst');
  });
});
