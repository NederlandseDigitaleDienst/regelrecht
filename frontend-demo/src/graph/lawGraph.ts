/**
 * The dependency graph as the POC drew it: one box per law with its register
 * sources, its inputs from other laws and its outputs, and an edge from every
 * supplying output to the input that reads it — the way the layout runs, with
 * the supplier on the left. Pure functions; the view hands the result to
 * vue-flow.
 */
import { MarkerType, Position } from '@vue-flow/core';

export const ITEM_H = 30;
export const ITEM_GAP = 6;
export const BOX_PAD = 8;
export const BOX_LABEL_H = 26;
export const COL_W = 200;
export const LAW_PAD = 12;
export const HEADER_H = 52;
export const LAW_W = LAW_PAD * 3 + COL_W * 2;
const LAYER_GAP = 96;
const ROW_GAP = 40;
const MAX_PER_COLUMN = 3;

/** An execution block of a parsed law document, as far as the graph reads it. */
interface GraphExecution {
  input?: { name: string; source?: { regulation?: string; output?: string } | null }[] | null;
  output?: { name: string }[] | null;
}

/**
 * The fields of a corpus law entry the graph reads. `caseRefs` is what
 * `caseRefsFor` (data/caseRefs) puts on each law.
 */
export interface GraphLaw {
  id: string;
  service?: string | null;
  doc?: { articles?: { machine_readable?: { execution?: GraphExecution | null } | null }[] | null } | null;
  caseRefs?: { name: string; regulation: string; output: string }[] | null;
}

/** A reference from an input to the law and output that supply it. */
export interface InputRef {
  regulation: string;
  output: string | undefined;
}

/** What a law reads and produces, as `lawShape` returns it. */
export interface GraphLawShape {
  sources: { name: string }[];
  inputs: { name: string; ref: InputRef }[];
  outputs: { name: string }[];
}

/** The three boxes of a law node. */
export type BoxKind = 'sources' | 'inputs' | 'outputs';

/** Display text per box kind, law id and item name. */
export type GraphValues = Partial<Record<BoxKind, Record<string, Record<string, string>>>>;

/** A box's size, and the laws it reads from, as `layout` takes them. */
export interface LayoutEntry {
  size: { width: number; height: number };
  deps: string[];
}

export interface Point {
  x: number;
  y: number;
}

/** Data of a `law` node. */
export interface LawNodeData<L extends GraphLaw = GraphLaw> {
  law: L;
  selected: boolean;
}

/** Data of a `box` node. */
export interface BoxNodeData {
  label: string;
  kind: BoxKind;
}

/** Data of an `item` node: one source, input or output. */
export interface ItemNodeData {
  name: string;
  kind: BoxKind;
  value: string | undefined;
  ref: InputRef | null;
}

interface NodeBase {
  id: string;
  hidden: boolean;
  position: Point;
  style: { width: string; height: string };
  class?: string;
  draggable?: boolean;
  selectable: boolean;
  /** Never set: vue-flow's own stacking applies (see the edges' `zIndex`). */
  zIndex?: number;
}

export type GraphNode<L extends GraphLaw = GraphLaw> =
  | (NodeBase & { type: 'law'; data: LawNodeData<L> })
  | (NodeBase & { type: 'box'; parentNode: string; data: BoxNodeData })
  | (NodeBase & {
      type: 'item';
      parentNode: string;
      data: ItemNodeData;
      sourcePosition: Position;
      targetPosition: Position;
    });

/** An edge for vue-flow. `data.from` is the reading law, `data.to` the supplier. */
export interface GraphEdge {
  hidden: boolean;
  class: string;
  id: string;
  source: string;
  target: string;
  data: { from: string; to: string };
  markerEnd: MarkerType;
  animated: boolean;
  style: { opacity: number };
  zIndex: number;
}

/** Every execution block of a law, in article order. */
function executions(law: GraphLaw): GraphExecution[] {
  return (law.doc?.articles ?? []).map((a) => a.machine_readable?.execution).filter((ex): ex is GraphExecution => Boolean(ex));
}

/**
 * What a law reads and produces, deduplicated by name. Inputs another law
 * supplies through its decided cases (`law.caseRefs`, see caseRefsFor) count
 * as references too, when that law has such an output.
 */
export function lawShape(law: GraphLaw): GraphLawShape {
  const sources = new Map<string, { name: string }>();
  const inputs = new Map<string, { name: string; ref: InputRef }>();
  const outputs = new Map<string, { name: string }>();
  for (const ex of executions(law)) {
    for (const input of ex.input ?? []) {
      const ref = input.source?.regulation ? { regulation: input.source.regulation, output: input.source.output } : null;
      if (ref) inputs.set(input.name, { name: input.name, ref });
      else sources.set(input.name, { name: input.name });
    }
    for (const output of ex.output ?? []) outputs.set(output.name, { name: output.name });
  }
  for (const r of law.caseRefs ?? []) {
    if (!sources.has(r.name)) continue;
    sources.delete(r.name);
    inputs.set(r.name, { name: r.name, ref: { regulation: r.regulation, output: r.output } });
  }
  return { sources: [...sources.values()], inputs: [...inputs.values()], outputs: [...outputs.values()] };
}

