import { describe, it, expect, beforeEach, vi } from 'vitest';
import { flushPromises, mount } from '@vue/test-utils';
import ScenarioStatusPane from './ScenarioStatusPane.vue';
import { useOpenScenariosStore } from '../stores/openScenarios';
import { installTestCorpus } from '../test-utils/corpusSetup';
import { res, stubFetch } from '../test-utils/fetchStubs';
import type { EditorEngine } from '../composables/useEngine';

const TRAJECT = 'tr-12345678';
const LAW_YAML = '$id: wet_a\narticles: []\n';
const FEATURE = `Feature: Basis

  Scenario: Heeft recht
    Given the calculation date is "2025-01-01"
    Given parameter "inkomen" is 1
    When I evaluate "heeft_recht" of "wet_a"
    Then output "heeft_recht" is true

  Scenario: Geen recht
    Given the calculation date is "2025-01-01"
    Given parameter "inkomen" is 2
    When I evaluate "heeft_recht" of "wet_a"
    Then output "heeft_recht" is true
`;

// An engine that has every law and grants entitlement only on inkomen 1.
function fakeEngine(): EditorEngine {
  return {
    hasLaw: () => true,
    loadLaw: () => '',
    unloadLaw: () => true,
    clearDataSources: () => {},
    registerDataSource: () => {},
    registerDataSourceForLaw: () => {},
    execute: () => ({ outputs: {} }),
    executeWithTrace: (_law: string, _output: string, params: Record<string, unknown>) => ({
      outputs: { heeft_recht: params.inkomen === 1 },
    }),
    resolveNote: () => null,
    resolveNotes: () => null,
  } as unknown as EditorEngine;
}

function mountPane() {
  return mount(ScenarioStatusPane, {
    props: {
      trajectRef: TRAJECT,
      lawId: 'wet_a',
      engine: fakeEngine(),
      engineReady: true,
      lawYaml: LAW_YAML,
      articles: [],
    },
  });
}

const tagsOf = (w: ReturnType<typeof mountPane>) => w.findAll('nldd-tag').map((t) => t.attributes('text'));

beforeEach(() => {
  vi.useFakeTimers({ shouldAdvanceTime: true });
  installTestCorpus({ [TRAJECT]: { laws: { wet_a: LAW_YAML } } }, TRAJECT);
  stubFetch(async (url: string) => {
    if (url.endsWith('/scenarios')) return res({ json: [{ filename: 'basis.feature', target_law_ids: ['wet_a'] }] });
    return res({ body: FEATURE, etag: '"v1"' });
  });
});

async function settle() {
  await flushPromises();
  await vi.advanceTimersByTimeAsync(400);
  await flushPromises();
}

describe('ScenarioStatusPane', () => {
  it('lists the regelwerk\'s scenarios with the status of a run against it', async () => {
    const w = mountPane();
    await vi.waitFor(async () => {
      await settle();
      expect(tagsOf(w)).toEqual(['Geslaagd', 'Mislukt']);
    });
    expect(w.text()).toContain('basis.feature');
    // Read only: no form and no save in the pane.
    expect(w.find('[data-testid="save-scenarios-btn"]').exists()).toBe(false);
  });

  it('runs a tab\'s unsaved edits, and says so', async () => {
    const w = mountPane();
    await vi.waitFor(async () => {
      await settle();
      expect(tagsOf(w)).toEqual(['Geslaagd', 'Mislukt']);
    });

    // The tab changes the second scenario's inkomen to 1, without saving.
    const file = useOpenScenariosStore().find({ trajectRef: TRAJECT, lawId: 'wet_a', filename: 'basis.feature' })!;
    file.setValues(1, { ...file.values.value[1]!, parameterValues: { inkomen: '1' } });
    await vi.waitFor(async () => {
      await settle();
      expect(tagsOf(w)).toEqual(['Geslaagd', 'Geslaagd']);
    });
    const banner = w.find('nldd-banner[variant="warning"]');
    expect(banner.attributes('text')).toBe('Niet-opgeslagen wijzigingen in het tabblad basis.feature');
  });

  it('opens a scenario in its tab on a click', async () => {
    const w = mountPane();
    await vi.waitFor(async () => {
      await settle();
      expect(w.findAll('[data-testid^="scenario-status-basis.feature-"]')).toHaveLength(2);
    });
    await w.find('[data-testid="scenario-status-basis.feature-1"]').trigger('click');
    expect(w.emitted('open')).toEqual([['basis.feature', 'Geen recht']]);
  });

  it('shows no status while the regelwerk is not in the engine yet', async () => {
    const engine = fakeEngine();
    engine.hasLaw = () => false;
    const w = mount(ScenarioStatusPane, {
      props: { trajectRef: TRAJECT, lawId: 'wet_a', engine, engineReady: true, lawYaml: LAW_YAML, articles: [] },
    });
    await vi.waitFor(async () => {
      await settle();
      expect(w.findAll('[data-testid^="scenario-status-basis.feature-"]')).toHaveLength(2);
    });
    expect(tagsOf(w)).toEqual([]);
  });
});
