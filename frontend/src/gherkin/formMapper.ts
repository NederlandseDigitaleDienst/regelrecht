/**
 * Bidirectional mapper between Gherkin AST and structured form state.
 *
 * Forward:  parseFeature() AST → mapFeatureToForm() → visual form state
 * Reverse:  visual form state → formStateToGherkin() → Gherkin text
 */

import { bareValue, quotedValue, tableCellValue, type DataTable } from './actions';
import { GRAMMAR } from './grammar.generated';
import type { ParsedFeature, Step } from './parser';
import type { GrammarEntry } from './steps';

// --- Form-state types ---

/** One element of a collection parameter: column name to typed cell. */
export type CollectionRecord = Record<string, unknown>;

/**
 * A parameter in a setup. `value` is a scalar or, for a collection
 * parameter (RFC-016), an array of records; `columns` exists only on a
 * collection.
 */
export interface FormParameter {
  name: string;
  value: unknown;
  columns?: string[];
}

/** A data source in state format: a header row and rows of cell text. */
export interface StateDataSource {
  sourceName: string;
  keyField: string;
  lawId?: string;
  headers: string[];
  rows: string[][];
}

export interface FormSetup {
  calculationDate: string | null;
  dependencies: string[];
  parameters: FormParameter[];
  dataSources: StateDataSource[];
}

export interface FormExecution {
  outputName: string;
  lawId: string;
}

export type AssertionType =
  | 'succeeds'
  | 'fails'
  | 'failsWith'
  | 'boolean'
  | 'equals'
  | 'equalsString'
  | 'null'
  | 'unknown'
  | 'unknownFor'
  | 'contains';

export interface FormAssertion {
  assertionType: AssertionType;
  outputName: string | null;
  value: unknown;
}

export interface FormScenario {
  name: string;
  tags: string[];
  setup: FormSetup;
  execution: FormExecution | null;
  assertions: FormAssertion[];
  unmatchedSteps: Step[];
}

export interface FormBackground extends FormSetup {
  unmatchedSteps: Step[];
}

/** Structured form state, as mapFeatureToForm() builds it. */
export interface FormState {
  featureName: string;
  background: FormBackground | null;
  scenarios: FormScenario[];
}

/**
 * A data source as the DataSourceTable form edits it (only the fields read
 * here): `{ sourceName, keyField, fields: [{name, type}], rows: [{_id, [h]: v}] }`.
 */
export interface FormDataSource {
  sourceName: string;
  keyField: string;
  fields?: { name: string; type?: string }[];
  rows?: Record<string, unknown>[];
}

/**
 * A collection as the form edits it (only the fields read here):
 * `{ name, columns: [{name, type}], rows: [{_id, [col]: v}] }`.
 */
export interface FormCollection {
  name: string;
  columns?: { name: string; type?: string; unit?: string | null }[];
  rows?: Record<string, unknown>[];
}

/** Edited form values, as ScenarioForm's getFormValues() hands them over. */
export interface EditedValues {
  parameterValues: Record<string, unknown>;
  calculationDate?: string | null;
  dataSources?: FormDataSource[];
  collections?: FormCollection[];
}

type Template = (args: unknown[]) => string;

/** The grammar entry ids this file emits steps for. */
type GrammarId =
  | 'set_calculation_date'
  | 'load_law'
  | 'set_parameter_string'
  | 'set_parameter_number'
  | 'set_parameter_collection'
  | 'set_data_source'
  | 'set_data_source_for_law'
  | 'evaluate'
  | 'assert_succeeds'
  | 'assert_fails'
  | 'assert_fails_with'
  | 'assert_boolean_true'
  | 'assert_boolean_false'
  | 'assert_equals_number'
  | 'assert_equals_string'
  | 'assert_null'
  | 'assert_unknown'
  | 'assert_unknown_for'
  | 'assert_contains';

/** What classifying one step yields, before it is merged into a setup. */
type Fragment =
  | { type: 'calculationDate'; value: string }
  | { type: 'dependency'; lawId: string }
  | { type: 'parameter'; name: string; value: unknown; columns?: string[] }
  | { type: 'parameterTable'; parameters: FormParameter[] }
  | {
      type: 'dataSource';
      sourceName: string;
      keyField: string;
      lawId?: string;
      headers: string[];
      rows: string[][];
    }
  | { type: 'execution'; outputName: string; lawId: string }
  | { type: 'assertion'; assertionType: AssertionType; outputName?: string; value?: unknown };

