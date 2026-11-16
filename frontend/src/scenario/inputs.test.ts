import { describe, it, expect } from 'vitest';
import { parseFeature } from '../gherkin/parser';
import { formStateToGherkin, mapFeatureToForm, type FormState } from '../gherkin/formMapper';
import { applyValues, engineInputs, initialValues, outputNameFor, valuesFromSetup, type ScenarioValues } from './inputs';

function feature(text: string): FormState {
  return mapFeatureToForm(parseFeature(text));
}

/** `value`, after asserting it is there. */
function defined<T>(value: T | null | undefined): T {
  expect(value).toBeDefined();
  expect(value).not.toBeNull();
  return value as T;
}

const BACKGROUND_FEATURE = `
Feature: Zorgtoeslag

  Background:
    Given the calculation date is "2025-01-01"
    Given parameter "inkomen" is "30000"

  Scenario: Met partner
    Given parameter "partner" is "true"
    Given the following "personen" data with key "bsn":
      | bsn       | leeftijd | naam  |
      | 999993653 | 30       |       |
    When I evaluate "heeft_recht" of "wet_op_de_zorgtoeslag"
    Then output "heeft_recht" is true

  Scenario: Zonder partner
    When I evaluate "heeft_recht" of "wet_op_de_zorgtoeslag"
    Then output "heeft_recht" is false
`;

describe('valuesFromSetup', () => {
  it('starts each form from the effective setup, background included', () => {
    const values = initialValues(feature(BACKGROUND_FEATURE));
    expect(values).toHaveLength(2);
    expect(values[0]!.calculationDate).toBe('2025-01-01');
    expect(values[0]!.parameterValues).toEqual({ inkomen: 30000, partner: true });
    expect(values[1]!.parameterValues).toEqual({ inkomen: 30000 });
    expect(values[0]!.dataSources).toEqual([
      {
        sourceName: 'personen',
        keyField: 'bsn',
        fields: [{ name: 'leeftijd' }, { name: 'naam' }],
        rows: [{ _id: 'init-0', bsn: '999993653', leeftijd: '30', naam: '' }],
      },
    ]);
  });

  it('shows a value the scenario overrides once, with the scenario value', () => {
    const values = valuesFromSetup({
      calculationDate: '2025-01-01',
      dependencies: [],
      parameters: [{ name: 'inkomen', value: 1 }, { name: 'inkomen', value: 2 }],
      dataSources: [
        { sourceName: 'p', keyField: 'k', headers: ['k'], rows: [['a']] },
        { sourceName: 'p', keyField: 'k', headers: ['k'], rows: [['b']] },
      ],
    });
    expect(values.parameterValues).toEqual({ inkomen: 2 });
    expect(values.dataSources).toHaveLength(1);
    expect(values.dataSources[0]!.rows[0]!.k).toBe('b');
  });
});

