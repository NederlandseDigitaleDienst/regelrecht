/**
 * useLawGraph - build a Vue Flow node/edge graph for a root law + its
 * transitive dependencies.
 *
 * Ported from demo/graph/src/routes/+page.svelte (branch
 * feature/demo-leenstelsel-tegemoetkoming). The YAML parsing, per-law
 * aggregation, layered layout and edge construction follow that demo
 * byte-for-byte - changes there and here need to stay in lockstep.
 *
 * Edge ID format MUST match what lib/traceEdges.js (PR2) looks up:
 *   - cross-law input:  "${lawId}-input-${name}->${targetLaw}-output-${name}"
 *   - implements:       "impl:${lawId}:${art}->${implLaw}:${openTerm}"
 *   - overrides:        "ovr:${lawId}:${art}->${ovrLaw}:${ovrArticle}"
 *   - hooks:            "hook:${hookLaw}:${art}->${producerLaw}:${producerArt}"
 *
 * INVARIANT: law $id slugs MUST NOT contain a hyphen. Node ids encode
 * `${lawId}-${nodeType}-${fieldName}` and `applyLayeredLayout` /
 * `LawGraphView.rootOfId` both split on the first hyphen to recover the
 * root law id. Current corpus slugs use underscores exclusively; if a
 * future source (e.g. CVDR imports) introduces a hyphen in a law id,
 * that scheme has to be rethought.
 */
import { ref, watch, type Ref } from 'vue';
import * as yaml from 'js-yaml';
import { decisionOf, type Produces } from '@regelrecht/frontend-shared/produces';
import { MarkerType, Position } from '@vue-flow/core';
import { extractRegulationRefs } from './useDependencies';
import { useLatest } from '../lib/useLatest';

/**
 * The fields of a parsed law YAML document (schema/latest/schema.json) the
 * graph reads. Read from YAML, so nothing is validated beyond these checks.
 */
interface LawSource {
  $id?: string;
  name?: unknown;
  regulatory_layer?: string;
  valid_from?: string;
  articles?: {
    number: unknown;
    text?: string;
    machine_readable?: {
      execution?: {
        input?: { name: string; source?: { regulation?: string; output?: string } | null }[];
        parameters?: { name: string }[];
        output?: { name: string }[];
        produces?: Produces;
      };
      implements?: { law: string; open_term: string }[];
      overrides?: { law: string; article: string; output: string }[];
      hooks?: { applies_to?: { legal_character?: string } }[];
      open_terms?: { id: string }[];
    } | null;
  }[];
}

/** An input as the graph keeps it. */
interface GraphInput {
  name: string;
  source_regulation?: string;
  source_output?: string;
}

/** One article reduced to what the graph draws. */
interface GraphArticle {
  number: string;
  parameters: string[];
  input: GraphInput[];
  output: string[];
  implements: { law: string; open_term: string }[];
  overrides: { law: string; article: string; output: string }[];
  hooks: string[];
  open_terms: string[];
  produces_legal_character: string | undefined;
  text: string;
}

/** One law reduced to what the graph draws. */
interface GraphLaw {
  id: string;
  name: unknown;
  layer: string;
  valid_from: string;
  articles: GraphArticle[];
}

/** A Vue Flow node as built here. */
export interface GraphNode {
  id: string;
  type: string;
  data: { label: string; tooltip?: string };
  position: { x: number; y: number };
  width: number;
  height: number;
  class?: string;
  parentNode?: string;
  extent?: 'parent';
  draggable?: boolean;
  selectable?: boolean;
  sourcePosition?: Position;
  targetPosition?: Position;
}

/** A Vue Flow edge as built here. */
export interface GraphEdge {
  id: string;
  source: string;
  target: string;
  type: string;
  data?: { refersToService: string };
  markerEnd: { type: MarkerType; width: number; height: number; color: string };
  style?: Record<string, string | number>;
  zIndex: number;
  label?: string;
  labelStyle?: Record<string, string | number>;
}

