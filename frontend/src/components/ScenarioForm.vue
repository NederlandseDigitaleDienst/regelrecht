<script setup lang="ts">
import { ref, computed, watch, useId } from 'vue';
import type { FormScenario } from '../gherkin/formMapper';
import type { ScenarioValues, EditedRow } from '../scenario/inputs';
import type { ScenarioRun } from '../scenario/run';
import { formatValue, formatMissing, normalizeForCompare, humanize, expectationsFromAssertions } from '../utils/outputFormat';
import { NOT_NULLABLE_MESSAGE, nullAllowed, isNullText } from '../utils/nullability';
import { tableCellValue } from '../gherkin/actions';
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

/** Where the form is drilled into: a data source or a collection, by index. */
export type ScenarioDrill = { kind: 'source' | 'collection'; index: number } | null;

/**
 * One scenario's inputs as a form. The values are the caller's (the open
 * feature file's edit buffer): the form shows `values` and emits every edit as
 * a new `values`. It keeps only view state of its own: where it is drilled
 * into, and which `null` it refused.
 */
const props = withDefaults(defineProps<{
  /** The scenario's expectations (only the fields read here). */
  scenario: Pick<FormScenario, 'assertions'>;
  /** What the form shows. */
  values: ScenarioValues;
  /** The latest run of these values, for the error banner and field marks. */
  run?: ScenarioRun | null;
  /** Show a button that asks for the result (when no result pane is in view). */
  showResultButton?: boolean;
  /** Which data source or collection is open; null for the overview. */
  drill?: ScenarioDrill;
  /** Article mapping: parameter name -> article number. */
  articleMap?: { paramToArticle?: ReadonlyMap<string | undefined, string> } | null;
  /** Datatype mapping from buildTypeMap(): name -> { type, unit, nullable } */
  typeMap?: ReadonlyMap<string, DeclaredField> | null;
  /** External data-source field types from buildExternalFieldTypeMap(). */
  externalFieldTypeMap?: ReadonlyMap<string, DeclaredField> | null;
}>(), {
  run: null,
  showResultButton: false,
  drill: null,
  articleMap: null,
  typeMap: null,
  externalFieldTypeMap: null,
});

const emit = defineEmits<{
  'update:values': [values: ScenarioValues];
  'update:drill': [drill: ScenarioDrill];
  'show-result': [];
}>();

// Resolve a parameter's declared datatype/unit; default to a plain text field
// for params not found in the map (background-only params, articles without
// machine_readable). Such a parameter has no `nullable` either: no
// declaration, no claim (utils/nullability.js).
function paramMeta(name: string): DeclaredField {
  return props.typeMap?.get(name) ?? { type: 'string', unit: null };
}

// Resolve an external data-source column's datatype/unit/nullability from the
// dependency graph; default to a plain text field, nullability unknown, for
// columns not found in the map. Read at render time, so the columns follow
// the map when it arrives after the form.
function typeField(name: string): TypedField {
  const meta = props.externalFieldTypeMap?.get(name);
  return { name, type: meta?.type ?? 'string', unit: meta?.unit ?? null, nullable: meta?.nullable };
}

// Every edit goes out as a new values object. The caller stores a copy, so
// what comes back is compared by content: a values prop that is not our own
// edit coming back (a discard, another scenario) is a replacement.
let lastSent: string | null = null;
function send(next: ScenarioValues) {
  lastSent = JSON.stringify(next);
  emit('update:values', next);
}

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

