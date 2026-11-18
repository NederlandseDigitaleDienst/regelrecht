import { describe, it, expect } from 'vitest';
import { parseFeature } from './parser';
import { mapFeatureToForm, getEffectiveSetup, formStateToGherkin, syncEditedValues } from './formMapper';
import { parseValue } from './steps';

/** `value`, after asserting it is there: a missing value fails the test at this point. */
function defined<T>(value: T | null | undefined): T {
  expect(value).toBeDefined();
  expect(value).not.toBeNull();
  return value as T;
}

describe('mapFeatureToForm', () => {
  it('maps a simple scenario with date, evaluate, and assertions', () => {
    const parsed = parseFeature(`
Feature: Simple test

  Scenario: Basic check
    Given the calculation date is "2025-01-01"
    When I evaluate "result" of "my_law"
    Then the execution succeeds
    Then output "result" is true
`);

    const form = mapFeatureToForm(parsed);
    expect(form.featureName).toBe('Simple test');
    expect(form.background).toBeNull();
    expect(form.scenarios).toHaveLength(1);

    const s = defined(form.scenarios[0]);
    expect(s.name).toBe('Basic check');
    expect(s.setup.calculationDate).toBe('2025-01-01');
    expect(s.execution).toEqual({ outputName: 'result', lawId: 'my_law' });
    expect(s.assertions).toHaveLength(2);
    expect(s.assertions[0]).toEqual({ assertionType: 'succeeds', outputName: null, value: null });
    expect(s.assertions[1]).toEqual({ assertionType: 'boolean', outputName: 'result', value: true });
    expect(s.unmatchedSteps).toHaveLength(0);
  });

  it('maps background with dependencies', () => {
    const parsed = parseFeature(`
Feature: With background

  Background:
    Given the calculation date is "2025-01-01"
    Given law "law_a" is loaded
    Given law "law_b" is loaded

  Scenario: Test
    Given parameter "bsn" is "999993653"
    When I evaluate "output" of "law_a"
    Then output "output" is false
`);

    const form = mapFeatureToForm(parsed);
    expect(form.background).not.toBeNull();
    expect(defined(form.background).calculationDate).toBe('2025-01-01');
    expect(defined(form.background).dependencies).toEqual(['law_a', 'law_b']);

    const s = defined(form.scenarios[0]);
    expect(s.setup.parameters).toEqual([{ name: 'bsn', value: 999993653 }]);
    expect(s.assertions[0]).toEqual({ assertionType: 'boolean', outputName: 'output', value: false });
  });

  it('maps data sources with headers and rows', () => {
    const parsed = parseFeature(`
Feature: Data sources

  Scenario: With data
    Given the calculation date is "2025-01-01"
    Given the following "personal_data" data with key "bsn":
      | bsn       | name  | age |
      | 999993653 | Alice | 30  |
      | 999993654 | Bob   | 25  |
    When I evaluate "result" of "law_x"
`);

    const form = mapFeatureToForm(parsed);
    const ds = defined(defined(form.scenarios[0]).setup.dataSources[0]);
    expect(ds.sourceName).toBe('personal_data');
    expect(ds.keyField).toBe('bsn');
    expect(ds.headers).toEqual(['bsn', 'name', 'age']);
    expect(ds.rows).toHaveLength(2);
    expect(ds.rows[0]).toEqual(['999993653', 'Alice', '30']);
  });

  it('maps all assertion types', () => {
    const parsed = parseFeature(`
Feature: Assertions

  Scenario: All types
    Given the calculation date is "2025-01-01"
    When I evaluate "x" of "law"
    Then the execution succeeds
    Then the execution fails
    Then the execution fails with "some error"
    Then output "a" is true
    Then output "b" is false
    Then output "c" equals 42
    Then output "d" equals "hello"
    Then output "e" is absent
    Then output "f" contains "sub"
`);

    const form = mapFeatureToForm(parsed);
    const a = defined(form.scenarios[0]).assertions;
    expect(a).toHaveLength(9);
    expect(defined(a[0]).assertionType).toBe('succeeds');
    expect(defined(a[1]).assertionType).toBe('fails');
    expect(a[2]).toEqual({ assertionType: 'failsWith', outputName: null, value: 'some error' });
    expect(a[3]).toEqual({ assertionType: 'boolean', outputName: 'a', value: true });
    expect(a[4]).toEqual({ assertionType: 'boolean', outputName: 'b', value: false });
    expect(a[5]).toEqual({ assertionType: 'equals', outputName: 'c', value: 42 });
    expect(a[6]).toEqual({ assertionType: 'equalsString', outputName: 'd', value: 'hello' });
    expect(a[7]).toEqual({ assertionType: 'null', outputName: 'e', value: null });
    expect(a[8]).toEqual({ assertionType: 'contains', outputName: 'f', value: 'sub' });
  });

  it('maps numeric and string parameters', () => {
    const parsed = parseFeature(`
Feature: Params

  Scenario: Mixed params
    Given the calculation date is "2025-01-01"
    Given parameter "bsn" is "999993653"
    Given parameter "amount" is 1500
    Given parameter "rate" is 3.14
    When I evaluate "x" of "law"
`);

    const form = mapFeatureToForm(parsed);
    const params = defined(form.scenarios[0]).setup.parameters;
    expect(params).toEqual([
      { name: 'bsn', value: 999993653 },
      { name: 'amount', value: 1500 },
      { name: 'rate', value: 3.14 },
    ]);
  });

  // A parameter table has no header row, so its first row is a parameter too.
  // Shape taken verbatim from
  // corpus/regulation/nl/wet/burgerlijk_wetboek_boek_5/scenarios/erfgrensbeplanting.feature.
  it('maps parameter table', () => {
    const parsed = parseFeature(`
Feature: Param table

  Scenario: Bulk params
    Given the calculation date is "2025-01-01"
    Given the following parameters:
      | gemeente_code   | GM0363 |
      | type_beplanting | boom   |
      | postcode        | 1012   |
    When I evaluate "minimale_afstand_cm" of "burgerlijk_wetboek_boek_5"
`);

    const form = mapFeatureToForm(parsed);
    const params = defined(form.scenarios[0]).setup.parameters;
    expect(params).toEqual([
      { name: 'gemeente_code', value: 'GM0363' },
      { name: 'type_beplanting', value: 'boom' },
      { name: 'postcode', value: 1012 },
    ]);
  });

  // RFC-036: an empty value cell is a parameter that is not passed. The runners
  // leave it out, so the form does too; the word null is an absence and stays.
  it('leaves a parameter with an empty cell out of the form', () => {
    const parsed = parseFeature(`
Feature: Blank cell

  Scenario: Not filled in
    Given the following parameters:
      | bsn             | 999993653 |
      | aanvraag_bedrag |           |
      | partner_bsn     | null      |
    When I evaluate "past_aanvraag" of "test_null_semantics_bron"
`);

    const form = mapFeatureToForm(parsed);
    expect(defined(form.scenarios[0]).setup.parameters).toEqual([
      { name: 'bsn', value: 999993653 },
      { name: 'partner_bsn', value: null },
    ]);
    const gherkin = formStateToGherkin(form);
    expect(gherkin).not.toContain('aanvraag_bedrag');
    expect(gherkin).toContain('Given parameter "partner_bsn" is "null"');
  });

  it('keeps a single-row parameter table', () => {
    const parsed = parseFeature(`
Feature: One param

  Scenario: Single row
    Given the following parameters:
      | indieningsdatum | 2025-01-01 |
    When I evaluate "tijdig_ingediend" of "test_date_operations"
`);

    const form = mapFeatureToForm(parsed);
    expect(defined(form.scenarios[0]).setup.parameters).toEqual([
      { name: 'indieningsdatum', value: '2025-01-01' },
    ]);
  });

  // Round-trip guard: saving from the visual form rewrites the table as
  // individual `Given parameter ...` steps, so a row lost on read is lost
  // in the .feature file for good.
  it('keeps every parameter through a table load/save round-trip', () => {
    const parsed = parseFeature(`
Feature: Round trip

  Scenario: Bulk params
    Given the following parameters:
      | gemeente_code   | GM0363 |
      | type_beplanting | boom   |
    When I evaluate "minimale_afstand_cm" of "burgerlijk_wetboek_boek_5"
`);

    const gherkin = formStateToGherkin(mapFeatureToForm(parsed));
    expect(gherkin).toContain('Given parameter "gemeente_code" is "GM0363"');
    expect(gherkin).toContain('Given parameter "type_beplanting" is "boom"');
  });

  it('puts unrecognized steps in unmatchedSteps', () => {
    const parsed = parseFeature(`
Feature: Custom steps

  Scenario: With custom
    Given the calculation date is "2025-01-01"
    When the healthcare allowance law is executed
    Then the execution succeeds
`);

    const form = mapFeatureToForm(parsed);
    const s = defined(form.scenarios[0]);
    expect(s.execution).toBeNull();
    expect(s.unmatchedSteps).toHaveLength(1);
    expect(defined(s.unmatchedSteps[0]).keyword).toBe('When');
    expect(defined(s.unmatchedSteps[0]).text).toBe('the healthcare allowance law is executed');
  });

  it('handles multiple scenarios', () => {
    const parsed = parseFeature(`
Feature: Multi scenario

  Background:
    Given the calculation date is "2025-01-01"

  Scenario: First
    When I evaluate "a" of "law"
    Then output "a" is true

  Scenario: Second
    When I evaluate "b" of "law"
    Then output "b" is false
`);

    const form = mapFeatureToForm(parsed);
    expect(form.scenarios).toHaveLength(2);
    expect(defined(form.scenarios[0]).name).toBe('First');
    expect(defined(form.scenarios[1]).name).toBe('Second');
  });
});

