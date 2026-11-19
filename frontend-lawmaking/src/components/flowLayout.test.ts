import { describe, expect, test } from 'vitest';
import type { Branch, Connection, Phase, PlacedStage, Stage } from '../data/types';
import {
  CROSS_COL_GAP,
  SAME_COL_GAP,
  branchRows,
  connectionPath,
  isBranchVisible,
  isConnectionActive,
  placeStages,
  resolvePhaseRows,
  stagePosition,
  visiblePhases,
} from './flowLayout';

function stage(id: string, step: number, col: number, branch = 'main'): Stage {
  return { id, step, col, branch, type: 'commit', gitLabel: id, lawLabel: id, subtitle: '', description: '' };
}

function branch(id: string, col: number, rows: Partial<Pick<Branch, 'startRow' | 'endRow'>> = {}): Branch {
  return { id, col, label: id, gitLabel: id, color: 'red', ...rows };
}

function conn(from: string, to: string): Connection {
  return { from, to, type: 'straight' };
}

const rowsById = (placed: PlacedStage[]) => Object.fromEntries(placed.map((s) => [s.id, s.row]));

describe('placeStages', () => {
  test('places stages in step order, whatever order the data lists them in', () => {
    const placed = placeStages([stage('b', 2, 0), stage('a', 1, 0)], [conn('a', 'b')]);
    expect(placed.map((s) => s.id)).toEqual(['a', 'b']);
  });

  test('puts a stage one row below its predecessor in the same column', () => {
    const placed = placeStages([stage('a', 1, 0), stage('b', 2, 0)], [conn('a', 'b')]);
    expect(rowsById(placed)).toEqual({ a: 0, b: SAME_COL_GAP });
  });

  test('puts a stage two rows below a predecessor in another column', () => {
    const placed = placeStages([stage('a', 1, 0), stage('f', 2, 1, 'fork')], [conn('a', 'f')]);
    expect(rowsById(placed)).toEqual({ a: 0, f: CROSS_COL_GAP });
  });

  test('puts a merge below the lowest of the stages that flow into it', () => {
    const stages = [stage('a', 1, 0), stage('f1', 2, 1), stage('f2', 3, 1), stage('m', 4, 0)];
    const connections = [conn('a', 'f1'), conn('f1', 'f2'), conn('f2', 'm'), conn('a', 'm')];
    // f1 at 2, f2 at 3; the merge crosses back from f2, so 3 + 2.
    expect(rowsById(placeStages(stages, connections)).m).toBe(5);
  });

  test('puts a stage without incoming connections below everything placed so far', () => {
    const stages = [stage('a', 1, 0), stage('f', 2, 1), stage('loose', 3, 2)];
    const placed = placeStages(stages, [conn('a', 'f')]);
    expect(rowsById(placed).loose).toBe(CROSS_COL_GAP + SAME_COL_GAP);
  });

  test('never stacks two stages of one column on the same row', () => {
    // Both stages hang off `a` across columns, so both would land on row 2.
    const stages = [stage('a', 1, 0), stage('x', 2, 1), stage('y', 3, 1)];
    const placed = placeStages(stages, [conn('a', 'x'), conn('a', 'y')]);
    expect(rowsById(placed)).toEqual({ a: 0, x: 2, y: 3 });
  });

  test('ignores a connection from a stage that comes later in the order', () => {
    const placed = placeStages([stage('a', 1, 0), stage('b', 2, 1)], [conn('b', 'a')]);
    expect(rowsById(placed).a).toBe(0);
  });
});

