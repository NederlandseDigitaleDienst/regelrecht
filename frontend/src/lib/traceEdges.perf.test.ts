/**
 * Microbenchmark for the trace-edge enrichment hot path.
 *
 * Constructs a synthetic graph (60 laws, ~480 nodes, ~395 edges) and a
 * trace of 500 steps mixing every node type the matcher handles, then
 * measures how long it takes to enrich each step with `edgeIds` and
 * `nodeIds`. This is the work `useTraceStepping.steps` does once per
 * (result, graph) change - the user-perceived hang after running a
 * scenario on a heavy law.
 *
 * Numbers are logged via `console.log`; the assert is a generous upper
 * bound so the test stays green on slow CI runners. The intent is the
 * log line - read the PR body for baseline → after numbers.
 */
import { describe, it, expect } from 'vitest';
import {
  flattenTraceSteps, edgeIdsForStep, graphNodeIdsForStep,
  buildEdgeIndex, buildNodeIdSet,
} from './traceEdges';
import type { TraceNode } from './traceEdges';

const LAW_COUNT = 60;
const STEP_COUNT = 500;

function buildSyntheticGraph() {
  const nodes = [];
  const edges = [];
  for (let i = 0; i < LAW_COUNT; i++) {
    const law = `wet_${i}`;
    nodes.push({ id: law });
    for (const out of ['out_a', 'out_b']) {
      nodes.push({ id: `${law}-output-${out}` });
    }
    for (const inp of ['in_a', 'in_b']) {
      nodes.push({ id: `${law}-input-${inp}` });
    }
    nodes.push({ id: `${law}-source-bsn` });
    nodes.push({ id: `${law}-delegate-norm_${i}` });
    nodes.push({ id: `${law}-impl-norm_${(i + 1) % LAW_COUNT}` });
  }
  // Cross-law edges: each law reads from 5 others
  for (let i = 0; i < LAW_COUNT; i++) {
    const law = `wet_${i}`;
    for (let k = 1; k <= 5; k++) {
      const target = `wet_${(i + k) % LAW_COUNT}`;
      edges.push({
        id: `${law}-input-in_a->${target}-output-out_a`,
        source: `${law}-input-in_a`,
        target: `${target}-output-out_a`,
      });
    }
  }
  // Implements
  for (let i = 0; i < LAW_COUNT; i++) {
    const implLaw = `wet_${i}`;
    const higher = `wet_${(i + 1) % LAW_COUNT}`;
    edges.push({
      id: `impl:${implLaw}:1->${higher}:norm_${i}`,
      source: `${implLaw}-impl-norm_${i}`,
      target: `${higher}-delegate-norm_${i}`,
    });
  }
  // Overrides
  for (let i = 0; i < LAW_COUNT; i += 3) {
    const a = `wet_${i}`;
    const b = `wet_${(i + 7) % LAW_COUNT}`;
    edges.push({
      id: `ovr:${a}:1->${b}:1`,
      source: `${a}-output-out_a`,
      target: `${b}-output-out_a`,
    });
  }
  // Hooks
  for (let i = 0; i < LAW_COUNT; i += 4) {
    const hookLaw = `wet_${i}`;
    const producer = `wet_${(i + 11) % LAW_COUNT}`;
    edges.push({
      id: `hook:${hookLaw}:1->${producer}:2`,
      source: `${hookLaw}-output-out_a`,
      target: `${producer}-output-out_a`,
    });
  }
  return { nodes, edges };
}

// Flat-ish trace tree - STEP_COUNT children under one root, every child
// rotates through the matcher's recognised node types so each step does
// non-trivial filter work.
function buildSyntheticTrace() {
  const types = [
    'cross_law_reference', 'open_term_resolution', 'hook_resolution',
    'override_resolution', 'article', 'action', 'requirement', 'resolve',
    'operation', 'cached',
  ];
  const root = {
    node_type: 'article',
    name: 'wet_0 (out_a)',
    children: [] as TraceNode[],
  };
  for (let i = 0; i < STEP_COUNT; i++) {
    const t = types[i % types.length]!; // index is in range by the modulo
    let name;
    if (t === 'cross_law_reference') name = `wet_${(i + 1) % LAW_COUNT}#out_a`;
    else if (t === 'hook_resolution') name = `wet_${i % LAW_COUNT}:1`;
    else if (t === 'open_term_resolution') name = `norm_${i % LAW_COUNT}`;
    else if (t === 'resolve') name = i % 2 === 0 ? 'in_a' : 'bsn';
    else name = i % 2 === 0 ? 'out_a' : `wet_${i % LAW_COUNT} (out_a)`;
    root.children.push({
      node_type: t,
      name,
      resolve_type: t === 'resolve' ? (i % 2 === 0 ? 'INPUT' : 'PARAMETER') : undefined,
      children: [],
    });
  }
  return root;
}

describe('trace edge enrichment perf', () => {
  it(`indexed enrichment beats unindexed on a ${LAW_COUNT}-law / ${STEP_COUNT}-step trace`, () => {
    const { nodes, edges } = buildSyntheticGraph();
    const trace = buildSyntheticTrace();
    const flat = flattenTraceSteps(trace, 'wet_0');

    // Called on the five timings collected below, so the middle element exists.
    const median = (xs: number[]) => { const s = [...xs].sort((a, b) => a - b); return s[Math.floor(s.length / 2)]!; };

    // --- Unindexed: per-step rebuilds the edge index / node id set. -----
    for (let r = 0; r < 3; r++) {
      for (const s of flat) {
        edgeIdsForStep(s, edges);
        graphNodeIdsForStep(s, nodes);
      }
    }
    const unindexed = [];
    for (let r = 0; r < 5; r++) {
      const t0 = performance.now();
      for (const s of flat) {
        s.edgeIds = edgeIdsForStep(s, edges);
        s.nodeIds = graphNodeIdsForStep(s, nodes);
      }
      unindexed.push(performance.now() - t0);
    }

    // --- Indexed: build edge index + node id set once, reuse per step. --
    const edgeIndex = buildEdgeIndex(edges);
    const nodeIdSet = buildNodeIdSet(nodes);
    for (let r = 0; r < 3; r++) {
      for (const s of flat) {
        edgeIdsForStep(s, edges, edgeIndex);
        graphNodeIdsForStep(s, nodes, nodeIdSet);
      }
    }
    const indexed = [];
    for (let r = 0; r < 5; r++) {
      const t0 = performance.now();
      const ei = buildEdgeIndex(edges);
      const ns = buildNodeIdSet(nodes);
      for (const s of flat) {
        s.edgeIds = edgeIdsForStep(s, edges, ei);
        s.nodeIds = graphNodeIdsForStep(s, nodes, ns);
      }
      indexed.push(performance.now() - t0);
    }

    const baseline = median(unindexed);
    const after = median(indexed);
    // eslint-disable-next-line no-console
    console.log(
      `[trace-perf] graph=${nodes.length}n/${edges.length}e steps=${flat.length} `
      + `unindexed=${baseline.toFixed(2)}ms indexed=${after.toFixed(2)}ms `
      + `speedup=${(baseline / after).toFixed(1)}×`,
    );

    expect(baseline).toBeLessThan(2000);
    expect(flat.length).toBeGreaterThanOrEqual(150);
    // Minimum bar: any measurable speedup. Slow CI runners are too noisy
    // for a tighter ratio assert - read the logged speedup line for the
    // real number (typically 70-90× on a developer laptop).
    expect(after).toBeLessThan(baseline);
  });
});