describe('getEffectiveSetup', () => {
  it('merges background and scenario setup', () => {
    const parsed = parseFeature(`
Feature: Merge test

  Background:
    Given the calculation date is "2025-01-01"
    Given law "dep_a" is loaded

  Scenario: Test
    Given law "dep_b" is loaded
    Given parameter "bsn" is "123"
    When I evaluate "x" of "law"
`);

    const form = mapFeatureToForm(parsed);
    const effective = defined(getEffectiveSetup(form, 0));

    expect(effective.calculationDate).toBe('2025-01-01');
    expect(effective.dependencies).toEqual(['dep_a', 'dep_b']);
    expect(effective.parameters).toEqual([{ name: 'bsn', value: 123 }]);
  });

  it('scenario date overrides background date', () => {
    const parsed = parseFeature(`
Feature: Date override

  Background:
    Given the calculation date is "2025-01-01"

  Scenario: Override
    Given the calculation date is "2026-06-15"
    When I evaluate "x" of "law"
`);

    const form = mapFeatureToForm(parsed);
    const effective = defined(getEffectiveSetup(form, 0));
    expect(effective.calculationDate).toBe('2026-06-15');
  });
});

describe('formStateToGherkin round-trip', () => {
  it('round-trips a simple scenario', () => {
    const original = `Feature: Round trip

  Scenario: Test
    Given the calculation date is "2025-01-01"
    Given parameter "bsn" is "999993653"
    When I evaluate "result" of "my_law"
    Then the execution succeeds
    Then output "result" is true
`;

    const parsed = parseFeature(original);
    const form = mapFeatureToForm(parsed);
    const regenerated = formStateToGherkin(form);
    const reparsed = parseFeature(regenerated);
    const reformatted = mapFeatureToForm(reparsed);

    expect(reformatted.featureName).toBe(form.featureName);
    expect(defined(reformatted.scenarios[0]).setup.calculationDate).toBe(defined(form.scenarios[0]).setup.calculationDate);
    expect(defined(reformatted.scenarios[0]).setup.parameters).toEqual(defined(form.scenarios[0]).setup.parameters);
    expect(defined(reformatted.scenarios[0]).execution).toEqual(defined(form.scenarios[0]).execution);
    expect(defined(reformatted.scenarios[0]).assertions).toEqual(defined(form.scenarios[0]).assertions);
  });

  it('round-trips a feature with background and data sources', () => {
    const original = `Feature: Complex round trip

  Background:
    Given the calculation date is "2025-01-01"
    Given law "dep_a" is loaded
    Given law "dep_b" is loaded

  Scenario: With data
    Given the following "personal_data" data with key "bsn":
      | bsn | name | age |
      | 123 | Alice | 30 |
    Given parameter "bsn" is "123"
    When I evaluate "eligible" of "my_law"
    Then output "eligible" is true
`;

    const parsed = parseFeature(original);
    const form = mapFeatureToForm(parsed);
    const regenerated = formStateToGherkin(form);
    const reparsed = parseFeature(regenerated);
    const reformatted = mapFeatureToForm(reparsed);

    expect(defined(reformatted.background).dependencies).toEqual(defined(form.background).dependencies);
    expect(defined(reformatted.scenarios[0]).setup.dataSources).toHaveLength(1);
    expect(defined(defined(reformatted.scenarios[0]).setup.dataSources[0]).sourceName).toBe('personal_data');
    expect(defined(defined(reformatted.scenarios[0]).setup.dataSources[0]).keyField).toBe('bsn');
  });
});