// --- Generated grammar lookups (single source of truth for phrasing) ---

// Emit-templates keyed by grammar entry id, and a capitalized keyword prefix
// (Given/When/Then) per entry id - both derived from GRAMMAR so phrasing and
// keyword stay single-sourced.
// GRAMMAR is generated plain JS, so its ids type as `string`; the casts name
// the ids this file uses, all of which are entries of that grammar.
const TPL = Object.fromEntries(GRAMMAR.map((e) => [e.id, e.template])) as Record<GrammarId, Template>;
const KW = Object.fromEntries(
  GRAMMAR.map((e) => [e.id, e.keyword.charAt(0).toUpperCase() + e.keyword.slice(1)]),
) as Record<GrammarId, string>;

// --- Extraction: classify a step line into a form-state fragment ---
//
// We reuse the GENERATED patterns (the drift surface) and keep only the
// extraction → form-state mapping hand-written, keyed on the grammar `action`.
// The editor consumes the `core` tier only.
const CORE_ENTRIES: readonly GrammarEntry[] = GRAMMAR.filter((e) => e.tier === 'core');

/**
 * Extract a form-state fragment from a matched core grammar entry.
 * Disambiguates the two parameter forms by entry id (string vs number).
 */
function extractFragment(entry: GrammarEntry, match: RegExpMatchArray, step: Step): Fragment | null {
  // Every capture group in a core pattern is mandatory, so a match fills each
  // group the entry's case reads.
  const m = match as unknown as readonly [string, string, string, string];
  switch (entry.action) {
    case 'set_calculation_date':
      return { type: 'calculationDate', value: m[1] };
    case 'load_law':
      return { type: 'dependency', lawId: m[1] };
    case 'set_parameter':
      return {
        type: 'parameter',
        name: m[1],
        value: entry.id === 'set_parameter_number' ? bareValue(m[2]) : quotedValue(m[2]),
      };
    case 'set_parameters_table':
      return { type: 'parameterTable', parameters: tableToParams(step.dataTable) };
    case 'set_parameter_collection': {
      // A collection-valued parameter (RFC-016): header row plus one row per
      // element. The columns are kept next to the records so a collection
      // without elements still serializes with its header.
      const { columns, records } = tableToCollection(step.dataTable);
      return { type: 'parameter', name: m[1], value: records, columns };
    }
    case 'set_data_source':
      return {
        type: 'dataSource',
        sourceName: m[1],
        keyField: m[2],
        headers: step.dataTable?.[0] || [],
        rows: step.dataTable?.slice(1) || [],
      };
    case 'set_data_source_for_law':
      // Same shape plus the law the source is bound to; the builder shows it
      // as a data source and writes the scoped step back out.
      return {
        type: 'dataSource',
        sourceName: m[1],
        keyField: m[2],
        lawId: m[3],
        headers: step.dataTable?.[0] || [],
        rows: step.dataTable?.slice(1) || [],
      };
    case 'evaluate':
      return { type: 'execution', outputName: m[1], lawId: m[2] };
    case 'assert_succeeds':
      return { type: 'assertion', assertionType: 'succeeds' };
    case 'assert_fails':
      return { type: 'assertion', assertionType: 'fails' };
    case 'assert_fails_with':
      return { type: 'assertion', assertionType: 'failsWith', value: m[1] };
    case 'assert_boolean':
      return {
        type: 'assertion',
        assertionType: 'boolean',
        outputName: m[1],
        value: entry.id === 'assert_boolean_true',
      };
    case 'assert_equals':
      return entry.id === 'assert_equals_number'
        ? { type: 'assertion', assertionType: 'equals', outputName: m[1], value: bareValue(m[2]) }
        : { type: 'assertion', assertionType: 'equalsString', outputName: m[1], value: m[2] };
    case 'assert_null':
      return { type: 'assertion', assertionType: 'null', outputName: m[1] };
    // Unknown (RFC-036): the second form names the fact that must be missing.
    case 'assert_unknown':
      return { type: 'assertion', assertionType: 'unknown', outputName: m[1] };
    case 'assert_unknown_for':
      return { type: 'assertion', assertionType: 'unknownFor', outputName: m[1], value: m[2] };
    case 'assert_contains':
      return { type: 'assertion', assertionType: 'contains', outputName: m[1], value: m[2] };
    default:
      return null;
  }
}

