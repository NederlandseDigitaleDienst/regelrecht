import { describe, it, expect, beforeEach } from 'vitest';
import { flushPromises, mount } from '@vue/test-utils';
import ScenarioImportDialog from './ScenarioImportDialog.vue';
import { installTestCorpus } from '../test-utils/corpusSetup';
import { res, stubFetch } from '../test-utils/fetchStubs';

const TRAJECT = 'tr-12345678';
const FITS = `Feature: Basis

  Scenario: Heeft recht
    Given law "wet_b" is loaded
    When I evaluate "heeft_recht" of "wet_a"
    Then output "heeft_recht" is true
`;
const OTHER = FITS.replace('of "wet_a"', 'of "wet_c"');

beforeEach(() => {
  installTestCorpus(
    {
      [TRAJECT]: {
        laws: { wet_a: '$id: wet_a\n', wet_b: '$id: wet_b\n' },
        outputs: { wet_a: [{ name: 'heeft_recht' }] },
      },
    },
    TRAJECT,
  );
});

function mountDialog(existing: string[] = []) {
  return mount(ScenarioImportDialog, {
    props: { trajectRef: TRAJECT, lawId: 'wet_a', existing },
    global: { stubs: { teleport: true } },
  });
}

async function choose(wrapper: ReturnType<typeof mountDialog>, files: Record<string, string>) {
  const input = wrapper.find('[data-testid="scenario-import-input"]');
  const list = Object.entries(files).map(([name, text]) => new File([text], name, { type: 'text/plain' }));
  Object.defineProperty(input.element, 'files', { value: list, configurable: true });
  await input.trigger('change');
  await flushPromises();
}

const line = (w: ReturnType<typeof mountDialog>, name: string) => w.find(`[data-testid="scenario-import-${name}"]`);
const submit = (w: ReturnType<typeof mountDialog>) => w.find('[data-testid="scenario-import-submit"]');

describe('ScenarioImportDialog', () => {
  it('shows one line per file and saves only the ones that fit', async () => {
    const fetchSpy = stubFetch(async (_url: string, _init?: RequestInit) => res({ json: { pr: null, etag: '"v1"' } }));
    const wrapper = mountDialog(['bestaand.feature']);
    await choose(wrapper, { 'goed.feature': FITS, 'ander.feature': OTHER, 'bestaand.feature': FITS });

    expect(line(wrapper, 'goed.feature').attributes('data-status')).toBe('ok');
    expect(line(wrapper, 'ander.feature').attributes('data-status')).toBe('error');
    expect(line(wrapper, 'bestaand.feature').attributes('data-status')).toBe('error');
    expect(line(wrapper, 'bestaand.feature').find('nldd-text-cell').attributes('supporting-text')).toBe(
      'Er bestaat al een scenariobestand met deze naam',
    );
    expect(submit(wrapper).attributes('text')).toBe('Importeer');

    await submit(wrapper).trigger('click');
    await flushPromises();

    const puts = fetchSpy.mock.calls.filter(([, init]) => init?.method === 'PUT');
    expect(puts.map(([url]) => String(url))).toEqual([
      `/api/trajects/${TRAJECT}/corpus/laws/wet_a/scenarios/goed.feature`,
    ]);
    expect(wrapper.emitted('imported')).toEqual([[{ trajectRef: TRAJECT, lawId: 'wet_a', filenames: ['goed.feature'] }]]);
  });

  it('reports a partial save line by line', async () => {
    stubFetch(async (url: string, init?: RequestInit) =>
      init?.method === 'PUT' && url.endsWith('/kapot.feature')
        ? res({ ok: false, status: 500, body: 'schijf vol' })
        : res({ json: {} }),
    );
    const wrapper = mountDialog();
    await choose(wrapper, { 'goed.feature': FITS, 'kapot.feature': FITS });
    await submit(wrapper).trigger('click');
    await flushPromises();

    expect(wrapper.emitted('imported')).toEqual([[{ trajectRef: TRAJECT, lawId: 'wet_a', filenames: ['goed.feature'] }]]);
    const reason = (name: string) => line(wrapper, name).find('nldd-text-cell').attributes('supporting-text');
    expect(reason('goed.feature')).toBe('Opgeslagen');
    expect(reason('kapot.feature')).toBe('Opslaan mislukt: schijf vol');
  });

  it('says so when the corpus cannot be read', async () => {
    stubFetch(async () => res({ json: {} }));
    installTestCorpus({ [TRAJECT]: { laws: { wet_a: '$id: wet_a\n' }, failures: { list: 502 } } }, TRAJECT);
    const wrapper = mountDialog();
    await choose(wrapper, { 'goed.feature': FITS });
    expect(wrapper.find('nldd-modal-dialog').attributes('supporting-text')).toMatch(/^Het corpus van dit traject kon niet worden gelezen/);
    expect(submit(wrapper).attributes('disabled')).toBeDefined();
  });

  it('cannot import when no file fits', async () => {
    stubFetch(async () => res({ json: {} }));
    const wrapper = mountDialog();
    await choose(wrapper, { 'ander.feature': OTHER });
    expect(submit(wrapper).attributes('disabled')).toBeDefined();
  });

  it('keeps the dialog open with the reason when a save fails', async () => {
    stubFetch(async () => res({ ok: false, status: 403, body: 'Geen schrijfrechten' }));
    const wrapper = mountDialog();
    await choose(wrapper, { 'goed.feature': FITS });
    await submit(wrapper).trigger('click');
    await flushPromises();

    expect(wrapper.emitted('imported')).toBeUndefined();
    expect(line(wrapper, 'goed.feature').attributes('data-status')).toBe('error');
    expect(line(wrapper, 'goed.feature').find('nldd-text-cell').attributes('supporting-text')).toContain('Opslaan mislukt');
    expect(wrapper.find('[data-testid="scenario-import-cancel"]').attributes('text')).toBe('Sluit');
  });
});