const LAYER_COLOR_INDEX: Record<string, number> = {
  WET: 0,
  AMVB: 1,
  MINISTERIELE_REGELING: 2,
  GEMEENTELIJKE_VERORDENING: 3,
  GRONDWET: 4,
  BELEIDSREGEL: 5,
  EU_VERORDENING: 6,
};

function layerColor(layer: string): number {
  return LAYER_COLOR_INDEX[layer] ?? 0;
}

// Outputs hidden in the graph: they duplicate law metadata and would
// clutter every node with the same three leaves.
type FetchLawYaml = (lawId: string) => Promise<string>;

const UTILITY_OUTPUTS = new Set(['wet_naam', 'bevoegd_gezag', 'datum_inwerkingtreding']);

/**
 * Extract the root law id from a node or edge id.
 *
 * Node ids encode `${lawId}-${nodeType}-${field}` and edges source/target
 * use the same scheme. The law id is everything up to the first hyphen
 * (see INVARIANT in the module header).
 */
export function rootOfId(nodeOrEdgeId: string): string {
  const i = nodeOrEdgeId.indexOf('-');
  return i === -1 ? nodeOrEdgeId : nodeOrEdgeId.substring(0, i);
}

function parseLawFromParsed(data: LawSource | null | undefined): GraphLaw | null {
  if (!data || !data.$id) return null;

  const articles: GraphArticle[] = [];
  for (const art of data.articles || []) {
    const mr = art.machine_readable;
    if (!mr) continue;
    const ex = mr.execution || {};

    const inputs: GraphInput[] = [];
    for (const inp of ex.input || []) {
      const li: GraphInput = { name: inp.name };
      const src = inp.source;
      if (src && typeof src === 'object') {
        if (src.regulation) {
          li.source_regulation = src.regulation;
          li.source_output = src.output || inp.name;
        } else if (src.output) {
          li.source_output = src.output;
        }
      }
      inputs.push(li);
    }

    articles.push({
      number: String(art.number),
      parameters: (ex.parameters || []).map((p) => p.name),
      input: inputs,
      output: (ex.output || []).map((o) => o.name),
      implements: (mr.implements || []).map((i) => ({ law: i.law, open_term: i.open_term })),
      overrides: (mr.overrides || []).map((o) => ({ law: o.law, article: o.article, output: o.output })),
      hooks: (mr.hooks || []).map((h) => h.applies_to?.legal_character || ''),
      open_terms: (mr.open_terms || []).map((ot) => ot.id),
      produces_legal_character: decisionOf(ex.produces)?.legal_character,
      text: art.text || '',
    });
  }

  const name = typeof data.name === 'string' && data.name.startsWith('#')
    ? data.$id.replace(/_/g, ' ')
    : data.name;

  return {
    id: data.$id,
    name,
    layer: data.regulatory_layer || 'WET',
    valid_from: data.valid_from || '',
    articles,
  };
}

/**
 * Walk from rootLawId, loading and parsing each transitively referenced law.
 * Returns { laws: Map<lawId, Law>, missing: string[] }. Missing laws (fetch
 * failures) are collected so the caller can surface a "kon X afhankelijkheden
 * niet laden" warning. Loads breadth-first.
 */