// A `Given the following parameters:` table is two columns of name/value with
// no header row (mirror of Rust `rows_to_params`), so every row is data.
function tableToParams(dataTable: DataTable | undefined): FormParameter[] {
  if (!dataTable) return [];
  return dataTable
    // An empty value cell is a parameter that is not passed (RFC-036), the
    // same as in the runners: it does not become a form value at all.
    .filter((row) => row.length >= 2 && row[1]!.trim() !== '')
    // The filter above guarantees both cells.
    .map((row) => ({
      name: row[0]!.trim(),
      value: tableCellValue(row[1]!),
    }));
}

// A `Given parameter "x" is the collection:` table has a header row and one
// row per element (mirror of Rust `rows_to_records` and the runner's
// `tableToRecords`): every element becomes an object keyed by the column
// names, cells typed by content. An empty cell leaves the key out of the
// record (RFC-036: no value stated), the literal `null` is an absence the
// author stated. Lenient on a short row - a missing cell counts as empty -
// so one ragged row does not take the whole feature down in the editor.
function tableToCollection(dataTable: DataTable | undefined): { columns: string[]; records: CollectionRecord[] } {
  if (!dataTable || dataTable.length === 0) return { columns: [], records: [] };
  const columns = dataTable[0]!.map((h) => h.trim());
  const records = dataTable.slice(1).map((row) => {
    const record: CollectionRecord = {};
    columns.forEach((h, i) => {
      if (i >= row.length || row[i]!.trim() === '') return;
      record[h] = tableCellValue(row[i]!);
    });
    return record;
  });
  return { columns, records };
}

/** True when a parameter value is a collection (array of records). */
// The narrowing to records holds because the mapper only ever puts arrays of
// records into a parameter value.
export function isCollectionValue(value: unknown): value is CollectionRecord[] {
  return Array.isArray(value);
}

/**
 * Type one cell of a collection as edited in the form. A blank or absent
 * cell is `undefined`: the record gets no such key, formatCell writes an
 * empty cell and the runner reads that back as "no value" (RFC-036). A
 * `null` (or the text `null`) is an absence the author stated and stays
 * null. Any other string is typed by content; anything else is already typed.
 */
export function collectionCell(v: unknown): unknown {
  if (v === undefined || v === '') return undefined;
  if (v === null) return null;
  return typeof v === 'string' ? tableCellValue(v) : v;
}

/** Column names of a collection parameter: the recorded header, or the keys of the first element. */
export function collectionColumns(param: { value?: unknown; columns?: string[] }): string[] {
  if (param.columns?.length) return param.columns;
  const first: unknown = ((param.value || []) as ArrayLike<unknown>)[0];
  return first ? Object.keys(first) : [];
}

function classifyStep(step: Step): Fragment | null {
  const text = step.text;
  for (const entry of CORE_ENTRIES) {
    const match = text.match(entry.pattern);
    if (match) {
      return extractFragment(entry, match, step);
    }
  }
  return null;
}

function classifySteps(steps: Step[]): {
  setup: FormSetup;
  executions: FormExecution[];
  assertions: FormAssertion[];
  unmatchedSteps: Step[];
} {
  const setup: FormSetup = {
    calculationDate: null,
    dependencies: [],
    parameters: [],
    dataSources: [],
  };
  const executions: FormExecution[] = [];
  const assertions: FormAssertion[] = [];
  const unmatchedSteps: Step[] = [];

  for (const step of steps) {
    const classified = classifyStep(step);
    if (!classified) {
      unmatchedSteps.push(step);
      continue;
    }

    switch (classified.type) {
      case 'calculationDate':
        setup.calculationDate = classified.value;
        break;
      case 'dependency':
        setup.dependencies.push(classified.lawId);
        break;
      case 'parameter':
        setup.parameters.push(
          classified.columns
            ? { name: classified.name, value: classified.value, columns: classified.columns }
            : { name: classified.name, value: classified.value },
        );
        break;
      case 'parameterTable':
        setup.parameters.push(...classified.parameters);
        break;
      case 'dataSource':
        setup.dataSources.push({
          sourceName: classified.sourceName,
          keyField: classified.keyField,
          ...(classified.lawId ? { lawId: classified.lawId } : {}),
          headers: classified.headers,
          rows: classified.rows,
        });
        break;
      case 'execution':
        executions.push({ outputName: classified.outputName, lawId: classified.lawId });
        break;
      case 'assertion':
        assertions.push({
          assertionType: classified.assertionType,
          outputName: classified.outputName || null,
          value: classified.value !== undefined ? classified.value : null,
        });
        break;
    }
  }

  return { setup, executions, assertions, unmatchedSteps };
}

