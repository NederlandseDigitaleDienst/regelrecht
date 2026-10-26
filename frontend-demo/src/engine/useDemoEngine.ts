/**
 * The demo's engine: the regelrecht WASM engine with every demo law loaded,
 * persona data registered as law-scoped sources, and citizen corrections
 * (claims) layered on top at a higher priority.
 *
 * Singleton: the whole workspace shares one engine instance.
 */
import { ref } from 'vue';
import {
  collectKeyValues,
  lawShape,
  materialiseAll,
  tablesFromProfiles,
} from '../data/materialize.js';
import { t } from '../i18n/index.js';
import type { ExecutionResult, GherkinEngine } from '@regelrecht/frontend-shared/gherkin';

/**
 * One step of an engine trace (RFC-039, `schema/trace/v1/trace-schema.json`).
 * The keys are the engine's own snake_case names.
 */
export interface TraceNode {
  node_type: string;
  name: string;
  result?: unknown;
  resolve_type?: string | null;
  children?: TraceNode[] | null;
  node_id?: string | null;
  anchor?: {
    law_id?: string | null;
    article?: string | null;
    paragraph?: string | null;
    url?: string | null;
    valid_from?: string | null;
    [field: string]: unknown;
  } | null;
  type_spec?: { unit?: string | null; precision?: number | null } | null;
  // RFC-039: a DATA_SOURCE resolve names its register in `provider`.
  source?: { provider?: string | null; [field: string]: unknown } | null;
  duration_us?: number | null;
  message?: string | null;
}

/** The trace document the engine hands back: `{trace_version, root}`. */
export interface TraceDocument {
  trace_version: string;
  root: TraceNode;
}

/** What `executeMultipleWithTrace` returns (engine keys), as far as the demo reads it. */
export interface TracedExecutionResult extends ExecutionResult {
  outputs?: Record<string, unknown>;
  trace?: TraceDocument | TraceNode | null;
  trace_text?: string;
  resolved_inputs?: Record<string, unknown>;
}

/**
 * The procedure state of a decision between two stages (RFC-008). Opaque to
 * the demo: it keeps it with the case and hands it back on the next stage.
 */
export type StageState = Record<string, unknown>;

/** What `executeStage` returns (engine keys). */
export interface RawStageResult {
  complete?: boolean;
  outputs?: Record<string, unknown>;
  state?: StageState | null;
  pending_inputs?: string[];
  current_stage?: string | null;
}

/**
 * The `WasmEngine` methods the demo calls. A local structural type: the
 * generated declarations in public/wasm/pkg/ are a gitignored build artifact.
 * It extends `GherkinEngine` because the scenario view hands the scenario
 * engine to the shared gherkin runner.
 */
export interface DemoEngine extends GherkinEngine {
  loadLaw(yaml: string): string;
  lawCount(): number;
  clearDataSources(): void;
  removeDataSource(name: string): boolean;
  registerDataSourceForLaw(
    lawId: string,
    name: string,
    keyField: string,
    records: Record<string, unknown>[],
    priority?: number,
  ): void;
  executeMultipleWithTrace(
    lawId: string,
    outputNames: string[],
    parameters: Record<string, unknown>,
    calculationDate: string,
  ): TracedExecutionResult;
  executeStage(
    lawId: string,
    outputName: string,
    state: StageState | null,
    parameters: Record<string, unknown>,
    calculationDate: string,
  ): RawStageResult;
}

/** The wasm-bindgen JS glue module, as far as it is used here. */
interface WasmModule {
  default(moduleOrPath: string): Promise<unknown>;
  WasmEngine: new () => DemoEngine;
}

/** A register row from `profiles.yaml`. */
type Row = Record<string, unknown>;
/** Register tables per organisation: service -> table -> rows. */
type TablesByService = Record<string, Record<string, Row[]> | null | undefined>;