async function loadLawGraph(
  rootLawId: string,
  fetchLawYaml: FetchLawYaml,
): Promise<{ laws: Map<string, GraphLaw>; missing: string[] }> {
  const laws = new Map<string, GraphLaw>();
  const missing: string[] = [];
  const seen = new Set([rootLawId]);
  let frontier = [rootLawId];

  // Breadth-first with batched parallel fetches (mirrors useDependencies.js).
  const BATCH_SIZE = 10;

  while (frontier.length > 0) {
    const nextFrontier: string[] = [];

    for (let i = 0; i < frontier.length; i += BATCH_SIZE) {
      const batch = frontier.slice(i, i + BATCH_SIZE);
      const results = await Promise.allSettled(
        batch.map(async (lawId) => {
          const yamlText = await fetchLawYaml(lawId);
          // A law body is a mapping; parseLawFromParsed skips anything without `$id`.
          return { lawId, parsed: yaml.load(yamlText) as LawSource | null };
        }),
      );

      for (let j = 0; j < results.length; j++) {
        // j is within results, which has one entry per batch item.
        const result = results[j]!;
        if (result.status !== 'fulfilled') {
          missing.push(batch[j]!);
          continue;
        }
        const parsed = result.value.parsed;
        const law = parseLawFromParsed(parsed);
        if (!law) continue;
        laws.set(law.id, law);

        // parseLawFromParsed returned a law, so `parsed` is a non-null mapping.
        for (const depId of extractRegulationRefs(parsed!)) {
          if (!seen.has(depId)) {
            seen.add(depId);
            nextFrontier.push(depId);
          }
        }
        // implements / overrides are also structural deps
        for (const art of law.articles) {
          for (const impl of art.implements) {
            if (!seen.has(impl.law)) {
              seen.add(impl.law);
              nextFrontier.push(impl.law);
            }
          }
          for (const ovr of art.overrides) {
            if (!seen.has(ovr.law)) {
              seen.add(ovr.law);
              nextFrontier.push(ovr.law);
            }
          }
        }
      }
    }

    frontier = nextFrontier;
  }

  return { laws, missing };
}

/**
 * Build Vue Flow nodes + edges from the loaded laws Map. Mirrors the
 * two-pass construction in demo/+page.svelte: first every law becomes a
 * root "law" node with nested "property-group" sub-nodes (params/inputs/
 * outputs/delegates/implements), then edges fan out across them.
 */
