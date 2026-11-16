import { describe, it, expect } from 'vitest';
import DataSourceTable from './DataSourceTable.vue';
import ScenarioParameterInput from './ScenarioParameterInput.vue';
import { mountScenarioForm } from '../test-utils/scenarioForm';
import type { FormParameter } from '../gherkin/formMapper';

// A collection-valued parameter (RFC-016) as formMapper hands it to the form:
// records plus the column list. What the engine receives for it is tested on
// the conversion (scenario/inputs.test.ts).
const parameters = (): FormParameter[] => [
  { name: 'leeftijd', value: 35 },
  { name: 'medebewoners', value: [{ leeftijd: 25 }, { leeftijd: 19 }], columns: ['leeftijd'] },
];

describe('ScenarioForm collection parameters', () => {
  it('shows a collection as a drill-in row, not as a scalar control', () => {
    const w = mountScenarioForm({ parameters: parameters() });
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
    const w = mountScenarioForm({
      parameters: [
        { name: 'medebewoners', value: [{ leeftijd: 25 }, { leeftijd: 19 }, { leeftijd: 50 }], columns: ['leeftijd'] },
        ...parameters(),
      ],
    });
    expect(w.findAll('[data-testid^="coll-row-"]')).toHaveLength(1);
    expect(w.values().collections.map((c) => c.rows.length)).toEqual([2]);
  });

  it('drills into a keyless table of the elements and reports the drill', async () => {
    const w = mountScenarioForm({ parameters: parameters() });
    await w.find('[data-testid="coll-row-0"]').trigger('click');
    expect(w.emitted('update:drill')?.at(-1)).toEqual([{ kind: 'collection', index: 0 }]);
    const table = w.findComponent(DataSourceTable);
    expect(table.exists()).toBe(true);
    expect(table.props('keyField')).toBeNull();
    expect(table.props('fields').map((f) => f.name)).toEqual(['leeftijd']);
    expect(table.props('modelValue')).toEqual([{ _id: 'init-0', leeftijd: 25 }, { _id: 'init-1', leeftijd: 19 }]);
  });

  it('hands an edited collection back as new values', async () => {
    const w = mountScenarioForm({ parameters: parameters() });
    await w.find('[data-testid="coll-row-0"]').trigger('click');
    w.findComponent(DataSourceTable).vm.$emit('update:modelValue', [{ _id: 0, leeftijd: '40' }]);
    await w.vm.$nextTick();
    expect(w.values().collections).toEqual([
      { name: 'medebewoners', columns: [{ name: 'leeftijd' }], rows: [{ _id: 0, leeftijd: '40' }] },
    ]);
    expect(w.values().parameterValues).toEqual({ leeftijd: 35 });
  });
});
