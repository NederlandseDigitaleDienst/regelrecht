<script setup lang="ts">
import { computed, defineAsyncComponent, ref, watch, toRef } from 'vue';
import ScenarioForm, { type ScenarioDrill } from './ScenarioForm.vue';
import ScenarioResult from './ScenarioResult.vue';
import { useOpenScenariosStore } from '../stores/openScenarios';
import { useScenarioEnvironment, type MappedArticle } from '../scenario/useScenarioEnvironment';
import { useScenarioRuns } from '../scenario/useScenarioRuns';
import { runStatus } from '../scenario/run';
import { STATUS_BADGE, STATUS_TEXT } from '../scenario/statusSummary';
import { scenarioLawsOf } from '../scenario/scenarioLaws';
import { useCorpus } from '../corpus';
import { buildArticleMap, buildOutputTypeMap, buildTypeMap } from '../utils/articleMapping';
import { useEngine, type EditorEngine } from '../composables/useEngine';
import type { ScenarioValues } from '../scenario/inputs';
import { CONFLICT_MESSAGE } from '../scenario/openScenario';
import type { NLDDSheet } from '@nldd/design-system/sheet';

// The graph pulls in Vue Flow; load it only when someone opens it.
const LawGraphView = defineAsyncComponent(() => import('./LawGraphView.vue'));

/**
 * One feature file in its own editor tab: the scenarios on the left, the form
 * of the selected one, and its result next to it. The edits live in the open
 * file (stores/openScenarios), so they survive a switch to another tab.
 *
 * The scenarios run against the open regelwerk as the editor holds it, unsaved
 * edits included (`lawYaml`); the editor loads that into the engine.
 */
const props = withDefaults(defineProps<{
  trajectRef: string;
  /** The regelwerk whose scenario folder holds the file. */
  lawId: string;
  filename: string;
  /** The scenario to show, by name; the first when null or unknown. */
  scenarioName?: string | null;
  /** The open regelwerk's display name. */
  lawName?: string;
  engine: EditorEngine | null;
  engineReady: boolean;
  /** The open regelwerk's YAML, unsaved edits included. */
  lawYaml: string | null;
  /** The open regelwerk's articles. */
  articles: readonly MappedArticle[];
  /** Whether the open regelwerk has edits that are not saved. */
  lawUnsaved?: boolean;
}>(), {
  scenarioName: null,
  lawName: '',
  lawUnsaved: false,
});

const emit = defineEmits<{
  'select-scenario': [name: string];
}>();

const store = useOpenScenariosStore();
const corpus = useCorpus();

const file = computed(() => store.open({ trajectRef: props.trajectRef, lawId: props.lawId, filename: props.filename }));
// Read the file each time it is shown, so it follows what others saved; an
// open file with unsaved edits is not read over (see `load`).
watch(file, (f) => void f.load({ fresh: true }), { immediate: true });

const scenarios = computed(() => file.value.scenarios.value);
const selected = computed(() => file.value.indexOfName(props.scenarioName));
const selectedScenario = computed(() => scenarios.value[selected.value] ?? null);
const selectedValues = computed(() => file.value.values.value[selected.value] ?? null);

// --- Engine ---------------------------------------------------------------
const environment = useScenarioEnvironment({
  engine: toRef(props, 'engine'),
  ready: toRef(props, 'engineReady'),
  lawYaml: toRef(props, 'lawYaml'),
  articles: toRef(props, 'articles'),
  scenarioLaws: computed(() => scenarioLawsOf(file.value.state.value)),
});
const { runsFor, runNow } = useScenarioRuns({
  files: computed(() => [file.value]),
  engine: toRef(props, 'engine'),
  environmentReady: environment.ready,
  environmentRevision: environment.revision,
  engineRevision: useEngine().lawsRevision,
  loadErrorFor: environment.loadErrorFor,
});
const runs = computed(() => runsFor(file.value));
const selectedRun = computed(() => runs.value[selected.value] ?? null);

