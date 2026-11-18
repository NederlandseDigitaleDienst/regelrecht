import { describe, it, expect } from 'vitest';
import DataSourceTable from './DataSourceTable.vue';
import ScenarioParameterInput from './ScenarioParameterInput.vue';
import AbsenceToggle from './AbsenceToggle.vue';
import { NOT_NULLABLE_MESSAGE } from '../utils/nullability';
import type { FormParameter } from '../gherkin/formMapper';
import { mountScenarioForm, type ScenarioFormWrapper } from '../test-utils/scenarioForm';
import { valuesFromSetup } from '../scenario/inputs';

// RFC-036 / schema v0.5.8: a scenario parameter takes the word `null` (a
// stated absence) only when the law declares the parameter `nullable: true`.
// The declaration reaches the form through the typeMap built from the law's
// articles (buildTypeMap); data-source columns get theirs through the
// externalFieldTypeMap and are checked in DataSourceTable.

const insurance = () => [
  { sourceName: 'insurance', keyField: 'bsn', headers: ['bsn', 'huur', 'spaargeld'], rows: [['1', '', '']] },
];

type FieldMetaMap = Map<string, { type: string; unit: string | null; nullable?: boolean }>;

const typeMap: FieldMetaMap = new Map([
  ['huur', { type: 'string', unit: null, nullable: true }],
  ['inkomen', { type: 'string', unit: null, nullable: false }],
  ['spaargeld', { type: 'amount', unit: 'eurocent', nullable: true }],
  ['leeftijd', { type: 'number', unit: null, nullable: false }],
  ['heeft_partner', { type: 'boolean', unit: null, nullable: true }],
]);

const mountForm = (parameters: FormParameter[], externalFieldTypeMap: FieldMetaMap | null = null) =>
  mountScenarioForm({ parameters, dataSources: insurance(), typeMap, externalFieldTypeMap });

type FormWrapper = ScenarioFormWrapper;
// The parameter control named `name`; fails the test when there is none.
const inputFor = (w: FormWrapper, name: string) => {
  const input = w.findAllComponents(ScenarioParameterInput).find((c) => c.props('name') === name);
  if (!input) throw new Error(`no parameter input ${name}`);
  return input;
};
const errorTexts = (w: FormWrapper) => w.findAll('nldd-validation-item').map((e) => e.text());
const values = (w: FormWrapper) => w.values().parameterValues;