/** The corpus fields this module reads (the corpus itself is `data/loadCorpus`'s). */
interface EngineCorpus {
  laws: { id: string; path: string; text: string; doc: object }[];
  latestById: Map<string, { doc: object }>;
  bindings: Parameters<typeof materialiseAll>[1];
  profiles: {
    globalServices?: TablesByService | null;
    profiles?: Record<string, { sources?: TablesByService | null }> | null;
  };
}

/** The fields of a parsed law document that the cross-law resolver reads. */
interface LawInputsView {
  articles?: {
    machine_readable?: {
      execution?: {
        input?: {
          name: string;
          source?: { regulation?: string; output?: string; parameters?: Record<string, unknown> };
        }[];
      };
    };
  }[];
}

/** Whatever the engine threw: a string, an `Error`, or an object `{error, trace}`. */
type Thrown = { error?: unknown; message?: unknown; trace?: TraceDocument | TraceNode | null } | null | undefined;

/** A law the engine refused to load. */
export interface LoadFailure {
  id: string;
  path: string;
  message: string;
}

/** An approved citizen correction, as the engine registers it. */
export interface EngineClaim {
  lawId: string;
  keyField: string;
  keyValue: string;
  input: string;
  newValue: unknown;
}

/** One source `registerPersonaData` registered: one per (law, service, key field). */
export type PersonaSource = ReturnType<typeof materialiseAll>[number];

let engineInstance: DemoEngine | null = null;
let initPromise: Promise<DemoEngine> | null = null;
/** The wasm-bindgen module once initialised; a second engine is constructed from it. */
let wasmModule: WasmModule | null = null;
export const engineReady = ref(false);
export const engineError = ref<unknown>(null);

const REGISTER_PRIORITY = 10;
const CLAIMS_PRIORITY = 100;
export const CLAIMS_SOURCE = 'correcties';

async function initEngine(): Promise<DemoEngine> {
  if (engineInstance) return engineInstance;
  if (initPromise) return initPromise;
  initPromise = (async () => {
    try {
      // The WASM glue lives in public/wasm/pkg; import it through a blob URL to
      // sidestep Vite's rule against importing from /public (same as the editor).
      const res = await fetch('/wasm/pkg/regelrecht_engine.js');
      if (!res.ok) throw new Error(`WASM-glue niet gevonden (${res.status})`);
      const blob = new Blob([await res.text()], { type: 'application/javascript' });
      const blobUrl = URL.createObjectURL(blob);
      const wasm = (await import(/* @vite-ignore */ blobUrl)) as WasmModule;
      URL.revokeObjectURL(blobUrl);
      await wasm.default('/wasm/pkg/regelrecht_engine_bg.wasm');
      wasmModule = wasm;
      engineInstance = new wasm.WasmEngine();
      engineReady.value = true;
      return engineInstance;
    } catch (e) {
      engineError.value = e;
      throw e;
    }
  })();
  return initPromise;
}

/**
 * Laws the engine refused to load: `{id, path, message}` per law version. The
 * loader type-checks every law (RFC-037), so a refused law is a law with a
 * finding, and it is simply not there for the portal, the graph and the
 * scenario runner. That must be visible, not a line in the console: the views
 * show these in a banner and the scenario runner names the refusal instead of
 * "law not found".
 */
export const loadFailures = ref<LoadFailure[]>([]);

function loadLaws(engine: DemoEngine, corpus: EngineCorpus) {
  const failures: LoadFailure[] = [];
  for (const law of corpus.laws) {
    try {
      engine.loadLaw(law.text);
    } catch (e) {
      const err = e as Thrown;
      const message = typeof e === 'string' ? e : err?.error ?? err?.message ?? JSON.stringify(e);
      console.warn(`Wet ${law.id} (${law.path}) kon niet geladen worden:`, e);
      failures.push({ id: law.id, path: law.path, message: String(message) });
    }
  }
  // Both engines load the same corpus; the second run says nothing new.
  if (failures.length || loadFailures.value.length === 0) loadFailures.value = failures;
}

/** The load failure of a law by id, or null when the engine holds it. */
export function loadFailureFor(lawId: string): LoadFailure | null {
  return loadFailures.value.find((f) => f.id === lawId) ?? null;
}

