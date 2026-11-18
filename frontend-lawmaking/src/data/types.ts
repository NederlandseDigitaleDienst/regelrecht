/**
 * The shape of a flow dataset: the law-making process drawn as a git branch
 * model. The three datasets (simple, advanced, Woo) are written by hand, and
 * flow-data.test.ts checks what the types cannot: that every reference
 * between them resolves.
 */

/** A lane in the diagram: main, develop, a fork, a topic branch. */
export interface Branch {
  id: string;
  label: string;
  /** The git name shown beside the lane, such as `fork/wetsvoorstel`. */
  gitLabel: string;
  color: string;
  col: number;
  /**
   * Ignored by the layout, which derives a branch's rows from its stages.
   * FlowDiagram falls back on them only for a branch without stages.
   */
  startRow?: number;
  endRow?: number;
}

export type StageType = 'commit' | 'branch' | 'merge' | 'review' | 'ci-check';

export interface StageTag {
  label: string;
  color: string;
}

/** One step in the process, drawn as a node. */
export interface Stage {
  id: string;
  /** The `id` of the branch it sits on. */
  branch: string;
  type: StageType;
  gitLabel: string;
  lawLabel: string;
  subtitle: string;
  description: string;
  col: number;
  /** When the node appears during playback; also its order in the layout. */
  step: number;
  tags?: StageTag[];
  date?: string;
}

/** A stage with the row FlowDiagram's layout gave it. */
export interface PlacedStage extends Stage {
  row: number;
}

export type ConnectionType =
  | 'branch-off'
  | 'straight'
  | 'merge-in'
  | 'main-continues'
  | 'ci-fork'
  | 'ci-return';

/** An edge between two stages, by `id`. */
export interface Connection {
  from: string;
  to: string;
  type: ConnectionType;
}

/**
 * A shaded band behind a stretch of the process. It is bounded by stages
 * (`startStage`/`endStage`, resolved after layout) or, in the Woo dataset, by
 * fixed rows.
 */
export interface Phase {
  id: string;
  label: string;
  color: string;
  startStage?: string;
  endStage?: string;
  startRow?: number;
  endRow?: number;
}

/** A label on the left axis, one per row, in the Woo dataset. */
export interface TimelineMarker {
  label: string;
}

export interface FlowDataset {
  stages: Stage[];
  branches: Branch[];
  connections: Connection[];
  phases: Phase[] | null;
  timeline: TimelineMarker[] | null;
}