describe('syncEditedValues', () => {
  it('updates scenario-level parameter values', () => {
    const parsed = parseFeature(`
Feature: Sync test

  Scenario: Test
    Given parameter "loon" is 30000
    When I evaluate "result" of "law"
    Then output "result" is true
`);

    const form = mapFeatureToForm(parsed);
    syncEditedValues(form, 0, {
      parameterValues: { loon: '50000' },
      calculationDate: null,
    });

    expect(defined(defined(form.scenarios[0]).setup.parameters[0]).value).toBe(50000);

    const gherkin = formStateToGherkin(form);
    expect(gherkin).toContain('parameter "loon" is 50000');
    expect(gherkin).not.toContain('30000');
  });

  it('adds scenario override when background parameter is changed', () => {
    const parsed = parseFeature(`
Feature: Background override

  Background:
    Given parameter "bsn" is "999993653"
    Given parameter "loon" is 30000

  Scenario: First
    When I evaluate "result" of "law"

  Scenario: Second
    When I evaluate "result" of "law"
`);

    const form = mapFeatureToForm(parsed);

    // Change loon only in the first scenario
    syncEditedValues(form, 0, {
      parameterValues: { bsn: '999993653', loon: '50000' },
      calculationDate: null,
    });

    // First scenario should have an override
    expect(defined(form.scenarios[0]).setup.parameters).toEqual([
      { name: 'loon', value: 50000 },
    ]);

    // Background should be unchanged
    expect(defined(defined(form.background).parameters[1]).value).toBe(30000);

    // Second scenario should still have no overrides
    expect(defined(form.scenarios[1]).setup.parameters).toHaveLength(0);
  });

  // RFC-036: a blank field is "not filled in", never `parameter "x" is ""`
  // (which the Rust runner would read back as an absence, null).
  it('drops a scenario parameter the form left blank instead of writing is ""', () => {
    const parsed = parseFeature(`
Feature: Blank field

  Background:
    Given parameter "bsn" is "999993653"

  Scenario: Test
    Given parameter "aanvraag_bedrag" is 250
    Given parameter "huur" is 650
    When I evaluate "result" of "law"
`);

    const form = mapFeatureToForm(parsed);
    syncEditedValues(form, 0, {
      parameterValues: { bsn: '999993653', aanvraag_bedrag: '', huur: '700' },
      calculationDate: null,
    });

    expect(defined(form.scenarios[0]).setup.parameters).toEqual([{ name: 'huur', value: 700 }]);
    const gherkin = formStateToGherkin(form);
    expect(gherkin).not.toContain('aanvraag_bedrag');
    expect(gherkin).not.toContain('is ""');
    expect(gherkin).toContain('Given parameter "huur" is 700');
  });

  it('never writes is "" for a background parameter the form left blank', () => {
    const parsed = parseFeature(`
Feature: Blank background field

  Background:
    Given parameter "bsn" is "999993653"
    Given parameter "loon" is 30000

  Scenario: Test
    When I evaluate "result" of "law"
`);

    const form = mapFeatureToForm(parsed);
    syncEditedValues(form, 0, {
      parameterValues: { bsn: '999993653', loon: '' },
      calculationDate: null,
    });

    // There is no step that un-passes a background parameter; the background
    // value stands and no override is written.
    expect(defined(form.scenarios[0]).setup.parameters).toHaveLength(0);
    expect(defined(defined(form.background).parameters[1]).value).toBe(30000);
    expect(formStateToGherkin(form)).not.toContain('is ""');
  });

  it('does not add override when value matches background', () => {
    const parsed = parseFeature(`
Feature: No-op sync

  Background:
    Given parameter "bsn" is "999993653"

  Scenario: Test
    When I evaluate "result" of "law"
`);

    const form = mapFeatureToForm(parsed);
    syncEditedValues(form, 0, {
      parameterValues: { bsn: '999993653' },
      calculationDate: null,
    });

    // No scenario-level override should be added
    expect(defined(form.scenarios[0]).setup.parameters).toHaveLength(0);
  });
});

