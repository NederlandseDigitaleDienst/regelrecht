<script lang="ts">
import type { TraceResult } from '../scenario/run';
import type { Expectation } from '../utils/outputFormat';

/** Which sheet the parent opens for a scenario's result. */
type DetailsView = 'trace' | 'graph';

/** What `executed` hands the parent: a scenario's last run and how to re-run it. */
export interface ScenarioExecutedPayload {
  result: TraceResult | null;
  traceText: string | null;
  error: string | null;
  running: boolean;
  expectations: Record<string, Expectation>;
  scenarioName: string;
  outputName: string | null;
  index: number;
  reload: () => void;
  view: DetailsView;
}
</script>

<script setup lang="ts">
import { ref, computed, watch, toRef, onBeforeUnmount } from 'vue';
import { useScenarios, isScenarioMismatch, preferredScenarioFile } from '../composables/useScenarios';
import { useEngine, type EditorEngine } from '../composables/useEngine';
import { useOpenScenariosStore } from '../stores/openScenarios';
import { useScenarioEnvironment, type MappedArticle } from '../scenario/useScenarioEnvironment';
import { useScenarioRuns } from '../scenario/useScenarioRuns';
import { scenarioLawsOf } from '../scenario/scenarioLaws';
import { CONFLICT_MESSAGE, type OpenScenario } from '../scenario/openScenario';
import type { ScenarioValues } from '../scenario/inputs';
import { humanize, formatOutputValue, normalizeForCompare, expectationsFromAssertions } from '../utils/outputFormat';
import { buildArticleMap, buildTypeMap } from '../utils/articleMapping';
import ScenarioForm, { type ScenarioDrill } from './ScenarioForm.vue';
import type { NLDDSheet } from '@nldd/design-system/sheet';

/**
 * The open regelwerk's scenarios as cards, with an edit sheet per scenario.
 *
 * The selected feature file is an open file (stores/openScenarios): it holds
 * the text, the ETag and the unsaved edits. Its scenarios run against the
 * regelwerk as it is being edited (`lawYaml`), through the same conversion a
 * save writes (scenario/inputs).
 */
const props = withDefaults(defineProps<{
  lawId: string;
  lawYaml?: string | null;
  engine?: EditorEngine | null;
  ready?: boolean;
  /** The shared article/law load is still in flight. The pane renders us during
   *  that load (no `lawYaml` yet) so our indicator can span BOTH phases as one
   *  mounted element: article -> scenario files -> dependency laws. Two separate
   *  indicators handing off would each restart the DS anti-flash timer on mount,
   *  giving spinner -> 1s blank -> spinner. */
  articleLoading?: boolean;
  /** Articles array from useOpenLaw() for article mapping */
  articles?: MappedArticle[];
  /** Active traject ref. Scenario files are read and written in a traject. */
  trajectRef?: string | null;
}>(), {
  lawYaml: null,
  engine: null,
  ready: false,
  articleLoading: false,
  articles: () => [],
  trajectRef: null,
});

const emit = defineEmits<{
  executed: [payload: ScenarioExecutedPayload];
  'dirty-change': [dirty: boolean];
}>();

// --- Maps for the form ---
const articleMap = computed(() => buildArticleMap(props.articles));
// Datatype mapping: drives the per-type scenario input controls (boolean ->
// switch, amount -> currency field, etc.) in ScenarioForm.
const typeMap = computed(() => buildTypeMap(props.articles));

// --- The feature files of this regelwerk, and the one that is shown ---
const {
  scenarios: scenarioFiles,
  loading: listLoading,
  error: listError,
} = useScenarios(toRef(props, 'lawId'), toRef(props, 'trajectRef'));

const chosenFilename = ref<string | null>(null);
// The chosen file; without a choice (or when it went away) the one that
// fits the open regelwerk best.
const selectedFilename = computed(() => {
  const listed = scenarioFiles.value;
  return (
    listed.find((sf) => sf.filename === chosenFilename.value)?.filename ??
    preferredScenarioFile(listed, props.lawId)?.filename ??
    null
  );
});
function onScenarioFileSelect(event: Event) {
  chosenFilename.value = (event.target as HTMLSelectElement).value || null;
}

const store = useOpenScenariosStore();
const file = computed<OpenScenario | null>(() => {
  const trajectRef = props.trajectRef;
  const filename = selectedFilename.value;
  if (!trajectRef || !filename) return null;
  return store.open({ trajectRef, lawId: props.lawId, filename });
});
// Read the file each time it is shown, so it follows what others saved; a
// file with unsaved edits is not read over (see `load`).
watch(file, (f) => void f?.load({ fresh: true }), { immediate: true });
// Edits live only while the edit sheet is open. Another file, regelwerk or
// traject closes it, and leaving the screen too: those edits are gone then,
// as the sheet showed them, rather than kept out of sight in the store.
watch(file, (_next, prev) => prev?.discard());
onBeforeUnmount(() => file.value?.discard());

