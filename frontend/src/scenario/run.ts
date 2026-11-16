/**
 * Running one scenario on the WASM engine, and reading its outcome against the
 * scenario's expectations.
 *
 * The engine runs in-process and synchronously: a run has settled when
 * `runScenario` returns.
 */
import type { EditorEngine } from '../composables/useEngine';
import type { FormAssertion } from '../gherkin/formMapper';
import { expectationsFromAssertions, matchStatus, type Expectation } from '../utils/outputFormat';
import type { EngineInputs } from './inputs';

/** The fields of an executeWithTrace() result read by the editor. */
export interface TraceResult {
  outputs?: Record<string, unknown>;
  trace_text?: string | null;
}

/** The outcome of one run. */
export interface ScenarioRun {
  result: TraceResult | null;
  /** The trace of the run, also of a failed one when the engine gave it. */
  traceText: string | null;
  /** The engine's error message; null when the run succeeded. */
  error: string | null;
  /** The output the run asked for, so the graph can mark where it starts. */
  outputName: string | null;
  expectations: Record<string, Expectation>;
}

/** How a run compares with what the scenario expects. */
export type ScenarioStatus = 'passed' | 'failed' | 'no-expectations';

/** The engine calls a run makes. */
export type RunEngine = Pick<
  EditorEngine,
  'clearDataSources' | 'registerDataSource' | 'registerDataSourceForLaw' | 'executeWithTrace'
>;

// The WASM boundary throws either `{ error, trace_text }`, an Error, a string,
// or any other value (null and undefined included).
function readFailure(e: unknown): { error: string; traceText: string | null } {
  if (e && typeof e === 'object' && 'error' in e && e.error) {
    const trace = 'trace_text' in e && typeof e.trace_text === 'string' ? e.trace_text : null;
    return { error: String(e.error), traceText: trace };
  }
  if (typeof e === 'string') return { error: e, traceText: null };
  const message = e && typeof e === 'object' && 'message' in e ? e.message : undefined;
  return { error: message ? String(message) : String(e), traceText: null };
}

/**
 * Run a scenario. The engine's data sources are replaced by the scenario's
 * own. Never throws: a failure comes back as `error`.
 */
export function runScenario(
  engine: RunEngine,
  inputs: EngineInputs,
  assertions: readonly FormAssertion[],
): ScenarioRun {
  const expectations = expectationsFromAssertions([...assertions]);
  const base = { outputName: inputs.outputName, expectations };
  if (!inputs.outputName) {
    return { ...base, result: null, traceText: null, error: 'Dit scenario noemt geen uitkomst om te berekenen.' };
  }
  try {
    engine.clearDataSources();
    // A source the feature registers for one regelwerk goes to that
    // regelwerk only, as the runner does with the saved file.
    for (const ds of inputs.dataSources) {
      if (ds.lawId) engine.registerDataSourceForLaw(ds.lawId, ds.sourceName, ds.keyField, ds.records);
      else engine.registerDataSource(ds.sourceName, ds.keyField, ds.records);
    }
    // The engine hands its result back untyped; this is the shape it has.
    const result = engine.executeWithTrace(
      inputs.lawId,
      inputs.outputName,
      inputs.parameters,
      inputs.calculationDate,
    ) as TraceResult;
    return { ...base, result, traceText: result?.trace_text ?? null, error: null };
  } catch (e) {
    const { error, traceText } = readFailure(e);
    return { ...base, result: null, traceText, error };
  }
}

/** Whether a run meets the scenario's expectations; null before a run. */
export function runStatus(run: ScenarioRun | null | undefined): ScenarioStatus | null {
  if (!run) return null;
  if (run.error) return 'failed';
  const names = Object.keys(run.expectations);
  if (names.length === 0) return 'no-expectations';
  for (const name of names) {
    if (matchStatus(name, run.result?.outputs?.[name], run.expectations) === 'failed') return 'failed';
  }
  return 'passed';
}
