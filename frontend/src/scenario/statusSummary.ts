/**
 * The outcome of a file's runs in the shape the editor shows it: what needs
 * attention one by one, and the passed scenarios as one group.
 */
import { runStatus, type ScenarioRun, type ScenarioStatus } from './run';

/** One scenario of a file, with the status of its latest run. */
export interface ScenarioLine {
  index: number;
  name: string;
  /** Null before the scenario ran, or while its regelwerk is not loaded yet. */
  status: ScenarioStatus | null;
}

export interface FileSummary {
  /** Everything that did not pass, in file order: failed, no expectations, not run yet. */
  attention: ScenarioLine[];
  passed: ScenarioLine[];
}

export interface StatusCounts {
  total: number;
  passed: number;
  failed: number;
  noExpectations: number;
  /** Not run yet. */
  pending: number;
}

/** Split a file's scenarios by the status of their runs. */
export function summarizeRuns(names: readonly string[], runs: readonly (ScenarioRun | undefined)[]): FileSummary {
  const attention: ScenarioLine[] = [];
  const passed: ScenarioLine[] = [];
  names.forEach((name, index) => {
    const line = { index, name, status: runStatus(runs[index]) };
    (line.status === 'passed' ? passed : attention).push(line);
  });
  return { attention, passed };
}

/** The statuses of all files together. */
export function countStatuses(files: readonly FileSummary[]): StatusCounts {
  const counts: StatusCounts = { total: 0, passed: 0, failed: 0, noExpectations: 0, pending: 0 };
  for (const file of files) {
    counts.passed += file.passed.length;
    for (const line of file.attention) {
      if (line.status === 'failed') counts.failed++;
      else if (line.status === 'no-expectations') counts.noExpectations++;
      else counts.pending++;
    }
    counts.total += file.passed.length + file.attention.length;
  }
  return counts;
}

/** The status in words, as the editor names it. */
export const STATUS_TEXT: Record<ScenarioStatus, string> = {
  passed: 'Geslaagd',
  failed: 'Mislukt',
  'no-expectations': 'Geen verwachting',
};

/** How the editor's toolbar shows the status of a regelwerk's scenarios. */
export interface StatusBadge {
  /** The visible count: the failed ones, else the passed ones, else all. */
  text: string;
  icon: 'error' | 'success' | 'lab';
  critical: boolean;
  /** The accessible name: the visible count first, then the whole tally. */
  label: string;
}

export function statusBadge(c: StatusCounts): StatusBadge {
  const tally = [
    c.failed && `${c.failed} mislukt`,
    c.passed && `${c.passed} geslaagd`,
    c.noExpectations && `${c.noExpectations} zonder verwachting`,
    c.pending && `${c.pending} nog zonder uitkomst`,
  ].filter(Boolean);
  if (c.failed) {
    return { text: String(c.failed), icon: 'error', critical: true, label: `Scenario's: ${tally.join(', ')}` };
  }
  if (c.passed) {
    return { text: String(c.passed), icon: 'success', critical: false, label: `Scenario's: ${tally.join(', ')}` };
  }
  // Nothing failed or passed: the count is all of them, and the name says so.
  const label = tally.length ? `Scenario's: ${c.total}, ${tally.join(', ')}` : `Scenario's: ${c.total}`;
  return { text: String(c.total), icon: 'lab', critical: false, label };
}
