import { describe, it, expect } from 'vitest';
import { parseFeature } from './parser';
import { mapFeatureToForm, getEffectiveSetup, formStateToGherkin, syncEditedValues, isCollectionValue, type CollectionRecord } from './formMapper';

/** `value`, after asserting it is there: a missing value fails the test at this point. */
function defined<T>(value: T | null | undefined): T {
  expect(value).toBeDefined();
  expect(value).not.toBeNull();
  return value as T;
}

/** A parameter value, after asserting it is a collection. */
function records(value: unknown): CollectionRecord[] {
  expect(isCollectionValue(value)).toBe(true);
  return value as CollectionRecord[];
}

// A collection-valued parameter (RFC-016): `Given parameter "x" is the
// collection:` with a header row and one row per element. Before this was
// handled the step landed in unmatchedSteps, so the editor ran the scenario
// without the parameter while the Rust runner passed it.
describe('collection parameters', () => {
  const FEATURE = `Feature: Kostendelersnorm

  Background:
    Given parameter "bsn" is "999993653"
    Given parameter "medebewoners" is the collection:
      | leeftijd | naam  |
      | 25       | Alice |
      | 19       | Bob   |

  Scenario: Three flatmates
    Given parameter "leeftijd" is 35
    Given parameter "medebewoners" is the collection:
      | leeftijd |
      | 25       |
      | 19       |
      | 34       |
    When I evaluate "aantal_kostendelende_medebewoners" of "participatiewet"
    Then output "aantal_kostendelende_medebewoners" equals 3

  Scenario: Inherits the background collection
    When I evaluate "aantal_kostendelende_medebewoners" of "participatiewet"
    Then output "aantal_kostendelende_medebewoners" equals 2
`;

  it('maps the table to one typed record per element, keeping the columns', () => {
    const form = mapFeatureToForm(parseFeature(FEATURE));
    const bg = defined(defined(form.background).parameters.find((p) => p.name === 'medebewoners'));
    expect(bg.columns).toEqual(['leeftijd', 'naam']);
    expect(bg.value).toEqual([
      { leeftijd: 25, naam: 'Alice' },
      { leeftijd: 19, naam: 'Bob' },
    ]);
    expect(defined(form.background).unmatchedSteps).toHaveLength(0);

    const sc = defined(defined(form.scenarios[0]).setup.parameters.find((p) => p.name === 'medebewoners'));
    expect(records(sc.value).map((r) => r.leeftijd)).toEqual([25, 19, 34]);
    expect(defined(form.scenarios[0]).unmatchedSteps).toHaveLength(0);
  });

  it('keeps the header of a collection without elements', () => {
    const form = mapFeatureToForm(parseFeature(`
Feature: Empty

  Scenario: Nobody
    Given parameter "medebewoners" is the collection:
      | leeftijd |
    When I evaluate "aantal_kostendelende_medebewoners" of "participatiewet"
`));
    const p = defined(defined(form.scenarios[0]).setup.parameters[0]);
    expect(p.value).toEqual([]);
    expect(p.columns).toEqual(['leeftijd']);
    expect(formStateToGherkin(form)).toContain(
      'Given parameter "medebewoners" is the collection:\n      | leeftijd |\n',
    );
  });

  it('merges the background collection into a scenario that does not override it', () => {
    const form = mapFeatureToForm(parseFeature(FEATURE));
    const second = defined(getEffectiveSetup(form, 1));
    const coll = second.parameters.filter((p) => p.name === 'medebewoners');
    expect(coll).toHaveLength(1);
    expect(defined(coll[0]).value).toHaveLength(2);
  });

  it('round-trips through Gherkin unchanged', () => {
    const form = mapFeatureToForm(parseFeature(FEATURE));
    const text = formStateToGherkin(form);
    expect(text).toContain(
      'Given parameter "medebewoners" is the collection:\n' +
      '      | leeftijd | naam |\n' +
      '      | 25 | Alice |\n' +
      '      | 19 | Bob |\n',
    );
    const again = mapFeatureToForm(parseFeature(text));
    expect(defined(again.background).parameters).toEqual(defined(form.background).parameters);
    expect(defined(again.scenarios[0]).setup.parameters).toEqual(defined(form.scenarios[0]).setup.parameters);
    // Serialising twice is a fixed point.
    expect(formStateToGherkin(again)).toBe(text);
  });

  it('leaves the key out for an empty cell and writes it back empty (RFC-036)', () => {
    const text =
      'Feature: Empty cell\n' +
      '\n' +
      '  Scenario: Age not stated\n' +
      '    Given parameter "medebewoners" is the collection:\n' +
      '      | leeftijd | naam |\n' +
      '      |  | Bob |\n' +
      '      | 40 | null |\n' +
      '    When I evaluate "x" of "law"\n';
    const form = mapFeatureToForm(parseFeature(text));
    const param = defined(defined(form.scenarios[0]).setup.parameters[0]);
    expect(param.value).toEqual([{ naam: 'Bob' }, { leeftijd: 40, naam: null }]);
    expect(Object.hasOwn(defined(records(param.value)[0]), 'leeftijd')).toBe(false);
    expect(formStateToGherkin(form)).toBe(text);
  });

  it('writes a null cell for a stated absence and reads it back', () => {
    const form = mapFeatureToForm(parseFeature(`
Feature: Null cell

  Scenario: Unknown age
    Given parameter "medebewoners" is the collection:
      | leeftijd | naam |
      | null     | Bob  |
    When I evaluate "x" of "law"
`));
    expect(defined(defined(form.scenarios[0]).setup.parameters[0]).value).toEqual([{ leeftijd: null, naam: 'Bob' }]);
    expect(formStateToGherkin(form)).toContain('| null | Bob |');
  });

  it('escapes a pipe and a backslash in a cell and in a header, and reads them back', () => {
    const form = mapFeatureToForm(parseFeature(`
Feature: Escapes

  Scenario: Odd cells
    Given parameter "items" is the collection:
      | naam    | pad\\|x |
      | a\\|b    | c\\\\d   |
    When I evaluate "x" of "law"
`));
    const p = defined(defined(form.scenarios[0]).setup.parameters[0]);
    expect(p.columns).toEqual(['naam', 'pad|x']);
    expect(p.value).toEqual([{ naam: 'a|b', 'pad|x': 'c\\d' }]);
    const text = formStateToGherkin(form);
    expect(text).toContain('| naam | pad\\|x |');
    expect(text).toContain('| a\\|b | c\\\\d |');
    expect(defined(mapFeatureToForm(parseFeature(text)).scenarios[0]).setup.parameters).toEqual(defined(form.scenarios[0]).setup.parameters);
  });

  it('types numeric cells by content, the same rule as a data-source table', () => {
    const form = mapFeatureToForm(parseFeature(`
Feature: Numbers

  Scenario: Numeric strings
    Given parameter "items" is the collection:
      | code | bedrag |
      | 007  | 1.0    |
    When I evaluate "x" of "law"
`));
    // 007 and 1.0 are numbers to the runner too; the leading zero and the
    // trailing .0 do not survive a save, and that is the documented rule.
    expect(defined(defined(form.scenarios[0]).setup.parameters[0]).value).toEqual([{ code: 7, bedrag: 1 }]);
    expect(formStateToGherkin(form)).toContain('| 7 | 1 |');
  });

  describe('syncEditedValues', () => {
    // Form format as ScenarioForm.getFormValues() hands it back: typed
    // columns and rows carrying an `_id`.
    const twoColumns = [{ name: 'leeftijd' }, { name: 'naam' }];
    const backgroundRows = [
      { _id: 0, leeftijd: '25', naam: 'Alice' },
      { _id: 1, leeftijd: '19', naam: 'Bob' },
    ];

    it('updates a scenario-level collection in place, dropping _id and typing cells', () => {
      const form = mapFeatureToForm(parseFeature(FEATURE));
      syncEditedValues(form, 0, {
        parameterValues: { leeftijd: '35' },
        calculationDate: null,
        collections: [{
          name: 'medebewoners',
          columns: [{ name: 'leeftijd', type: 'string', unit: null }],
          rows: [{ _id: 0, leeftijd: '40' }, { _id: 1, leeftijd: '41' }],
        }],
      });
      const p = defined(defined(form.scenarios[0]).setup.parameters.find((x) => x.name === 'medebewoners'));
      expect(p.value).toEqual([{ leeftijd: 40 }, { leeftijd: 41 }]);
      expect(p.columns).toEqual(['leeftijd']);
      expect(defined(form.scenarios[0]).setup.parameters).toHaveLength(2);
    });

    it('adds a scenario override when the background collection is changed', () => {
      const form = mapFeatureToForm(parseFeature(FEATURE));
      syncEditedValues(form, 1, {
        parameterValues: {},
        calculationDate: null,
        collections: [{ name: 'medebewoners', columns: twoColumns, rows: backgroundRows.slice(0, 1) }],
      });
      expect(defined(form.scenarios[1]).setup.parameters).toEqual([
        { name: 'medebewoners', value: [{ leeftijd: 25, naam: 'Alice' }], columns: ['leeftijd', 'naam'] },
      ]);
      expect(defined(defined(form.background).parameters.find((p) => p.name === 'medebewoners')).value).toHaveLength(2);
    });

    it('does not add an override when the collection still matches the background', () => {
      const form = mapFeatureToForm(parseFeature(FEATURE));
      syncEditedValues(form, 1, {
        parameterValues: {},
        calculationDate: null,
        collections: [{ name: 'medebewoners', columns: twoColumns, rows: backgroundRows }],
      });
      expect(defined(form.scenarios[1]).setup.parameters).toHaveLength(0);
    });

    it('removes a scenario override that is edited back to the background value', () => {
      const form = mapFeatureToForm(parseFeature(FEATURE));
      syncEditedValues(form, 0, {
        parameterValues: { leeftijd: '35' },
        calculationDate: null,
        collections: [{ name: 'medebewoners', columns: twoColumns, rows: backgroundRows }],
      });
      expect(defined(form.scenarios[0]).setup.parameters.map((p) => p.name)).toEqual(['leeftijd']);
    });
  });
});
