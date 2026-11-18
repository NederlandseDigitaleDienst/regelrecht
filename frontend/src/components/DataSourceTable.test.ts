import { mount } from '@vue/test-utils';
import type { VueWrapper } from '@vue/test-utils';
import { describe, it, expect } from 'vitest';
import DataSourceTable from './DataSourceTable.vue';
import ScenarioParameterInput from './ScenarioParameterInput.vue';
import AbsenceToggle from './AbsenceToggle.vue';

/** A column as the table's `fields` prop takes it. */
interface Field {
  name: string;
  type?: string;
  unit?: string | null;
  nullable?: boolean;
}
type Row = Record<string, unknown>;

const fields: Field[] = [
  { name: 'verdragsinschrijving', type: 'boolean', unit: null },
  { name: 'spaargeld', type: 'amount', unit: 'eurocent' },
  { name: 'geboortedatum', type: 'date', unit: null },
  { name: 'land_verblijf', type: 'string', unit: null },
];

const mountTable = (rows: Row[]) =>
  mount(DataSourceTable, {
    props: { title: 'box', keyField: 'bsn', fields, modelValue: rows, defaultExpanded: true, drilledIn: true },
  });

const findSpi = (w: VueWrapper, name: string) =>
  w.findAllComponents(ScenarioParameterInput).find((c) => c.props('name') === name);
// The ScenarioParameterInput of a column; throws when the column has none.
const spiFor = (w: VueWrapper, name: string) => {
  const spi = findSpi(w, name);
  if (!spi) throw new Error(`no ScenarioParameterInput for ${name}`);
  return spi;
};

// The rows of the last update:modelValue; throws when none was emitted.
const lastEmittedRows = (w: VueWrapper): Row[] => {
  const rows = w.emitted('update:modelValue')?.at(-1)?.[0];
  if (!Array.isArray(rows)) throw new Error('expected an update:modelValue emit');
  return rows as Row[];
};
// The first row of the last update:modelValue.
const lastEmittedRow = (w: VueWrapper): Row => {
  const row = lastEmittedRows(w)[0];
  if (!row) throw new Error('expected the emitted rows to hold a row');
  return row;
};

describe('DataSourceTable typed cells', () => {
  it('renders the control matching each column datatype', () => {
    const w = mountTable([
      { _id: 1, bsn: '1', verdragsinschrijving: 'false', spaargeld: '79547', geboortedatum: '2005-01-01', land_verblijf: 'NEDERLAND' },
    ]);
    // boolean -> kept tri-state dropdown (true/false/null)
    expect(w.find('select').exists()).toBe(true);
    expect(w.findAll('option').some((o) => o.attributes('value') === 'null')).toBe(true);
    // amount/date/string -> reused ScenarioParameterInput with correct type/unit
    expect(spiFor(w, 'spaargeld').props('type')).toBe('amount');
    expect(spiFor(w, 'spaargeld').props('unit')).toBe('eurocent');
    expect(spiFor(w, 'geboortedatum').props('type')).toBe('date');
    expect(spiFor(w, 'land_verblijf').props('type')).toBe('string');
    // boolean column is NOT routed through ScenarioParameterInput
    expect(findSpi(w, 'verdragsinschrijving')).toBeUndefined();
  });

  // RFC-036: a blank cell ("no value stated") and the word `null` (an
  // absence the author stated) are different cells and must stay apart.
  it('shows a null cell as the word null, in a text control whatever the column type', () => {
    const w = mountTable([
      { _id: 1, bsn: '1', verdragsinschrijving: 'null', spaargeld: 'null', geboortedatum: null, land_verblijf: 'null' },
    ]);
    // The column keeps its type; ScenarioParameterInput renders the null in
    // a text field regardless of it.
    expect(spiFor(w, 'spaargeld').props('value')).toBe('null');
    expect(spiFor(w, 'spaargeld').props('type')).toBe('amount');
    expect(spiFor(w, 'spaargeld').find('nldd-text-field').attributes('value')).toBe('null');
    expect(spiFor(w, 'spaargeld').find('nldd-number-field').exists()).toBe(false);
    expect(spiFor(w, 'geboortedatum').props('value')).toBe('null'); // a JS null from a typed record
    expect(spiFor(w, 'geboortedatum').find('nldd-text-field').attributes('value')).toBe('null');
    expect(spiFor(w, 'geboortedatum').find('nldd-date-field').exists()).toBe(false);
    expect(spiFor(w, 'land_verblijf').props('value')).toBe('null');
    expect(w.find('select').element.value).toBe('null');
  });

  it('shows a blank or absent cell as empty, in the control of its column type', () => {
    const w = mountTable([
      { _id: 1, bsn: '1', verdragsinschrijving: '', spaargeld: '', land_verblijf: 'NL' },
    ]);
    expect(spiFor(w, 'spaargeld').props('value')).toBe('');
    expect(spiFor(w, 'spaargeld').props('type')).toBe('amount');
    expect(spiFor(w, 'geboortedatum').props('value')).toBe(''); // key absent from the row
    expect(spiFor(w, 'geboortedatum').props('type')).toBe('date');
    expect(w.find('select').element.value).toBe('');
    expect(w.findAll('option').map((o) => o.attributes('value'))).toEqual(['true', 'false', 'null', '']);
  });

  it('clears a field to a blank cell, not to null', async () => {
    const w = mountTable([
      { _id: 1, bsn: '1', verdragsinschrijving: 'true', spaargeld: '79547', geboortedatum: '2005-01-01', land_verblijf: 'NL' },
    ]);
    spiFor(w, 'spaargeld').vm.$emit('update', '');
    await w.vm.$nextTick();
    expect(lastEmittedRow(w).spaargeld).toBe('');
    spiFor(w, 'land_verblijf').vm.$emit('update', null);
    await w.vm.$nextTick();
    expect(lastEmittedRow(w).land_verblijf).toBe('');
  });

  it('lets the author state an absence by typing null or picking it', async () => {
    const w = mountTable([
      { _id: 1, bsn: '1', verdragsinschrijving: 'true', spaargeld: '79547', geboortedatum: '2005-01-01', land_verblijf: 'NL' },
    ]);
    spiFor(w, 'land_verblijf').vm.$emit('update', 'null');
    await w.vm.$nextTick();
    expect(lastEmittedRow(w).land_verblijf).toBe('null');
    const select = w.find('select');
    select.element.value = 'null';
    await select.trigger('change');
    expect(lastEmittedRow(w).verdragsinschrijving).toBe('null');
    select.element.value = '';
    await select.trigger('change');
    expect(lastEmittedRow(w).verdragsinschrijving).toBe('');
  });
});

