/**
 * Run the laws over a synthetic population.
 *
 * The generated register rows replace the personas' data in the engine for
 * the duration of the run (the caller restores them afterwards), every portal
 * law of the audience is evaluated per subject through the same materialiser
 * and engine the portal uses, and the outcomes are summarised. Law constants
 * can be overridden for the run: the law is reloaded with changed
 * `definitions` and put back when the run ends.
 *
 * Long runs yield to the event loop between chunks so the progress bar moves
 * and the page stays responsive.
 */
import { collectKeyValues, lawShape, materialiseAll } from '../data/materialize.js';
import { fieldSpec, isAmountSpec, verdictOf } from '../data/format';
import { isEntrypointFor } from '../data/entrypoints';
import { evaluateLaw } from '../engine/useDemoEngine';
import type { DemoEngine, EvaluateResult } from '../engine/useDemoEngine';
import type { Corpus, LawDocument, LawEntry } from '../data/loadCorpus';
import type { Verdict } from '../data/format';
import { applyOverrides, effectiveOverrides, overridableDefinitions } from './lawParameters';
import type { DefinitionOverrides } from './lawParameters';
import { generateBusinesses, generateCitizens, rowsForTables, templatesFromProfiles } from './population';
import type { BusinessFormValues, BusinessParams, CitizenParams, Population, SimulationSubject, TablesByService } from './population';
import { describePopulation, summariseLaw } from './stats';
import type { LawSummary, PopulationSummary, SimulationResult } from './stats';

/** Who a run simulates: citizens or businesses (the view's own values). */
export type SimulationKind = 'burgers' | 'ondernemers';

/** The fields of a law entry the law selection reads; `LawEntry` has them all. */
export interface SimulationLaw {
  id: string;
  name: string;
  doc: LawDocument;
}

/** A law the population cannot run, with the parameters it lacks. */
export interface SkippedLaw<L extends SimulationLaw = LawEntry> {
  law: L;
  missing: string[];
}

/** The outcome of one evaluation reduced to what the statistics need (see `reduceOutcome`). */
export type LawOutcome =
  | { ok: false; met: null; amount: null; amountName: null; error: string }
  | { ok: true; met: Verdict; amount: number | null; amountName: string | null; error: null; outputs: Record<string, unknown> };

/** A subject's application-form answers; empty for citizens. */
export type ApplicationForm = Partial<BusinessFormValues> & Record<string, unknown>;

/** A law of a run, as the result lists it. */
export interface SimulatedLaw {
  id: string;
  name: string;
  service: string;
  law_path: string;
  primaryName: string | null | undefined;
}

/** A finished (or cancelled) simulation run, as the view shows and compares it. */
export interface SimulationRun {
  id: string;
  kind: SimulationKind;
  params: Partial<CitizenParams> | Partial<BusinessParams>;
  /** law id -> the overrides that differed from the law and were applied */
  overrides: Record<string, DefinitionOverrides>;
  referenceDate: string;
  cancelled: boolean;
  laws: SimulatedLaw[];
  skipped: Array<{ id: string; name: string; missing: string[] }>;
  subjects: SimulationSubject[];
  results: Array<SimulationResult<SimulationSubject> & { laws: Record<string, LawOutcome> }>;
  /** law id -> summary */
  summary: Record<string, LawSummary>;
  population: PopulationSummary;
  durationMs: number;
}

/** The binding fields `caseSourceLaws` reads (a `Binding` has them). */
interface CaseBindingView {
  kind?: string;
  select_on?: Array<{ name: string; value: unknown }> | null;
}

/** The subject keys the runner passes to the engine. */
interface SubjectKeys {
  id: string;
  bsn?: string;
  kvk_nummer?: string;
}

const REGISTER_PRIORITY = 10;
const FORM_PRIORITY = 100;
const FORM_SOURCE = 'simulatie-opgaven';
const CHUNK = 5;

const IDENTITY = new Set(['bsn', 'kvk_nummer']);

/** The laws the simulation runs for an audience: the portal's laws, minus those needing parameters the population cannot supply. */
export function simulationLaws<L extends SimulationLaw>(
  corpus: { latestById: Map<string, L> },
  kind: SimulationKind,
  isLawEnabled: (law: L) => boolean = () => true,
): { runnable: L[]; skipped: SkippedLaw<L>[] } {
  const wanted = kind === 'ondernemers' ? 'BUSINESS' : 'CITIZEN';
  const laws = [...corpus.latestById.values()].filter((law) => isEntrypointFor(law.doc, wanted) && isLawEnabled(law));
  const known = new Set([...IDENTITY, ...FORM_PARAMETERS]);
  const runnable: L[] = [];
  const skipped: SkippedLaw<L>[] = [];
  for (const law of laws) {
    const params: string[] = lawShape(law.doc).parameters;
    const missing = params.filter((p) => !known.has(p));
    if (missing.length) skipped.push({ law, missing });
    else runnable.push(law);
  }
  runnable.sort((a, b) => a.name.localeCompare(b.name));
  return { runnable, skipped };
}

