<script setup lang="ts">
import { ref, computed, watch, onBeforeUnmount, useId } from 'vue';
import { quotedValue, tableCellValue } from '../gherkin/actions';
import { isCollectionValue, collectionColumns, formCollectionToState } from '../gherkin/formMapper';
import type { CollectionRecord, EditedValues, FormParameter, FormScenario, FormSetup } from '../gherkin/formMapper';
import type { EditorEngine } from '../composables/useEngine';
import type { Expectation } from '../utils/outputFormat';
import { formatValue, formatMissing, normalizeForCompare, matchStatus as _matchStatus, humanize, expectationsFromAssertions } from '../utils/outputFormat';
import { NOT_NULLABLE_MESSAGE, nullAllowed, isNullText } from '../utils/nullability';
import DataSourceTable from './DataSourceTable.vue';
import ScenarioParameterInput from './ScenarioParameterInput.vue';
import AbsenceToggle from './AbsenceToggle.vue';

/** A field's declared datatype, amount unit and (when known) nullability. */
interface DeclaredField {
  type: string;
  unit: string | null;
  nullable?: boolean;
}

/** A field as the data-source table types its column. */
interface TypedField {
  name: string;
  type: string;
  unit: string | null;
  nullable: boolean | undefined;
}

/** A table row as the form edits it: an `_id` plus one cell per column. */
type EditedRow = Record<string, unknown>;

/** A data source as the form edits it (DataSourceTable format). */
interface EditedDataSource {
  sourceName: string;
  keyField: string;
  fields: TypedField[];
  rows: EditedRow[];
}

/** A collection parameter as the form edits it (DataSourceTable format, no key). */
interface EditedCollection {
  name: string;
  columns: { name: string; type: string; unit: string | null }[];
  rows: EditedRow[];
}

/** The fields of an executeWithTrace() result read here and by ScenarioBuilder. */
export interface TraceResult {
  outputs?: Record<string, unknown>;
  trace_text?: string | null;
}

/** What getExecutionData() hands the parent. */
export interface ExecutionData {
  result: TraceResult | null;
  traceText: unknown;
  error: unknown;
  running: boolean;
  expectations: Record<string, Expectation>;
  outputName: string | null;
}

const props = withDefaults(defineProps<{
  /** Scenario object from mapFeatureToForm() (only the fields read here) */
  scenario: Pick<FormScenario, 'assertions'> & { execution?: FormScenario['execution'] };
  /** Merged setup from getEffectiveSetup() (only the fields read here) */
  setup: Pick<FormSetup, 'calculationDate' | 'parameters' | 'dataSources'>;
  /** WASM engine instance */
  engine?: Pick<EditorEngine, 'clearDataSources' | 'registerDataSource' | 'executeWithTrace'> | null;
  /** Engine ready state */
  ready?: boolean;
  /** Law ID for execution */
  lawId: string;
  /** Article mapping: { outputToArticle, inputToArticle, paramToArticle } */
  articleMap?: { paramToArticle?: ReadonlyMap<string | undefined, string> } | null;
  /** Datatype mapping from buildTypeMap(): name -> { type, unit, nullable } */
  typeMap?: ReadonlyMap<string, DeclaredField> | null;
  /** External data-source field types from buildExternalFieldTypeMap(): name -> { type, unit, nullable } */
  externalFieldTypeMap?: ReadonlyMap<string, DeclaredField> | null;
}>(), {
  engine: null,
  ready: false,
  articleMap: null,
  typeMap: null,
  externalFieldTypeMap: null,
});

// Resolve a parameter's declared datatype/unit; default to a plain text field
// for params not found in the map (background-only params, articles without
// machine_readable). Such a parameter has no `nullable` either: no
// declaration, no claim (utils/nullability.js).
function paramMeta(name: string): DeclaredField {
  return props.typeMap?.get(name) ?? { type: 'string', unit: null };
}

// Resolve an external data-source column's datatype/unit/nullability from the
// dependency graph; default to a plain text field, nullability unknown, for
// columns not found in the map.
function typeField(name: string): TypedField {
  const meta = props.externalFieldTypeMap?.get(name);
  return { name, type: meta?.type ?? 'string', unit: meta?.unit ?? null, nullable: meta?.nullable };
}

const emit = defineEmits<{
  'show-details': [];
  executed: [data: ExecutionData];
  change: [];
  'drill-change': [name: string | null];
}>();