describe('typed scenario values round-trip', () => {
  // ScenarioParameterInput now emits real JS types (boolean true, integer
  // eurocents) instead of strings. Verify these serialize to valid Gherkin
  // and parse back unchanged, so the new typed controls don't corrupt the
  // .feature file. See ScenarioForm / ScenarioParameterInput.
  it('serializes a real boolean and an eurocent amount, then parses back', () => {
    const parsed = parseFeature(`
Feature: Typed values

  Scenario: Test
    Given parameter "is_verzekerde" is "false"
    Given parameter "inkomen" is 0
    When I evaluate "result" of "law"
`);

    const form = mapFeatureToForm(parsed);
    syncEditedValues(form, 0, {
      parameterValues: { is_verzekerde: true, inkomen: 150000 },
      calculationDate: null,
    });

    const gherkin = formStateToGherkin(form);
    // boolean → quoted; integer eurocents → unquoted number
    expect(gherkin).toContain('Given parameter "is_verzekerde" is "true"');
    expect(gherkin).toContain('Given parameter "inkomen" is 150000');

    // Re-parse. Quoted params (booleans/strings) reload as strings, which the
    // engine and ScenarioParameterInput coerce via parseValue at use time;
    // unquoted numbers reload as numbers directly.
    const reform = mapFeatureToForm(parseFeature(gherkin));
    const reparams = Object.fromEntries(
      defined(reform.scenarios[0]).setup.parameters.map((p) => [p.name, p.value]),
    );
    expect(parseValue(reparams.is_verzekerde)).toBe(true);
    expect(reparams.inkomen).toBe(150000);
  });
});

