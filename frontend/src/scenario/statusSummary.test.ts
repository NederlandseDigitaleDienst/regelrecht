import { describe, it, expect } from 'vitest';
import { countStatuses, statusBadge, summarizeRuns } from './statusSummary';
import type { ScenarioRun } from './run';

const run = (over: Partial<ScenarioRun>): ScenarioRun => ({
  result: { outputs: { ok: true } },
  traceText: null,
  error: null,
  outputName: 'ok',
  expectations: { ok: 'true' },
  ...over,
});

const PASSED = run({});
const FAILED = run({ error: 'stuk' });
const NO_EXPECTATIONS = run({ expectations: {} });
const PENDING = run({ result: null, pending: true });

describe('summarizeRuns', () => {
  it('keeps what needs attention in file order, and the passed ones apart', () => {
    const summary = summarizeRuns(
      ['a', 'b', 'c', 'd', 'e'],
      [PASSED, FAILED, NO_EXPECTATIONS, PASSED, PENDING],
    );
    expect(summary.attention.map((l) => [l.name, l.status])).toEqual([
      ['b', 'failed'],
      ['c', 'no-expectations'],
      ['e', null],
    ]);
    expect(summary.passed.map((l) => [l.index, l.name])).toEqual([[0, 'a'], [3, 'd']]);
  });

  it('counts a scenario that has not run as needing attention, without a status', () => {
    expect(summarizeRuns(['a'], []).attention).toEqual([{ index: 0, name: 'a', status: null }]);
  });
});

describe('countStatuses', () => {
  it('adds up the files', () => {
    const one = summarizeRuns(['a', 'b'], [PASSED, FAILED]);
    const two = summarizeRuns(['c', 'd', 'e'], [NO_EXPECTATIONS, PASSED, PENDING]);
    expect(countStatuses([one, two])).toEqual({ total: 5, passed: 2, failed: 1, noExpectations: 1, pending: 1 });
    expect(countStatuses([])).toEqual({ total: 0, passed: 0, failed: 0, noExpectations: 0, pending: 0 });
  });
});

describe('statusBadge', () => {
  const counts = (over: Partial<ReturnType<typeof countStatuses>>) =>
    ({ total: 0, passed: 0, failed: 0, noExpectations: 0, pending: 0, ...over });

  it('shows the failed ones first, critical, with the whole tally in its name', () => {
    expect(statusBadge(counts({ total: 22, failed: 4, passed: 18 }))).toEqual({
      text: '4', icon: 'error', critical: true, label: "Scenario's: 4 mislukt, 18 geslaagd",
    });
  });

  it('shows all of them, green, when all passed', () => {
    expect(statusBadge(counts({ total: 22, passed: 22 }))).toEqual({
      text: '22', icon: 'success', critical: false, label: "Scenario's: 22 geslaagd",
    });
  });

  it('shows passed out of all, not green, when some have no outcome', () => {
    expect(statusBadge(counts({ total: 21, passed: 18, pending: 3 }))).toEqual({
      text: '18/21', icon: 'lab', critical: false, label: "Scenario's: 18/21 geslaagd, 3 nog zonder uitkomst",
    });
    expect(statusBadge(counts({ total: 3, noExpectations: 2, pending: 1 })).label)
      .toBe("Scenario's: 0/3 geslaagd, 2 zonder verwachting, 1 nog zonder uitkomst");
    expect(statusBadge(counts({})).text).toBe('0/0');
  });
});
