/**
 * Where the walkthrough meets the graph's camera.
 *
 * Panning and zooming the graph (vue-flow) happens through dragging and the
 * mouse wheel, which the action log does not see: there is no element that
 * was clicked. So the graph reports where it looks, and the replay tells it
 * where to look. Both as the point in the middle of the pane, in graph
 * coordinates, plus the zoom: a position in pixels would land elsewhere on a
 * screen of another size.
 *
 * Dragging a law aside is the same kind of action: the graph reports which
 * node moved and by how much from where the layout put it, and the replay
 * moves it there again. Relative to the layout, so a take still lands right
 * after the layout itself shifted.
 *
 * GraafView fills `graphView.api` while it is mounted; the recorder sets
 * `graphView.onView` and `graphView.onNodeMove` while a take runs.
 */

import type { Ref } from 'vue';

/**
 * Where the graph looks: the middle of the pane in graph coordinates, and the zoom.
 *
 * A type alias rather than an interface, so a viewport action stays assignable
 * to the recorder's `LoggedEvent`, which has an index signature.
 */
export type GraphViewPoint = {
  cx: number;
  cy: number;
  zoom: number;
};

/** A dragged node: its offset from where the layout put it, and how long the drag took (ms). */
export interface NodeMove {
  id: string;
  dx: number;
  dy: number;
  ms: number;
}

/** A vue-flow viewport: the pane's offset in pixels and its zoom. */
export interface FlowViewport {
  x: number;
  y: number;
  zoom: number;
}

/** What GraafView hands the bridge while it is mounted: vue-flow's camera, plus its own `moveNode`. */
export interface GraphApi {
  setCenter: (x: number, y: number, options: { zoom?: number; duration?: number }) => unknown;
  getViewport: () => FlowViewport;
  dimensions: Readonly<Ref<{ width: number; height: number }>>;
  moveNode?: (id: string, dx: number, dy: number, duration: number) => boolean;
}

export interface GraphBridge {
  /** `{ setCenter, getViewport, dimensions }` from vue-flow, or null. */
  api: GraphApi | null;
  /** Called with `{ cx, cy, zoom }` whenever the view moves. */
  onView: ((view: GraphViewPoint) => void) | null;
  /** Called with `{ id, dx, dy, ms }` when a node was dragged: its offset from the layout, and how long the drag took. */
  onNodeMove: ((move: NodeMove) => void) | null;
}

export const graphView: GraphBridge = {
  api: null,
  onView: null,
  onNodeMove: null,
};

/** The middle of the pane in graph coordinates, from a vue-flow viewport. */
export function centerOf(viewport: Partial<FlowViewport> | null | undefined, dimensions: { width?: number; height?: number } | null | undefined): GraphViewPoint {
  const { x = 0, y = 0, zoom = 1 }: Partial<FlowViewport> = viewport ?? {};
  const w = dimensions?.width ?? 0;
  const h = dimensions?.height ?? 0;
  const round = (n: number) => Math.round(n * 10) / 10;
  return { cx: round((w / 2 - x) / zoom), cy: round((h / 2 - y) / zoom), zoom: Math.round(zoom * 1000) / 1000 };
}

/** Move node `id` to `dx, dy` from where the layout puts it, over `duration` ms. False when no graph is mounted. */
export function moveNode(id: string, dx: number, dy: number, duration = 0) {
  const api = graphView.api;
  if (!api?.moveNode) return false;
  return api.moveNode(id, dx, dy, duration);
}

/** Tell the graph to look at `view`, over `duration` ms. False when no graph is mounted. */
export function showView(view: GraphViewPoint, duration = 0) {
  const api = graphView.api;
  if (!api?.setCenter) return false;
  api.setCenter(view.cx, view.cy, { zoom: view.zoom, duration });
  return true;
}