describe('ScenarioForm parameter nullability', () => {
  it('accepts null for a nullable parameter', async () => {
    const w = mountForm([{ name: 'huur', value: '500' }]);
    inputFor(w, 'huur').vm.$emit('update', 'null');
    await w.vm.$nextTick();
    expect(values(w).huur).toBe('null');
    expect(inputFor(w, 'huur').props('invalid')).toBe(false);
    expect(errorTexts(w)).toEqual([]);
    expect(w.emitted('update:values')).toHaveLength(1);
  });

  it('refuses null for a non-nullable parameter: blank stored, field invalid with the message', async () => {
    const w = mountForm([{ name: 'inkomen', value: '500' }]);
    inputFor(w, 'inkomen').vm.$emit('update', 'null');
    await w.vm.$nextTick();
    expect(values(w).inkomen).toBe('');
    const field = inputFor(w, 'inkomen');
    expect(field.props('invalid')).toBe(true);
    expect(field.props('unmet')).toBeTruthy();
    expect(errorTexts(w)).toEqual([NOT_NULLABLE_MESSAGE]);
    expect(w.find('nldd-validation-item').attributes('id')).toBe(field.props('unmet'));
    const listControl = w.find(`[id="${w.find('nldd-validation-list').attributes('for')}"]`);
    expect(listControl.attributes('unmet')).toBe(field.props('unmet'));

    // Typing on clears the refusal.
    field.vm.$emit('update', '600');
    await w.vm.$nextTick();
    expect(values(w).inkomen).toBe('600');
    expect(inputFor(w, 'inkomen').props('invalid')).toBe(false);
    expect(errorTexts(w)).toEqual([]);
  });

  it('accepts null for a parameter the law does not declare (no claim)', async () => {
    const w = mountForm([{ name: 'onbekend', value: '' }]);
    inputFor(w, 'onbekend').vm.$emit('update', 'null');
    await w.vm.$nextTick();
    expect(values(w).onbekend).toBe('null');
    expect(errorTexts(w)).toEqual([]);
  });

  it('marks a null read from the file for a non-nullable parameter, and keeps it', () => {
    const w = mountForm([{ name: 'inkomen', value: 'null' }]);
    expect(values(w).inkomen).toBe('null');
    expect(inputFor(w, 'inkomen').props('invalid')).toBe(true);
    expect(errorTexts(w)).toEqual([NOT_NULLABLE_MESSAGE]);
  });

  it('forgets a refusal when the edits are discarded', async () => {
    const parameters = [{ name: 'inkomen', value: '500' }];
    const w = mountForm(parameters);
    inputFor(w, 'inkomen').vm.$emit('update', 'null');
    await w.vm.$nextTick();
    expect(errorTexts(w)).toHaveLength(1);
    // A discard hands the form the saved values again.
    await w.replaceValues(valuesFromSetup({ calculationDate: '2025-01-01', dependencies: [], parameters, dataSources: insurance() }));
    expect(values(w).inkomen).toBe('500');
    expect(errorTexts(w)).toEqual([]);
  });

  // The "afwezig" checkbox (AbsenceToggle) is the explicit way to state an
  // absence for a parameter whose control cannot hold the word `null`
  // (number, amount, date, boolean). Offered only where the law declares the
  // parameter nullable.
  describe('the afwezig toggle', () => {
    // The toggle for `name`, or undefined: several tests assert it is absent.
    const toggleFor = (w: FormWrapper, name: string) => w.findAllComponents(AbsenceToggle).find((t) => t.attributes('data-testid') === `absent-${name}`);
    // The toggle for `name`; fails the test when there is none.
    const requireToggle = (w: FormWrapper, name: string) => {
      const t = toggleFor(w, name);
      if (!t) throw new Error(`no afwezig toggle for ${name}`);
      return t;
    };
    const check = (t: ReturnType<typeof toggleFor>, checked: boolean) => {
      if (!t) throw new Error('no afwezig toggle');
      return t.find('nldd-checkbox-field').element.dispatchEvent(new CustomEvent('change', { detail: { checked } }));
    };

    it('is offered for a nullable parameter of any type, not for a non-nullable or undeclared one', () => {
      const w = mountForm([
        { name: 'huur', value: '500' },
        { name: 'spaargeld', value: '1500' },
        { name: 'heeft_partner', value: 'true' },
        { name: 'inkomen', value: '500' },
        { name: 'leeftijd', value: '40' },
        { name: 'onbekend', value: 'x' },
      ]);
      expect(toggleFor(w, 'huur')).toBeDefined();
      expect(toggleFor(w, 'spaargeld')).toBeDefined();
      expect(toggleFor(w, 'heeft_partner')).toBeDefined();
      expect(toggleFor(w, 'inkomen')).toBeUndefined(); // nullable: false
      expect(toggleFor(w, 'leeftijd')).toBeUndefined(); // nullable: false
      expect(toggleFor(w, 'onbekend')).toBeUndefined(); // no declaration
      expect(toggleFor(w, 'Datum')).toBeUndefined(); // the calculation date is never absent
    });

    it('checking it stores null for an amount parameter, which then shows null as text', async () => {
      const w = mountForm([{ name: 'spaargeld', value: '1500' }]);
      expect(inputFor(w, 'spaargeld').find('nldd-number-field').exists()).toBe(true);
      check(toggleFor(w, 'spaargeld'), true);
      await w.vm.$nextTick();
      expect(values(w).spaargeld).toBe('null');
      const field = inputFor(w, 'spaargeld');
      expect(field.find('nldd-text-field').attributes('value')).toBe('null');
      expect(field.props('invalid')).toBe(false);
      expect(requireToggle(w, 'spaargeld').find('nldd-checkbox-field').attributes('checked')).toBeDefined();
      expect(errorTexts(w)).toEqual([]);
      expect(w.emitted('update:values')).toHaveLength(1);
    });

    it('checking it on a boolean parameter replaces the switch with the word null', async () => {
      const w = mountForm([{ name: 'heeft_partner', value: 'true' }]);
      expect(inputFor(w, 'heeft_partner').find('nldd-switch-field').exists()).toBe(true);
      check(toggleFor(w, 'heeft_partner'), true);
      await w.vm.$nextTick();
      expect(values(w).heeft_partner).toBe('null');
      expect(inputFor(w, 'heeft_partner').find('nldd-switch-field').exists()).toBe(false);
      expect(inputFor(w, 'heeft_partner').find('nldd-text-field').attributes('value')).toBe('null');
    });

    it('unchecking it stores a blank (unknown) and brings the typed control back', async () => {
      const w = mountForm([{ name: 'spaargeld', value: 'null' }]);
      expect(requireToggle(w, 'spaargeld').find('nldd-checkbox-field').attributes('checked')).toBeDefined();
      check(toggleFor(w, 'spaargeld'), false);
      await w.vm.$nextTick();
      expect(values(w).spaargeld).toBe('');
      expect(inputFor(w, 'spaargeld').find('nldd-number-field').exists()).toBe(true);
      expect(requireToggle(w, 'spaargeld').find('nldd-checkbox-field').attributes('checked')).toBeUndefined();
    });

    it('a null read from the file for a number parameter shows as the word null, not NaN', () => {
      const w = mountForm([{ name: 'leeftijd', value: 'null' }]);
      const field = inputFor(w, 'leeftijd');
      expect(field.find('nldd-number-field').exists()).toBe(false);
      expect(field.find('nldd-text-field').attributes('value')).toBe('null');
      // Non-nullable: flagged, kept, and no toggle to offer.
      expect(field.props('invalid')).toBe(true);
      expect(errorTexts(w)).toEqual([NOT_NULLABLE_MESSAGE]);
      expect(toggleFor(w, 'leeftijd')).toBeUndefined();
    });

    it('leaves the null out of the run only when unchecked, and passes it when checked', async () => {
      const w = mountForm([{ name: 'spaargeld', value: '1500' }]);
      check(toggleFor(w, 'spaargeld'), true);
      await w.vm.$nextTick();
      expect(values(w)).toEqual({ spaargeld: 'null' });
      check(toggleFor(w, 'spaargeld'), false);
      await w.vm.$nextTick();
      expect(values(w)).toEqual({ spaargeld: '' });
    });
  });

  it('passes each data-source column its declared nullability, unknown when the law never names it', async () => {
    const w = mountForm([], new Map([
      ['huur', { type: 'amount', unit: 'eurocent', nullable: true }],
      ['spaargeld', { type: 'amount', unit: 'eurocent', nullable: false }],
    ]));
    await w.find('[data-testid="ds-row-0"]').trigger('click');
    // The column named `name` of the drilled-in table; fails the test when there is none.
    const column = (name: string) => {
      const field = w.findComponent(DataSourceTable).props('fields').find((c) => c.name === name);
      if (!field) throw new Error(`no column ${name}`);
      return field;
    };
    expect(column('huur').nullable).toBe(true);
    expect(column('spaargeld').nullable).toBe(false);

    // Re-typing in place when the map arrives later carries nullability too.
    await w.setProps({ externalFieldTypeMap: new Map([['spaargeld', { type: 'amount', unit: 'eurocent', nullable: true }]]) });
    expect(column('spaargeld').nullable).toBe(true);
    expect(column('huur').nullable).toBeUndefined();
  });
});
