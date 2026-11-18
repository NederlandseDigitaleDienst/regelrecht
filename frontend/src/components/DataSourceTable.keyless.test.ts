import { mount } from '@vue/test-utils';
import { describe, it, expect } from 'vitest';
import DataSourceTable from './DataSourceTable.vue';
import ScenarioParameterInput from './ScenarioParameterInput.vue';

// The elements of a collection-valued parameter (RFC-016) edit in the same
// table as a data source, but a collection has no key column.
describe('DataSourceTable without a key field', () => {
  const mountKeyless = (rows: Record<string, unknown>[]) =>
    mount(DataSourceTable, {
      props: {
        title: 'medebewoners',
        keyField: null,
        fields: [{ name: 'leeftijd', type: 'string', unit: null }],
        modelValue: rows,
        drilledIn: true,
      },
    });

  it('renders only the declared columns', () => {
    const w = mountKeyless([{ _id: 1, leeftijd: '25' }]);
    const names = w.findAllComponents(ScenarioParameterInput).map((c) => c.props('name'));
    expect(names).toEqual(['leeftijd']);
    expect(w.html()).not.toContain('bsn');
  });

  it('adds a row without a key cell', async () => {
    const w = mountKeyless([]);
    await w.find('nldd-button[start-icon="plus-small"]').trigger('click');
    const rows = w.emitted('update:modelValue')?.at(-1)?.[0];
    expect(Array.isArray(rows)).toBe(true);
    const row = (rows as Record<string, unknown>[])[0];
    expect(row).toBeDefined();
    expect(Object.keys(row ?? {}).sort()).toEqual(['_id', 'leeftijd']);
  });
});