function boxHeight(count: number): number {
  return BOX_LABEL_H + BOX_PAD * 2 + count * ITEM_H + Math.max(0, count - 1) * ITEM_GAP;
}

/** Size of a law box, from what it holds. */
export function lawSize(shape: GraphLawShape): { width: number; height: number } {
  let left = 0;
  if (shape.sources.length) left += boxHeight(shape.sources.length);
  if (shape.inputs.length) left += (left ? LAW_PAD : 0) + boxHeight(shape.inputs.length);
  const right = shape.outputs.length ? boxHeight(shape.outputs.length) : 0;
  return { width: LAW_W, height: HEADER_H + Math.max(left, right, BOX_LABEL_H) + LAW_PAD };
}

/**
 * Layered positions: a law sits right of every law it reads from. A layer with
 * more than MAX_PER_COLUMN laws wraps into side-by-side sub-columns.
 */
export function layout(laws: Map<string, LayoutEntry>): Map<string, Point> {
  const depth = new Map<string, number>();
  const visit = (id: string, stack = new Set<string>()): number => {
    const known = depth.get(id);
    if (known !== undefined) return known;
    if (stack.has(id)) return 0;
    stack.add(id);
    const d = (laws.get(id)?.deps ?? []).filter((r) => laws.has(r)).reduce((m, r) => Math.max(m, visit(r, stack) + 1), 0);
    stack.delete(id);
    depth.set(id, d);
    return d;
  };
  for (const id of laws.keys()) visit(id);
  const layers = new Map<number, string[]>();
  for (const [id, d] of depth) {
    if (!layers.has(d)) layers.set(d, []);
    // Set just above when missing.
    layers.get(d)!.push(id);
  }
  const positions = new Map<string, Point>();
  let x = 0;
  for (const d of [...layers.keys()].sort((a, b) => a - b)) {
    // `d` comes from the layer keys.
    const ids = layers.get(d)!.sort();
    const subCols = Math.ceil(ids.length / MAX_PER_COLUMN);
    const perCol = Math.ceil(ids.length / subCols);
    let widest = 0;
    for (let c = 0; c < subCols; c += 1) {
      let y = 0;
      for (const id of ids.slice(c * perCol, (c + 1) * perCol)) {
        // Every id in a layer is a key of `laws` (depth is filled from its keys).
        const size = laws.get(id)!.size;
        positions.set(id, { x: x + c * (LAW_W + LAW_PAD * 2), y });
        y += size.height + ROW_GAP;
        widest = Math.max(widest, c * (LAW_W + LAW_PAD * 2) + size.width);
      }
    }
    x += widest + LAYER_GAP;
  }
  return positions;
}

export const itemId = (lawId: string, kind: string, name: string | undefined): string => `${lawId}::${kind}::${name}`;

/**
 * The laws to draw for a selection, as the POC did it: the selected laws plus
 * every law directly connected to one of them (what they read from and what
 * reads from them), so the picture is the connected neighbourhood, not one
 * chain.
 * @param laws   all corpus laws
 */
export function neighbourhood(selected: Set<string>, laws: GraphLaw[]): Set<string> {
  const shown = new Set(selected);
  const known = new Set(laws.map((l) => l.id));
  for (const law of laws) {
    const refs = lawShape(law).inputs.map((i) => i.ref.regulation).filter((id) => known.has(id));
    if (selected.has(law.id)) refs.forEach((id) => shown.add(id));
    else if (refs.some((id) => selected.has(id))) shown.add(law.id);
  }
  return shown;
}

/** Seven Rijkshuisstijl colours; a service keeps its colour by first appearance. */
export const COLOUR_COUNT = 7;
export function colourIndex(services: (string | null | undefined)[]): (service: string | null | undefined) => number {
  const order = [...new Set(services)];
  return (service) => order.indexOf(service) % COLOUR_COUNT;
}

/**
 * Nodes and edges for vue-flow. Every law is laid out, so nothing shifts when
 * the presenter shows more of them; a law outside `visible` is only hidden.
 * @param laws      corpus law entries (all of them)
 * @param values    { outputs: {lawId: {name: text}}, sources: {lawId: {name: text}}, inputs: {lawId: {name: text}} }
 * @param focus     law id whose edges stay bright
 * @param visible   law ids to show; null shows all
 */