const scenarios = computed(() => file.value?.scenarios.value ?? []);
const dirty = computed(() => !!file.value?.dirty.value);
watch(dirty, (val) => emit('dirty-change', val));

// Mismatch warning: the selected scenario file declares execution targets
// that do not include the opened law. Running it would evaluate that other
// law - surface this instead of letting the run fail confusingly.
const selectedScenarioMismatchTargets = computed(() => {
  const entry = scenarioFiles.value.find((sf) => sf.filename === selectedFilename.value);
  return entry && isScenarioMismatch(entry, props.lawId) ? entry.target_law_ids : null;
});
const mismatchSupportingText = computed(() =>
  selectedScenarioMismatchTargets.value
    ? `Dit scenario evalueert '${selectedScenarioMismatchTargets.value.join("', '")}', niet dit regelwerk ('${props.lawId}'). Uitvoeren gebruikt dat andere regelwerk.`
    : '',
);

// --- Engine: the laws the scenarios need, and their runs ---
const environment = useScenarioEnvironment({
  engine: toRef(props, 'engine'),
  ready: toRef(props, 'ready'),
  lawYaml: toRef(props, 'lawYaml'),
  articles: toRef(props, 'articles'),
  scenarioLaws: computed(() => scenarioLawsOf(file.value?.state.value ?? null)),
});
const { runsFor, runNow } = useScenarioRuns({
  files: computed(() => (file.value ? [file.value] : [])),
  engine: toRef(props, 'engine'),
  environmentReady: environment.ready,
  environmentRevision: environment.revision,
  engineRevision: useEngine().lawsRevision,
  loadErrorFor: environment.loadErrorFor,
});
const runs = computed(() => (file.value ? runsFor(file.value) : []));

// The same map the result views compare against, rendered the way they
// render an outcome, so the card says `geen` / `onbekend` where the sheet
// does (RFC-036).
function scenarioExpectations(index: number) {
  const assertions = scenarios.value[index]?.assertions || [];
  return Object.entries(expectationsFromAssertions(assertions)).map(([name, expected]) => ({
    name: humanize(name),
    value: humanize(formatOutputValue(normalizeForCompare(expected), null)),
  }));
}

// --- Result: handed to the parent, which opens the trace or graph sheet ---
// Not gated on a run: the result sheet handles its own loading / empty /
// error (with reload) states, so the buttons are never a dead end.
function onShowDetails(index: number, view: DetailsView = 'trace') {
  const run = runs.value[index] ?? null;
  emit('executed', {
    result: run?.result ?? null,
    traceText: run?.traceText ?? null,
    error: run?.error ?? null,
    running: !!run?.pending,
    expectations: run?.expectations ?? {},
    scenarioName: scenarios.value[index]?.name ?? '',
    // The scenario's entry output, so the graph view can pin its
    // "▶ start" marker to the right output leaf.
    outputName: run?.outputName ?? null,
    index,
    // Re-runs this builder's file, then hands the fresh outcome over. The
    // engine runs synchronously, so the run has settled when runNow returns.
    reload: () => {
      runNow();
      onShowDetails(index, view);
    },
    view,
  });
}

// --- Edit sheet ---
const selectedScenarioIndex = ref<number | null>(null);
const scenarioSheetEl = ref<NLDDSheet | null>(null);
// The file the open sheet edits. The sheet's close event comes after its
// animation, when `file` may already be another one.
let editingFile: OpenScenario | null = null;
const drill = ref<ScenarioDrill>(null);
const saveSuccess = ref(false);

watch(selectedScenarioIndex, (idx) => {
  // Opening or switching a scenario always lands on its overview.
  drill.value = null;
  if (idx !== null) {
    editingFile = file.value;
    scenarioSheetEl.value?.show();
  }
  else scenarioSheetEl.value?.hide();
});
// Another file: the open sheet belonged to the previous one.
watch(file, () => {
  editingFile = null;
  selectedScenarioIndex.value = null;
});

const selectedValues = computed(() => {
  const idx = selectedScenarioIndex.value;
  return idx === null ? null : file.value?.values.value[idx] ?? null;
});
const currentScenarioName = computed(() =>
  selectedScenarioIndex.value !== null ? scenarios.value[selectedScenarioIndex.value]?.name ?? '' : '',
);
// The data source or collection the form is drilled into, for the back button.
const drilledName = computed(() => {
  const d = drill.value;
  const v = selectedValues.value;
  if (!d || !v) return null;
  return d.kind === 'source' ? v.dataSources[d.index]?.sourceName ?? null : v.collections[d.index]?.name ?? null;
});