/** Load every law version of the corpus into the engine (idempotent). */
let lawsLoaded = false;
export async function prepareEngine(corpus: EngineCorpus): Promise<DemoEngine> {
  const engine = await initEngine();
  if (!lawsLoaded) {
    loadLaws(engine, corpus);
    lawsLoaded = true;
  }
  return engine;
}

/**
 * A second engine for the scenario runner, created on first use with every
 * corpus law loaded and no persona data at all.
 *
 * Scenario steps register their own data sources and the runner clears them
 * between runs. Done on the portal's engine, that threw away the persona data
 * after every run and forced a full re-registration, which bumps `dataVersion`
 * and makes every kept-alive tile re-evaluate its law: one scenario run cost
 * dozens of law evaluations elsewhere. On its own engine a run touches nothing
 * the portal sees.
 */
let scenarioEngine: DemoEngine | null = null;
let scenarioPromise: Promise<DemoEngine> | null = null;
export async function prepareScenarioEngine(corpus: EngineCorpus): Promise<DemoEngine> {
  if (scenarioEngine) return scenarioEngine;
  if (scenarioPromise) return scenarioPromise;
  scenarioPromise = (async () => {
    try {
      await initEngine();
      const t0 = performance.now();
      // initEngine() resolved, so it has set the module.
      const engine = new wasmModule!.WasmEngine();
      loadLaws(engine, corpus);
      console.debug(`[scenario-engine] tweede engine met ${engine.lawCount()} wetten geladen in ${(performance.now() - t0).toFixed(0)}ms`);
      scenarioEngine = engine;
      return engine;
    } catch (e) {
      // Do not cache the failure: the next run tries again.
      scenarioPromise = null;
      throw e;
    }
  })();
  return scenarioPromise;
}

/** Every parameter name any demo law declares; these are the candidate keys. */
function allParameterNames(corpus: EngineCorpus): string[] {
  const names = new Set<string>();
  for (const law of corpus.laws) for (const p of lawShape(law.doc).parameters) names.add(p);
  return [...names];
}

/**
 * Materialise all persona data and register it, one scoped source per
 * (law, organisation, key field). Called once at start-up and again when the
 * reference date changes (a few bindings select on the year).
 */