describe('engineInputs', () => {
  it('reads the saved feature when nothing was edited', () => {
    const state = feature(BACKGROUND_FEATURE);
    const inputs = defined(engineInputs(state, 0, null, 'folder_law'));
    expect(inputs).toEqual({
      lawId: 'wet_op_de_zorgtoeslag',
      outputName: 'heeft_recht',
      calculationDate: '2025-01-01',
      parameters: { inkomen: 30000, partner: true },
      // The blank `naam` cell is left out: the engine reports it as unknown.
      dataSources: [{ sourceName: 'personen', keyField: 'bsn', records: [{ bsn: 999993653, leeftijd: 30 }] }],
    });
  });

  it('runs exactly what a save of the same values would write', () => {
    const state = feature(BACKGROUND_FEATURE);
    const values = initialValues(state);
    const edited: ScenarioValues = {
      ...values[0]!,
      calculationDate: '2026-01-01',
      parameterValues: { ...values[0]!.parameterValues, inkomen: '45000', partner: '' },
    };

    const run = defined(engineInputs(state, 0, edited, 'folder_law'));
    // The saved text, read back as a fresh feature, gives the same inputs.
    const saved = feature(formStateToGherkin(applyValues(state, [edited, null])));
    const reread = defined(engineInputs(saved, 0, null, 'folder_law'));

    expect(run).toEqual(reread);
    expect(run.calculationDate).toBe('2026-01-01');
    expect(run.parameters).toEqual({ inkomen: 45000 });
  });

  it('keeps a background value whose field was blanked, as the saved file does', () => {
    const state = feature(BACKGROUND_FEATURE);
    const values = initialValues(state);
    const edited = { ...values[1]!, parameterValues: { inkomen: '' } };
    // A scenario has no step that un-passes a background parameter, so the
    // file still passes it, and so does the run.
    expect(defined(engineInputs(state, 1, edited, 'folder_law')).parameters).toEqual({ inkomen: 30000 });
  });

  it('passes a stated absence on as null', () => {
    const state = feature(BACKGROUND_FEATURE);
    const values = initialValues(state);
    const edited = { ...values[0]!, parameterValues: { inkomen: 30000, partner: 'null' } };
    expect(defined(engineInputs(state, 0, edited, 'folder_law')).parameters).toEqual({ inkomen: 30000, partner: null });
  });

  it('passes a collection whole, an empty one included', () => {
    const state = feature(`
Feature: Collecties

  Scenario: Huishouden
    Given parameter "medebewoners" is the collection:
      | leeftijd |
      | 30       |
    When I evaluate "heeft_recht" of "wet_a"
`);
    const values = initialValues(state);
    expect(defined(engineInputs(state, 0, null, 'folder_law')).parameters.medebewoners).toEqual([{ leeftijd: 30 }]);
    const emptied: ScenarioValues = {
      ...values[0]!,
      collections: [{ name: 'medebewoners', columns: [{ name: 'leeftijd' }], rows: [] }],
    };
    expect(defined(engineInputs(state, 0, emptied, 'folder_law')).parameters.medebewoners).toEqual([]);
    // RFC-036: a blank cell is no value stated; the record has no such key.
    const blankCell: ScenarioValues = {
      ...values[0]!,
      collections: [{ name: 'medebewoners', columns: [{ name: 'leeftijd' }], rows: [{ _id: 7, leeftijd: '' }] }],
    };
    expect(defined(engineInputs(state, 0, blankCell, 'folder_law')).parameters.medebewoners).toEqual([{}]);
  });

  it('names the regelwerk the scenario executes, not the folder it lives in', () => {
    const state = feature(BACKGROUND_FEATURE);
    expect(defined(engineInputs(state, 0, null, 'folder_law')).lawId).toBe('wet_op_de_zorgtoeslag');
  });

  it('is null for a scenario that does not exist', () => {
    expect(engineInputs(feature(BACKGROUND_FEATURE), 5, null, 'folder_law')).toBeNull();
  });

  it('does not touch the feature it reads', () => {
    const state = feature(BACKGROUND_FEATURE);
    const before = JSON.stringify(state);
    const values = initialValues(state);
    engineInputs(state, 0, { ...values[0]!, calculationDate: '2030-01-01' }, 'folder_law');
    expect(JSON.stringify(state)).toBe(before);
  });
});

describe('a data source registered for one regelwerk', () => {
  const FOR_LAW = `
Feature: Per regelwerk

  Scenario: Bron voor wet_b
    Given the calculation date is "2025-01-01"
    Given the following "personen" data with key "bsn" for law "wet_b":
      | bsn | leeftijd |
      | 1   | 30       |
    When I evaluate "x" of "wet_a"
`;

  it('keeps its regelwerk through the form values, in a run and in a save', () => {
    const state = feature(FOR_LAW);
    const values = initialValues(state);
    expect(values[0]!.dataSources[0]!.lawId).toBe('wet_b');

    const edited = { ...values[0]!, calculationDate: '2026-01-01' };
    expect(defined(engineInputs(state, 0, edited, 'wet_a')).dataSources[0]!.lawId).toBe('wet_b');

    const saved = formStateToGherkin(applyValues(state, [edited]));
    expect(saved).toContain('Given the following "personen" data with key "bsn" for law "wet_b":');
  });
});

describe('outputNameFor', () => {
  it('prefers the execution step, then the first assertion', () => {
    expect(outputNameFor({ execution: { outputName: 'a', lawId: 'l' }, assertions: [] })).toBe('a');
    expect(
      outputNameFor({ execution: null, assertions: [{ assertionType: 'boolean', outputName: 'b', value: true }] }),
    ).toBe('b');
    expect(outputNameFor({ execution: null, assertions: [] })).toBeNull();
  });
});
