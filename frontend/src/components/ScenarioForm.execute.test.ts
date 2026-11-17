import { mount } from '@vue/test-utils';
import { describe, it, expect, vi } from 'vitest';
import ScenarioForm from './ScenarioForm.vue';

// An engine whose run throws `thrown`. The WASM boundary can hand back any
// JS value, so the form must not assume an Error.
const throwingEngine = (thrown: unknown) => ({
  clearDataSources: vi.fn(),
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
