<script setup lang="ts">
import { computed, toRef, watch } from 'vue';
import { useScenarios, isScenarioMismatch } from '../composables/useScenarios';
import { useOpenScenariosStore } from '../stores/openScenarios';
import { useScenarioEnvironment, type MappedArticle } from '../scenario/useScenarioEnvironment';
import { useScenarioRuns } from '../scenario/useScenarioRuns';
import { runStatus, type ScenarioStatus } from '../scenario/run';
import { scenarioLawsOf } from '../scenario/scenarioLaws';
import { useEngine, type EditorEngine } from '../composables/useEngine';
import type { OpenScenario } from '../scenario/openScenario';

/**
 * The open regelwerk's scenarios next to its articles, read only: each
 * scenario with the status of a run against the regelwerk as it is being
 * edited, unsaved edits included. Changing a scenario happens in its own tab;
 * a click opens it there.
 *
 * A file whose tab holds unsaved edits runs as the tab has it, and says so:
 * the pane and the tab read the same open file (stores/openScenarios).
 */
const props = withDefaults(defineProps<{
  trajectRef: string | null;
  lawId: string;
  engine: EditorEngine | null;
  engineReady: boolean;
  /** The open regelwerk's YAML, unsaved edits included. */
  lawYaml: string | null;
  articles: readonly MappedArticle[];
  /** The open regelwerk is still loading. */
  lawLoading?: boolean;
}>(), {
  lawLoading: false,
});

const emit = defineEmits<{
  /** Open a file in its tab, at a scenario. */
  open: [filename: string, scenarioName: string | null];
}>();

const store = useOpenScenariosStore();
const { scenarios: listed, loading: listLoading, error: listError } = useScenarios(
  toRef(props, 'lawId'),
  toRef(props, 'trajectRef'),
);

// The pane reads through the open files, so a tab's edits show here. Without
// a traject there is nothing to edit, and no open files either.
const files = computed<OpenScenario[]>(() => {
  const trajectRef = props.trajectRef;
  if (!trajectRef) return [];
  return listed.value.map((entry) => store.open({ trajectRef, lawId: props.lawId, filename: entry.filename }));
});
// Read each file when the pane shows it, so the statuses follow what others
// saved; a file with unsaved edits in its tab is not read over.
watch(files, (list) => list.forEach((f) => void f.load({ fresh: true })), { immediate: true });

const environment = useScenarioEnvironment({
  engine: toRef(props, 'engine'),
  ready: toRef(props, 'engineReady'),
  lawYaml: toRef(props, 'lawYaml'),
  articles: toRef(props, 'articles'),
  scenarioLaws: computed(() => [...new Set(files.value.flatMap((f) => scenarioLawsOf(f.state.value)))]),
});
const { runsFor } = useScenarioRuns({
  files,
  engine: toRef(props, 'engine'),
  environmentReady: environment.ready,
  environmentRevision: environment.revision,
  engineRevision: useEngine().lawsRevision,
});

const STATUS_TAG: Record<ScenarioStatus, { text: string; color: string }> = {
  passed: { text: 'Geslaagd', color: 'success' },
  failed: { text: 'Mislukt', color: 'critical' },
  'no-expectations': { text: 'Geen verwachting', color: 'neutral' },
};
function statusTag(file: OpenScenario, index: number) {
  const status = runStatus(runsFor(file)[index]);
  return status ? STATUS_TAG[status] : null;
}

function mismatch(file: OpenScenario) {
  return isScenarioMismatch(listed.value.find((e) => e.filename === file.file.filename), props.lawId);
}

const loading = computed(
  () => props.lawLoading || listLoading.value || files.value.some((f) => f.loading.value) || environment.loading.value,
);
const loadingText = computed(() => (environment.loading.value && environment.progress.value) || "Scenario's laden");
</script>

<template>
  <nldd-simple-section data-testid="scenario-status-pane">
    <nldd-inline-dialog
      v-if="listError"
      variant="alert"
      text="Scenario's konden niet worden geladen"
      :supporting-text="listError.message"
    ></nldd-inline-dialog>
    <nldd-inline-dialog
      v-if="environment.error.value"
      variant="alert"
      text="Afhankelijkheden konden niet worden geladen"
      :supporting-text="String(environment.error.value)"
    ></nldd-inline-dialog>

    <template v-for="file in files" :key="file.file.filename">
      <nldd-title size="display-5">
        <h2>{{ file.featureName.value || file.file.filename }}</h2>
      </nldd-title>
      <nldd-text v-if="file.featureName.value" size="sm" color="secondary">{{ file.file.filename }}</nldd-text>
      <nldd-spacer size="8"></nldd-spacer>
      <template v-if="file.dirty.value">
        <nldd-banner
          size="sm"
          variant="warning"
          :text="`Niet-opgeslagen wijzigingen in het tabblad ${file.file.filename}`"
          supporting-text="De status hieronder rekent met die wijzigingen."
        ></nldd-banner>
        <nldd-spacer size="8"></nldd-spacer>
      </template>
      <template v-if="mismatch(file)">
        <nldd-banner
          size="sm"
          variant="neutral"
          text="Dit bestand rekent met een ander regelwerk"
        ></nldd-banner>
        <nldd-spacer size="8"></nldd-spacer>
      </template>
      <nldd-inline-dialog
        v-if="file.error.value"
        variant="alert"
        text="Dit bestand kon niet worden geladen"
        :supporting-text="file.error.value.message"
      ></nldd-inline-dialog>
      <nldd-list v-else appearance="box-tinted" :accessible-label="`Scenario's in ${file.file.filename}`">
        <nldd-list-item
          v-for="(scenario, i) in file.scenarios.value"
          :key="i"
          size="md"
          button
          :data-testid="`scenario-status-${file.file.filename}-${i}`"
          @click="emit('open', file.file.filename, scenario.name)"
        >
          <nldd-text-cell :text="scenario.name"></nldd-text-cell>
          <nldd-cell v-if="statusTag(file, i)" width="fit-content">
            <nldd-tag size="sm" :color="statusTag(file, i)!.color" :text="statusTag(file, i)!.text"></nldd-tag>
          </nldd-cell>
          <nldd-icon-cell size="20"><nldd-icon icon="chevron-right"></nldd-icon></nldd-icon-cell>
        </nldd-list-item>
      </nldd-list>
      <nldd-spacer size="24"></nldd-spacer>
    </template>

    <nldd-activity-indicator v-if="loading" show-text :text="loadingText"></nldd-activity-indicator>
    <nldd-inline-dialog
      v-else-if="!listError && !files.length"
      text="Geen scenario's voor dit regelwerk."
    ></nldd-inline-dialog>
  </nldd-simple-section>
</template>
