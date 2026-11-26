/**
 * The open regelwerk's scenarios, run against the regelwerk as it is being
 * edited, unsaved edits included. One instance serves the editor: the status
 * in the Machine pane's toolbar and the *Scenario's* pane read the same runs.
 *
 * The files are read through the open files (stores/openScenarios), so a
 * scenario tab's unsaved edits run here too.
 */
import { computed, watch, type Ref } from 'vue';
import { useScenarios, isScenarioMismatch } from '../composables/useScenarios';
import { useEngine, type EditorEngine } from '../composables/useEngine';
import { useOpenScenariosStore } from '../stores/openScenarios';
import type { OpenScenario } from './openScenario';
import { useScenarioEnvironment, type MappedArticle } from './useScenarioEnvironment';
import { useScenarioRuns } from './useScenarioRuns';
import { scenarioLawsOf } from './scenarioLaws';
import { countStatuses, summarizeRuns, type FileSummary } from './statusSummary';

export interface LawScenarioInputs {
  trajectRef: Readonly<Ref<string | null>>;
  /** The open regelwerk; null when none is open. */
  lawId: Readonly<Ref<string | null>>;
  engine: Readonly<Ref<EditorEngine | null>>;
  engineReady: Readonly<Ref<boolean>>;
  /** The open regelwerk's YAML, unsaved edits included. */
  lawYaml: Readonly<Ref<string | null>>;
  articles: Readonly<Ref<readonly MappedArticle[]>>;
  /** The open regelwerk is still loading. */
  lawLoading: Readonly<Ref<boolean>>;
}

/** One feature file with the outcome of its runs. */
export interface FileStatus extends FileSummary {
  file: OpenScenario;
  /** The file runs another regelwerk than the open one. */
  mismatch: boolean;
}

export function useLawScenarios(inputs: LawScenarioInputs) {
  const store = useOpenScenariosStore();
  const { scenarios: listed, loading: listLoading, error: listError, fetchScenarios } = useScenarios(inputs.lawId, inputs.trajectRef);

  // Without a traject there is nothing to edit, and no open files either.
  const files = computed<OpenScenario[]>(() => {
    const trajectRef = inputs.trajectRef.value;
    const lawId = inputs.lawId.value;
    if (!trajectRef || !lawId) return [];
    return listed.value.map((entry) => store.open({ trajectRef, lawId, filename: entry.filename }));
  });
  // Read each file when it is listed, so the statuses follow what others
  // saved; a file with unsaved edits in its tab is not read over.
  watch(files, (list) => list.forEach((f) => void f.load({ fresh: true })), { immediate: true });

  // Only a regelwerk with scenario files needs its dependencies in the engine
  // for them. Without a regelwerk (the status is off, or a scenario tab runs
  // its own file) or without files, the environment stays idle: the YAML
  // changes on every pause in typing, and each change would load again.
  const active = computed(() => files.value.length > 0);
  const environment = useScenarioEnvironment({
    engine: inputs.engine,
    ready: inputs.engineReady,
    lawYaml: computed(() => (active.value ? inputs.lawYaml.value : null)),
    articles: inputs.articles,
    scenarioLaws: computed(() => [...new Set(files.value.flatMap((f) => scenarioLawsOf(f.state.value)))]),
  });
  const { runsFor } = useScenarioRuns({
    files,
    engine: inputs.engine,
    environmentReady: environment.ready,
    environmentRevision: environment.revision,
    engineRevision: useEngine().lawsRevision,
    loadErrorFor: environment.loadErrorFor,
  });

  const fileStatuses = computed<FileStatus[]>(() =>
    files.value.map((file) => ({
      file,
      mismatch: isScenarioMismatch(listed.value.find((e) => e.filename === file.file.filename), inputs.lawId.value),
      ...summarizeRuns(file.scenarios.value.map((s) => s.name), runsFor(file)),
    })),
  );
  const counts = computed(() => countStatuses(fileStatuses.value));

  const loading = computed(
    () =>
      listLoading.value ||
      (active.value &&
        (inputs.lawLoading.value || files.value.some((f) => f.loading.value) || environment.loading.value)),
  );
  const loadingText = computed(() => (environment.loading.value && environment.progress.value) || "Scenario's laden");

  return {
    files: fileStatuses,
    counts,
    loading,
    loadingText,
    listError,
    environmentError: computed(() => (active.value ? environment.error.value : null)),
    /** The list of files is being read; until it is in, which names are taken is unknown. */
    listLoading,
    /** Read the list of files again, after a write that added one. */
    refresh: fetchScenarios,
  };
}

export type LawScenarios = ReturnType<typeof useLawScenarios>;