// RFC-036 / schema v0.5.8: `null` is a value of a field only where the law
// declares it `nullable: true`. The column carries that declaration; a
// column without one (collection elements, a column the law never names)
// makes no claim and behaves as before.
describe('DataSourceTable nullability', () => {
  const nullableFields: Field[] = [
    { name: 'verdragsinschrijving', type: 'boolean', unit: null, nullable: true },
    { name: 'land_verblijf', type: 'string', unit: null, nullable: true },
  ];
  const strictFields: Field[] = [
    { name: 'verdragsinschrijving', type: 'boolean', unit: null, nullable: false },
    { name: 'land_verblijf', type: 'string', unit: null, nullable: false },
  ];
  const mountWith = (fields: Field[], rows: Row[]) =>
    mount(DataSourceTable, {
      props: { title: 'box', keyField: 'bsn', fields, modelValue: rows, defaultExpanded: true, drilledIn: true },
    });
  const optionValues = (w: VueWrapper) => w.findAll('option').map((o) => o.attributes('value'));
  const errorTexts = (w: VueWrapper) => w.findAll('nldd-validation-item').map((e) => e.text());

  it('offers the null option in a boolean column only when the field is nullable', () => {
    const nullable = mountWith(nullableFields, [{ _id: 1, bsn: '1', verdragsinschrijving: 'true', land_verblijf: 'NL' }]);
    expect(optionValues(nullable)).toEqual(['true', 'false', 'null', '']);
    const strict = mountWith(strictFields, [{ _id: 1, bsn: '1', verdragsinschrijving: 'true', land_verblijf: 'NL' }]);
    expect(optionValues(strict)).toEqual(['true', 'false', '']); // (leeg) stays
    expect(errorTexts(strict)).toEqual([]);
  });

  it('accepts a typed null into a nullable text column', async () => {
    const w = mountWith(nullableFields, [{ _id: 1, bsn: '1', verdragsinschrijving: 'true', land_verblijf: 'NL' }]);
    spiFor(w, 'land_verblijf').vm.$emit('update', 'null');
    await w.vm.$nextTick();
    expect(lastEmittedRow(w).land_verblijf).toBe('null');
    expect(spiFor(w, 'land_verblijf').props('invalid')).toBe(false);
    expect(errorTexts(w)).toEqual([]);
  });

  it('refuses a typed null in a non-nullable text column: blank in the record, field invalid with the message', async () => {
    const w = mountWith(strictFields, [{ _id: 1, bsn: '1', verdragsinschrijving: 'true', land_verblijf: 'NL' }]);
    spiFor(w, 'land_verblijf').vm.$emit('update', 'null');
    await w.vm.$nextTick();
    expect(lastEmittedRow(w).land_verblijf).toBe('');
    await w.setProps({ modelValue: lastEmittedRows(w) });
    const field = spiFor(w, 'land_verblijf');
    expect(field.props('invalid')).toBe(true);
    expect(field.props('unmet')).toBeTruthy();
    expect(errorTexts(w)).toEqual(['Dit gegeven kan niet afwezig zijn (niet nullable)']);
    expect(w.find('nldd-validation-item').attributes('id')).toBe(field.props('unmet'));
    const listControl = w.find(`[id="${w.find('nldd-validation-list').attributes('for')}"]`);
    expect(listControl.attributes('unmet')).toBe(field.props('unmet'));

    // Typing on clears the refusal.
    field.vm.$emit('update', 'nul');
    await w.vm.$nextTick();
    expect(lastEmittedRow(w).land_verblijf).toBe('nul');
    await w.setProps({ modelValue: lastEmittedRows(w) });
    expect(spiFor(w, 'land_verblijf').props('invalid')).toBe(false);
    expect(errorTexts(w)).toEqual([]);
  });

  it('still accepts null in a column without a known declaration', async () => {
    const w = mountWith([{ name: 'land_verblijf', type: 'string', unit: null }], [{ _id: 1, bsn: '1', land_verblijf: 'NL' }]);
    spiFor(w, 'land_verblijf').vm.$emit('update', 'null');
    await w.vm.$nextTick();
    expect(lastEmittedRow(w).land_verblijf).toBe('null');
    expect(errorTexts(w)).toEqual([]);
  });

  it('marks a null already in a non-nullable column as invalid but keeps the value', () => {
    // A null read from the feature file, or a column re-typed as non-nullable
    // once the law's declaration arrived: shown, flagged, not altered.
    const w = mountWith(strictFields, [{ _id: 1, bsn: '1', verdragsinschrijving: 'null', land_verblijf: 'null' }]);
    const field = spiFor(w, 'land_verblijf');
    expect(field.props('value')).toBe('null');
    expect(field.props('invalid')).toBe(true);
    // The boolean dropdown keeps the null option while the cell holds one, so
    // the state is visible, and marks the dropdown invalid.
    expect(optionValues(w)).toEqual(['true', 'false', 'null', '']);
    expect(w.find('select').element.value).toBe('null');
    expect(w.find('nldd-dropdown').attributes('invalid')).toBeDefined();
    expect(errorTexts(w)).toHaveLength(2);
    expect(w.emitted('update:modelValue')).toBeUndefined();
  });

  // The "afwezig" checkbox (AbsenceToggle) is the explicit way to state an
  // absence in a cell whose control cannot hold the word `null` (number,
  // amount, date, boolean). Offered only where the law declares the column
  // nullable; a non-nullable or undeclared column gets no control.
  describe('the afwezig toggle', () => {
    const typedNullable: Field[] = [
      { name: 'verdragsinschrijving', type: 'boolean', unit: null, nullable: true },
      { name: 'spaargeld', type: 'amount', unit: 'eurocent', nullable: true },
      { name: 'leeftijd', type: 'number', unit: null, nullable: true },
      { name: 'geboortedatum', type: 'date', unit: null, nullable: true },
      { name: 'inkomen', type: 'amount', unit: 'eurocent', nullable: false },
      { name: 'land_verblijf', type: 'string', unit: null },
    ];
    const row: Row = { _id: 1, bsn: '1', verdragsinschrijving: 'true', spaargeld: '79547', leeftijd: '40', geboortedatum: '2005-01-01', inkomen: '100', land_verblijf: 'NL' };
    const findToggle = (w: VueWrapper, name: string) => w.findAllComponents(AbsenceToggle).find((t) => t.attributes('data-testid') === `absent-${name}`);
    // The toggle of a column; throws when the column has none.
    const toggleFor = (w: VueWrapper, name: string) => {
      const toggle = findToggle(w, name);
      if (!toggle) throw new Error(`no AbsenceToggle for ${name}`);
      return toggle;
    };
    const check = (t: VueWrapper, checked: boolean) => t.find('nldd-checkbox-field').element.dispatchEvent(new CustomEvent('change', { detail: { checked } }));
    const lastRow = (w: VueWrapper) => lastEmittedRow(w);

    it('is offered for every nullable column whatever its type, and for no other', () => {
      const w = mountWith(typedNullable, [row]);
      for (const name of ['verdragsinschrijving', 'spaargeld', 'leeftijd', 'geboortedatum']) {
        expect(findToggle(w, name), name).toBeDefined();
        expect(toggleFor(w, name).props('value')).toBe(row[name]);
      }
      expect(findToggle(w, 'inkomen')).toBeUndefined(); // nullable: false
      expect(findToggle(w, 'land_verblijf')).toBeUndefined(); // no declaration
      expect(findToggle(w, 'bsn')).toBeUndefined(); // key column
    });

    it('is not offered in a read-only table', () => {
      const w = mount(DataSourceTable, {
        props: { title: 'box', keyField: 'bsn', fields: typedNullable, modelValue: [row], defaultExpanded: true, drilledIn: true, readonly: true },
      });
      expect(w.findAllComponents(AbsenceToggle)).toHaveLength(0);
    });

    it('checking it stores the word null in a number cell, which then shows null as text', async () => {
      const w = mountWith(typedNullable, [row]);
      check(toggleFor(w, 'spaargeld'), true);
      await w.vm.$nextTick();
      expect(lastRow(w).spaargeld).toBe('null');
      expect(lastRow(w).leeftijd).toBe('40'); // the other cells are untouched
      await w.setProps({ modelValue: lastEmittedRows(w) });
      const field = spiFor(w, 'spaargeld');
      expect(field.props('value')).toBe('null');
      expect(field.find('nldd-text-field').attributes('value')).toBe('null');
      expect(field.props('invalid')).toBe(false);
      expect(toggleFor(w, 'spaargeld').find('nldd-checkbox-field').attributes('checked')).toBeDefined();
      expect(errorTexts(w)).toEqual([]);
    });

    it('unchecking it stores a blank (unknown), not the old value', async () => {
      const w = mountWith(typedNullable, [{ ...row, leeftijd: 'null' }]);
      expect(toggleFor(w, 'leeftijd').find('nldd-checkbox-field').attributes('checked')).toBeDefined();
      check(toggleFor(w, 'leeftijd'), false);
      await w.vm.$nextTick();
      expect(lastRow(w).leeftijd).toBe('');
      await w.setProps({ modelValue: lastEmittedRows(w) });
      expect(spiFor(w, 'leeftijd').find('nldd-number-field').exists()).toBe(true);
      expect(toggleFor(w, 'leeftijd').find('nldd-checkbox-field').attributes('checked')).toBeUndefined();
    });

    it('works on a boolean column too, in step with the dropdown', async () => {
      const w = mountWith(typedNullable, [row]);
      check(toggleFor(w, 'verdragsinschrijving'), true);
      await w.vm.$nextTick();
      expect(lastRow(w).verdragsinschrijving).toBe('null');
      await w.setProps({ modelValue: lastEmittedRows(w) });
      expect(w.find('select').element.value).toBe('null');
      // And the dropdown's own null option keeps the checkbox in step.
      const select = w.find('select');
      select.element.value = 'false';
      await select.trigger('change');
      await w.setProps({ modelValue: lastEmittedRows(w) });
      expect(toggleFor(w, 'verdragsinschrijving').find('nldd-checkbox-field').attributes('checked')).toBeUndefined();
    });

    it('typing over the null text clears the toggle', async () => {
      const w = mountWith(typedNullable, [{ ...row, geboortedatum: 'null' }]);
      spiFor(w, 'geboortedatum').vm.$emit('update', '2005-01-01');
      await w.vm.$nextTick();
      expect(lastRow(w).geboortedatum).toBe('2005-01-01');
      await w.setProps({ modelValue: lastEmittedRows(w) });
      expect(toggleFor(w, 'geboortedatum').find('nldd-checkbox-field').attributes('checked')).toBeUndefined();
      expect(spiFor(w, 'geboortedatum').find('nldd-date-field').exists()).toBe(true);
    });
  });

  it('drops the invalid mark once the boolean cell is set to a value', async () => {
    const w = mountWith(strictFields, [{ _id: 1, bsn: '1', verdragsinschrijving: 'null', land_verblijf: 'NL' }]);
    const select = w.find('select');
    select.element.value = 'false';
    await select.trigger('change');
    await w.setProps({ modelValue: lastEmittedRows(w) });
    expect(optionValues(w)).toEqual(['true', 'false', '']);
    expect(w.find('nldd-dropdown').attributes('invalid')).toBeUndefined();
    expect(errorTexts(w)).toEqual([]);
  });
});