// --- Form state (initialized from scenario setup) ---
const calculationDate = ref(props.setup.calculationDate || new Date().toISOString().slice(0, 10));

// Scalar parameters edit as one control each; a collection-valued parameter
// (RFC-016, `Given parameter "x" is the collection:`) is a table and gets
// the same drill-in treatment as a data source. The two are kept apart so
// the control loop below never meets an array.
const scalarParams = () => (props.setup.parameters || []).filter((p) => !isCollectionValue(p.value));

const parameterValues = ref<Record<string, unknown>>(
  Object.fromEntries(scalarParams().map((p) => [p.name, p.value ?? ''])),
);

// A stated absence (`null`) is a value of a parameter only where the law
// declares it nullable (RFC-036). Typing `null` into a parameter the law
// declares as never absent is refused: a blank is stored (the parameter is
// then left out of the run, unknown) and the field is marked invalid, with
// the message, until the author types something else. The field itself
// keeps showing what was typed, so the message points at the text it is
// about. A `null` that is already there (read from the feature file) is
// marked the same way but kept: it came from the file, a human decides.
const rejectedNullParams = ref(new Set<string>());
const paramErrorIdPrefix = useId();

function paramNullAllowed(name: string) {
  return nullAllowed(paramMeta(name));
}
// The "afwezig" checkbox (AbsenceToggle) is the explicit way to state an
// absence, offered where the law declares the parameter `nullable: true`,
// whatever its type: a number field cannot hold the word `null` and a switch
// cannot show it. Not offered on `false` (refused anyway) nor on an unknown
// declaration (a text field, where typing `null` already works; a control
// there would read as a claim the form cannot make).
function paramOffersAbsenceToggle(name: string) {
  return paramMeta(name).nullable === true;
}
function updateParameter(name: string, value: unknown) {
  if (isNullText(value) && !paramNullAllowed(name)) {
    rejectedNullParams.value.add(name);
    parameterValues.value = { ...parameterValues.value, [name]: '' };
  } else {
    rejectedNullParams.value.delete(name);
    parameterValues.value = { ...parameterValues.value, [name]: value };
  }
  emit('change');
}
function paramNullInvalid(name: string, value: unknown) {
  return rejectedNullParams.value.has(name) || (isNullText(value) && !paramNullAllowed(name));
}
function paramErrorId(name: string) {
  return `${paramErrorIdPrefix}-${name}`;
}
function paramControlId(name: string) {
  return `${paramErrorId(name)}-control`;
}

// Convert collection parameters to DataSourceTable format. A collection has
// no key field; the element fields are not typed by the law (an `array`
// input declares no item shape), so cells are typed by content at run time,
// the same rule the runner applies to the saved table.
//
// The setup is the merged background plus scenario list, so a scenario that
// overrides a background collection lists the name twice. Last wins, the
// same rule `Object.fromEntries` applies to the scalars above.
function initCollections(): EditedCollection[] {
  const byName = new Map<string, FormParameter>();
  for (const p of props.setup.parameters || []) {
    if (isCollectionValue(p.value)) byName.set(p.name, p);
  }
  return [...byName.values()].map((p) => ({
    name: p.name,
    columns: collectionColumns(p).map((c) => ({ name: c, type: 'string', unit: null })),
    // Only collection-valued parameters were put in `byName`.
    rows: (p.value as CollectionRecord[]).map((record, i) => ({ _id: `init-${i}`, ...record })),
  }));
}

const collections = ref<EditedCollection[]>(initCollections());

// Convert scenario data sources to DataSourceTable format
function initDataSources(): EditedDataSource[] {
  return (props.setup.dataSources || []).map((ds) => ({
    sourceName: ds.sourceName,
    keyField: ds.keyField,
    fields: ds.headers.filter((h) => h !== ds.keyField).map((h) => typeField(h)),
    // A string id: DataSourceTable.addRow numbers new rows from a counter
    // of its own, and an integer here would collide with it in `:key`.
    rows: ds.rows.map((row, i) => {
      const obj: EditedRow = { _id: `init-${i}` };
      ds.headers.forEach((h, j) => { obj[h] = row[j] ?? ''; });
      return obj;
    }),
  }));
}

const dataSources = ref<EditedDataSource[]>(initDataSources());