export function registerPersonaData(
  engine: DemoEngine,
  corpus: EngineCorpus,
  referenceDate: string,
  cases: unknown[] = [],
  claims: EngineClaim[] = [],
): PersonaSource[] {
  engine.clearDataSources();
  // A persona's answers to a law's application form (approved claims) count as
  // that law's parameters while its register rows are looked up: the terrace
  // location decides which BGT row applies.
  const paramsFor = (keyField: string, keyValue: string, lawId: string) => {
    const out: Record<string, unknown> = {};
    for (const c of claims) if (c.lawId === lawId && c.keyField === keyField && c.keyValue === keyValue) out[c.input] = c.newValue;
    return out;
  };
  const rowsFor = tablesFromProfiles(corpus.profiles);
  const allTables: Row[][] = [];
  const seen = new Set<Row[]>();
  const collectTables = (byService: TablesByService | null | undefined) => {
    for (const byTable of Object.values(byService ?? {})) {
      for (const rows of Object.values(byTable ?? {})) {
        if (!seen.has(rows)) {
          seen.add(rows);
          allTables.push(rows);
        }
      }
    }
  };
  collectTables(corpus.profiles.globalServices);
  for (const profile of Object.values(corpus.profiles.profiles ?? {})) collectTables(profile.sources);

  // materialize.js builds the result from `{}`, so its inferred type is `{}`;
  // it holds one array of string values per requested column.
  const keyValues = collectKeyValues(allTables, allParameterNames(corpus));
  for (const bsn of Object.keys(corpus.profiles.profiles ?? {})) {
    // `bsn` is there when some demo law declares a `bsn` parameter; the
    // persona laws all do. Without one this throws, as it always did.
    if (!keyValues.bsn!.includes(bsn)) keyValues.bsn!.push(bsn);
  }
  const lawsById: Record<string, object> = {};
  for (const [id, law] of corpus.latestById) lawsById[id] = law.doc;
  const register = (sources: PersonaSource[]) => {
    for (const { law, service, keyField, records } of sources) {
      try {
        engine.registerDataSourceForLaw(law, service, keyField, records, REGISTER_PRIORITY);
      } catch (e) {
        console.warn(`Databron ${service} voor ${law} niet geregistreerd:`, e);
      }
    }
  };

  // Pass 1: everything that follows from parameters and register rows alone.
  register(materialiseAll(lawsById, corpus.bindings, rowsFor, keyValues, { referencedate: referenceDate, cases, paramsFor }));

  // Pass 2: a few bindings select on a cross-law input (`adres: $vestigingsadres`,
  // the address the KVK law derives). With pass 1 registered, the engine can
  // answer those; re-materialise with that resolver and replace the sources.
  const cache = new Map<string, unknown>();
  const resolveRef = (lawId: string, inputName: string, params: Record<string, unknown>) => {
    const doc = lawsById[lawId] as LawInputsView | undefined;
    const input = (doc?.articles ?? [])
      .flatMap((a) => a.machine_readable?.execution?.input ?? [])
      .find((i) => i.name === inputName);
    const src = input?.source;
    if (!src?.regulation) return undefined;
    const callParams: Record<string, unknown> = {};
    for (const [name, ref] of Object.entries(src.parameters ?? {})) {
      const v = typeof ref === 'string' && ref.startsWith('$') ? params[ref.slice(1)] : ref;
      if (v === undefined || v === null) return undefined;
      callParams[name] = v;
    }
    const key = `${src.regulation}#${src.output ?? inputName}|${JSON.stringify(callParams)}`;
    if (cache.has(key)) return cache.get(key);
    let value: unknown;
    try {
      const result = engine.execute(src.regulation, src.output ?? inputName, callParams, referenceDate);
      value = result?.outputs?.[src.output ?? inputName];
    } catch {
      value = undefined;
    }
    cache.set(key, value);
    return value;
  };
  const sources = materialiseAll(lawsById, corpus.bindings, rowsFor, keyValues, { referencedate: referenceDate, cases, resolveRef, paramsFor });
  engine.clearDataSources();
  register(sources);
  return sources;
}

/**
 * Register approved citizen corrections as a higher-priority scoped source per
 * law, replacing the previous set.
 *
 * A record never carries `undefined`: the WASM boundary would read it as an
 * absence (`null`), turning "not supplied" into "there is none" (RFC-036). A
 * claim without a value is simply not in the record, so the input stays
 * unknown.
 */
export function registerClaims(
  engine: Pick<DemoEngine, 'removeDataSource' | 'registerDataSourceForLaw'>,
  claims: EngineClaim[],
): void {
  engine.removeDataSource(CLAIMS_SOURCE);
  const grouped = new Map<string, Map<string, Record<string, unknown>>>();
  for (const claim of claims) {
    if (claim.newValue === undefined) continue;
    const key = JSON.stringify([claim.lawId, claim.keyField]);
    if (!grouped.has(key)) grouped.set(key, new Map());
    // Both entries are set just above when missing.
    const records = grouped.get(key)!;
    if (!records.has(claim.keyValue)) records.set(claim.keyValue, { [claim.keyField]: claim.keyValue });
    records.get(claim.keyValue)![claim.input] = claim.newValue;
  }
  for (const [key, records] of grouped) {
    const [lawId, keyField] = JSON.parse(key) as [string, string];
    engine.registerDataSourceForLaw(lawId, CLAIMS_SOURCE, keyField, [...records.values()], CLAIMS_PRIORITY);
  }
}

/** What `evaluateLaw` returns. */
export type EvaluateResult =
  | {
      ok: true;
      outputs: Record<string, unknown>;
      trace: TraceNode | null;
      traceText: string;
      resolvedInputs: Record<string, unknown>;
      raw: TracedExecutionResult;
    }
  | { ok: false; error: string; trace?: TraceNode | null };

