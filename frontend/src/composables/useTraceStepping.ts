/**
 * useTraceStepping - step through a WASM `TraceResult` against the current
 * law graph and expose active/visited node & edge sets for CSS
 * highlighting.
 *
 * Ported from demo/graph/src/routes/+page.svelte:984-1200 on branch
 * feature/demo-leenstelsel-tegemoetkoming.
 *
 * Cost model:
 *   - `steps`            - O(N) per (result, nodes, edges) change. Cached.
 *   - `next`/`prev`/`goto` - O(1) on the pointer itself.
 *   - `activeEdgeIds` / `activeNodeIds` - O(1) per `currentStepIdx` change.
 *   - `visitedEdgeIds` / `visitedNodeIds` - O(currentStepIdx) per
 *     `currentStepIdx` change. Forward traversal of an N-step trace is
 *     therefore O(N²). Acceptable today (typical traces are <100 steps).
 *     PR3 polish: amortise via mutated refs in next/prev/goto.
 */
import { computed, ref, watch, type Ref } from 'vue';
import {
  flattenTraceSteps,
  edgeIdsForStep,
  graphNodeIdsForStep,
  buildEdgeIndex,
  buildNodeIdSet,
  type TraceNode,
  type TraceStep,
  type TraceGraphEdge,
  type TraceGraphNode,
} from '../lib/traceEdges';

/**
 * The part of the engine's `TraceResult` read here: `trace` is either the
 * RFC-039 envelope `{trace_version, root}` or, for older recordings, a bare
 * root step.
 */
interface TraceResultLike {
  trace?: { root?: TraceNode | null } | TraceNode | null;
}

export interface TraceSteppingOptions {
  result: Readonly<Ref<TraceResultLike | null | undefined>>;
  nodes: Readonly<Ref<readonly TraceGraphNode[] | null | undefined>>;
  edges: Readonly<Ref<readonly TraceGraphEdge[] | null | undefined>>;
  rootLawId: Readonly<Ref<string | null | undefined>>;
  outputName: Readonly<Ref<string | null | undefined>>;
}

/**
 * @param {object} opts
 * @param {import('vue').Ref<object|null>} opts.result  - TraceResult from engine
 * @param {import('vue').Ref<Array>} opts.nodes         - Vue Flow nodes
 * @param {import('vue').Ref<Array>} opts.edges         - Vue Flow edges
 * @param {import('vue').Ref<string|null>} opts.rootLawId
 * @param {import('vue').Ref<string|null>} opts.outputName
 */
export function useTraceStepping({ result, nodes, edges, rootLawId, outputName }: TraceSteppingOptions) {
  const currentStepIdx = ref(-1);

  // Raw trace → enriched steps (with edgeIds / nodeIds resolved against the
  // current graph). Recomputes when the trace, graph, or root law changes.
  //
  // Build the edge index + node id set ONCE per recompute and pass them
  // through to every per-step matcher. Without this, each call would
  // rebuild a Set of all node ids and rescan the full edge array, which
  // dominated profiling on heavy laws (O(steps × (edges + nodes))).
  const steps = computed((): TraceStep[] => {
    // The engine hands back a trace document, `{trace_version, root}`
    // (RFC-039); the stepper walks steps. A bare step still works, so a trace
    // recorded before the envelope renders unchanged.
    const doc = result.value?.trace;
    // A bare step has no `root`, so this falls back to the step itself.
    const trace = ((doc as { root?: TraceNode | null } | null | undefined)?.root ?? doc) as
      | TraceNode
      | null
      | undefined;
    if (!trace || !rootLawId.value) return [];
    const flat = flattenTraceSteps(trace, rootLawId.value);
    const ns = nodes.value || [];
    const es = edges.value || [];
    const edgeIndex = buildEdgeIndex(es);
    const nodeIdSet = buildNodeIdSet(ns);
    return flat.map((s) => ({
      ...s,
      edgeIds: edgeIdsForStep(s, es, edgeIndex),
      nodeIds: graphNodeIdsForStep(s, ns, nodeIdSet),
    }));
  });

  // Two watchers cooperate on the step pointer:
  // - The first listens to `result`: when a fresh trace arrives we want
  //   step 0 selected immediately, even before the graph has been
  //   enriched. The pointer may briefly point at a step that does not
  //   exist yet - that's fine, the second watcher clamps it once
  //   `steps` updates.
  // - The second listens to `steps`: when enrichment finishes, or the
  //   graph rebuilds and `steps` shrinks, the pointer is clamped to a
  //   valid index (or -1 when there's nothing to step through).
  watch(
    () => result.value,
    (r) => {
      currentStepIdx.value = r?.trace ? 0 : -1;
    },
  );

  watch(steps, (list) => {
    if (list.length === 0) {
      currentStepIdx.value = -1;
    } else if (currentStepIdx.value >= list.length) {
      currentStepIdx.value = list.length - 1;
    } else if (currentStepIdx.value < 0) {
      currentStepIdx.value = 0;
    }
  });

  function next() {
    if (currentStepIdx.value < steps.value.length - 1) currentStepIdx.value += 1;
  }

  function prev() {
    if (currentStepIdx.value > 0) currentStepIdx.value -= 1;
  }

  function goto(idx: number): void {
    if (idx < 0 || idx >= steps.value.length) return;
    currentStepIdx.value = idx;
  }

  // Start point: the scenario's root law + its initial output leaf. Sticky
  // across all steps so the user can always see where the trace began.
  const startNodeIds = computed(() => {
    const ids = new Set<string>();
    const lawId = rootLawId.value;
    const out = outputName.value;
    if (!lawId) return ids;
    const nodeIdSet = new Set((nodes.value || []).map((n) => n.id));
    if (nodeIdSet.has(lawId)) ids.add(lawId);
    if (out) {
      const outId = `${lawId}-output-${out}`;
      if (nodeIdSet.has(outId)) ids.add(outId);
    }
    return ids;
  });

  const activeEdgeIds = computed(() => {
    const idx = currentStepIdx.value;
    if (idx < 0 || idx >= steps.value.length) return new Set<string>();
    // idx is within bounds (checked above).
    return new Set(steps.value[idx]!.edgeIds);
  });

  const activeNodeIds = computed(() => {
    const idx = currentStepIdx.value;
    if (idx < 0 || idx >= steps.value.length) return new Set<string>();
    // idx is within bounds (checked above).
    return new Set(steps.value[idx]!.nodeIds);
  });

  const visitedEdgeIds = computed(() => {
    const idx = currentStepIdx.value;
    const out = new Set<string>();
    // The watchers above keep currentStepIdx within `steps`.
    for (let i = 0; i < idx; i++) {
      for (const id of steps.value[i]!.edgeIds) out.add(id);
    }
    return out;
  });

  const visitedNodeIds = computed(() => {
    const idx = currentStepIdx.value;
    const out = new Set<string>();
    // The watchers above keep currentStepIdx within `steps`.
    for (let i = 0; i < idx; i++) {
      for (const id of steps.value[i]!.nodeIds) out.add(id);
    }
    return out;
  });

  return {
    steps,
    currentStepIdx,
    next,
    prev,
    goto,
    startNodeIds,
    activeEdgeIds,
    activeNodeIds,
    visitedEdgeIds,
    visitedNodeIds,
  };
}