/**
 * Map a parsed Gherkin feature AST to structured form state.
 *
 * @param {object} parsed - Output of parseFeature()
 * @returns {object} Structured form state
 */
export function mapFeatureToForm(parsed: ParsedFeature): FormState {
  const backgroundResult = parsed.background
    ? classifySteps(parsed.background)
    : null;

  const scenarios = (parsed.scenarios || []).map((scenario): FormScenario => {
    const result = classifySteps(scenario.steps);

    return {
      name: scenario.name,
      tags: scenario.tags || [],
      setup: {
        calculationDate: result.setup.calculationDate,
        dependencies: result.setup.dependencies,
        parameters: result.setup.parameters,
        dataSources: result.setup.dataSources,
      },
      execution: result.executions[0] || null,
      assertions: result.assertions,
      unmatchedSteps: result.unmatchedSteps,
    };
  });

  return {
    featureName: parsed.feature || '',
    background: backgroundResult
      ? {
          calculationDate: backgroundResult.setup.calculationDate,
          dependencies: backgroundResult.setup.dependencies,
          parameters: backgroundResult.setup.parameters,
          dataSources: backgroundResult.setup.dataSources,
          unmatchedSteps: backgroundResult.unmatchedSteps,
        }
      : null,
    scenarios,
  };
}

/**
 * Get the effective setup for a scenario by merging background + scenario setup.
 *
 * @param {object} formState - Output of mapFeatureToForm()
 * @param {number} scenarioIndex - Which scenario
 * @returns {object} Merged setup
 */
export function getEffectiveSetup(formState: FormState, scenarioIndex: number): FormSetup | null {
  const scenario = formState.scenarios[scenarioIndex];
  if (!scenario) return null;

  const bg: Partial<FormBackground> = formState.background || {};
  return {
    calculationDate: scenario.setup.calculationDate || bg.calculationDate || null,
    dependencies: [...(bg.dependencies || []), ...scenario.setup.dependencies],
    parameters: [...(bg.parameters || []), ...scenario.setup.parameters],
    dataSources: [...(bg.dataSources || []), ...scenario.setup.dataSources],
  };
}

/**
 * Convert a DataSourceTable form-format data source to the formState format.
 *
 * Form format: `{ sourceName, keyField, fields: [{name, type}], rows: [{_id, [h]: v}] }`
 * State format: `{ sourceName, keyField, headers: string[], rows: string[][] }`
 */
function formDataSourceToState(ds: FormDataSource): StateDataSource {
  const headers = [ds.keyField, ...(ds.fields || []).map((f) => f.name)];
  const rows = (ds.rows || []).map((row) =>
    headers.map((h) => {
      const v = row[h];
      return v === undefined || v === null ? '' : String(v);
    }),
  );
  return { sourceName: ds.sourceName, keyField: ds.keyField, headers, rows };
}

/**
 * Convert a collection as edited in the form to the formState parameter
 * value: the column names plus one record per row, `_id` dropped and cells
 * typed by content (the same rule the runner applies to the saved table).
 *
 * Form format:  `{ name, columns: [{name, type}], rows: [{_id, [col]: v}] }`
 * State format: `{ columns: string[], records: object[] }`
 */
export function formCollectionToState(coll: FormCollection): { columns: string[]; records: CollectionRecord[] } {
  const columns = (coll.columns || []).map((c) => c.name);
  const records = (coll.rows || []).map((row) => {
    const record: CollectionRecord = {};
    for (const c of columns) {
      const v = collectionCell(row[c]);
      if (v !== undefined) record[c] = v;
    }
    return record;
  });
  return { columns, records };
}

/**
 * Equality for two parameter values, scalar or collection. A collection is
 * compared record by record; `String()` on an array would flatten every
 * collection to "[object Object]" and call them all equal.
 */
function parameterValuesEqual(a: unknown, b: unknown): boolean {
  // Key order counts: an override that lists the same columns in another
  // order never collapses into the background. That keeps a step, it never
  // loses one, so it is left as is.
  if (isCollectionValue(a) || isCollectionValue(b)) {
    return isCollectionValue(a) && isCollectionValue(b) && JSON.stringify(a) === JSON.stringify(b);
  }
  return String(a) === String(b);
}