// --- Status per scenario --------------------------------------------------
function statusBadgeFor(index: number) {
  const status = runStatus(runs.value[index]);
  return status ? { ...STATUS_BADGE[status], label: STATUS_TEXT[status] } : null;
}
const dirtySet = computed(() => new Set(file.value.dirtyIndexes.value));

// --- Maps for the form ----------------------------------------------------
const articleMap = computed(() => buildArticleMap(props.articles));
const typeMap = computed(() => buildTypeMap(props.articles));

// The regelwerk the selected scenario executes. Its output types are known
// only when that is the open one.
const executedLawId = computed(() => selectedScenario.value?.execution?.lawId ?? props.lawId);
const executesOther = computed(() => executedLawId.value !== props.lawId);
const outputTypes = computed(() => (executesOther.value ? null : buildOutputTypeMap(props.articles)));
const executedLawName = computed(() => corpus.displayName(executedLawId.value));

// --- Drill-in and edits ---------------------------------------------------
const drill = ref<ScenarioDrill>(null);
watch(selected, () => { drill.value = null; });
const drilledName = computed(() => {
  const d = drill.value;
  const v = selectedValues.value;
  if (!d || !v) return null;
  return d.kind === 'source' ? v.dataSources[d.index]?.sourceName ?? null : v.collections[d.index]?.name ?? null;
});

function onValues(values: ScenarioValues) {
  file.value.setValues(selected.value, values);
}

function select(index: number) {
  const name = scenarios.value[index]?.name;
  if (name !== undefined) emit('select-scenario', name);
}

const saved = ref(false);
let savedTimer: ReturnType<typeof setTimeout> | undefined;
async function save() {
  saved.value = false;
  if (await file.value.save()) {
    saved.value = true;
    clearTimeout(savedTimer);
    savedTimer = setTimeout(() => { saved.value = false; }, 3000);
  }
}
function discard() {
  file.value.discard();
  drill.value = null;
}

// --- Result: a pane when it fits, a sheet when only the form fits -------
const singleColumn = ref(false);
const resultSheetEl = ref<NLDDSheet | null>(null);
function onSingleColumnChange(e: Event) {
  singleColumn.value = !!(e as CustomEvent<{ singleColumn: boolean }>).detail?.singleColumn;
  if (!singleColumn.value) resultSheetEl.value?.hide?.();
}
function showResult() {
  resultSheetEl.value?.show?.();
}

// --- Graph: a sheet of its own, over the full width ----------------------
// Mounted only while open: building the Vue Flow graph for a regelwerk with
// many cross-law references is not cheap.
const graphOpen = ref(false);
function showGraph() {
  resultSheetEl.value?.hide?.();
  graphOpen.value = true;
}

const saving = computed(() => file.value.saving.value);
const saveError = computed(() => file.value.saveError.value);
// Someone else saved the file since it was read: the way out is its newest version.
const saveConflict = computed(() => saveError.value?.message === CONFLICT_MESSAGE);
const loadError = computed(() => file.value.error.value);
const loading = computed(() => file.value.loading.value);
const dirty = computed(() => file.value.dirty.value);
</script>