// Values replaced from outside (discarded, or another scenario) start clean:
// no refusal carries over.
watch(() => props.values, (next) => {
  if (JSON.stringify(next) === lastSent) return;
  lastSent = null;
  rejectedNullParams.value = new Set();
});

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
  const next = new Set(rejectedNullParams.value);
  let stored = value;
  if (isNullText(value) && !paramNullAllowed(name)) {
    next.add(name);
    stored = '';
  } else {
    next.delete(name);
  }
  rejectedNullParams.value = next;
  // A parameter the law declares arrives typed from its control. One without
  // a declaration is shown in a text field, and what is typed there is read
  // by its content: digits are a number, as they were in `is 50000`.
  if (!props.typeMap?.has(name) && typeof stored === 'string') {
    const read = tableCellValue(stored);
    if (typeof read === 'number') stored = read;
  }
  send({ ...props.values, parameterValues: { ...props.values.parameterValues, [name]: stored } });
}
function updateDate(value: unknown) {
  send({ ...props.values, calculationDate: String(value ?? '') });
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

// The tables as DataSourceTable reads them: data-source columns typed from
// the dependency graph, collection elements typed by content (an `array`
// input declares no item shape).
const dataSources = computed(() =>
  props.values.dataSources.map((ds) => ({ ...ds, fields: ds.fields.map((f) => typeField(f.name)) })),
);
const collections = computed(() =>
  props.values.collections.map((c) => ({
    ...c,
    columns: c.columns.map((col) => ({ name: col.name, type: 'string', unit: null })),
  })),
);

function updateDataSourceRows(index: number, rows: EditedRow[]) {
  const updated = [...props.values.dataSources];
  // `index` is the drilled-in source, an index into this list.
  updated[index] = { ...updated[index]!, rows };
  send({ ...props.values, dataSources: updated });
}

function updateCollectionRows(index: number, rows: EditedRow[]) {
  const updated = [...props.values.collections];
  // `index` is the drilled-in collection, an index into this list.
  updated[index] = { ...updated[index]!, rows };
  send({ ...props.values, collections: updated });
}

// The drilled-in table, when the drill points at one that exists.
const drilledSource = computed(() =>
  props.drill?.kind === 'source' ? dataSources.value[props.drill.index] ?? null : null,
);
const drilledCollection = computed(() =>
  props.drill?.kind === 'collection' ? collections.value[props.drill.index] ?? null : null,
);

// Data-source names render human-readable AND sentence-cased ("personal_data"
// → "Personal data"). humanize() only de-snakes; it doesn't capitalise.
function sourceLabel(name: string) {
  const h = humanize(name);
  return h ? h.charAt(0).toUpperCase() + h.slice(1) : h;
}

const expectations = computed(() => expectationsFromAssertions(props.scenario.assertions));
const hasExpectations = computed(() => Object.keys(expectations.value).length > 0);
const runError = computed(() => props.run?.error ?? null);

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
const dateInvalid = computed(() => !props.values.calculationDate);
// Unique ids for the date's validation item and for the control its list
// reads (several forms can be mounted at once, so a static id would clash).
const dateErrorId = useId();
const dateControlId = useId();
</script>

<template>
  <div class="sf-root">
    <!-- Scenario overview -->
    <template v-if="!drilledSource && !drilledCollection">
      <!-- Expected outputs -->
      <template v-if="hasExpectations">
        <nldd-title size="display-5"><h2>Verwachte uitkomsten</h2></nldd-title>
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
          <nldd-list-item v-if="showResultButton" size="md">
            <nldd-cell width="full">
              <nldd-button
                size="md"
                width="full"
                @click="emit('show-result')"
                text="Bekijk resultaat"
              ></nldd-button>
            </nldd-cell>
          </nldd-list-item>
        </nldd-list>
      </template>

      <!-- Error -->
      <nldd-banner v-if="runError" variant="critical" :text="runError"></nldd-banner>

      <!-- Input: date + parameters -->
      <nldd-spacer v-if="hasExpectations" size="16"></nldd-spacer>
      <nldd-title size="display-5"><h2>Invoer</h2></nldd-title>
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
              :value="values.calculationDate"
              :invalid="dateInvalid"
              :unmet="dateErrorId"
              @update="updateDate"
            />
            <nldd-validation-list v-if="dateInvalid" :for="dateControlId">
              <nldd-validation-item :id="dateErrorId">Datum is verplicht</nldd-validation-item>
            </nldd-validation-list>
          </nldd-cell>
        </nldd-list-item>
        <nldd-list-item v-for="(value, name) in values.parameterValues" :key="name" size="md">
          <nldd-text-cell :text="name" :supporting-text="articleMap?.paramToArticle?.get(name) ? `Artikel ${articleMap.paramToArticle.get(name)}` : undefined" min-width="120px" max-width="200px"></nldd-text-cell>
          <nldd-spacer-cell size="8"></nldd-spacer-cell>
          <nldd-cell width="full" min-width="120px">
            <ScenarioParameterInput
              :id="paramControlId(name)"
              :type="paramMeta(name).type"
              :unit="paramMeta(name).unit"
              :name="name"
              :value="value"
              :invalid="(!!runError && (value === '' || value == null)) || paramNullInvalid(name, value)"
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
          @click="emit('update:drill', { kind: 'collection', index: i })"
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
        <nldd-title size="display-5"><h2>Bronnen</h2></nldd-title>
        <nldd-spacer size="8"></nldd-spacer>
        <nldd-list appearance="box-tinted">
          <nldd-list-item
            v-for="(ds, i) in dataSources"
            :key="ds.sourceName"
            size="md"
            button
            :data-testid="`ds-row-${i}`"
            @click="emit('update:drill', { kind: 'source', index: i })"
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
         overview is the caller's (it owns the title bar), through `drill`. -->
    <template v-else-if="drilledSource && drill">
      <DataSourceTable
        :key="drilledSource.sourceName"
        :title="sourceLabel(drilledSource.sourceName)"
        :key-field="drilledSource.keyField"
        :fields="drilledSource.fields"
        :model-value="drilledSource.rows"
        :drilled-in="true"
        @update:model-value="updateDataSourceRows(drill.index, $event)"
      />
    </template>

    <!-- A collection parameter's elements: the same table, without a key column. -->
    <template v-else-if="drilledCollection && drill">
      <DataSourceTable
        :key="drilledCollection.name"
        :title="drilledCollection.name"
        :key-field="null"
        :fields="drilledCollection.columns"
        :model-value="drilledCollection.rows"
        :drilled-in="true"
        @update:model-value="updateCollectionRows(drill.index, $event)"
      />
    </template>
  </div>
</template>

<style scoped>
/* A single root that must not generate a box - otherwise it blocks the
 * enclosing simple-section's nldd flex layout (flex-grow / centering of an
 * empty data source's dialog). */
.sf-root {
  display: contents;
}
</style>