/** What `executeStage` returns. */
export type StageResult =
  | {
      ok: true;
      complete: boolean;
      outputs: Record<string, unknown>;
      state: StageState | null;
      pendingInputs: string[];
      currentStage: string | null;
    }
  | { ok: false; error: string };

/**
 * The root step of a trace document (RFC-039).
 *
 * The engine hands back `{trace_version, root}`; everything downstream walks
 * steps, so the document is opened once here rather than at every consumer.
 * Tolerates a bare step so a recorded fixture from before the envelope still
 * renders.
 */
function traceRoot(trace: TraceDocument | TraceNode | null | undefined): TraceNode | null {
  if (!trace) return null;
  return (trace as Partial<TraceDocument>).root ?? (trace as TraceNode);
}

/** Evaluate a law for the given parameters with a full trace. */
export function evaluateLaw(
  engine: Pick<DemoEngine, 'executeMultipleWithTrace'>,
  lawEntry: { id: string; outputs: string[] },
  params: Record<string, unknown>,
  referenceDate: string,
  outputs: string[] | null = null,
): EvaluateResult {
  const names = outputs ?? lawEntry.outputs;
  if (names.length === 0) return { ok: false, error: t('engine.no_outputs') };
  try {
    const result = engine.executeMultipleWithTrace(lawEntry.id, names, params, referenceDate);
    return {
      ok: true,
      outputs: result.outputs ?? {},
      trace: traceRoot(result.trace),
      traceText: result.trace_text ?? '',
      resolvedInputs: result.resolved_inputs ?? {},
      raw: result,
    };
  } catch (e) {
    // The engine throws either a string or an object {error, trace}.
    const err = e as Thrown;
    const message = typeof e === 'string' ? e : err?.error ?? err?.message ?? JSON.stringify(e);
    return { ok: false, error: String(message), trace: traceRoot(err?.trace) };
  }
}

/**
 * Eén stap in de levensloop van een besluit (RFC-007, RFC-008).
 *
 * Waar `evaluateLaw` een wet doorrekent en klaar is, loopt dit de procedure af
 * die de Awb aan dit soort besluit geeft: het vuurt de haken die bij elke fase
 * horen, en stopt bij de eerste fase waarvan het de gegevens niet heeft. Zo
 * komt de bezwaartermijn als datum uit de wet in plaats van als vaste tekst uit
 * het scherm.
 *
 * De engine bewaart niets. `state` is `null` bij de eerste stap en daarna wat
 * de vorige stap teruggaf; de demo bewaart dat bij de zaak. Die verdeling is de
 * kern van RFC-008: de engine blijft een zuivere functie per fase, de
 * orkestratielaag houdt het besluit vast.
 *
 * Een wet die in geen procedure zit rekent gewoon door en is meteen klaar, dus
 * dit is voor elke wet veilig aan te roepen.
 */
export function executeStage(
  engine: Pick<DemoEngine, 'executeStage'>,
  lawEntry: { id: string },
  outputName: string,
  state: StageState | null | undefined,
  params: Record<string, unknown>,
  referenceDate: string,
): StageResult {
  try {
    const result = engine.executeStage(lawEntry.id, outputName, state ?? null, params, referenceDate);
    return {
      ok: true,
      complete: !!result.complete,
      outputs: result.outputs ?? {},
      state: result.state ?? null,
      pendingInputs: result.pending_inputs ?? [],
      currentStage: result.current_stage ?? null,
    };
  } catch (e) {
    const err = e as Thrown;
    const message = typeof e === 'string' ? e : err?.error ?? err?.message ?? JSON.stringify(e);
    return { ok: false, error: String(message) };
  }
}

export function useDemoEngine() {
  return { initEngine, prepareEngine, prepareScenarioEngine, registerPersonaData, registerClaims, evaluateLaw, executeStage, engineReady, engineError, loadFailures, loadFailureFor };
}