/**
 * The laws the simulated laws lean on and that carry tunable constants: what
 * they read through `source.regulation`, and what fills one of their open
 * terms through `implements`, followed all the way down.
 *
 * The standaardpremie is the reason. It used to be a constant of the
 * zorgtoeslag law itself; it now lives in the ministeriële regeling that
 * fixes it each year and fills the law's open term. Changing it is still the
 * first "what if" a presenter reaches for, and it is not a constant of any law
 * the simulation runs.
 *
 * @param corpus
 * @param runnable  the laws the simulation runs
 * @param hasConstants  a law's tunable constants
 * @param isLawEnabled  the demo's visibility
 *   choice, as for `simulationLaws`: a hidden law reached through
 *   `source.regulation` is not offered here either. A regeling that fills an
 *   open term is, even when hidden: `hidden_laws` keeps it off the portal
 *   (the standaardpremie regeling is hidden there), and this list is not a
 *   portal but exactly where that regeling belongs.
 * @returns sorted by name, without the runnable laws themselves
 */
export function supportingLaws<L extends SimulationLaw>(
  corpus: { latestById: Map<string, L> },
  runnable: readonly L[],
  hasConstants: (doc: L['doc']) => readonly unknown[],
  isLawEnabled: (law: L) => boolean = () => true,
): L[] {
  const implementers = new Map<string, Set<string>>();
  for (const law of corpus.latestById.values()) {
    for (const article of law.doc?.articles ?? []) {
      // `implements` (the open terms an article fills) is not in LawDocument's
      // listed fields; the law format makes it a list of {law, article, open_term}.
      for (const impl of (article.machine_readable?.implements ?? []) as Array<{ law?: string } | null | undefined>) {
        if (!impl?.law) continue;
        if (!implementers.has(impl.law)) implementers.set(impl.law, new Set());
        implementers.get(impl.law)!.add(law.id);
      }
    }
  }
  const runIds = new Set(runnable.map((law) => law.id));
  const seen = new Set(runIds);
  const fillsOpenTerm = new Set<string>();
  const queue = [...runIds];
  while (queue.length) {
    // The queue is non-empty by the loop condition.
    const law = corpus.latestById.get(queue.shift()!);
    const next = new Set(law ? implementers.get(law.id) ?? [] : []);
    for (const id of next) fillsOpenTerm.add(id);
    for (const article of law?.doc?.articles ?? []) {
      for (const input of article.machine_readable?.execution?.input ?? []) {
        if (input?.source?.regulation) next.add(input.source.regulation);
      }
    }
    for (const id of next) {
      if (seen.has(id) || !corpus.latestById.has(id)) continue;
      seen.add(id);
      queue.push(id);
    }
  }
  return [...seen]
    .filter((id) => !runIds.has(id))
    // Every id in `seen` besides the run's own was checked with `has` above.
    .map((id) => corpus.latestById.get(id)!)
    .filter((law) => (fillsOpenTerm.has(law.id) || isLawEnabled(law)) && hasConstants(law.doc).length > 0)
    .sort((a, b) => a.name.localeCompare(b.name));
}

/** Application-form parameters the business generator supplies (see population.ts). */
const FORM_PARAMETERS = [
  'aangevraagde_categorie',
  'activiteiten',
  'bereidt_of_serveert_voedsel',
  'terras_locatie',
  'terras_oppervlakte',
  'obstakelvrije_ruimte',
  'seizoen',
  'gewenste_openingstijd',
  'gewenste_sluitingstijd_doordeweeks',
  'gewenste_sluitingstijd_weekend',
  'activiteitsdatum',
  'activiteitsstarttijd',
];

/**
 * The laws another law reads decided cases of (the `law` criterion of a
 * `kind: cases` binding). A law that reads its own cases, such as the
 * ontheffingspas counting this year's exemptions, is not one: nobody else
 * waits for it to be applied for.
 */