function buildGraph(lawsMap: Map<string, GraphLaw>): { nodes: GraphNode[]; edges: GraphEdge[] } {
  const laws = [...lawsMap.values()];
  const nodes: GraphNode[] = [];
  const edges: GraphEdge[] = [];

  // "law_id:output_name" → law_id[] for cross-law edge resolution.
  const serviceOutputToIDs = new Map<string, string[]>();
  for (const law of laws) {
    for (const art of law.articles) {
      for (const out of art.output) {
        const key = `${law.id}:${out}`;
        const cur = serviceOutputToIDs.get(key) || [];
        cur.push(law.id);
        serviceOutputToIDs.set(key, cur);
      }
    }
  }

  let rootIdx = 0;
  for (const law of laws) {
    if (law.articles.length === 0) continue;

    const colorIndex = layerColor(law.layer);

    const allParams = new Set<string>();
    const allInputs: { name: string; ref?: { regulation: string; output: string } }[] = [];
    const allOutputs = new Set<string>();
    const allDelegates = new Set<string>();
    const allImplements = new Set<string>();
    const inputsSeen = new Set<string>();
    const fieldProvenance = new Map<string, { artNumber: string; text: string }>();

    // Delegates that other laws implement FOR this law. Hoisted out of the
    // per-article loop because the scan is law-level, not article-level.
    for (const otherLaw of laws) {
      for (const otherArt of otherLaw.articles) {
        for (const impl of otherArt.implements) {
          if (impl.law === law.id) allDelegates.add(impl.open_term);
        }
      }
    }

    for (const art of law.articles) {
      for (const p of art.parameters) {
        allParams.add(p);
        fieldProvenance.set(p, { artNumber: art.number, text: art.text || '' });
      }
      for (const out of art.output) {
        allOutputs.add(out);
        fieldProvenance.set(out, { artNumber: art.number, text: art.text || '' });
      }
      for (const ot of art.open_terms) {
        allDelegates.add(ot);
        fieldProvenance.set(ot, { artNumber: art.number, text: art.text || '' });
      }
      for (const impl of art.implements) {
        allImplements.add(impl.open_term);
        fieldProvenance.set(`impl:${impl.open_term}`, { artNumber: art.number, text: art.text || '' });
      }
      for (const inp of art.input) {
        if (inputsSeen.has(inp.name)) continue;
        inputsSeen.add(inp.name);
        fieldProvenance.set(`input:${inp.name}`, { artNumber: art.number, text: art.text || '' });
        if (inp.source_regulation && inp.source_regulation !== law.id) {
          allInputs.push({
            name: inp.name,
            ref: { regulation: inp.source_regulation, output: inp.source_output || inp.name },
          });
        } else {
          allInputs.push({ name: inp.name });
        }
      }
    }

    const filteredOutputs = [...allOutputs].filter(
      (o) => !UTILITY_OUTPUTS.has(o) && !allDelegates.has(o),
    );
    const filteredParams = [...allParams];
    const filteredDelegates = [...allDelegates];
    const filteredImplements = [...allImplements];

    const leftCount = filteredParams.length + allInputs.length + filteredImplements.length;
    const rightCount = filteredOutputs.length + filteredDelegates.length;
    const leftHeight = leftCount * 50
      + (filteredParams.length > 0 ? 70 : 0)
      + (allInputs.length > 0 ? 70 : 0)
      + (filteredImplements.length > 0 ? 70 : 0);
    const rightHeight = rightCount * 50
      + (filteredOutputs.length > 0 ? 70 : 0)
      + (filteredDelegates.length > 0 ? 70 : 0);

    nodes.push({
      id: law.id,
      type: 'law',
      data: { label: `${law.layer} - ${law.name}` },
      position: { x: rootIdx++ * 400, y: 0 },
      width: 500,
      height: Math.max(leftHeight, rightHeight) + 120,
      class: `root service-${colorIndex}`,
      selectable: false,
    });

    // Parameters column (left-top)
    if (filteredParams.length > 0) {
      const sourcesID = `${law.id}-sources`;
      nodes.push({
        id: sourcesID,
        type: 'default',
        data: { label: 'Parameters' },
        position: { x: 10, y: 60 },
        width: 220,
        height: filteredParams.length * 50 + 50,
        parentNode: law.id,
        class: `property-group service-${colorIndex}`,
        draggable: false,
        selectable: false,
      });
      let j = 0;
      for (const param of filteredParams) {
        const prov = fieldProvenance.get(param);
        nodes.push({
          id: `${law.id}-source-${param}`,
          type: 'leaf',
          sourcePosition: Position.Left,
          data: { label: param, tooltip: prov ? `Art. ${prov.artNumber}\n\n${prov.text}` : '' },
          position: { x: 10, y: (j++ + 1) * 50 },
          width: 200,
          height: 40,
          parentNode: sourcesID,
          extent: 'parent',
          draggable: false,
          selectable: false,
        });
      }
    }

    // Input column (left, below Parameters)
    const inputYOffset = filteredParams.length > 0 ? filteredParams.length * 50 + 130 : 60;
    if (allInputs.length > 0) {
      const inputsID = `${law.id}-input`;
      nodes.push({
        id: inputsID,
        type: 'default',
        data: { label: 'Input' },
        position: { x: 10, y: inputYOffset },
        width: 220,
        height: allInputs.length * 50 + 50,
        parentNode: law.id,
        class: `property-group service-${colorIndex}`,
        draggable: false,
        selectable: false,
      });
      let j = 0;
      for (const input of allInputs) {
        const iProv = fieldProvenance.get(`input:${input.name}`);
        nodes.push({
          id: `${law.id}-input-${input.name}`,
          type: 'leaf',
          sourcePosition: Position.Left,
          data: { label: input.name, tooltip: iProv ? `Art. ${iProv.artNumber}\n\n${iProv.text}` : '' },
          position: { x: 10, y: (j++ + 1) * 50 },
          width: 200,
          height: 40,
          parentNode: inputsID,
          extent: 'parent',
          draggable: false,
          selectable: false,
        });
      }
    }

    // Output column (right-top)
    if (filteredOutputs.length > 0) {
      const outputsID = `${law.id}-output`;
      nodes.push({
        id: outputsID,
        type: 'default',
        data: { label: 'Output' },
        position: { x: 250, y: 60 },
        width: 220,
        height: filteredOutputs.length * 50 + 50,
        parentNode: law.id,
        class: `property-group service-${colorIndex}`,
        draggable: false,
        selectable: false,
      });
      let j = 0;
      for (const output of filteredOutputs) {
        const oProv = fieldProvenance.get(output);
        nodes.push({
          id: `${law.id}-output-${output}`,
          type: 'leaf',
          sourcePosition: Position.Right,
          targetPosition: Position.Right,
          data: { label: output, tooltip: oProv ? `Art. ${oProv.artNumber}\n\n${oProv.text}` : '' },
          position: { x: 10, y: (j++ + 1) * 50 },
          width: 200,
          height: 40,
          parentNode: outputsID,
          extent: 'parent',
          draggable: false,
          selectable: false,
        });
      }
    }

    // Delegates column (right, below Output)
    const delegatesYOffset = filteredOutputs.length > 0
      ? 60 + filteredOutputs.length * 50 + 70
      : 60;
    if (filteredDelegates.length > 0) {
      const delegatesID = `${law.id}-delegates`;
      nodes.push({
        id: delegatesID,
        type: 'default',
        data: { label: 'Delegeert' },
        position: { x: 250, y: delegatesYOffset },
        width: 220,
        height: filteredDelegates.length * 50 + 50,
        parentNode: law.id,
        class: `property-group service-${colorIndex}`,
        draggable: false,
        selectable: false,
      });
      let j = 0;
      for (const del of filteredDelegates) {
        const dProv = fieldProvenance.get(del);
        nodes.push({
          id: `${law.id}-delegate-${del}`,
          type: 'leaf',
          sourcePosition: Position.Right,
          targetPosition: Position.Right,
          data: { label: del, tooltip: dProv ? `Art. ${dProv.artNumber}\n\n${dProv.text}` : '' },
          position: { x: 10, y: (j++ + 1) * 50 },
          width: 200,
          height: 40,
          parentNode: delegatesID,
          extent: 'parent',
          draggable: false,
          selectable: false,
        });
      }
    }

    // Implements column (left, below Input)
    const implementsYOffset = filteredParams.length > 0 || allInputs.length > 0
      ? (filteredParams.length > 0 ? filteredParams.length * 50 + 130 : 60)
        + (allInputs.length > 0 ? allInputs.length * 50 + 70 : 0)
      : 60;
    if (filteredImplements.length > 0) {
      const implementsID = `${law.id}-implements`;
      nodes.push({
        id: implementsID,
        type: 'default',
        data: { label: 'Implementeert' },
        position: { x: 10, y: implementsYOffset },
        width: 220,
        height: filteredImplements.length * 50 + 50,
        parentNode: law.id,
        class: `property-group service-${colorIndex}`,
        draggable: false,
        selectable: false,
      });
      let j = 0;
      for (const impl of filteredImplements) {
        const imProv = fieldProvenance.get(`impl:${impl}`);
        nodes.push({
          id: `${law.id}-impl-${impl}`,
          type: 'leaf',
          sourcePosition: Position.Left,
          data: { label: impl, tooltip: imProv ? `Art. ${imProv.artNumber}\n\n${imProv.text}` : '' },
          position: { x: 10, y: (j++ + 1) * 50 },
          width: 200,
          height: 40,
          parentNode: implementsID,
          extent: 'parent',
          draggable: false,
          selectable: false,
        });
      }
    }
  }

  const nodeIds = new Set(nodes.map((n) => n.id));
  const seenEdgeIds = new Set<string>();

  // Cross-law source references
  for (const law of laws) {
    for (const art of law.articles) {
      for (const input of art.input) {
        if (!input.source_regulation || input.source_regulation === law.id) continue;

        const inputID = `${law.id}-input-${input.name}`;
        const key = `${input.source_regulation}:${input.source_output || input.name}`;

        for (const targetLawId of serviceOutputToIDs.get(key) || []) {
          const target = `${targetLawId}-output-${input.source_output || input.name}`;
          if (!nodeIds.has(inputID) || !nodeIds.has(target)) continue;
          const edgeId = `${inputID}->${target}`;
          if (seenEdgeIds.has(edgeId)) continue;
          seenEdgeIds.add(edgeId);
          edges.push({
            id: edgeId,
            source: inputID,
            target,
            data: { refersToService: input.source_regulation },
            type: 'bezier',
            markerEnd: { type: MarkerType.ArrowClosed, width: 20, height: 40, color: '#3b82f6' },
            zIndex: 2,
          });
        }
      }

      // Implements
      for (const impl of art.implements) {
        const sourceId = `${law.id}-impl-${impl.open_term}`;
        const targetId = `${impl.law}-delegate-${impl.open_term}`;
        if (!nodeIds.has(sourceId) || !nodeIds.has(targetId)) continue;
        const edgeId = `impl:${law.id}:${art.number}->${impl.law}:${impl.open_term}`;
        if (seenEdgeIds.has(edgeId)) continue;
        seenEdgeIds.add(edgeId);
        edges.push({
          id: edgeId,
          source: sourceId,
          target: targetId,
          type: 'bezier',
          markerEnd: { type: MarkerType.ArrowClosed, width: 20, height: 40, color: '#10b981' },
          style: { stroke: '#10b981', strokeWidth: 3, strokeDasharray: '8 4' },
          zIndex: 2,
          label: 'implements',
          labelStyle: { fill: '#065f46', fontWeight: 600 },
        });
      }

      // Overrides
      for (const ovr of art.overrides) {
        if (!lawsMap.has(ovr.law)) continue;
        const sourceId = `${law.id}-output-${ovr.output}`;
        const targetId = `${ovr.law}-output-${ovr.output}`;
        if (!nodeIds.has(sourceId) || !nodeIds.has(targetId)) continue;
        const edgeId = `ovr:${law.id}:${art.number}->${ovr.law}:${ovr.article}`;
        if (seenEdgeIds.has(edgeId)) continue;
        seenEdgeIds.add(edgeId);
        edges.push({
          id: edgeId,
          source: sourceId,
          target: targetId,
          type: 'bezier',
          markerEnd: { type: MarkerType.ArrowClosed, width: 20, height: 40, color: '#ef4444' },
          style: { stroke: '#ef4444', strokeWidth: 3, strokeDasharray: '4 4' },
          zIndex: 2,
          label: 'overrides',
          labelStyle: { fill: '#991b1b', fontWeight: 600 },
        });
      }
    }
  }

  // Hook edges: hook laws → laws producing a matching legal_character
  const producers: { lawId: string; artNumber: string; legalCharacter: string }[] = [];
  for (const law of laws) {
    for (const art of law.articles) {
      if (art.produces_legal_character) {
        producers.push({ lawId: law.id, artNumber: art.number, legalCharacter: art.produces_legal_character });
      }
    }
  }

  for (const law of laws) {
    for (const art of law.articles) {
      for (const hookTarget of art.hooks) {
        if (!hookTarget) continue;
        const hookOutputs = art.output.filter((o) => !UTILITY_OUTPUTS.has(o));
        if (hookOutputs.length === 0) continue;
        const sourceId = `${law.id}-output-${hookOutputs[0]}`;
        if (!nodeIds.has(sourceId)) continue;

        for (const producer of producers) {
          if (producer.legalCharacter !== hookTarget || producer.lawId === law.id) continue;
          const producerLaw = lawsMap.get(producer.lawId);
          if (!producerLaw) continue;
          const producerArt = producerLaw.articles.find((a) => a.number === producer.artNumber);
          if (!producerArt) continue;
          const targetOutputs = producerArt.output.filter((o) => !UTILITY_OUTPUTS.has(o));
          if (targetOutputs.length === 0) continue;
          const targetId = `${producer.lawId}-output-${targetOutputs[0]}`;
          if (!nodeIds.has(targetId)) continue;

          const edgeId = `hook:${law.id}:${art.number}->${producer.lawId}:${producer.artNumber}`;
          if (seenEdgeIds.has(edgeId)) continue;
          seenEdgeIds.add(edgeId);
          edges.push({
            id: edgeId,
            source: sourceId,
            target: targetId,
            type: 'bezier',
            markerEnd: { type: MarkerType.ArrowClosed, width: 20, height: 40, color: '#7c3aed' },
            style: { stroke: '#7c3aed', strokeWidth: 3, strokeDasharray: '3 6' },
            zIndex: 1,
            label: `hook: ${hookTarget}`,
            labelStyle: { fill: '#5b21b6', fontWeight: 600 },
          });
        }
      }
    }
  }

  return { nodes, edges };
}