// Drill-in navigation: null = scenario overview, otherwise the index of the
// data source whose table is shown one level deeper in the sheet. The back
// affordance lives in the sheet's top-title-bar (a single level deep, so the
// ActionSheet-style breadcrumb rows would be overkill), so the parent needs
// to know the drilled source name and be able to pop back out.
const selectedSource = ref<number | null>(null);
// Same, for a collection parameter (index into `collections`). At most one
// of the two is non-null.
const selectedCollection = ref<number | null>(null);

const drilledIn = computed(() => selectedSource.value !== null || selectedCollection.value !== null);

function clearDrill() {
  selectedSource.value = null;
  selectedCollection.value = null;
}

// Data-source names render human-readable AND sentence-cased ("personal_data"
// → "Personal data"). humanize() only de-snakes; it doesn't capitalise.
function sourceLabel(name: string) {
  const h = humanize(name);
  return h ? h.charAt(0).toUpperCase() + h.slice(1) : h;
}

watch([selectedSource, selectedCollection], ([src, coll]) => {
  if (src != null) emit('drill-change', dataSources.value[src]?.sourceName ?? null);
  else if (coll != null) emit('drill-change', collections.value[coll]?.name ?? null);
  else emit('drill-change', null);
});

// Expectations from scenario assertions (null and unknown included, RFC-036)
const expectations = ref(expectationsFromAssertions(props.scenario.assertions));

// Output selection: default to outputs referenced in execution + assertions
const initOutputs = () => {
  const names = new Set<string>();
  if (props.scenario.execution?.outputName) names.add(props.scenario.execution.outputName);
  for (const a of props.scenario.assertions || []) {
    if (a.outputName) names.add(a.outputName);
  }
  return [...names];
};
const selectedOutputs = ref(initOutputs());

// --- Execution state (per-scenario) ---
const result = ref<TraceResult | null>(null);
const running = ref(false);
const error = ref<unknown>(null);
const errorTraceText = ref<unknown>(null);

// Re-initialise every local input/result ref from the (read-only) props.
// All edits live in these refs and are only written back to formState on
// save, so re-initialising here fully discards unsaved edits. Used both
// when the scenario/setup props change and when the parent discards edits
// (cancel / click-away) without replacing formState - see ScenarioBuilder.
function discardEdits() {
  calculationDate.value = props.setup.calculationDate || new Date().toISOString().slice(0, 10);
  parameterValues.value = Object.fromEntries(
    scalarParams().map((p) => [p.name, p.value ?? '']),
  );
  rejectedNullParams.value.clear();
  collections.value = initCollections();
  dataSources.value = initDataSources();
  selectedSource.value = null;
  selectedCollection.value = null;
  expectations.value = expectationsFromAssertions(props.scenario.assertions);
  selectedOutputs.value = initOutputs();
  // Wiping the local result is safe even on a cancel-with-edits: the
  // builder keeps the last run in its scenarioResults map, and
  // onShowDetails() falls back to that when getExecutionData() returns
  // no fresh result (hasFresh === false). So the result sheet keeps
  // showing the previous outcome instead of going blank.
  result.value = null;
  error.value = null;
  errorTraceText.value = null;
}

// Re-init when scenario/setup changes
watch([() => props.setup, () => props.scenario], discardEdits, { deep: true });

// Column types come from the dependency graph, which loads asynchronously after
// the panel mounts. Re-type the data-source columns in place when the map
// arrives - without rebuilding rows, so unsaved cell edits survive.
watch(() => props.externalFieldTypeMap, () => {
  for (const ds of dataSources.value) {
    ds.fields = ds.fields.map((f) => typeField(f.name));
  }
});