/** Deep equality check for two state-format data sources. */
function dataSourcesEqual(a: StateDataSource | undefined, b: StateDataSource | undefined): boolean {
  if (!a || !b) return false;
  if (a.sourceName !== b.sourceName || a.keyField !== b.keyField || (a.lawId ?? null) !== (b.lawId ?? null)) return false;
  if ((a.headers || []).length !== (b.headers || []).length) return false;
  for (let i = 0; i < a.headers.length; i++) {
    if (a.headers[i] !== b.headers[i]) return false;
  }
  if ((a.rows || []).length !== (b.rows || []).length) return false;
  for (let i = 0; i < a.rows.length; i++) {
    // In bounds for both: the lengths were just compared equal.
    const ra = a.rows[i]!;
    const rb = b.rows[i]!;
    if (ra.length !== rb.length) return false;
    for (let j = 0; j < ra.length; j++) {
      if (String(ra[j]) !== String(rb[j])) return false;
    }
  }
  return true;
}

/**
 * Sync edited form values back into formState for a given scenario.
 *
 * Parameters that exist in the scenario's own setup are updated in-place.
 * Background-only parameters that were changed get added as scenario-level
 * overrides so other scenarios are not affected. Scenario-level overrides
 * that end up matching the background value are removed so the Gherkin
 * round-trip does not accumulate redundant `Given parameter ...` steps.
 *
 * Data sources follow the same rule: scenario-level overrides exist only
 * when they differ from the background source with the same name.
 *
 * @param {object} formState - Mutable form state
 * @param {number} scenarioIndex - Which scenario was edited
 * @param {object} values - { parameterValues, calculationDate, dataSources }
 */
export function syncEditedValues(formState: FormState, scenarioIndex: number, values: EditedValues): void {
  const scenario = formState.scenarios[scenarioIndex];
  if (!scenario) return;

  const { parameterValues, calculationDate, dataSources, collections } = values;

  // --- Parameters ---
  const scenarioParamMap = new Map(
    scenario.setup.parameters.map((p, i) => [p.name, i]),
  );
  const bgParams = formState.background?.parameters || [];
  const bgParamMap = new Map(bgParams.map((p) => [p.name, p]));

  // Scalar parameters and collections share the override rule: a value that
  // lives in the scenario is updated in place, a background value that was
  // changed becomes a scenario-level override. `columns` only exists on a
  // collection.
  const applyParameter = (name: string, value: unknown, columns?: string[]) => {
    const entry: FormParameter = columns ? { name, value, columns } : { name, value };
    if (scenarioParamMap.has(name)) {
      // The map holds the index of every scenario parameter, rebuilt on each drop.
      Object.assign(scenario.setup.parameters[scenarioParamMap.get(name)!]!, entry);
    } else if (bgParamMap.has(name)) {
      if (!parameterValuesEqual(bgParamMap.get(name)!.value, value)) {
        scenario.setup.parameters.push(entry);
      }
    }
  };

  // A field left blank is a parameter that is not filled in, not a value
  // (RFC-036): the scenario must not say `parameter "x" is ""`. A scenario-
  // level entry for it is dropped; a background value stands, because a
  // scenario has no step that un-passes a parameter its background passes.
  // The live run (ScenarioForm) drops blanks the same way.
  const isBlank = (v: unknown) => v === undefined || v === null || v === '';
  const dropParameter = (name: string) => {
    if (!scenarioParamMap.has(name)) return;
    scenario.setup.parameters = scenario.setup.parameters.filter((p) => p.name !== name);
    scenarioParamMap.clear();
    scenario.setup.parameters.forEach((p, i) => scenarioParamMap.set(p.name, i));
  };

  for (const [name, rawValue] of Object.entries(parameterValues)) {
    if (isBlank(rawValue)) {
      dropParameter(name);
      continue;
    }
    // Same rule as reading a step: the content decides. An input control hands
    // back a raw string, and leaving it at that would write `is "50000"` where
    // the scenario said `is 50000`.
    // A form control may hand back a number or boolean too; parseValue passes
    // those through untouched.
    applyParameter(name, quotedValue(rawValue as string));
  }

  for (const coll of collections || []) {
    const { columns, records } = formCollectionToState(coll);
    applyParameter(coll.name, records, columns);
  }

  // Drop scenario-level overrides that now match the background - otherwise
  // a save/edit/save cycle accumulates redundant `Given parameter ...` steps.
  scenario.setup.parameters = scenario.setup.parameters.filter((p) => {
    if (!bgParamMap.has(p.name)) return true;
    return !parameterValuesEqual(bgParamMap.get(p.name)!.value, p.value);
  });

  // --- Data sources ---
  if (Array.isArray(dataSources)) {
    const bgDataSources = formState.background?.dataSources || [];
    const bgDsMap = new Map(bgDataSources.map((ds) => [ds.sourceName, ds]));
    const scenarioDsMap = new Map(
      scenario.setup.dataSources.map((ds, i) => [ds.sourceName, i]),
    );

    for (const formDs of dataSources) {
      const stateDs = formDataSourceToState(formDs);

      if (scenarioDsMap.has(stateDs.sourceName)) {
        // Update existing scenario-level data source in place
        scenario.setup.dataSources[scenarioDsMap.get(stateDs.sourceName)!] = stateDs;
      } else if (bgDsMap.has(stateDs.sourceName)) {
        // Background data source - add scenario override only if it differs
        if (!dataSourcesEqual(bgDsMap.get(stateDs.sourceName), stateDs)) {
          scenario.setup.dataSources.push(stateDs);
        }
      } else {
        // Wholly new data source - add at scenario level
        scenario.setup.dataSources.push(stateDs);
      }
    }

    // Drop scenario-level data sources that now match the background.
    scenario.setup.dataSources = scenario.setup.dataSources.filter((ds) => {
      const bg = bgDsMap.get(ds.sourceName);
      return !bg || !dataSourcesEqual(bg, ds);
    });
  }

  // Sync calculation date (scenario-level override).
  // Treat `null`, `undefined` and `""` as "user cleared the field" so that
  // a clear is not silently dropped - otherwise the next save would write
  // the stale date back to the .feature file.
  if (calculationDate !== undefined) {
    const cleared = calculationDate === null || calculationDate === '';
    const bgDate = formState.background?.calculationDate || null;
    if (cleared || calculationDate === bgDate) {
      // Either the user cleared the field, or it now matches the background:
      // in both cases the scenario-level override should disappear.
      scenario.setup.calculationDate = null;
    } else {
      scenario.setup.calculationDate = calculationDate;
    }
  }
}