function onValues(values: ScenarioValues) {
  const idx = selectedScenarioIndex.value;
  if (idx !== null) file.value?.setValues(idx, values);
}

const saving = computed(() => !!file.value?.saving.value);
const saveError = computed(() => file.value?.saveError.value ?? null);
// Someone else saved the file since it was read: the way out is its newest version.
const saveConflict = computed(() => saveError.value?.message === CONFLICT_MESSAGE);

async function onSave(): Promise<boolean> {
  const doc = file.value;
  if (!doc) return false;
  saveSuccess.value = false;
  const saved = await doc.save();
  if (!saved) return false;
  saveSuccess.value = true;
  // What was typed while the save ran is not in it: the sheet stays open
  // with those edits rather than throwing them away on close.
  if (!doc.dirty.value) selectedScenarioIndex.value = null;
  setTimeout(() => { saveSuccess.value = false; }, 3000);
  return true;
}

async function onSaveAndShow() {
  const idx = selectedScenarioIndex.value;
  if (idx === null || !(await onSave())) return;
  // Run the saved file before showing it, rather than wait for the next run.
  runNow();
  onShowDetails(idx);
}

// Closing the sheet throws its edits away; only one scenario is edited at a
// time, so these are the file's only edits.
function cancelEdits() {
  editingFile?.discard();
  editingFile = null;
  selectedScenarioIndex.value = null;
}

const loading = computed(
  () => props.articleLoading || listLoading.value || !!file.value?.loading.value || environment.loading.value,
);
</script>

