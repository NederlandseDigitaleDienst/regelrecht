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
  makeRun({ job_id: 'failed', status: 'failed', attempts: 3, error: 'geen geldige YAML na 3 pogingen' }),
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

  async function mountList(props: { lawId?: string | null; bwbId?: string | null } = {}) {
    const wrapper = mount(RunsListPane, {
      props: { trajectRef: 't-abcd1234', lawId: 'wet_op_de_zorgtoeslag', bwbId: 'BWBR0018451', ...props },
    });
    await flushPromises();
    return wrapper;
  }

  it('asks for the runs of its law, with the harvest that fetched it', async () => {
    await mountList();
    expect(apiFetchJson).toHaveBeenCalledWith(
      '/api/trajects/t-abcd1234/runs?law_id=wet_op_de_zorgtoeslag&bwb_id=BWBR0018451',
    );
  });

  it('loads nothing without a law', async () => {
    const wrapper = await mountList({ lawId: null });
    expect(apiFetchJson).not.toHaveBeenCalled();
    expect(wrapper.find('nldd-list-item').exists()).toBe(false);
  });

  it('lists what is going above what is done, without the law the page names', async () => {
    const wrapper = await mountList();
    expect(wrapper.findAll('h4').map((h) => h.text())).toEqual(['Bezig', 'Afgerond']);
    const rows = wrapper.findAll('nldd-list-item');
    expect(rows).toHaveLength(5);
    // A running run shows an activity indicator, a finished one an icon.
    // Order: running and retrying first, then the finished ones.
    expect(rows[0]!.find('nldd-activity-indicator').exists()).toBe(true);
    // The row is a button named by its title; the indicator only says "Bezig".
    expect(rows[0]!.get('nldd-activity-indicator').attributes('text')).toBe('Bezig');
    expect(rows[0]!.get('nldd-text-cell').attributes('text')).toBe('Verrijking');
    expect(rows[1]!.get('nldd-text-cell').attributes('text')).toBe('Ophalen van wetten.overheid.nl');
    expect(rows[2]!.get('nldd-icon-cell').attributes('icon')).toBe('exclamation-circle-filled');
    expect(rows[2]!.get('nldd-icon-cell').attributes('color')).toBe('critical');
    expect(rows[2]!.get('nldd-text-cell').attributes('text')).toBe('Verrijking');
    expect(rows[3]!.get('nldd-icon-cell').attributes('icon')).toBe('done');
  });

  it('emits the run clicked', async () => {
    const wrapper = await mountList();
    await wrapper.findAll('nldd-list-item')[3]!.trigger('click');
    expect(wrapper.emitted('select')?.[0]).toEqual(['done']);
  });

  it('says so when the law has no runs yet', async () => {
    apiFetchJson.mockResolvedValue({ runs: [] });
    const wrapper = await mountList();
    expect(wrapper.get('nldd-inline-dialog').attributes('text')).toBe('Nog geen verwerkingen');
  });
});

describe('RunDetailPane', () => {
  beforeEach(() => {
    setActivePinia(createPinia());
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
    // The page is the law's own, so the one button leads to its open proposals.
    const buttons = wrapper.findAll('nldd-button');
    expect(buttons.map((b) => b.attributes('text'))).toEqual(['Beoordeel 2 open voorstellen']);
    await buttons[0]!.trigger('click');
    expect(wrapper.emitted('open-tasks')?.[0]).toEqual(['wet_op_de_zorgtoeslag']);
  });

  it('lays out its rows like the table on Algemeen', async () => {
    const wrapper = mount(RunDetailPane, { props: { jobId: 'done' } });
    await flushPromises();
    const [label, value] = wrapper.get('nldd-list-item').findAll('nldd-text-cell');
    expect(label!.attributes('width')).toBe('200px');
    expect(value!.attributes('horizontal-alignment')).toBe('right');
  });

  // Review tasks go to the requester, and Taken lists only your own: a button
  // would lead someone else to an empty list.
  it('says whose open proposals they are when they are not yours', async () => {
    const wrapper = mount(RunDetailPane, { props: { jobId: 'bobs' } });
    await flushPromises();
    expect(wrapper.get('nldd-rich-text').text()).toContain('3 voorstellen wachten op beoordeling door Bob Voorbeeld');
    expect(wrapper.find('nldd-button').exists()).toBe(false);
  });

  it('shows why a run waits for its next attempt, without the old attempt\'s times', () => {
    const wrapper = mount(RunDetailPane, { props: { jobId: 'retry' } });
    expect(rows(wrapper)).toMatchObject({ Status: 'Nieuwe poging (2 van 3)' });
    expect(rows(wrapper)).not.toHaveProperty('Gestart');
    expect(rows(wrapper)).not.toHaveProperty('Loopt al');
    expect(wrapper.get('nldd-rich-text').text()).toContain('wetten.overheid.nl antwoordde niet');
  });

  it('shows the error of a failed run, without a button', () => {
    const wrapper = mount(RunDetailPane, { props: { jobId: 'failed' } });
    expect(rows(wrapper)).toMatchObject({ Status: 'Mislukt' });
    expect(wrapper.get('nldd-rich-text').text()).toContain('geen geldige YAML na 3 pogingen');
    expect(wrapper.find('nldd-button').exists()).toBe(false);
  });

  it('says so for a run that is not in the list', () => {
    const wrapper = mount(RunDetailPane, { props: { jobId: 'gone' } });
    expect(wrapper.get('nldd-inline-dialog').attributes('text')).toBe('Deze verwerking staat niet (meer) in de lijst');
  });
});
