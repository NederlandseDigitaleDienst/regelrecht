/**
 * The small pieces the scenario view needs to show a Gherkin step as written,
 * only easier to read.
 *
 * Nothing here computes or converts: an amount stays the number in the file,
 * a date stays its ISO text. What a scenario *means* is the engine's business,
 * and only after "Uitvoeren". This module decides how the text is set.
 */
import { matchStep } from '@regelrecht/frontend-shared/gherkin';
import type { Step } from '@regelrecht/frontend-shared/gherkin';
import { t } from '../i18n/index.js';

/** A table cell as it is shown. */
export interface DisplayCell {
  text: string;
  /** Set for the words that stand for "nothing", shown in a quieter colour. */
  quiet: boolean;
}

/** The result the runner records for one step. */
export interface StepResult {
  status: string;
  error?: string | null;
}

/** A design-system icon and colour marking a step after a run. */
export interface StepMark {
  icon: string;
  color: string;
}

/** A step as these helpers take it; they read only its text. */
type StepText = Pick<Step, 'text'> & Partial<Step>;

/**
 * A table cell for display.
 *
 * Returns `{ text, quiet }`. The three kinds of "nothing" in a data table read
 * as words, in a quieter colour, instead of as tokens:
 * - `null` and `[]` state an absence and read "geen";
 * - an empty cell states nothing at all, so the engine sees the field as
 *   unknown (RFC-036), and it reads "niet opgegeven".
 * Everything else is the cell text as it stands in the file.
 */
export function displayCell(raw: unknown): DisplayCell {
  const text = String(raw ?? '').trim();
  if (text === '') return { text: t('scenario.value.not_stated'), quiet: true };
  if (text === 'null' || text === '[]') return { text: t('format.none'), quiet: true };
  return { text, quiet: false };
}

/**
 * A step's text with its quoted arguments set in bold rather than in quotes:
 * `is "hoogte_toeslag" gelijk aan 165412` becomes
 * `is **hoogte_toeslag** gelijk aan 165412`, which the design system's text
 * cells render as bold. An empty argument keeps its quotes, since `****`
 * would show nothing.
 */
export function emphasiseArguments(text: unknown): string {
  return String(text).replace(/"([^"]*)"/g, (whole: string, inner: string) => (inner ? `**${inner}**` : whole));
}

/**
 * The width of the keyword column, from the longest keyword it has to hold.
 *
 * In `ch`, not pixels, so it follows the font; with room for the bold weight
 * (wider than the `0` a `ch` measures) plus one `ch` of air. A column narrower
 * than its keyword breaks it mid-word: the text cell wraps anywhere, so a
 * 64px column set "Gegeven" as "Gegeve" / "n".
 */
export function keywordColumnWidth(keywords: readonly unknown[]): string {
  const longest = Math.max(1, ...keywords.map((k) => String(k).length));
  return `${Math.ceil(longest * 1.2) + 1}ch`;
}

/**
 * Whether a step's data table starts with a header row. Every table does
 * except the parameters table (`the following parameters:`), whose rows are
 * name/value pairs from the first line on; the runner reads it that way too.
 */
export function hasHeaderRow(step: StepText): boolean {
  return matchStep(step.text)?.entry.id !== 'set_parameters_table';
}

/**
 * A run's per-step results, split where the background ends.
 *
 * The runner runs the background steps first and records one result per step,
 * so result `i` of the scenario's own steps is result `backgroundLength + i` of
 * the run. The view renders the scenario's steps on their own, so it needs
 * them with that offset taken off, and the background's failure separately
 * (the background box above the scenarios is shared and shows no run).
 */
export function splitRunResults(
  results: StepResult[] | null | undefined,
  backgroundLength: number,
): { background: StepResult[]; scenario: StepResult[]; backgroundError: string | null } {
  const all = results ?? [];
  const background = all.slice(0, backgroundLength);
  return {
    background,
    scenario: all.slice(backgroundLength),
    backgroundError: background.find((r) => r.status === 'fail')?.error ?? null,
  };
}

/**
 * The mark a step gets after a run, or null.
 *
 * A failed step is marked whatever it is: a data table the runner refused or
 * a law that would not load stops the scenario there, and that is where the
 * reader has to look. A pass is only marked on an expectation, since a Given
 * that "passed" says nothing more than that it was read.
 */
export function stepMark(step: StepText, result: StepResult | null | undefined): StepMark | null {
  if (result?.status === 'fail') return { icon: 'dismiss-circle', color: 'critical' };
  if (result?.status === 'pass' && isExpectation(step)) return { icon: 'check-mark-circle', color: 'success' };
  return null;
}

/** Whether a step is an expectation (a Then step in the grammar), which gets a pass/fail mark after a run. */
export function isExpectation(step: StepText): boolean {
  return matchStep(step.text)?.entry.keyword === 'then';
}
