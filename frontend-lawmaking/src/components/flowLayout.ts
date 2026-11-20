/**
 * The layout of the flow diagram, apart from the SVG that draws it: which row
 * each stage lands on, which rows a branch or phase spans, what is visible at a
 * playback step, and the path of a connection. FlowDiagram and FlowConnection
 * render what these functions return.
 */
import type { Branch, Connection, Phase, PlacedStage, Stage } from '../data/types';

/** Rows between two consecutive stages in one column. */
export const SAME_COL_GAP = 1;
/** Rows between a stage and one it connects to in another column. */
export const CROSS_COL_GAP = 2;

/** Where the first column and the first row sit in the SVG; every component draws from here. */
export const ORIGIN = { x: 80, y: 50 };

/** A branch or phase whose rows are known, so it can be drawn. */
export type Rows = { startRow: number; endRow: number };

/**
 * Gives every stage a row. Stages are placed in `step` order; a stage goes
 * below every stage that connects to it (two rows when the connection crosses
 * columns), below the previous stage in its own column, and, when nothing
 * connects to it, below everything placed so far.
 */
export function placeStages(stages: readonly Stage[], connections: readonly Connection[]): PlacedStage[] {
  const sorted = [...stages].sort((a, b) => a.step - b.step);
  const rowMap = new Map<string, number>();
  const colLastRow = new Map<number, number>();
  let globalLastRow = -1;

  for (const stage of sorted) {
    const inbound = connections.filter((c) => c.to === stage.id);
    let row = 0;

    for (const conn of inbound) {
      const fromRow = rowMap.get(conn.from);
      if (fromRow === undefined) continue;
      const fromStage = sorted.find((s) => s.id === conn.from);
      const crossColumn = fromStage && fromStage.col !== stage.col;
      const gap = crossColumn ? CROSS_COL_GAP : SAME_COL_GAP;
      row = Math.max(row, fromRow + gap);
    }

    if (inbound.length === 0 && globalLastRow >= 0) {
      row = Math.max(row, globalLastRow + SAME_COL_GAP);
    }

    const lastInCol = colLastRow.get(stage.col);
    if (lastInCol !== undefined) {
      row = Math.max(row, lastInCol + SAME_COL_GAP);
    }

    rowMap.set(stage.id, row);
    colLastRow.set(stage.col, row);
    globalLastRow = Math.max(globalLastRow, row);
  }

  return sorted.map((s) => ({ ...s, row: rowMap.get(s.id) ?? 0 }));
}

/**
 * The rows each branch spans: from its first stage to its last. A branch
 * without stages keeps the rows written in its data, or row 0.
 */
export function branchRows(branches: readonly Branch[], placed: readonly PlacedStage[]): (Branch & Rows)[] {
  return branches.map((branch) => {
    const rows = placed.filter((s) => s.branch === branch.id).map((s) => s.row);
    if (rows.length === 0) {
      return { ...branch, startRow: branch.startRow ?? 0, endRow: branch.endRow ?? 0 };
    }
    return { ...branch, startRow: Math.min(...rows), endRow: Math.max(...rows) };
  });
}

/**
 * The rows of each phase. A phase bounded by stages takes their rows; a stage
 * id that resolves to nothing leaves the fixed row from the data in place.
 */
export function resolvePhaseRows(phases: readonly Phase[], placed: readonly PlacedStage[]): Phase[] {
  const rowOf = (id: string | undefined) => (id ? placed.find((s) => s.id === id)?.row : undefined);
  return phases.map((p) => ({
    ...p,
    startRow: rowOf(p.startStage) ?? p.startRow,
    endRow: rowOf(p.endStage) ?? p.endRow,
  }));
}

/**
 * The phases to shade at `activeStep`: those with known rows whose earliest
 * stage has been reached. A phase that covers no stage is never shown.
 */
export function visiblePhases(
  phases: readonly Phase[],
  placed: readonly PlacedStage[],
  activeStep: number,
): (Phase & Rows)[] {
  return phases.filter((p): p is Phase & Rows => {
    const { startRow, endRow } = p;
    if (startRow === undefined || endRow === undefined) return false;
    const steps = placed.filter((s) => s.row >= startRow && s.row <= endRow).map((s) => s.step);
    return steps.length > 0 && activeStep >= Math.min(...steps);
  });
}

/** Whether a branch is drawn at `activeStep`: once its first stage is reached. */
export function isBranchVisible(branch: Branch, placed: readonly PlacedStage[], activeStep: number): boolean {
  const steps = placed.filter((s) => s.branch === branch.id).map((s) => s.step);
  return steps.length > 0 && activeStep >= Math.min(...steps);
}

/**
 * Whether a connection is lit at `activeStep`: once the stage it leads to is
 * reached. A connection with an end that resolves to no stage never lights.
 */
export function isConnectionActive(conn: Connection, placed: readonly PlacedStage[], activeStep: number): boolean {
  const from = placed.find((s) => s.id === conn.from);
  const to = placed.find((s) => s.id === conn.to);
  if (!from || !to) return false;
  return activeStep >= to.step;
}

/** The centre of a placed stage in SVG coordinates. */
export function stagePosition(stage: PlacedStage, columnWidth: number, rowHeight: number): { x: number; y: number } {
  return { x: ORIGIN.x + stage.col * columnWidth, y: ORIGIN.y + stage.row * rowHeight };
}

/**
 * The SVG path between two points: a straight line within a column, an S-curve
 * that bends halfway between the two rows across columns.
 */
export function connectionPath(from: { x: number; y: number }, to: { x: number; y: number }): string {
  if (from.x === to.x) {
    return `M ${from.x} ${from.y} L ${to.x} ${to.y}`;
  }
  const midY = (from.y + to.y) / 2;
  return `M ${from.x} ${from.y} C ${from.x} ${midY}, ${to.x} ${midY}, ${to.x} ${to.y}`;
}
