/**
 * Importing feature files into a regelwerk's scenario folder: which of the
 * chosen files fit the open regelwerk and the traject's corpus, decided in the
 * browser before anything is written.
 *
 * `checkImport` is pure: it judges one file against what is already known
 * (the listed files, the ruleworks the corpus has, the outputs of the open
 * regelwerk). `checkImports` gathers that knowledge from the corpus for a
 * whole selection, with one request per question.
 *
 * Each file gets one verdict with one line of text. An `error` is not saved;
 * a `warning` is saved, but the line says what the editor will not be able to
 * do with it.
 */
import { parseFeature } from '../gherkin/parser';
import { mapFeatureToForm, type FormState } from '../gherkin/formMapper';
import { isScenarioMismatch } from '../composables/useScenarios';
import type { TrajectCorpusView } from '../corpus';
import { scenarioLawsOf } from './scenarioLaws';

/** `MAX_SCENARIO_BODY` in packages/editor-api/src/router.rs. */
export const MAX_SCENARIO_BYTES = 1024 * 1024;

/** A chosen file, read. */
export interface ImportCandidate {
  filename: string;
  text: string;
  /** Size in bytes; defaults to the UTF-8 length of `text`. */
  size?: number;
}

export type ImportStatus = 'ok' | 'warning' | 'error';

export interface ImportVerdict {
  filename: string;
  text: string;
  status: ImportStatus;
  /** One line for the user, in Dutch. */
  reason: string;
}

/** Files saved into one regelwerk's folder, in the order they were chosen. */
export interface ScenarioImport {
  trajectRef: string;
  lawId: string;
  filenames: string[];
}

/** What a file is checked against. */
export interface ImportContext {
  /** The open regelwerk, which the files are imported into. */
  lawId: string;
  /** Filenames already in the regelwerk's scenario folder. */
  existing: readonly string[];
  /** The ruleworks the traject's corpus has, of the ones the files name. */
  knownLaws: ReadonlySet<string>;
  /** The outputs the open regelwerk declares; null when they could not be read. */
  lawOutputs: ReadonlySet<string> | null;
}

/**
 * Why the backend would refuse this filename (`validate_scenario_filename` in
 * packages/editor-api/src/corpus_handlers.rs), or null when it accepts it.
 */
export function filenameProblem(filename: string): string | null {
  if (!filename.endsWith('.feature')) return 'Alleen .feature-bestanden kunnen worden geïmporteerd';
  if (/[/\\\0]/.test(filename) || filename.includes('..')) return 'Deze bestandsnaam is niet toegestaan';
  return null;
}

function byteLength(text: string): number {
  return new TextEncoder().encode(text).length;
}

// The parser's message starts with the position and the problem; the rest
// lists every token it expected, which helps nobody here.
function firstLine(message: string): string {
  // split always yields at least one element, so index 0 exists.
  return message.split('\n')[0]!.trim();
}

/** The feature parsed into the editor's form, or the parser's complaint. */
export function parseCandidate(text: string): { state: FormState } | { error: string } {
  try {
    return { state: mapFeatureToForm(parseFeature(text)) };
  } catch (e) {
    return { error: firstLine(e instanceof Error ? e.message : String(e)) };
  }
}

function scenarioCount(n: number): string {
  return n === 1 ? '1 scenario' : `${n} scenario's`;
}

/** The ruleworks the scenarios execute, in file order. */
function targetsOf(state: FormState): string[] {
  return [...new Set(state.scenarios.flatMap((s) => (s.execution ? [s.execution.lawId] : [])))];
}

/** The outputs the scenarios that execute `lawId` evaluate or assert. */
function outputsAsked(state: FormState, lawId: string): string[] {
  const names = new Set<string>();
  for (const s of state.scenarios) {
    if (s.execution?.lawId !== lawId) continue;
    names.add(s.execution.outputName);
    for (const a of s.assertions) if (a.outputName) names.add(a.outputName);
  }
  return [...names];
}

function unmatchedCount(state: FormState): number {
  return (
    (state.background?.unmatchedSteps.length ?? 0) +
    state.scenarios.reduce((n, s) => n + s.unmatchedSteps.length, 0)
  );
}

/** Filenames compared the way a case-insensitive filesystem (macOS) would. */
function sameName(a: string, b: string): boolean {
  return a.toLowerCase() === b.toLowerCase();
}

/**
 * The checks that need no parsing: the name, the size, and whether the name is
 * taken in the folder or earlier in the selection (`seen`). Null when they pass.
 */