// --- Reverse: Form State → Gherkin text ---

// A blank cell stays blank ("no value stated", RFC-036); only a `null` the
// author stated is written as the word. Data-source rows carry the cell text
// as read, so the word `null` in one of them passes through String() intact.
function formatCell(value: unknown): string {
  if (value === undefined || value === '') return '';
  if (value === null) return 'null';
  return String(value).replace(/\\/g, '\\\\').replace(/\|/g, '\\|');
}

function formatValue(value: unknown): unknown {
  if (value === true) return 'true';
  if (value === false) return 'false';
  if (value === null) return 'null';
  if (typeof value === 'number') return String(value);
  return value;
}

/**
 * Serialize structured form state back to Gherkin feature text.
 *
 * @param {object} formState - Output of mapFeatureToForm()
 * @returns {string} Gherkin text
 */
export function formStateToGherkin(formState: FormState): string {
  const lines: string[] = [];
  lines.push(`Feature: ${formState.featureName}`);

  // Background
  if (formState.background) {
    lines.push('');
    lines.push('  Background:');
    writeSetupSteps(lines, formState.background, '    ');
    writeUnmatchedSteps(lines, formState.background.unmatchedSteps, '    ');
  }

  // Scenarios
  for (const scenario of formState.scenarios) {
    lines.push('');
    if (scenario.tags?.length > 0) {
      lines.push(`  ${scenario.tags.join(' ')}`);
    }
    lines.push(`  Scenario: ${scenario.name}`);
    writeSetupSteps(lines, scenario.setup, '    ');

    // Execution
    if (scenario.execution) {
      const line = TPL.evaluate([scenario.execution.outputName, scenario.execution.lawId]);
      lines.push(`    ${KW.evaluate} ${line}`);
    }

    // Assertions
    for (const assertion of scenario.assertions) {
      const [id, line] = formatAssertion(assertion);
      lines.push(`    ${KW[id]} ${line}`);
    }

    // Unmatched steps
    writeUnmatchedSteps(lines, scenario.unmatchedSteps, '    ');
  }

  return lines.join('\n') + '\n';
}

