import { mount } from '@vue/test-utils';
import { describe, it, expect, vi } from 'vitest';
import ScenarioForm from './ScenarioForm.vue';

// An engine whose run throws `thrown`. The WASM boundary can hand back any
// JS value, so the form must not assume an Error.
const throwingEngine = (thrown: unknown) => ({
  clearDataSources: vi.fn(),
  clearTerritories: vi.fn(),
  registerTerritory: vi.fn(),
  registerDataSource: vi.fn(),
  executeWithTrace: vi.fn(() => {
    throw thrown;
  }),
});

const mountForm = (thrown: unknown) =>
  mount(ScenarioForm, {
    props: {
      scenario: { assertions: [], execution: { outputName: 'aantal', lawId: 'participatiewet' } },
      setup: { calculationDate: '2025-01-01', parameters: [], dataSources: [] },
      lawId: 'participatiewet',
      engine: throwingEngine(thrown),
      ready: true,
    },
  });

describe('ScenarioForm execute errors', () => {
  it('reports the message of a thrown Error', () => {
    const data = mountForm(new Error('kapot')).vm.execute();
    expect(data?.error).toBe('kapot');
  });

  it('reports a thrown string as is', () => {
    const data = mountForm('kapot').vm.execute();
    expect(data?.error).toBe('kapot');
  });

  it.each([null, undefined, 42])('reports a thrown %s instead of crashing', (thrown) => {
    const w = mountForm(thrown);
    const data = w.vm.execute();
    expect(data?.error).toBe(String(thrown));
    expect(w.emitted('executed')).toHaveLength(1);
  });
});

describe('ScenarioForm execute territories', () => {
  it('gives the engine the territories of the scenario before it runs', () => {
    const engine = throwingEngine(new Error('kapot'));
    const territory = { code: 'GM9901', shape: 'POLYGON((0 0, 10 0, 10 10, 0 10, 0 0))' };
    const w = mount(ScenarioForm, {
      props: {
        scenario: { assertions: [], execution: { outputName: 'aantal', lawId: 'participatiewet' } },
        setup: { calculationDate: '2025-01-01', parameters: [], dataSources: [], territories: [territory] },
        lawId: 'participatiewet',
        engine,
        ready: true,
      },
    });
    w.vm.execute();
    // Cleared first, so a territory of an earlier scenario does not stay behind.
    expect(engine.clearTerritories).toHaveBeenCalledTimes(1);
    expect(engine.registerTerritory.mock.calls).toEqual([[territory.code, territory.shape]]);
    expect(engine.clearTerritories.mock.invocationCallOrder[0]).toBeLessThan(
      engine.registerTerritory.mock.invocationCallOrder[0]!,
    );
  });
});