export function caseSourceLaws(bindings: Record<string, Record<string, CaseBindingView>> | null | undefined): Set<string> {
  const out = new Set<string>();
  for (const [owner, lawBindings] of Object.entries(bindings ?? {})) {
    for (const b of Object.values(lawBindings)) {
      const law = b.kind === 'cases' ? (b.select_on ?? []).find((c) => c.name === 'law')?.value : null;
      if (typeof law === 'string' && !law.startsWith('$') && law !== owner) out.add(law);
    }
  }
  return out;
}

function primaryOutputName(corpus: Pick<Corpus, 'config'>, law: { service: string; law_path: string }): string | null {
  const cfg = corpus.config?.dashboard_outputs ?? {};
  return cfg[`${law.service}/${law.law_path}`] ?? null;
}

/** The outcome of one evaluation reduced to what the statistics need. */
export function reduceOutcome(law: { doc: LawDocument }, primaryName: string | null | undefined, evaluation: EvaluateResult): LawOutcome {
  if (!evaluation.ok) return { ok: false, met: null, amount: null, amountName: null, error: evaluation.error };
  const outputs = evaluation.outputs ?? {};
  // true/false when the law decided, 'unknown' when it could not for lack of
  // facts (RFC-036), null when the law has no verdict output.
  const met = verdictOf(outputs);
  // The amount shown for a law: its configured primary output, or else the
  // first monetary output. A count or a rate is not an amount to average.
  let name = primaryName && outputs[primaryName] !== undefined ? primaryName : null;
  if (!name) {
    name = Object.keys(outputs).find((k) => k !== 'voldoet_aan_voorwaarden' && typeof outputs[k] === 'number' && isAmountSpec(fieldSpec(law.doc, k))) ?? null;
  }
  let amount: number | null = null;
  if (name && typeof outputs[name] === 'number') {
    // Checked to be a number just above; `name` is a `let`, so TS does not narrow the access.
    const value = outputs[name] as number;
    amount = isAmountSpec(fieldSpec(law.doc, name)) ? value / 100 : value;
  }
  return { ok: true, met, amount, amountName: name, error: null, outputs };
}

function sleep(ms = 0): Promise<void> {
  return new Promise((resolve) => setTimeout(resolve, ms));
}

/**
 * Of een onderwerp een aanvraagwet niet aanvroeg. Zo'n wet krijgt voor hem
 * geen uitkomst: wie geen terras vraagt, valt buiten art. 2:30b APV en heeft
 * geen recht en geen weigering. Doorrekenen gaf "recht op een terras van
 * 0 m²" en telde hem mee als iemand met recht. Een ontbrekende uitkomst telt
 * in de samenvatting niet mee, net als op het portaal, waar een
 * terrasvergunning pas verschijnt als je hem aanvraagt.
 *
 * @param law
 * @param form            de formulierwaarden van het onderwerp
 * @param applicationPaths  de law_paths waarvoor aangevraagd wordt (caseSourceLaws)
 */
export function notAppliedFor(law: { law_path: string }, form: { aanvragen?: readonly string[] | null }, applicationPaths: Set<string>): boolean {
  return applicationPaths.has(law.law_path) && !(form.aanvragen ?? []).includes(law.law_path);
}

/** The arguments of `runSimulation`. */
export interface RunSimulationArgs {
  /** the WASM engine */
  engine: DemoEngine;
  /** loaded corpus */
  corpus: Corpus;
  kind: SimulationKind;
  /** population parameters (see population.ts defaults) */
  params: Partial<CitizenParams> | Partial<BusinessParams>;
  /** law id -> definition -> value */
  overrides?: Record<string, Record<string, unknown>>;
  referenceDate: string;
  onProgress?: (done: number, total: number) => void;
  signal?: { cancelled: boolean };
  isLawEnabled?: (law: LawEntry) => boolean;
}