/**
 * Topological layered layout. Laws become columns; each column stacks up
 * to 4 laws vertically before wrapping to a sub-column. Returns a new
 * `nodes` array with updated root positions.
 */
function applyLayeredLayout(nodes: GraphNode[], edges: GraphEdge[]): GraphNode[] {
  const rootNodes = nodes.filter((n) => n.class?.includes('root'));
  // dependents: rootId → Set of root ids that depend on it (forward edges in the topo sort)
  // dependencies: rootId → Set of root ids it depends on (used to check "all deps processed")
  const dependents = new Map<string, Set<string>>();
  const dependencies = new Map<string, Set<string>>();
  const incomingCount = new Map<string, number>();

  for (const node of rootNodes) {
    dependents.set(node.id, new Set());
    dependencies.set(node.id, new Set());
    incomingCount.set(node.id, 0);
  }

  for (const edge of edges) {
    const sourceRoot = rootOfId(edge.source);
    const targetRoot = rootOfId(edge.target);
    if (sourceRoot === targetRoot) continue;
    if (!dependents.has(sourceRoot)) {
      dependents.set(sourceRoot, new Set());
      dependencies.set(sourceRoot, new Set());
      incomingCount.set(sourceRoot, 0);
    }
    if (!dependents.has(targetRoot)) {
      dependents.set(targetRoot, new Set());
      dependencies.set(targetRoot, new Set());
      incomingCount.set(targetRoot, 0);
    }
    // Both roots were registered just above.
    if (!dependents.get(targetRoot)!.has(sourceRoot)) {
      dependents.get(targetRoot)!.add(sourceRoot);
      dependencies.get(sourceRoot)!.add(targetRoot);
      incomingCount.set(sourceRoot, (incomingCount.get(sourceRoot) || 0) + 1);
    }
  }

  const layers: string[][] = [];
  const processed = new Set<string>();
  let currentLayer = rootNodes
    .map((n) => n.id)
    .filter((id) => (incomingCount.get(id) || 0) === 0);

  while (currentLayer.length > 0) {
    layers.push(currentLayer);
    for (const nodeId of currentLayer) processed.add(nodeId);
    const next = new Set<string>();
    for (const nodeId of currentLayer) {
      for (const dependent of dependents.get(nodeId) || new Set()) {
        if (processed.has(dependent)) continue;
        // All of `dependent`'s own dependencies must already be processed.
        // Using the precomputed inverse map makes this O(deps) per candidate
        // instead of O(edges).
        let allDepsDone = true;
        for (const dep of dependencies.get(dependent) || new Set()) {
          if (!processed.has(dep)) {
            allDepsDone = false;
            break;
          }
        }
        if (allDepsDone) next.add(dependent);
      }
    }
    currentLayer = [...next];
  }
  const unprocessed = rootNodes.map((n) => n.id).filter((id) => !processed.has(id));
  if (unprocessed.length > 0) layers.push(unprocessed);

  const nodeSpacing = 580;
  const layerSpacing = 100;
  const maxPerColumn = 4;
  const maxColumnPerLayer = new Map<number, number>();
  const indexById = new Map(nodes.map((n, i) => [n.id, i]));

  const out = nodes.slice();

  for (let l = 0; l < layers.length; l++) {
    // l is within the layers bound.
    const layer = layers[l]!;
    let column = 0;
    let y = 0;
    let inColumn = 0;

    for (const nodeId of layer) {
      const idx = indexById.get(nodeId);
      if (idx === undefined) continue;
      if (inColumn >= maxPerColumn) {
        column++;
        y = 0;
        inColumn = 0;
      }
      let xOffset = 0;
      for (let prev = 0; prev < l; prev++) {
        xOffset += ((maxColumnPerLayer.get(prev) || 0) + 1) * nodeSpacing;
      }
      // idx comes from indexById, built from the same array `out` copies.
      out[idx] = { ...out[idx]!, position: { x: xOffset + column * nodeSpacing, y } };
      y += (out[idx]!.height || 0) + layerSpacing;
      inColumn++;
    }
    maxColumnPerLayer.set(l, column);
  }

  return out;
}