// CONTRACT: execute() must stay fully synchronous. The WASM engine runs
// in-process with no I/O, so `result`/`error`/`running` are all settled
// (running reset in finally) before this returns, and callers read them
// back immediately from the return value or getExecutionData() - e.g.
// ScenarioBuilder.reExecute() / onSaveAndShow() and the auto-execute
// loop. Introducing any await/timer/microtask here would make those
// callers observe `running: true` and stale data. If async execution is
// ever needed, return a Promise and update every caller to await it.
function execute() {
  if (!props.engine || !props.ready) return;

  const outputName = props.scenario.execution?.outputName || selectedOutputs.value[0];
  if (!outputName) return;

  running.value = true;
  result.value = null;
  error.value = null;
  errorTraceText.value = null;

  try {
    const engine = props.engine;
    engine.clearDataSources();

    // Register data sources
    for (const ds of dataSources.value) {
      if (ds.rows.length === 0) continue;
      // A blank cell is left out of the record, the same rule the runner
      // applies to the saved table (RFC-036): the engine then reports the
      // input as unknown instead of reading an empty string.
      const typedRows = ds.rows.map((row) => {
        const typed: Record<string, unknown> = {};
        for (const [k, v] of Object.entries(row)) {
          if (k === '_id' || v === '' || v === undefined) continue;
          typed[k] = typeof v === 'string' ? tableCellValue(v) : v;
        }
        return typed;
      });
      engine.registerDataSource(ds.sourceName, ds.keyField, typedRows);
    }

    // Build parameters
    // Type each parameter by the grammar's rule, so a live run in the editor
    // sees the same values the Rust runner reads back from the saved feature.
    const params: Record<string, unknown> = {};
    for (const [k, v] of Object.entries(parameterValues.value)) {
      if (v !== '' && v !== null && v !== undefined) {
        params[k] = typeof v === 'string' ? quotedValue(v) : v;
      }
    }
    // A collection is passed whole, an empty one included: "no elements" is
    // a value (no medebewoners), not a missing input. The same conversion
    // that a save applies, so the engine runs what the file will say.
    for (const coll of collections.value) {
      params[coll.name] = formCollectionToState(coll).records;
    }

    const execResult = engine.executeWithTrace(
      props.lawId,
      outputName,
      params,
      calculationDate.value,
    );

    // The engine hands its result back untyped; this is the shape it has.
    result.value = execResult as TraceResult;
  } catch (e) {
    if (e && typeof e === 'object' && 'error' in e && e.error) {
      error.value = e.error;
      errorTraceText.value = ('trace_text' in e && e.trace_text) || null;
    } else {
      // The WASM boundary can throw any value, null and undefined included.
      const message = e && typeof e === 'object' && 'message' in e ? e.message : undefined;
      const msg = typeof e === 'string' ? e : (message || String(e));
      error.value = msg;
    }
  } finally {
    running.value = false;
    // Always emit so the parent's result panel reflects the latest state,
    // including the error path: getExecutionData() returns the error and
    // any partial trace, which the parent renders instead of stale data
    // from a previous successful run.
    emit('executed', getExecutionData());
  }
  // Return the data so callers can synchronously read the post-execution
  // result without relying on Vue reactivity timing.
  return getExecutionData();
}

/** Returns the current execution data for use by parent components */
function getExecutionData(): ExecutionData {
  return {
    result: result.value,
    traceText: result.value?.trace_text || errorTraceText.value || null,
    error: error.value,
    running: running.value,
    expectations: expectations.value,
    // Expose the scenario's entry output so the graph view can mark the
    // starting leaf (see useTraceStepping.startNodeIds).
    outputName: props.scenario.execution?.outputName || selectedOutputs.value[0] || null,
  };
}

/** Returns the current form input values for syncing back to formState.
 *  All collections are returned as fresh shallow copies so a caller cannot
 *  accidentally mutate the component's reactive form state. */
function getFormValues() {
  return {
    parameterValues: { ...parameterValues.value },
    calculationDate: calculationDate.value,
    dataSources: [...dataSources.value],
    collections: [...collections.value],
  } satisfies EditedValues;
}

defineExpose({ execute, getExecutionData, getFormValues, clearDrill, discardEdits });

// --- Auto-re-execute when input values change ---
let executeTimer: ReturnType<typeof setTimeout> | undefined;
watch(
  [parameterValues, calculationDate, dataSources, collections],
  () => {
    if (!props.engine || !props.ready) return;
    clearTimeout(executeTimer);
    executeTimer = setTimeout(() => execute(), 300);
  },
  { deep: true },
);

onBeforeUnmount(() => {
  clearTimeout(executeTimer);
});

function updateDataSourceRows(index: number, rows: EditedRow[]) {
  const updated = [...dataSources.value];
  // `index` is the drilled-in source, an index into this list.
  updated[index] = { ...updated[index]!, rows };
  dataSources.value = updated;
  emit('change');
}

function updateCollectionRows(index: number, rows: EditedRow[]) {
  const updated = [...collections.value];
  // `index` is the drilled-in collection, an index into this list.
  updated[index] = { ...updated[index]!, rows };
  collections.value = updated;
  emit('change');
}

// --- Result formatting (delegates to shared utils) ---
function matchStatus(outputName: string, actualValue: unknown) {
  return _matchStatus(outputName, actualValue, expectations.value);
}