// The canonical rule (bdd/grammar.yaml `value_typing`) is that the quotes do
// not change what a value is — the content decides, on both sides of the
// language. The editor used to keep the quoted form as a raw string, so it ran
// a scenario with different types than the Rust runner did (#1160). A save may
// normalise a quoted number to its bare form; what it may not do is leave the
// two engines reading the same line differently.
describe('a quoted value carries its content, not its quotes', () => {
  const feature = `
Feature: Round trip

  Scenario: Test
    Given parameter "bsn" is "999993653"
    Given parameter "is_verzekerde" is "true"
    Given parameter "gemeente" is "GM0384"
    When I evaluate "result" of "law"
`;

  it('reads a quoted value by its content', () => {
    const form = mapFeatureToForm(parseFeature(feature));
    const params = Object.fromEntries(
      defined(form.scenarios[0]).setup.parameters.map((p) => [p.name, p.value]),
    );
    expect(params.bsn).toBe(999993653);
    expect(params.is_verzekerde).toBe(true);
    expect(params.gemeente).toBe('GM0384');
  });

  it('round-trips every form to the same values', () => {
    const form = mapFeatureToForm(parseFeature(feature));
    syncEditedValues(form, 0, {
      parameterValues: { bsn: '999993653', is_verzekerde: 'true', gemeente: 'GM0384' },
      calculationDate: null,
    });
    const reform = mapFeatureToForm(parseFeature(formStateToGherkin(form)));
    const params = Object.fromEntries(
      defined(reform.scenarios[0]).setup.parameters.map((p) => [p.name, p.value]),
    );
    expect(params).toEqual({ bsn: 999993653, is_verzekerde: true, gemeente: 'GM0384' });
  });
});

describe('unknown assertions (RFC-036)', () => {
  const feature = `
Feature: Unknown

  Scenario: Missing rent
    Given the calculation date is "2025-01-01"
    When I evaluate "toeslag" of "wet"
    Then output "toeslag" is unknown
    Then output "recht" is unknown for lack of "huur"
`;

  it('extracts both forms', () => {
    const form = mapFeatureToForm(parseFeature(feature));
    expect(defined(form.scenarios[0]).unmatchedSteps).toEqual([]);
    expect(defined(form.scenarios[0]).assertions).toEqual([
      { assertionType: 'unknown', outputName: 'toeslag', value: null },
      { assertionType: 'unknownFor', outputName: 'recht', value: 'huur' },
    ]);
  });

  it('writes both forms back out unchanged', () => {
    const text = formStateToGherkin(mapFeatureToForm(parseFeature(feature)));
    expect(text).toContain('    Then output "toeslag" is unknown\n');
    expect(text).toContain('    Then output "recht" is unknown for lack of "huur"\n');
  });
});

describe('data-source blank cells (RFC-036)', () => {
  const text =
    'Feature: Blank\n' +
    '\n' +
    '  Scenario: No rent known\n' +
    '    Given the following "huurgegevens" data with key "bsn":\n' +
    '      | bsn | huur | partner_bsn |\n' +
    '      | 1 |  | null |\n' +
    '    When I evaluate "x" of "law"\n';

  it('keeps a blank cell blank and the word null as it is through a round trip', () => {
    const form = mapFeatureToForm(parseFeature(text));
    expect(defined(defined(form.scenarios[0]).setup.dataSources[0]).rows).toEqual([['1', '', 'null']]);
    expect(formStateToGherkin(form)).toBe(text);
  });

  it('writes a cell the form cleared as blank, not as null', () => {
    const form = mapFeatureToForm(parseFeature(text));
    syncEditedValues(form, 0, {
      parameterValues: {},
      dataSources: [{
        sourceName: 'huurgegevens',
        keyField: 'bsn',
        fields: [{ name: 'huur', type: 'amount' }, { name: 'partner_bsn', type: 'string' }],
        rows: [{ _id: 1, bsn: '1', huur: '', partner_bsn: '' }],
      }],
    });
    expect(formStateToGherkin(form)).toContain('      | 1 |  |  |\n');
  });
});