export function buildGraph<L extends GraphLaw>(
  laws: L[],
  values: GraphValues = {},
  focus: string | null = null,
  visible: Set<string> | null = null,
): { nodes: GraphNode<L>[]; edges: GraphEdge[] } {
  const shown = (id: string) => !visible || visible.has(id);
  const byId = new Map(laws.map((l) => [l.id, l]));
  const shapes = new Map(laws.map((l) => [l.id, lawShape(l)]));
  // Every law of `laws` has a shape: the map is built from the same list.
  const shapeOf = (id: string) => shapes.get(id)!;
  const sized = new Map<string, LayoutEntry>();
  for (const law of laws) {
    const shape = shapeOf(law.id);
    sized.set(law.id, { size: lawSize(shape), deps: shape.inputs.map((i) => i.ref.regulation) });
  }
  const positions = layout(sized);

  const colour = colourIndex(laws.map((l) => l.service));
  const nodes: GraphNode<L>[] = [];
  const edges: GraphEdge[] = [];
  const dimmed = (lawId: string) => !!focus && lawId !== focus && !edges.some((e) => !e.hidden && ((e.data.from === focus && e.data.to === lawId) || (e.data.to === focus && e.data.from === lawId)));

  // Edges first: the dimming of a law depends on them.
  for (const law of laws) {
    for (const input of shapeOf(law.id).inputs) {
      const supplier = byId.get(input.ref.regulation);
      if (!supplier || !shapeOf(supplier.id).outputs.some((o) => o.name === input.ref.output)) continue;
      const bright = !focus || law.id === focus || supplier.id === focus;
      // With a law in focus: what it reads (outgoing) and who reads it (incoming) in their own colour, as the POC did.
      const direction = !focus ? '' : law.id === focus ? 'graph-edge--out' : supplier.id === focus ? 'graph-edge--in' : '';
      edges.push({
        hidden: !(shown(law.id) && shown(supplier.id)),
        class: direction,
        // De lijn loopt van de leverancier naar de afnemer, dezelfde kant op als
        // de layout die de leverancier links zet. Andersom (van de invoer terug
        // naar de uitvoer links ervan) maakte de bezier een lus dwars over de
        // tussenliggende wetkaders, en wees de pijl de verkeerde kant op.
        id: `${itemId(supplier.id, 'out', input.ref.output)}->${itemId(law.id, 'in', input.name)}`,
        source: itemId(supplier.id, 'out', input.ref.output),
        target: itemId(law.id, 'in', input.name),
        // `from`/`to` beschrijven de verwijzing (wie leest van wie), niet de
        // tekenrichting: `from` is de lezende wet, `to` de leverancier. Dat is
        // de kant op die `dimmed()` hierboven en de lijstjes "leest van" /
        // "gelezen door" in GraafView lezen, en het staat los van source/target.
        data: { from: law.id, to: supplier.id },
        markerEnd: MarkerType.ArrowClosed,
        animated: !!focus && bright,
        style: { opacity: bright ? 0.85 : 0.12 },
        // Onder de wetkaders, niet erboven: een lijn die over een kader loopt,
        // loopt over de naam van de wet en over de item-labels heen.
        zIndex: 0,
      });
    }
  }

  for (const law of laws) {
    const shape = shapeOf(law.id);
    // Filled for every law above.
    const { size } = sized.get(law.id)!;
    const dim = dimmed(law.id);
    const hidden = !shown(law.id);
    nodes.push({
      id: law.id,
      type: 'law',
      hidden,
      // `layout` places every law it was given.
      position: positions.get(law.id)!,
      style: { width: `${size.width}px`, height: `${size.height}px` },
      data: { law, selected: focus === law.id },
      class: `graph-c${colour(law.service)}${dim ? ' graph-dim' : ''}`,
      selectable: false,
    });
    const box = (kind: BoxKind, label: string, items: { name: string; ref?: InputRef }[], x: number, y: number) => {
      if (!items.length) return 0;
      const h = boxHeight(items.length);
      const boxId = `${law.id}::box::${kind}`;
      nodes.push({
        id: boxId,
        type: 'box',
        hidden,
        parentNode: law.id,
        position: { x, y },
        style: { width: `${COL_W}px`, height: `${h}px` },
        data: { label, kind },
        class: `graph-c${colour(law.service)}`,
        draggable: false,
        selectable: false,
      });
      items.forEach((item, i) => {
        nodes.push({
          id: itemId(law.id, kind === 'sources' ? 'src' : kind === 'inputs' ? 'in' : 'out', item.name),
          type: 'item',
          hidden,
          parentNode: boxId,
          position: { x: BOX_PAD, y: BOX_LABEL_H + BOX_PAD + i * (ITEM_H + ITEM_GAP) },
          style: { width: `${COL_W - BOX_PAD * 2}px`, height: `${ITEM_H}px` },
          data: { name: item.name, kind, value: values[kind]?.[law.id]?.[item.name], ref: item.ref ?? null },
          // De uitvoer van de leverancier vertrekt rechts, de invoer van de
          // afnemer komt links binnen: de lijn loopt mee met de layout.
          sourcePosition: Position.Right,
          targetPosition: Position.Left,
          draggable: false,
          selectable: false,
        });
      });
      return h;
    };
    let y = HEADER_H;
    const sourcesH = box('sources', 'Bronnen', shape.sources, LAW_PAD, y);
    if (sourcesH) y += sourcesH + LAW_PAD;
    box('inputs', 'Invoer', shape.inputs, LAW_PAD, y);
    box('outputs', 'Uitvoer', shape.outputs, LAW_PAD * 2 + COL_W, HEADER_H);
  }
  return { nodes, edges };
}