function writeSetupSteps(lines: string[], setup: FormSetup, indent: string): void {
  // All setup phrasings + keywords come from the generated grammar by entry id.
  if (setup.calculationDate) {
    lines.push(`${indent}${KW.set_calculation_date} ${TPL.set_calculation_date([setup.calculationDate])}`);
  }

  for (const dep of setup.dependencies || []) {
    lines.push(`${indent}${KW.load_law} ${TPL.load_law([dep])}`);
  }

  for (const param of setup.parameters || []) {
    // A parameter without a value is not passed; there is no step for that
    // (RFC-036), and `parameter "x" is ""` would pass an absence instead.
    if (param.value === undefined || param.value === '') continue;
    if (isCollectionValue(param.value)) {
      lines.push(`${indent}${KW.set_parameter_collection} ${TPL.set_parameter_collection([param.name])}`);
      const columns = collectionColumns(param);
      lines.push(`${indent}  | ${columns.map(formatCell).join(' | ')} |`);
      for (const record of param.value) {
        const cells = columns.map((c) => formatCell(record[c]));
        lines.push(`${indent}  | ${cells.join(' | ')} |`);
      }
    } else if (typeof param.value === 'number') {
      lines.push(`${indent}${KW.set_parameter_number} ${TPL.set_parameter_number([param.name, formatValue(param.value)])}`);
    } else {
      lines.push(`${indent}${KW.set_parameter_string} ${TPL.set_parameter_string([param.name, formatValue(param.value)])}`);
    }
  }

  for (const ds of setup.dataSources || []) {
    if (ds.headers.length === 0) continue;
    lines.push(
      ds.lawId
        ? `${indent}${KW.set_data_source_for_law} ${TPL.set_data_source_for_law([ds.sourceName, ds.keyField, ds.lawId])}`
        : `${indent}${KW.set_data_source} ${TPL.set_data_source([ds.sourceName, ds.keyField])}`,
    );

    // Header
    lines.push(`${indent}  | ${ds.headers.join(' | ')} |`);

    // Data rows
    for (const row of ds.rows) {
      const cells = row.map((cell) => formatCell(cell));
      lines.push(`${indent}  | ${cells.join(' | ')} |`);
    }
  }
}

function writeUnmatchedSteps(lines: string[], unmatchedSteps: Step[] | undefined, indent: string): void {
  for (const step of unmatchedSteps || []) {
    lines.push(`${indent}${step.keyword} ${step.text}`);
    if (step.dataTable) {
      for (const row of step.dataTable) {
        lines.push(`${indent}  | ${row.join(' | ')} |`);
      }
    }
    if (step.docString) {
      lines.push(`${indent}  """`);
      lines.push(step.docString);
      lines.push(`${indent}  """`);
    }
  }
}

/**
 * Render an assertion to its canonical line via the generated grammar templates.
 * Returns [grammarEntryId, line] so the caller can prefix the right keyword.
 */
function formatAssertion(assertion: FormAssertion): [GrammarId, string] {
  switch (assertion.assertionType) {
    case 'succeeds':
      return ['assert_succeeds', TPL.assert_succeeds([])];
    case 'fails':
      return ['assert_fails', TPL.assert_fails([])];
    case 'failsWith':
      return ['assert_fails_with', TPL.assert_fails_with([assertion.value])];
    case 'boolean':
      return assertion.value
        ? ['assert_boolean_true', TPL.assert_boolean_true([assertion.outputName])]
        : ['assert_boolean_false', TPL.assert_boolean_false([assertion.outputName])];
    case 'equals':
      return ['assert_equals_number', TPL.assert_equals_number([assertion.outputName, assertion.value])];
    case 'equalsString':
      return ['assert_equals_string', TPL.assert_equals_string([assertion.outputName, assertion.value])];
    case 'null':
      return ['assert_null', TPL.assert_null([assertion.outputName])];
    case 'unknown':
      return ['assert_unknown', TPL.assert_unknown([assertion.outputName])];
    case 'unknownFor':
      return ['assert_unknown_for', TPL.assert_unknown_for([assertion.outputName, assertion.value])];
    case 'contains':
      return ['assert_contains', TPL.assert_contains([assertion.outputName, assertion.value])];
    default:
      throw new Error(`formatAssertion: unhandled assertionType '${assertion.assertionType}' - classifier/serializer out of sync`);
  }
}