const hasExpectations = computed(() => Object.keys(expectations.value).length > 0);

// Minimal field-level validation: a missing date, or an empty input that
// caused an execution error, is marked invalid so the user sees which
// field to fill (the raw engine message still shows as context). No
// auto-revert - the input stays. We deliberately do NOT map engine error
// strings onto fields (incl. substring-matching the error against a param
// name): that produced confusing cross-field invalid states (changing bsn
// flagged loon_uit_dienstbetrekking) and was explicitly reverted. The
// broad "any error + this field empty" mark is the accepted trade - it
// can over-mark a legitimately-blank optional param, but it never
// mis-points at an unrelated field, which was the worse failure.
const dateInvalid = computed(() => !calculationDate.value);
// Unique ids for the date's validation item and for the control its list
// reads (ScenarioForm is mounted once per scenario, so a static id would clash).
const dateErrorId = useId();
const dateControlId = useId();
</script>

<template>
  <div class="sf-root">
    <!-- Scenario overview -->
    <template v-if="!drilledIn">
      <!-- Expected outputs -->
      <template v-if="hasExpectations">
        <nldd-title size="5"><h2>Verwachte uitkomsten</h2></nldd-title>
        <nldd-spacer size="8"></nldd-spacer>
        <nldd-list appearance="box-tinted">
          <nldd-list-item v-for="(exp, name) in expectations" :key="name" size="md">
            <nldd-text-cell size="md" :text="humanize(name)"></nldd-text-cell>
            <nldd-text-cell
              size="md"
              horizontal-alignment="right"
              :text="humanize(formatValue(normalizeForCompare(exp)))"
              :supporting-text="formatMissing(exp) || undefined"
            ></nldd-text-cell>
          </nldd-list-item>
          <nldd-list-item size="md">
            <nldd-cell width="full">
              <nldd-button
                size="md"
                width="full"
                @click="emit('show-details')"
                text="Bekijk resultaat"
              ></nldd-button>
            </nldd-cell>
          </nldd-list-item>
        </nldd-list>
      </template>

      <!-- Error -->
      <nldd-banner v-if="error && !running" variant="critical" :text="error"></nldd-banner>

      <!-- Loading indicator -->
      <nldd-text v-if="running" size="xs" color="secondary">Uitvoeren...</nldd-text>

      <!-- Input: date + parameters -->
      <nldd-spacer v-if="hasExpectations" size="16"></nldd-spacer>
      <nldd-title size="5"><h2>Invoer</h2></nldd-title>
      <nldd-spacer size="8"></nldd-spacer>
      <nldd-list appearance="box-tinted">
        <nldd-list-item size="md">
          <nldd-text-cell text="Datum" min-width="120px" max-width="200px"></nldd-text-cell>
          <nldd-spacer-cell size="8"></nldd-spacer-cell>
          <nldd-cell width="full" min-width="120px">
            <!-- The date control is a date field: it emits the ISO date as text. -->
            <ScenarioParameterInput
              :id="dateControlId"
              type="date"
              name="Datum"
              :value="calculationDate"
              :invalid="dateInvalid"
              :unmet="dateErrorId"
              @update="calculationDate = $event as string; emit('change')"
            />
            <nldd-validation-list v-if="dateInvalid" :for="dateControlId">
              <nldd-validation-item :id="dateErrorId">Datum is verplicht</nldd-validation-item>
            </nldd-validation-list>
          </nldd-cell>
        </nldd-list-item>
        <nldd-list-item v-for="(value, name) in parameterValues" :key="name" size="md">
          <nldd-text-cell :text="name" :supporting-text="articleMap?.paramToArticle?.get(name) ? `Artikel ${articleMap.paramToArticle.get(name)}` : undefined" min-width="120px" max-width="200px"></nldd-text-cell>
          <nldd-spacer-cell size="8"></nldd-spacer-cell>
          <nldd-cell width="full" min-width="120px">
            <ScenarioParameterInput
              :id="paramControlId(name)"
              :type="paramMeta(name).type"
              :unit="paramMeta(name).unit"
              :name="name"
              :value="value"
              :invalid="(!!error && (value === '' || value == null)) || paramNullInvalid(name, value)"
              :unmet="paramNullInvalid(name, value) ? paramErrorId(name) : undefined"
              @update="updateParameter(name, $event)"
            />
            <nldd-validation-list v-if="paramNullInvalid(name, value)" :for="paramControlId(name)">
              <nldd-validation-item :id="paramErrorId(name)">
                {{ NOT_NULLABLE_MESSAGE }}
              </nldd-validation-item>
            </nldd-validation-list>
          </nldd-cell>
          <template v-if="paramOffersAbsenceToggle(name)">
            <nldd-spacer-cell size="8"></nldd-spacer-cell>
            <nldd-cell width="fit-content">
              <AbsenceToggle
                :value="value"
                :data-testid="`absent-${name}`"
                @update="updateParameter(name, $event)"
              />
            </nldd-cell>
          </template>
        </nldd-list-item>
        <!-- Collection parameters: a row per collection, drill in one level
             deeper to edit the elements, the same way a data source works. -->
        <nldd-list-item
          v-for="(coll, i) in collections"
          :key="coll.name"
          size="md"
          button
          :data-testid="`coll-row-${i}`"
          @click="selectedCollection = i"
        >
          <nldd-text-cell :text="coll.name" :supporting-text="articleMap?.paramToArticle?.get(coll.name) ? `Artikel ${articleMap.paramToArticle.get(coll.name)}` : undefined" min-width="120px" max-width="200px"></nldd-text-cell>
          <nldd-spacer-cell size="12"></nldd-spacer-cell>
          <nldd-text-cell horizontal-alignment="right" :text="coll.rows.length ? String(coll.rows.length) : ''"></nldd-text-cell>
          <nldd-spacer-cell size="12"></nldd-spacer-cell>
          <nldd-icon-cell size="20"><nldd-icon icon="chevron-right"></nldd-icon></nldd-icon-cell>
        </nldd-list-item>
      </nldd-list>

      <!-- Data sources: a row per source, drill in one level deeper -->
      <template v-if="dataSources.length">
        <nldd-spacer size="16"></nldd-spacer>
        <nldd-title size="5"><h2>Bronnen</h2></nldd-title>
        <nldd-spacer size="8"></nldd-spacer>
        <nldd-list appearance="box-tinted">
          <nldd-list-item
            v-for="(ds, i) in dataSources"
            :key="ds.sourceName"
            size="md"
            button
            :data-testid="`ds-row-${i}`"
            @click="selectedSource = i"
          >
            <nldd-text-cell :text="sourceLabel(ds.sourceName)"></nldd-text-cell>
            <nldd-spacer-cell size="12"></nldd-spacer-cell>
            <nldd-text-cell horizontal-alignment="right" :text="ds.rows.length ? String(ds.rows.length) : ''"></nldd-text-cell>
            <nldd-spacer-cell size="12"></nldd-spacer-cell>
            <nldd-icon-cell size="20"><nldd-icon icon="chevron-right"></nldd-icon></nldd-icon-cell>
          </nldd-list-item>
        </nldd-list>
      </template>
    </template>

    <!-- One level deeper: a single data source's table. Back to the scenario
         overview is the sheet's top-title-bar back button (driven by the
         parent via clearDrill / drill-change). -->
    <!-- selectedSource / selectedCollection hold an index from the v-for over
         the same list and are reset whenever that list is rebuilt, so the
         entry exists. -->
    <template v-else-if="selectedSource !== null">
      <DataSourceTable
        :key="dataSources[selectedSource]!.sourceName"
        :title="sourceLabel(dataSources[selectedSource]!.sourceName)"
        :key-field="dataSources[selectedSource]!.keyField"
        :fields="dataSources[selectedSource]!.fields"
        :model-value="dataSources[selectedSource]!.rows"
        :drilled-in="true"
        @update:model-value="updateDataSourceRows(selectedSource, $event)"
      />
    </template>

    <!-- A collection parameter's elements: the same table, without a key column.
         Reached only while drilled in and not into a source, so
         selectedCollection is set. -->
    <template v-else>
      <DataSourceTable
        :key="collections[selectedCollection!]!.name"
        :title="collections[selectedCollection!]!.name"
        :key-field="null"
        :fields="collections[selectedCollection!]!.columns"
        :model-value="collections[selectedCollection!]!.rows"
        :drilled-in="true"
        @update:model-value="updateCollectionRows(selectedCollection!, $event)"
      />
    </template>

  </div>
</template>

<style scoped>
/* Single component root required for v-show, but it must not generate a
 * box - otherwise it blocks the enclosing simple-section's nldd flex
 * layout (flex-grow / centering of an empty data source's dialog). */
.sf-root {
  display: contents;
}
</style>
