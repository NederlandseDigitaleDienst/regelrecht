import { describe, it, expect } from 'vitest';
import DataSourceTable from './DataSourceTable.vue';
import { mountScenarioForm, type ScenarioFormWrapper } from '../test-utils/scenarioForm';

type FieldMetaMap = Map<string, { type: string; unit: string | null; nullable?: boolean }>;

const mountForm = (externalFieldTypeMap: FieldMetaMap) =>
  mountScenarioForm({
    dataSources: [
      { sourceName: 'insurance', keyField: 'bsn', headers: ['bsn', 'verdragsinschrijving', 'onbekend'], rows: [['1', 'false', 'x']] },
    ],
    externalFieldTypeMap,
  });

// The editable DataSourceTable only renders once a source is drilled into.
const drillIn = (w: ScenarioFormWrapper) => w.find('[data-testid="ds-row-0"]').trigger('click');
const fieldsOf = (w: ScenarioFormWrapper) => w.findComponent(DataSourceTable).props('fields');
// The column named `name`; fails the test when there is none.
const fieldNamed = (w: ScenarioFormWrapper, name: string) => {
  const field = fieldsOf(w).find((c) => c.name === name);
  if (!field) throw new Error(`no column ${name}`);
  return field;
};

describe('ScenarioForm data-source column typing', () => {
  it('types data-source columns from the map (fallback string, key excluded)', async () => {
    const w = mountForm(new Map([['verdragsinschrijving', { type: 'boolean', unit: null }]]));
    await drillIn(w);
    expect(fieldNamed(w, 'verdragsinschrijving').type).toBe('boolean');
    expect(fieldNamed(w, 'onbekend').type).toBe('string'); // not in map -> fallback
    expect(fieldsOf(w).some((c) => c.name === 'bsn')).toBe(false); // key field excluded
  });

  it('re-types columns when the map arrives later, without touching the rows', async () => {
    const w = mountForm(new Map()); // empty at mount -> all string
    await drillIn(w);
    expect(fieldNamed(w, 'verdragsinschrijving').type).toBe('string');
    const before = w.values();
    await w.setProps({ externalFieldTypeMap: new Map([['verdragsinschrijving', { type: 'boolean', unit: null }]]) });
    expect(fieldNamed(w, 'verdragsinschrijving').type).toBe('boolean');
    // Typing is the form's view: the values (the edit buffer) stay as they were.
    expect(w.values()).toBe(before);
  });

  it('hands edited rows back as new values', async () => {
    const w = mountForm(new Map());
    await drillIn(w);
    w.findComponent(DataSourceTable).vm.$emit('update:modelValue', [{ _id: 'init-0', bsn: '2', verdragsinschrijving: 'true', onbekend: '' }]);
    await w.vm.$nextTick();
    expect(w.values().dataSources[0]!.rows).toEqual([{ _id: 'init-0', bsn: '2', verdragsinschrijving: 'true', onbekend: '' }]);
    expect(w.values().dataSources[0]!.fields).toEqual([{ name: 'verdragsinschrijving' }, { name: 'onbekend' }]);
  });
});
