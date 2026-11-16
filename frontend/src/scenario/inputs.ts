/**
 * What a scenario hands the engine, derived from one place for both a run and
 * a save.
 *
 * The form edits a scenario as `EditedValues` (one value per control, tables
 * as rows). A save writes those values into the feature's `FormState`
 * (`syncEditedValues`) and serialises it. A run used to convert the same
 * values to engine inputs a second time, with its own rules for blanks and
 * overrides, so the editor could run something other than what the file would
 * say. Here a run applies the edits exactly as a save does, to a copy, and
 * reads the engine inputs from that copy's effective setup: the run is the
 * saved file, executed.
 */
import { tableCellValue } from '../gherkin/actions';
import {
  collectionColumns,
  getEffectiveSetup,
  isCollectionValue,
  syncEditedValues,
  type CollectionRecord,
  type EditedValues,
  type FormParameter,
  type FormScenario,
  type FormSetup,
  type FormState,
} from '../gherkin/formMapper';
import { todayLocalIso } from '../lib/localDate';

/** A table row as the form edits it: an `_id` plus one cell per column. */
export type EditedRow = Record<string, unknown>;

/** A data source as the form edits it, columns named but not typed. */
export interface EditedDataSource {
  sourceName: string;
  keyField: string;
  /** The regelwerk a `for law "..."` source is registered for. */
  lawId?: string;
  fields: { name: string }[];
  rows: EditedRow[];
}

/** A collection parameter as the form edits it (no key column). */
export interface EditedCollection {
  name: string;
  columns: { name: string }[];
  rows: EditedRow[];
}

/** The edit buffer of one scenario: what its form shows. */
export interface ScenarioValues extends EditedValues {
  parameterValues: Record<string, unknown>;
  calculationDate: string;
  dataSources: EditedDataSource[];
  collections: EditedCollection[];
}

/** A data source as the engine registers it. */
export interface EngineDataSource {
  sourceName: string;
  keyField: string;
  /** The regelwerk the source is registered for; absent for a global source. */
  lawId?: string;
  records: Record<string, unknown>[];
}

/** Everything one run of a scenario needs. */
export interface EngineInputs {
  /** The regelwerk the scenario executes. */
  lawId: string;
  /** The output it asks for; null when the scenario names none. */
  outputName: string | null;
  calculationDate: string;
  parameters: Record<string, unknown>;
  dataSources: EngineDataSource[];
}

// The setup lists a background value before the scenario's own; the last one
// with a name is the one that counts, for scalars, collections and sources.
function lastByName<T>(items: readonly T[], name: (item: T) => string): T[] {
  const byName = new Map<string, T>();
  for (const item of items) {
    byName.delete(name(item));
    byName.set(name(item), item);
  }
  return [...byName.values()];
}

/**
 * The values a scenario's form starts from: its effective setup (background
 * plus the scenario's own steps), one control per scalar parameter, tables for
 * collections and data sources. A scenario without a calculation date starts
 * on today.
 */
export function valuesFromSetup(setup: FormSetup): ScenarioValues {
  const parameters = lastByName(setup.parameters, (p) => p.name);
  const scalars = parameters.filter((p) => !isCollectionValue(p.value));
  const collections = parameters.filter((p) => isCollectionValue(p.value));
  return {
    calculationDate: setup.calculationDate || todayLocalIso(),
    parameterValues: Object.fromEntries(scalars.map((p) => [p.name, p.value ?? ''])),
    collections: collections.map((p) => ({
      name: p.name,
      columns: collectionColumns(p).map((name) => ({ name })),
      // Only collection-valued parameters were kept above.
      rows: (p.value as CollectionRecord[]).map((record, i) => ({ _id: `init-${i}`, ...record })),
    })),
    dataSources: lastByName(setup.dataSources, (ds) => ds.sourceName).map((ds) => ({
      sourceName: ds.sourceName,
      keyField: ds.keyField,
      ...(ds.lawId ? { lawId: ds.lawId } : {}),
      fields: ds.headers.filter((h) => h !== ds.keyField).map((name) => ({ name })),
      // A string id: DataSourceTable numbers new rows from a counter of its
      // own, and an integer here would collide with it in `:key`.
      rows: ds.rows.map((row, i) => {
        const record: EditedRow = { _id: `init-${i}` };
        ds.headers.forEach((h, j) => { record[h] = row[j] ?? ''; });
        return record;
      }),
    })),
  };
}