export async function runSimulation({ engine, corpus, kind, params, overrides = {}, referenceDate, onProgress, signal, isLawEnabled }: RunSimulationArgs): Promise<SimulationRun> {
  const started = performance.now();
  const templateRow = templatesFromProfiles(corpus.profiles);
  const population: Population = kind === 'ondernemers' ? generateBusinesses(params, referenceDate, templateRow) : generateCitizens(params, referenceDate, templateRow);
  const { runnable: laws, skipped } = simulationLaws(corpus, kind, isLawEnabled);

  const applied: Record<string, DefinitionOverrides> = {};
  const results: SimulationRun['results'] = [];
  const primaries = new Map(laws.map((law) => [law.id, primaryOutputName(corpus, law)]));
  try {
    // Law constants for this run, also those of the regelingen the laws lean
    // on. Inside the `try`: if one of them fails to load, the `finally` still
    // puts back the ones that were already swapped in.
    for (const law of [...laws, ...supportingLaws(corpus, laws, overridableDefinitions, isLawEnabled)]) {
      const eff = effectiveOverrides(law.doc, overrides[law.id]);
      if (Object.keys(eff).length) {
        engine.loadLaw(applyOverrides(law.doc, eff));
        applied[law.id] = eff;
      }
    }

    // Register data: the generated rows, plus the shared tables every persona has.
    const shared: TablesByService = {};
    for (const [service, byTable] of Object.entries(corpus.profiles.globalServices ?? {})) shared[service] = byTable;
    const rowsFor = rowsForTables(shared, population.tables);
    const lawsById: Record<string, LawDocument> = {};
    for (const [id, law] of corpus.latestById) lawsById[id] = law.doc;
    // Every law is keyed on one of its parameters; besides bsn and kvk_nummer
    // that is an address (wet_bag), a year (CBS) and the like. Collect the
    // values every parameter name takes in the generated and shared tables,
    // exactly as the persona registration does, so those laws get records too.
    const parameterNames = new Set<string>();
    for (const doc of Object.values(lawsById)) for (const p of lawShape(doc).parameters) parameterNames.add(p);
    const allTables = [shared, population.tables].flatMap((set) => Object.values(set).flatMap((byTable) => Object.values(byTable)));
    const keyValues = collectKeyValues(allTables, [...parameterNames]);
    for (const [key, values] of Object.entries(population.keyValues)) {
      keyValues[key] = [...new Set([...(keyValues[key] ?? []), ...values])];
    }
    // A business's application-form answers (terrace location and size) are
    // known here, unlike on the portal; bindings that select on them get them.
    const paramsFor = (keyField: string, keyValue: string): Record<string, unknown> => (keyField === 'kvk_nummer' ? population.formValues?.[keyValue] ?? {} : {});
    const registerSources = (cases: Record<string, unknown>[]) => {
      engine.clearDataSources();
      const register = (sources: ReturnType<typeof materialiseAll>) => {
        for (const { law, service, keyField, records } of sources) {
          try {
            engine.registerDataSourceForLaw(law, service, keyField, records, REGISTER_PRIORITY);
          } catch (e) {
            console.warn(`Simulatie: databron ${service} voor ${law} niet geregistreerd:`, e);
          }
        }
      };
      register(materialiseAll(lawsById, corpus.bindings, rowsFor, keyValues, { referencedate: referenceDate, cases, paramsFor }));
      // Second pass for bindings that select on a cross-law input (the KVK address).
      const cache = new Map<string, unknown>();
      const resolveRef = (lawId: string, inputName: string, callerParams: Record<string, unknown>): unknown => {
        const doc = lawsById[lawId];
        const input = (doc?.articles ?? []).flatMap((a) => a.machine_readable?.execution?.input ?? []).find((i) => i.name === inputName);
        const src = input?.source;
        if (!src?.regulation) return undefined;
        const callParams: Record<string, unknown> = {};
        for (const [name, ref] of Object.entries(src.parameters ?? {})) {
          const v = typeof ref === 'string' && ref.startsWith('$') ? callerParams[ref.slice(1)] : ref;
          if (v === undefined || v === null) return undefined;
          callParams[name] = v;
        }
        const key = `${src.regulation}#${src.output ?? inputName}|${JSON.stringify(callParams)}`;
        if (cache.has(key)) return cache.get(key);
        let value: unknown;
        try {
          value = engine.execute(src.regulation, src.output ?? inputName, callParams, referenceDate)?.outputs?.[src.output ?? inputName];
        } catch {
          value = undefined;
        }
        cache.set(key, value);
        return value;
      };
      const sources = materialiseAll(lawsById, corpus.bindings, rowsFor, keyValues, { referencedate: referenceDate, cases, resolveRef, paramsFor });
      engine.clearDataSources();
      register(sources);
      // Form answers (`kind: claim` inputs) as a higher-priority source. A claim
      // without a value is left out of the record: `undefined` would cross the
      // WASM boundary as null and turn "not supplied" into "none" (RFC-036).
      const grouped = new Map<string, Map<string, Record<string, unknown>>>();
      for (const c of population.claims) {
        if (c.value === undefined) continue;
        const key = JSON.stringify([c.lawId, c.keyField]);
        if (!grouped.has(key)) grouped.set(key, new Map());
        // Both entries were set just above when missing.
        const records = grouped.get(key)!;
        if (!records.has(c.keyValue)) records.set(c.keyValue, { [c.keyField]: c.keyValue });
        records.get(c.keyValue)![c.input] = c.value;
      }
      for (const [key, records] of grouped) {
        const [lawId, keyField] = JSON.parse(key) as [string, string];
        engine.registerDataSourceForLaw(lawId, FORM_SOURCE, keyField, [...records.values()], FORM_PRIORITY);
      }
    };
    registerSources([]);

    // Only what the population states is passed. A form parameter the
    // generator did not answer is left out, so the engine reports it as
    // unknown (not passed) instead of receiving a null it would read as
    // "none" (RFC-036).
    const callParamsFor = (law: LawEntry, subject: SubjectKeys, form: ApplicationForm) => {
      const callParams: Record<string, unknown> = {};
      for (const p of lawShape(law.doc).parameters) {
        const value = p === 'bsn' ? subject.bsn ?? form.bsn : p === 'kvk_nummer' ? subject.kvk_nummer : form[p];
        if (value !== undefined) callParams[p] = value;
      }
      return callParams;
    };

    // Applications first. A law another law reads as a decided case (a
    // `kind: cases` binding on `law`) is applied for by the subjects the
    // population says apply (`aanvragen`), and what it grants becomes a case,
    // as a granted application does on the portal: the precario tax reads the
    // terrace permit that way. Then the sources are registered again with them.
    const applicationPaths = caseSourceLaws(corpus.bindings);
    const applicationLaws = laws.filter((law) => applicationPaths.has(law.law_path));
    if (applicationLaws.length) {
      const granted: Record<string, unknown>[] = [];
      const subjects: readonly SubjectKeys[] = population.subjects;
      for (const subject of subjects) {
        const form: ApplicationForm = population.formValues?.[subject.id] ?? {};
        for (const law of applicationLaws) {
          if (notAppliedFor(law, form, applicationPaths)) continue;
          const callParams = callParamsFor(law, subject, form);
          const evaluation = evaluateLaw(engine, law, callParams, referenceDate);
          const outcome = reduceOutcome(law, null, evaluation);
          // Zoals op het portaal: wat besloten is (de vergunde oppervlakte),
          // wat gevraagd is, en de feiten van de zaak zelf erbovenop.
          if (outcome.ok && outcome.met === true) granted.push({ ...((evaluation.ok ? evaluation.outputs : undefined) ?? {}), ...callParams, law: law.law_path, service: law.service, status: 'DECIDED', approved: true, bsn: subject.bsn ?? null, kvk_nummer: subject.kvk_nummer ?? null });
        }
      }
      if (granted.length) registerSources(granted);
    }

    // Evaluate.
    const total = population.subjects.length;
    for (let i = 0; i < total; i += 1) {
      if (signal?.cancelled) break;
      // i < total = population.subjects.length.
      const subject = population.subjects[i]!;
      const form: ApplicationForm = population.formValues?.[subject.id] ?? {};
      const outcome: SimulationRun['results'][number] = { subject, laws: {} };
      for (const law of laws) {
        if (notAppliedFor(law, form, applicationPaths)) continue;
        const callParams = callParamsFor(law, subject, form);
        outcome.laws[law.id] = reduceOutcome(law, primaries.get(law.id), evaluateLaw(engine, law, callParams, referenceDate));
      }
      results.push(outcome);
      if (i % CHUNK === CHUNK - 1) {
        onProgress?.(i + 1, total);
        await sleep(0);
      }
    }
    onProgress?.(results.length, total);
  } finally {
    // Put the laws back the way the corpus has them; the caller re-registers persona data.
    for (const lawId of Object.keys(applied)) {
      const law = corpus.latestById.get(lawId);
      if (law) engine.loadLaw(law.text);
    }
    engine.clearDataSources();
  }

  const lawIds = laws.map((l) => l.id);
  return {
    id: `run-${Date.now().toString(36)}`,
    kind,
    params: JSON.parse(JSON.stringify(params)),
    overrides: applied,
    referenceDate,
    cancelled: !!signal?.cancelled,
    laws: laws.map((law) => ({ id: law.id, name: law.name, service: law.service, law_path: law.law_path, primaryName: primaries.get(law.id) })),
    skipped: skipped.map(({ law, missing }) => ({ id: law.id, name: law.name, missing })),
    subjects: population.subjects,
    results,
    summary: Object.fromEntries(lawIds.map((id) => [id, summariseLaw(results, id)])),
    population: describePopulation(kind, population.subjects),
    durationMs: Math.round(performance.now() - started),
  };
}
