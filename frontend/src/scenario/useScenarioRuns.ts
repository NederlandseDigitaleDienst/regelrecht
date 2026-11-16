/**
 * The runs of every scenario in a set of open feature files, kept current.
 *
 * Runs again when the laws in the engine settle or change (`environmentRevision`,
 * `engineRevision`), when a file loads, and ~300ms after its edits stop
 * changing. The engine runs in
 * process and synchronously, so a run never interleaves with another.
 */
import { onBeforeUnmount, shallowRef, watch, type Ref } from 'vue';
import type { EditorEngine } from '../composables/useEngine';
import { scenarioFileKey, type OpenScenario } from './openScenario';
import { runScenario, type ScenarioRun } from './run';

export interface ScenarioRunInputs {
  files: Readonly<Ref<readonly OpenScenario[]>>;
  engine: Readonly<Ref<EditorEngine | null>>;
  /** Whether the laws the scenarios need are in the engine. */
  environmentReady: Readonly<Ref<boolean>>;
  /** Goes up whenever the scenarios' dependencies settled. */
  environmentRevision: Readonly<Ref<number>>;
  /**
   * Goes up whenever the engine's laws change (useEngine's `lawsRevision`):
   * the open regelwerk is loaded by the editor, on its own schedule.
   */
  engineRevision: Readonly<Ref<number>>;
  /** Why a regelwerk is not in the engine, once that is known (useScenarioEnvironment). */
  loadErrorFor?: (lawId: string) => string | null;
}

/** Run every scenario of one file as it stands now, edits included. */
export function runFile(
  engine: EditorEngine,
  file: OpenScenario,
  loadErrorFor: (lawId: string) => string | null = () => null,
): ScenarioRun[] {
  return file.scenarios.value.map((scenario, i) => {
    const inputs = file.inputsFor(i);
    if (!inputs) {
      return { result: null, traceText: null, error: 'Dit scenario kan niet worden gelezen.', outputName: null, expectations: {} };
    }
    return runScenario(engine, inputs, scenario.assertions, loadErrorFor(inputs.lawId));
  });
}

export function useScenarioRuns(inputs: ScenarioRunInputs) {
  /** Runs per file key, in scenario order. */
  const runs = shallowRef(new Map<string, ScenarioRun[]>());

  function runNow() {
    const engine = inputs.engine.value;
    if (!engine || !inputs.environmentReady.value) return;
    const next = new Map<string, ScenarioRun[]>();
    for (const file of inputs.files.value) {
      if (!file.state.value) continue;
      next.set(scenarioFileKey(file.file), runFile(engine, file, inputs.loadErrorFor));
    }
    runs.value = next;
  }

  let timer: ReturnType<typeof setTimeout> | undefined;
  function runSoon() {
    clearTimeout(timer);
    timer = setTimeout(runNow, 300);
  }

  // The laws settled, or the set of files (or one of them) loaded: run now.
  watch(
    [
      inputs.environmentRevision,
      inputs.environmentReady,
      () => inputs.files.value.map((f) => f.state.value),
    ],
    runNow,
    { immediate: true },
  );
  // Someone is typing, or the editor reloads the regelwerk being typed in:
  // run once they pause.
  watch(() => inputs.files.value.map((f) => f.values.value), runSoon, { deep: true });
  watch(inputs.engineRevision, runSoon);

  onBeforeUnmount(() => clearTimeout(timer));

  /** The runs of one file; empty until it ran. */
  function runsFor(file: OpenScenario): ScenarioRun[] {
    return runs.value.get(scenarioFileKey(file.file)) ?? [];
  }

  return { runs, runsFor, runNow };
}