export function precheck(
  candidate: ImportCandidate,
  existing: readonly string[],
  seen: readonly string[] = [],
): string | null {
  const { filename, text } = candidate;
  const badName = filenameProblem(filename);
  if (badName) return badName;
  if ((candidate.size ?? byteLength(text)) > MAX_SCENARIO_BYTES) return 'Het bestand is groter dan 1 MB';
  if (existing.some((e) => sameName(e, filename))) return 'Er bestaat al een scenariobestand met deze naam';
  if (seen.some((e) => sameName(e, filename))) return 'Er is al een bestand met deze naam gekozen';
  return null;
}

/**
 * Judge one file. `seen` holds the filenames chosen before this one in the
 * same selection. `parsed` saves a second parse when the caller has one.
 */
export function checkImport(
  candidate: ImportCandidate,
  context: ImportContext,
  seen: readonly string[] = [],
  parsed?: ReturnType<typeof parseCandidate>,
): ImportVerdict {
  const { filename, text } = candidate;
  const verdict = (status: ImportStatus, reason: string): ImportVerdict => ({ filename, text, status, reason });

  const refused = precheck(candidate, context.existing, seen);
  if (refused) return verdict('error', refused);
  const result = parsed ?? parseCandidate(text);
  if ('error' in result) return verdict('error', `Geen geldig Gherkin: ${result.error}`);
  const { state } = result;
  if (state.scenarios.length === 0) return verdict('error', "Het bestand bevat geen scenario's");

  const targets = targetsOf(state);
  if (isScenarioMismatch({ target_law_ids: targets }, context.lawId)) {
    return verdict('error', `Rekent met een ander regelwerk: ${targets.join(', ')}`);
  }
  const missing = scenarioLawsOf(state).filter((id) => id !== context.lawId && !context.knownLaws.has(id));
  if (missing.length) {
    return verdict('error', `Niet in het corpus van dit traject: ${missing.join(', ')}`);
  }

  const warnings: string[] = [];
  if (targets.length === 0) warnings.push('geen stap rekent dit regelwerk uit');
  const unmatched = unmatchedCount(state);
  if (unmatched) {
    warnings.push(unmatched === 1 ? '1 stap herkent de editor niet' : `${unmatched} stappen herkent de editor niet`);
  }
  const declared = context.lawOutputs;
  if (declared) {
    const unknown = outputsAsked(state, context.lawId).filter((o) => !declared.has(o));
    if (unknown.length) warnings.push(`onbekende uitvoer: ${unknown.join(', ')}`);
  }
  const count = scenarioCount(state.scenarios.length);
  if (warnings.length) return verdict('warning', `${count}; ${warnings.join('; ')}`);
  return verdict('ok', `${count}, past bij dit regelwerk`);
}

/** The ruleworks the parsed files name, besides the open one: what the corpus must have. */
export function lawsNamed(states: readonly (FormState | null)[], lawId: string): string[] {
  const ids = new Set<string>();
  for (const state of states) if (state) for (const id of scenarioLawsOf(state)) ids.add(id);
  ids.delete(lawId);
  return [...ids];
}

/**
 * Judge a selection against the traject's corpus. A file that fails the cheap
 * checks is not parsed and its ruleworks are not looked up; the others are
 * parsed once. Asks the corpus once which of the named ruleworks it has, and
 * once for the open regelwerk's outputs; a failed outputs read skips that
 * check rather than the import. A failed law lookup rejects: without it, no
 * file can be said to fit.
 */
export async function checkImports(
  candidates: readonly ImportCandidate[],
  { lawId, existing }: { lawId: string; existing: readonly string[] },
  corpus: Pick<TrajectCorpusView, 'searchLaws' | 'outputs'>,
): Promise<ImportVerdict[]> {
  const before = (i: number) => candidates.slice(0, i).map((c) => c.filename);
  const parsed = candidates.map((c, i) => (precheck(c, existing, before(i)) ? null : parseCandidate(c.text)));
  const named = lawsNamed(parsed.map((p) => (p && 'state' in p ? p.state : null)), lawId);
  const [found, outputs] = await Promise.all([
    named.length ? corpus.searchLaws({ ids: named, limit: named.length }) : Promise.resolve({ laws: [], total: 0 }),
    corpus.outputs(lawId).catch(() => null),
  ]);
  const context: ImportContext = {
    lawId,
    existing,
    knownLaws: new Set(found.laws.map((l) => l.law_id)),
    lawOutputs: outputs ? new Set(outputs.map((o) => o.name)) : null,
  };
  return candidates.map((c, i) => checkImport(c, context, before(i), parsed[i] ?? undefined));
}