<template>
  <nldd-navigation-split-view primary-sidebar-accessible-label="Scenario's" data-testid="scenario-screen">
    <!-- The file's scenarios, each with the status of its latest run. -->
    <nldd-split-view-pane slot="primary-sidebar" has-content>
      <nldd-page sticky-header>
        <nldd-container slot="header" padding="8" padding-bottom="0">
          <nldd-top-title-bar :text="filename" :supporting-text="lawName || undefined"></nldd-top-title-bar>
        </nldd-container>
        <nldd-simple-section width="full">
          <nldd-activity-indicator v-if="loading" text="Scenario's laden" show-text></nldd-activity-indicator>
          <nldd-inline-dialog
            v-else-if="loadError"
            variant="alert"
            text="Scenario's konden niet worden geladen"
            :supporting-text="loadError.message"
          >
            <nldd-button slot="actions" appearance="secondary" text="Probeer opnieuw" @click="file.load({ fresh: true })"></nldd-button>
          </nldd-inline-dialog>
          <nldd-inline-dialog
            v-else-if="file.loaded.value && !scenarios.length"
            text="Dit bestand bevat geen scenario's die de editor kan lezen."
          ></nldd-inline-dialog>
          <template v-else>
            <nldd-banner
              v-if="file.duplicateNames.value.length"
              size="sm"
              variant="warning"
              :text="file.duplicateNames.value.length === 1
                ? `Meer dan één scenario heet '${file.duplicateNames.value[0]}'`
                : `Meer dan één scenario heet ${file.duplicateNames.value.map((n) => `'${n}'`).join(', ')}`"
              supporting-text="Een link naar zo'n scenario opent het eerste met die naam. Geef elk scenario een eigen naam."
              data-testid="scenario-duplicate-names"
            ></nldd-banner>
            <nldd-spacer v-if="file.duplicateNames.value.length" size="8"></nldd-spacer>
            <nldd-list accessible-label="Scenario's in dit bestand">
              <nldd-list-item
                v-for="(scenario, i) in scenarios"
                :key="i"
                size="md"
                button
                :current="i === selected || undefined"
                :data-testid="`scenario-item-${i}`"
                @click="select(i)"
              >
                <nldd-text-cell
                  :text="scenario.name"
                  :supporting-text="dirtySet.has(i) ? 'Niet opgeslagen' : undefined"
                ></nldd-text-cell>
                <nldd-spacer-cell v-if="statusBadgeFor(i)" size="8"></nldd-spacer-cell>
                <nldd-cell v-if="statusBadgeFor(i)" width="fit-content">
                  <nldd-badge
                    size="sm"
                    :color="statusBadgeFor(i)!.color"
                    :icon="statusBadgeFor(i)!.icon"
                    :accessible-label="statusBadgeFor(i)!.label"
                  ></nldd-badge>
                </nldd-cell>
              </nldd-list-item>
            </nldd-list>
          </template>
        </nldd-simple-section>
      </nldd-page>
    </nldd-split-view-pane>

    <!-- The selected scenario: its form, and its result next to it. -->
    <nldd-split-view-pane slot="main" has-content>
      <nldd-side-by-side-split-view panes="2" @nldd-single-column-change="onSingleColumnChange">
        <nldd-split-view-pane slot="pane-1" data-testid="scenario-form-pane">
          <nldd-page sticky-header :sticky-footer="dirty || undefined" landmarks="page">
            <nldd-container slot="header" padding="8" padding-bottom="0">
              <nldd-top-title-bar
                v-if="!drilledName"
                :text="selectedScenario?.name ?? ''"
                :supporting-text="executesOther ? `Rekent met ${executedLawName}` : undefined"
              ></nldd-top-title-bar>
              <nldd-top-title-bar
                v-else
                :text="drilledName"
                :back-text="selectedScenario?.name ?? ''"
                @back="drill = null"
              ></nldd-top-title-bar>
            </nldd-container>
            <nldd-simple-section>
              <nldd-banner
                v-if="lawUnsaved"
                size="sm"
                variant="warning"
                :text="`Rekent met niet-opgeslagen wijzigingen in ${lawName || lawId}`"
              ></nldd-banner>
              <nldd-inline-dialog v-if="saved" text="Opgeslagen"></nldd-inline-dialog>
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
                  data-testid="scenario-reload-btn"
                  @click="file.reload()"
                ></nldd-button>
              </nldd-inline-dialog>
              <nldd-inline-dialog
                v-if="environment.error.value"
                variant="alert"
                text="Afhankelijkheden konden niet worden geladen"
                :supporting-text="String(environment.error.value)"
              ></nldd-inline-dialog>
              <ScenarioForm
                v-if="selectedScenario && selectedValues"
                :key="selected"
                :scenario="selectedScenario"
                :values="selectedValues"
                :run="selectedRun"
                :show-result-button="singleColumn"
                :drill="drill"
                :article-map="articleMap"
                :type-map="typeMap"
                :external-field-type-map="environment.externalFieldTypeMap.value"
                @update:values="onValues"
                @update:drill="drill = $event"
                @show-result="showResult"
              />
            </nldd-simple-section>
            <nldd-activity-indicator
              v-if="environment.loading.value"
              show-text
              :text="environment.progress.value || 'Regelwerken laden'"
            ></nldd-activity-indicator>
            <nldd-container v-if="dirty" slot="footer" padding="16">
              <nldd-button-group orientation="horizontal">
                <nldd-button
                  appearance="primary"
                  size="md"
                  data-testid="save-scenarios-btn"
                  :disabled="saving || undefined"
                  :text="saving ? 'Opslaan…' : 'Opslaan'"
                  @click="save"
                ></nldd-button>
                <nldd-button
                  size="md"
                  :disabled="saving || undefined"
                  text="Wijzigingen weggooien"
                  @click="discard"
                ></nldd-button>
              </nldd-button-group>
            </nldd-container>
          </nldd-page>
        </nldd-split-view-pane>
        <nldd-split-view-pane slot="pane-2" data-testid="scenario-result-pane">
          <nldd-page sticky-header>
            <nldd-container slot="header" padding="8" padding-bottom="0">
              <nldd-top-title-bar text="Resultaat">
                <nldd-button
                  slot="toolbar"
                  appearance="secondary"
                  text="Graaf"
                  data-testid="scenario-graph-btn"
                  @click="showGraph"
                ></nldd-button>
              </nldd-top-title-bar>
            </nldd-container>
            <ScenarioResult
              v-if="!singleColumn"
              :run="selectedRun"
              :output-types="outputTypes"
              @rerun="runNow"
            />
          </nldd-page>
        </nldd-split-view-pane>
      </nldd-side-by-side-split-view>
    </nldd-split-view-pane>
  </nldd-navigation-split-view>

  <!-- The result when only the form fits. -->
  <Teleport to="body">
    <nldd-sheet ref="resultSheetEl" placement="bottom">
      <nldd-page sticky-header>
        <nldd-container slot="header" padding="8" padding-bottom="0">
          <nldd-top-title-bar
            :text="`Resultaat: ${selectedScenario?.name ?? ''}`"
            dismiss-text="Sluit"
            @dismiss="resultSheetEl?.hide?.()"
          >
            <nldd-button slot="toolbar" appearance="secondary" text="Graaf" @click="showGraph"></nldd-button>
          </nldd-top-title-bar>
        </nldd-container>
        <ScenarioResult
          v-if="singleColumn"
          :run="selectedRun"
          :output-types="outputTypes"
          @rerun="runNow"
        />
      </nldd-page>
    </nldd-sheet>

    <!-- The law graph with the run's trace laid over it. -->
    <nldd-sheet
      placement="bottom"
      :open="graphOpen || undefined"
      data-testid="scenario-graph-sheet"
      @close="graphOpen = false"
    >
      <nldd-page sticky-header>
        <nldd-container slot="header" padding="8" padding-bottom="0">
          <nldd-top-title-bar
            :text="`Graaf: ${selectedScenario?.name ?? ''}`"
            dismiss-text="Sluit"
            @dismiss="graphOpen = false"
          ></nldd-top-title-bar>
        </nldd-container>
        <LawGraphView
          v-if="graphOpen"
          :law-id="executedLawId"
          :result="selectedRun?.result ?? null"
          :output-name="selectedRun?.outputName ?? null"
          :expectations="selectedRun?.expectations ?? {}"
        />
      </nldd-page>
    </nldd-sheet>
  </Teleport>
</template>