<template>
  <!-- Overview. Wrapped in a positioned container so the loading overlay can
       cover the whole pane. -->
  <div class="sb-pane">
    <nldd-simple-section>
      <nldd-dropdown v-if="scenarioFiles.length > 1" size="md">
        <select
          :value="selectedFilename"
          @change="onScenarioFileSelect"
        >
          <option v-for="sf in scenarioFiles" :key="sf.filename" :value="sf.filename">
            {{ isScenarioMismatch(sf, lawId) ? '⚠ ' + sf.filename : sf.filename }}
          </option>
        </select>
      </nldd-dropdown>

      <template v-if="selectedScenarioMismatchTargets">
        <nldd-banner
          variant="warning"
          text="Scenario hoort bij een ander regelwerk"
          :supporting-text="mismatchSupportingText"
        ></nldd-banner>
        <nldd-spacer size="16"></nldd-spacer>
      </template>

      <nldd-inline-dialog v-if="saveSuccess" text="Opgeslagen"></nldd-inline-dialog>

      <nldd-inline-dialog
        v-if="environment.error.value"
        variant="alert"
        text="Fout"
        :supporting-text="String(environment.error.value)"
      ></nldd-inline-dialog>

      <nldd-inline-dialog
        v-if="listError || file?.error.value"
        variant="alert"
        text="Scenario's konden niet worden geladen"
        :supporting-text="(listError ?? file?.error.value)?.message"
      ></nldd-inline-dialog>

      <template v-if="scenarios.length">
        <nldd-collection layout="grid" item-min-width="320px">
          <nldd-card v-for="(scenario, i) in scenarios" :key="i">
            <nldd-container slot="header" padding-top="16" padding-inline="16">
              <nldd-title size="display-4"><h3>{{ scenario.name }}</h3></nldd-title>
            </nldd-container>
            <nldd-container padding="16">
              <template v-if="scenarioExpectations(i).length">
                <nldd-title size="display-6"><h4>Verwachte uitkomsten</h4></nldd-title>
                <nldd-spacer size="4"></nldd-spacer>
                <nldd-list appearance="simple">
                  <nldd-list-item v-for="(exp, j) in scenarioExpectations(i)" :key="j" size="sm">
                    <nldd-text-cell size="sm" :text="exp.name"></nldd-text-cell>
                    <nldd-text-cell size="sm" horizontal-alignment="right" :text="exp.value"></nldd-text-cell>
                  </nldd-list-item>
                </nldd-list>
              </template>
              <template v-else>
                <nldd-text size="sm" color="secondary">Geen verwachte uitkomsten gedefinieerd</nldd-text>
              </template>
            </nldd-container>
            <nldd-container slot="footer" padding-inline="16" padding-bottom="16">
              <nldd-button-group orientation="horizontal">
                <nldd-button
                  text="Resultaat"
                  @click="onShowDetails(i, 'trace')"
                ></nldd-button>
                <nldd-button
                  appearance="secondary"
                  text="Graaf"
                  @click="onShowDetails(i, 'graph')"
                ></nldd-button>
                <nldd-button
                  text="Bewerk"
                  @click="selectedScenarioIndex = i"
                ></nldd-button>
              </nldd-button-group>
            </nldd-container>
          </nldd-card>
        </nldd-collection>
      </template>

      <nldd-inline-dialog
        v-else-if="!loading && !listError && !file?.error.value"
        text="Geen scenario's beschikbaar voor dit artikel."
      ></nldd-inline-dialog>
    </nldd-simple-section>
    <!-- Full-pane loading overlay, shown across the WHOLE scenario story: the
         article/law load, then the scenario files, then their dependency laws
         ("X/Y regelwerken geladen"). Deliberately one condition over all of
         them so this element stays mounted throughout - the DS resets its
         anti-flash timer on every connectedCallback, so handing off between
         separate indicators produced spinner -> 1s blank -> spinner. -->
    <nldd-activity-indicator
      v-if="loading"
      show-text
      :text="environment.loading.value && environment.progress.value ? environment.progress.value : 'Scenario\'s laden'"
      style="position: absolute; inset: 0;"
    ></nldd-activity-indicator>
  </div>

  <!-- Edit a scenario in a side sheet. -->
  <Teleport to="body">
    <nldd-sheet
      v-if="file"
      ref="scenarioSheetEl"
      placement="right"
      width="640px"
      @close="cancelEdits"
    >
      <nldd-page sticky-header :sticky-footer="dirty || undefined">
        <!-- Overview header: scenario name (revealed in the bar on scroll
             via the content-title anchor), no back. -->
        <nldd-top-title-bar
          v-if="!drilledName"
          slot="header"
          :text="currentScenarioName"
          collapse-anchor="scenario-title-anchor"
          dismiss-text="Annuleer"
          @dismiss="cancelEdits"
        ></nldd-top-title-bar>
        <!-- Drilled-in header: its own bar - a back button to the scenario
             overview (the data-source heading lives in the content). -->
        <nldd-top-title-bar
          v-else
          slot="header"
          :back-text="currentScenarioName"
          dismiss-text="Annuleer"
          @back="drill = null"
          @dismiss="cancelEdits"
        ></nldd-top-title-bar>
        <nldd-simple-section>
          <template v-if="!drilledName">
            <nldd-title id="scenario-title-anchor" size="display-3"><h2>{{ currentScenarioName }}</h2></nldd-title>
            <nldd-spacer size="16"></nldd-spacer>
          </template>
          <nldd-inline-dialog
            v-if="saveError"
            variant="alert"
            text="Opslaan mislukt"
            :supporting-text="saveError.message"
          >
            <nldd-button
              v-if="saveConflict"
              slot="actions"
              appearance="secondary"
              text="Laad nieuwste versie"
              @click="file.reload()"
            ></nldd-button>
          </nldd-inline-dialog>
          <!-- selectedValues is set only for an index within the file's
               scenarios, so the scenario at that index exists. -->
          <ScenarioForm
            v-if="selectedScenarioIndex !== null && selectedValues"
            :key="selectedScenarioIndex"
            :scenario="scenarios[selectedScenarioIndex]!"
            :values="selectedValues"
            :run="runs[selectedScenarioIndex] ?? null"
            show-result-button
            :drill="drill"
            :article-map="articleMap"
            :type-map="typeMap"
            :external-field-type-map="environment.externalFieldTypeMap.value"
            @update:values="onValues"
            @update:drill="drill = $event"
            @show-result="onShowDetails(selectedScenarioIndex)"
          />
        </nldd-simple-section>
        <nldd-container v-if="dirty" slot="footer" padding="16">
          <nldd-button-group orientation="vertical">
            <nldd-button
              appearance="primary"
              size="md"
              data-testid="save-scenarios-btn"
              :disabled="saving || undefined"
              :text="saving ? 'Opslaan…' : 'Opslaan'"
              @click="onSave"
            ></nldd-button>
            <nldd-button
              size="md"
              :disabled="saving || undefined"
              text="Opslaan en toon resultaat"
              @click="onSaveAndShow"
            ></nldd-button>
          </nldd-button-group>
        </nldd-container>
      </nldd-page>
    </nldd-sheet>
  </Teleport>
</template>

<style scoped>
/* Positioning context for the full-pane loading overlay. min-height fills the
   pane's scroll viewport so the backdrop covers the whole area, not just the
   (possibly empty) content. Flex column so the simple-section grows to the full
   height - its empty-state inline-dialog then self-centers like elsewhere,
   instead of sitting at the top. The absolute overlay is unaffected. */
.sb-pane {
  display: flex;
  position: relative;
  min-height: 100%;
  flex-direction: column;
}
</style>