/**
 * Composable entry point.
 *
 * @param {object} opts
 * @param {import('vue').Ref<string|null>} opts.rootLawId
 * @param {(lawId: string) => Promise<string>} opts.fetchLawYaml
 */
export function useLawGraph({
  rootLawId,
  fetchLawYaml,
}: {
  rootLawId: Readonly<Ref<string | null | undefined>>;
  fetchLawYaml: FetchLawYaml;
}) {
  const nodes = ref<GraphNode[]>([]);
  const edges = ref<GraphEdge[]>([]);
  const loading = ref(false);
  const error = ref<string | null>(null);
  // Law ids whose YAML could not be fetched during the BFS walk. The graph
  // still renders the laws that did load, but the UI should warn the user
  // that some dependencies are missing - otherwise a partial graph looks
  // complete and silently omits connections.
  const missingDeps = ref<string[]>([]);

  const claimRebuild = useLatest();

  async function rebuild(lawId: string | null | undefined): Promise<void> {
    if (!lawId) {
      // Claim (and discard) a generation so any in-flight fetch cancels
      // its own finally branch, and clear loading so the composable's
      // public contract doesn't stay stuck at true after a lawId → null
      // transition.
      claimRebuild();
      nodes.value = [];
      edges.value = [];
      loading.value = false;
      error.value = null;
      missingDeps.value = [];
      return;
    }
    const isCurrent = claimRebuild();
    loading.value = true;
    error.value = null;
    missingDeps.value = [];
    try {
      const { laws, missing } = await loadLawGraph(lawId, fetchLawYaml);
      if (!isCurrent()) return; // superseded
      const { nodes: ns, edges: es } = buildGraph(laws);
      const laidOut = applyLayeredLayout(ns, es);
      nodes.value = laidOut;
      edges.value = es;
      missingDeps.value = missing;
    } catch (e) {
      if (!isCurrent()) return;
      error.value = (e as Error).message || String(e);
      nodes.value = [];
      edges.value = [];
    } finally {
      if (isCurrent()) loading.value = false;
    }
  }

  watch(rootLawId, (id) => { rebuild(id); }, { immediate: true });

  return { nodes, edges, loading, error, missingDeps, rebuild };
}
