import { mount } from '@vue/test-utils';
import { describe, it, expect, vi } from 'vitest';
import ScenarioForm from './ScenarioForm.vue';
import DataSourceTable from './DataSourceTable.vue';
import ScenarioParameterInput from './ScenarioParameterInput.vue';
import type { EditorEngine } from '../composables/useEngine';

// A collection-valued parameter (RFC-016) as formMapper hands it to the form:
// records plus the column list.
const setup = () => ({
  calculationDate: '2025-01-01',
  parameters: [
    { name: 'leeftijd', value: 35 },
    { name: 'medebewoners', value: [{ leeftijd: 25 }, { leeftijd: 19 }], columns: ['leeftijd'] },
  ],
  dataSources: [],
});

const fakeEngine = () => ({
  clearDataSources: vi.fn(),
  registerDataSource: vi.fn(),
  executeWithTrace: vi.fn<EditorEngine['executeWithTrace']>(() => ({ outputs: { aantal: 2 }, trace_text: '' })),
});
type FakeEngine = ReturnType<typeof fakeEngine>;

// The parameters of the engine's executeWithTrace call at `index` (default: the last).
function paramsOf(engine: FakeEngine, index = -1): Record<string, unknown> {
  const call = engine.executeWithTrace.mock.calls.at(index);
  if (!call) throw new Error('executeWithTrace was not called');
  return call[2];
}

const mountForm = (engine: FakeEngine | null = null) =>
  mount(ScenarioForm, {
    props: {
      scenario: { assertions: [], execution: { outputName: 'aantal', lawId: 'participatiewet' } },
      setup: setup(),
      lawId: 'participatiewet',
      engine,
      ready: !!engine,
    },
  });

describe('ScenarioForm collection parameters', () => {
  it('shows a collection as a drill-in row, not as a scalar control', () => {
    const w = mountForm();
    const scalarNames = w.findAllComponents(ScenarioParameterInput).map((c) => c.props('name'));
    expect(scalarNames).toContain('leeftijd');
    expect(scalarNames).not.toContain('medebewoners');
    const row = w.find('[data-testid="coll-row-0"]');
    expect(row.exists()).toBe(true);
    expect(row.html()).toContain('medebewoners');
    expect(row.html()).toContain('text="2"');
  });

  it('shows a background collection overridden by the scenario once, with the scenario rows', () => {
    // getEffectiveSetup() concatenates background and scenario parameters,
    // so an override lists the same name twice; the form shows the last.
    const merged = setup();
    merged.parameters = [
      { name: 'medebewoners', value: [{ leeftijd: 25 }, { leeftijd: 19 }, { leeftijd: 50 }], columns: ['leeftijd'] },
      ...merged.parameters,
    ];
    const w = mount(ScenarioForm, {
      props: { scenario: { assertions: [] }, setup: merged, lawId: 'participatiewet' },
    });
    expect(w.findAll('[data-testid^="coll-row-"]')).toHaveLength(1);
    expect(w.vm.getFormValues().collections.map((c) => c.rows.length)).toEqual([2]);
  });

  it('drills into a keyless table of the elements and reports the drill', async () => {
    const w = mountForm();
    await w.find('[data-testid="coll-row-0"]').trigger('click');
    const table = w.findComponent(DataSourceTable);
    expect(table.exists()).toBe(true);
    expect(table.props('keyField')).toBeNull();
    expect(table.props('fields').map((f) => f.name)).toEqual(['leeftijd']);
    expect(table.props('modelValue')).toEqual([{ _id: 'init-0', leeftijd: 25 }, { _id: 'init-1', leeftijd: 19 }]);
    expect(w.emitted('drill-change')?.at(-1)).toEqual(['medebewoners']);
  });

  it('passes the collection to the engine as an array of typed records', () => {
    const engine = fakeEngine();
    const w = mountForm(engine);
    w.vm.execute();
    const params = paramsOf(engine, 0);
    expect(params.leeftijd).toBe(35);
    expect(params.medebewoners).toEqual([{ leeftijd: 25 }, { leeftijd: 19 }]);
  });

  // RFC-036: a blank cell is no value stated; the record has no such key, the
  // same as the runner reads back from the saved table.
  it('leaves an untouched new cell out of the record, as the saved table does', async () => {
    const engine = fakeEngine();
    const w = mountForm(engine);
    await w.find('[data-testid="coll-row-0"]').trigger('click');
    w.findComponent(DataSourceTable).vm.$emit('update:modelValue', [{ _id: 7, leeftijd: '' }]);
    await w.vm.$nextTick();
    w.vm.execute();
    const medebewoners = paramsOf(engine).medebewoners as Record<string, unknown>[];
    expect(medebewoners).toEqual([{}]);
    // One element, asserted on the line above.
    expect(Object.hasOwn(medebewoners[0]!, 'leeftijd')).toBe(false);
  });

  it('passes an emptied collection as an empty array, not as a missing input', async () => {
    const engine = fakeEngine();
    const w = mountForm(engine);
    await w.find('[data-testid="coll-row-0"]').trigger('click');
    w.findComponent(DataSourceTable).vm.$emit('update:modelValue', []);
    await w.vm.$nextTick();
    w.vm.execute();
    const params = paramsOf(engine);
    expect(params.medebewoners).toEqual([]);
  });

  it('hands the edited collection back through getFormValues', async () => {
    const w = mountForm();
    await w.find('[data-testid="coll-row-0"]').trigger('click');
    w.findComponent(DataSourceTable).vm.$emit('update:modelValue', [{ _id: 0, leeftijd: '40' }]);
    await w.vm.$nextTick();
    const values = w.vm.getFormValues();
    expect(values.collections).toEqual([
      { name: 'medebewoners', columns: [{ name: 'leeftijd', type: 'string', unit: null }], rows: [{ _id: 0, leeftijd: '40' }] },
    ]);
    expect(values.parameterValues).toEqual({ leeftijd: 35 });
    expect(w.emitted('change')).toBeTruthy();
  });
});