describe('branchRows', () => {
  test('spans a branch from its first stage to its last', () => {
    const placed = placeStages(
      [stage('a', 1, 0), stage('f1', 2, 1, 'fork'), stage('f2', 3, 1, 'fork')],
      [conn('a', 'f1'), conn('f1', 'f2')],
    );
    const [, fork] = branchRows([branch('main', 0), branch('fork', 1)], placed);
    expect(fork).toMatchObject({ startRow: 2, endRow: 3 });
  });

  test('derives the rows from the stages, not from rows written in the data', () => {
    const placed = placeStages([stage('a', 1, 0)], []);
    const [main] = branchRows([branch('main', 0, { startRow: 7, endRow: 9 })], placed);
    expect(main).toMatchObject({ startRow: 0, endRow: 0 });
  });

  test('keeps the written rows of a branch without stages, or row 0', () => {
    const rows = branchRows([branch('empty', 2, { startRow: 4, endRow: 6 }), branch('bare', 3)], []);
    expect(rows.map(({ startRow, endRow }) => [startRow, endRow])).toEqual([
      [4, 6],
      [0, 0],
    ]);
  });
});

describe('phases', () => {
  const placed = placeStages(
    [stage('a', 1, 0), stage('b', 2, 0), stage('c', 3, 0)],
    [conn('a', 'b'), conn('b', 'c')],
  );
  const phase = (fields: Partial<Phase>): Phase => ({ id: 'p', label: 'P', color: 'blue', ...fields });

  test('a phase bounded by stages takes their rows', () => {
    const [p] = resolvePhaseRows([phase({ startStage: 'b', endStage: 'c' })], placed);
    expect(p).toMatchObject({ startRow: 1, endRow: 2 });
  });

  test('a stage id that resolves to nothing falls back on the fixed row', () => {
    const [p] = resolvePhaseRows([phase({ startStage: 'gone', startRow: 0, endRow: 1 })], placed);
    expect(p).toMatchObject({ startRow: 0, endRow: 1 });
  });

  test('a phase appears once playback reaches its earliest stage', () => {
    const phases = resolvePhaseRows([phase({ startStage: 'b', endStage: 'c' })], placed);
    expect(visiblePhases(phases, placed, 1)).toEqual([]);
    expect(visiblePhases(phases, placed, 2).map((p) => p.id)).toEqual(['p']);
  });

  test('a phase without rows, or over rows without stages, never appears', () => {
    const phases = [phase({ id: 'norows' }), phase({ id: 'empty', startRow: 10, endRow: 12 })];
    expect(visiblePhases(phases, placed, 99)).toEqual([]);
  });
});

describe('playback', () => {
  const placed = placeStages(
    [stage('a', 1, 0), stage('f', 3, 1, 'fork')],
    [conn('a', 'f')],
  );

  test('a branch is drawn from the step of its first stage on', () => {
    expect(isBranchVisible(branch('fork', 1), placed, 2)).toBe(false);
    expect(isBranchVisible(branch('fork', 1), placed, 3)).toBe(true);
  });

  test('a branch without stages is never drawn', () => {
    expect(isBranchVisible(branch('empty', 2), placed, 99)).toBe(false);
  });

  test('a connection lights up when playback reaches the stage it leads to', () => {
    expect(isConnectionActive(conn('a', 'f'), placed, 2)).toBe(false);
    expect(isConnectionActive(conn('a', 'f'), placed, 3)).toBe(true);
  });

  test('a connection to a stage that does not exist never lights up', () => {
    expect(isConnectionActive(conn('a', 'missing'), placed, 99)).toBe(false);
  });
});

describe('connection geometry', () => {
  test('a stage sits at its column and row, offset from the origin', () => {
    const [, f] = placeStages([stage('a', 1, 0), stage('f', 2, 1)], [conn('a', 'f')]);
    expect(stagePosition(f!, 220, 80)).toEqual({ x: 80 + 220, y: 50 + 2 * 80 });
  });

  test('within a column the path is a straight line', () => {
    expect(connectionPath({ x: 80, y: 50 }, { x: 80, y: 130 })).toBe('M 80 50 L 80 130');
  });

  test('across columns the path is an S-curve that bends halfway between the rows', () => {
    expect(connectionPath({ x: 80, y: 50 }, { x: 300, y: 210 })).toBe('M 80 50 C 80 130, 300 130, 300 210');
  });

  test('the bend sits halfway also when the path runs upwards', () => {
    expect(connectionPath({ x: 300, y: 210 }, { x: 80, y: 50 })).toBe('M 300 210 C 300 130, 80 130, 80 50');
  });
});