/** The values each scenario of a feature starts from, in scenario order. */
export function initialValues(state: FormState): ScenarioValues[] {
  return state.scenarios.map((_, i) => {
    // `i` indexes `state.scenarios`, so the setup exists.
    const setup = getEffectiveSetup(state, i)!;
    return valuesFromSetup(setup);
  });
}

// A deep copy of the parsed feature. JSON is enough: a FormState holds plain
// data (strings, numbers, booleans, null, arrays, records).
function copyState(state: FormState): FormState {
  return JSON.parse(JSON.stringify(state)) as FormState;
}

/**
 * The feature with every scenario's values written in, the way a save writes
 * them: blanks dropped, a value equal to the background's left to the
 * background. `values[i]` null leaves scenario `i` as it is. Returns a copy;
 * `state` is not touched.
 */
export function applyValues(state: FormState, values: readonly (ScenarioValues | null | undefined)[]): FormState {
  const next = copyState(state);
  values.forEach((v, i) => {
    if (v) syncEditedValues(next, i, v);
  });
  return next;
}

/** The output a scenario runs for: its execution step's, else its first assertion's. */
export function outputNameFor(scenario: Pick<FormScenario, 'execution' | 'assertions'>): string | null {
  if (scenario.execution?.outputName) return scenario.execution.outputName;
  return scenario.assertions.find((a) => a.outputName)?.outputName ?? null;
}

// A stated value goes to the engine; a value the setup leaves undefined does
// not exist. `null` is passed: it is an absence the author stated (RFC-036).
function parameterInputs(parameters: readonly FormParameter[]): Record<string, unknown> {
  const inputs: Record<string, unknown> = {};
  for (const p of lastByName(parameters, (q) => q.name)) {
    if (p.value === undefined || p.value === '') continue;
    inputs[p.name] = p.value;
  }
  return inputs;
}

// A blank cell is left out of the record, the rule the runner applies to the
// saved table (RFC-036): the engine then reports the input as unknown instead
// of reading an empty string.
function dataSourceInputs(setup: FormSetup): EngineDataSource[] {
  return lastByName(setup.dataSources, (ds) => ds.sourceName)
    .filter((ds) => ds.rows.length > 0)
    .map((ds) => ({
      sourceName: ds.sourceName,
      keyField: ds.keyField,
      ...(ds.lawId ? { lawId: ds.lawId } : {}),
      records: ds.rows.map((row) => {
        const record: Record<string, unknown> = {};
        ds.headers.forEach((h, j) => {
          const cell = row[j];
          if (cell === undefined || cell === '') return;
          record[h] = tableCellValue(cell);
        });
        return record;
      }),
    }));
}

/**
 * The engine inputs of scenario `index`, as the feature with these values
 * would state them. `fallbackLawId` is the regelwerk the file belongs to, used
 * when the scenario has no execution step naming one. Null when the scenario
 * does not exist.
 */
export function engineInputs(
  state: FormState,
  index: number,
  values: ScenarioValues | null,
  fallbackLawId: string,
): EngineInputs | null {
  const scenario = state.scenarios[index];
  if (!scenario) return null;
  const applied = values ? applyValues(state, state.scenarios.map((_, i) => (i === index ? values : null))) : state;
  // `index` exists in `state`, and so in its copy.
  const setup = getEffectiveSetup(applied, index)!;
  return {
    lawId: scenario.execution?.lawId || fallbackLawId,
    outputName: outputNameFor(scenario),
    calculationDate: setup.calculationDate || todayLocalIso(),
    parameters: parameterInputs(setup.parameters),
    dataSources: dataSourceInputs(setup),
  };
}
