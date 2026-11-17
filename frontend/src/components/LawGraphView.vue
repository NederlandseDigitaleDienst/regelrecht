<script setup lang="ts">
import { computed, ref, toRef, markRaw, watch } from 'vue';
import { VueFlow } from '@vue-flow/core';
import type { GraphNode as FlowGraphNode, NodeMouseEvent, NodeTypesObject } from '@vue-flow/core';
import { Background } from '@vue-flow/background';
import { Controls } from '@vue-flow/controls';
import { MiniMap } from '@vue-flow/minimap';
import '@vue-flow/core/dist/style.css';
import '@vue-flow/core/dist/theme-default.css';
import '@vue-flow/controls/dist/style.css';
import '@vue-flow/minimap/dist/style.css';
import './graph/graph-styles.css';

import LawNode from './graph/LawNode.vue';
import LeafNode from './graph/LeafNode.vue';
import TraceStepList from './graph/TraceStepList.vue';
import TraceStepDetail from './graph/TraceStepDetail.vue';
import { useLawGraph, rootOfId } from '../composables/useLawGraph';
import { useTraceStepping } from '../composables/useTraceStepping';
import type { TraceSteppingOptions } from '../composables/useTraceStepping';
import { useColorScheme } from '../composables/useColorScheme';
import { stepHasHighlights } from '../lib/traceEdges';
import type { TraceStep } from '../lib/traceEdges';
import { useCorpus } from '../corpus';
import type { RequestError } from '../lib/apiFetch';

/** The engine's TraceResult: the trace read by useTraceStepping, plus the outputs. */
type GraphTraceResult = NonNullable<TraceSteppingOptions['result']['value']> & {
  outputs?: Record<string, unknown> | null;
};

const props = withDefaults(
  defineProps<{
    lawId?: string | null;
    // Latest scenario execution result (TraceResult). When set, the graph
    // enters trace-stepping mode: step list + detail panel appear below
    // the graph and nodes/edges get .trace-active/.trace-visited classes.
    result?: GraphTraceResult | null;
    // The scenario's entry output name - pins the "▶ start" marker to the
    // matching output leaf on the root law.
    outputName?: string | null;
    expectations?: Record<string, unknown>;
  }>(),
  {
    lawId: null,
    result: null,
    outputName: null,
    expectations: () => ({}),
  },
);

// Dependency reads go through the active traject, so the graph mirrors what
// the editor pane shows.
const corpus = useCorpus();
async function fetchLawYaml(lawId: string) {
  try {
    return (await corpus.law(lawId)).rawYaml;
  } catch (e) {
    const status = (e as RequestError).status;
    // An HTTP failure gets the message the graph shows; a network error keeps its own.
    if (status === undefined) throw e;
    throw new Error(`Law '${lawId}' niet gevonden (${status})`);
  }
}

// Vue Flow types a node component against its full NodeProps; ours declare
// only the props they read (useLawGraph gives every node a `data.label`).
const nodeTypes = markRaw({
  law: LawNode,
  leaf: LeafNode,
} as NodeTypesObject);

const { nodes, edges, loading, error, missingDeps } = useLawGraph({
  rootLawId: toRef(props, 'lawId'),
  fetchLawYaml,
});

// --- Trace stepping state -----------------------------------------------

const {
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
} = useTraceStepping({
  result: toRef(props, 'result'),
  nodes,
  edges,
  rootLawId: toRef(props, 'lawId'),
  outputName: toRef(props, 'outputName'),
});

const hasTrace = computed(() => steps.value.length > 0);
const filter = ref('highlights'); // 'highlights' | 'all'

function onFilterChange(e: Event) {
  const next =
    (e as CustomEvent<{ value?: string } | null | undefined>).detail?.value ??
    (e.target as { value?: string } | null)?.value;
  if (next === 'highlights' || next === 'all') filter.value = next;
}

function stepIsVisible(step: TraceStep) {
  if (filter.value === 'all') return true;
  return stepHasHighlights(step);
}

// Filter-aware nav: when the user has the "Met highlights" filter on,
// Vorige/Volgende must skip context-only steps that aren't rendered in
// the list - otherwise the counter advances onto a row that the user
// can't see and the step list looks frozen. (The `!`s below index inside
// the loop bounds.)
function nextVisible() {
  for (let i = currentStepIdx.value + 1; i < steps.value.length; i++) {
    if (stepIsVisible(steps.value[i]!)) {
      goto(i);
      return;
    }
  }
}
function prevVisible() {
  for (let i = currentStepIdx.value - 1; i >= 0; i--) {
    if (stepIsVisible(steps.value[i]!)) {
      goto(i);
      return;
    }
  }
}
const hasNextVisible = computed(() => {
  for (let i = currentStepIdx.value + 1; i < steps.value.length; i++) {
    if (stepIsVisible(steps.value[i]!)) return true;
  }
  return false;
});
const hasPrevVisible = computed(() => {
  for (let i = currentStepIdx.value - 1; i >= 0; i--) {
    if (stepIsVisible(steps.value[i]!)) return true;
  }
  return false;
});

// --- Click-highlight state (PR1 feature, preserved) ---------------------

// Which root law's edges are currently highlighted (inbound/outbound).
// null = no highlight. Toggles off when the same law is clicked twice.
const selectedRoot = ref<string | null>(null);

// Laws the user hid via the close button (visible but excluded).
const hiddenLaws = ref(new Set<string>());

// Reset local UI state when the underlying law changes - the hidden set /
// selection shouldn't bleed into an unrelated graph.
watch(() => props.lawId, () => {
  selectedRoot.value = null;
  hiddenLaws.value = new Set<string>();
});

const visibleNodes = computed(() => {
  const hidden = hiddenLaws.value;
  const starts = startNodeIds.value;
  const active = activeNodeIds.value;
  const visited = visitedNodeIds.value;
  return nodes.value.map((n) => {
    const classes: string[] = [];
    if (n.class) classes.push(n.class);
    if (starts.has(n.id)) classes.push('trace-start');
    if (active.has(n.id)) classes.push('trace-active');
    else if (visited.has(n.id)) classes.push('trace-visited');
    return {
      ...n,
      hidden: hidden.has(rootOfId(n.id)),
      class: classes.join(' ') || undefined,
    };
  });
});

const visibleEdges = computed(() => {
  const hidden = hiddenLaws.value;
  const highlight = selectedRoot.value;
  const active = activeEdgeIds.value;
  const visited = visitedEdgeIds.value;
  return edges.value.map((e) => {
    const sourceRoot = rootOfId(e.source);
    const targetRoot = rootOfId(e.target);
    const edgeHidden = hidden.has(sourceRoot) || hidden.has(targetRoot);
    const classes: string[] = [];
    if (highlight) {
      if (sourceRoot === highlight) classes.push('inbound');
      else if (targetRoot === highlight) classes.push('outbound');
    }
    if (active.has(e.id)) classes.push('trace-active');
    else if (visited.has(e.id)) classes.push('trace-visited');
    return { ...e, hidden: edgeHidden, class: classes.join(' ') || undefined };
  });
});

function handleNodeClick({ node, event }: NodeMouseEvent) {
  // A11Y LIMITATION (tracked for PR3 polish): the close button on a root
  // law node is detected here by DOM sniffing the click target. Keyboard
  // activation (Enter/Space on a focused button) dispatches a synthetic
  // `click` that bubbles normally, so this path works for keyboards too -
  // but the close button has no independent activation path if Vue Flow
  // ever stops forwarding inner clicks via `node-click`. Wiring a proper
  // data-callback from the custom node component is deferred to PR3.
  // Duck-typed as before: anything with a closest() counts.
  const target = event?.target as { closest?: Element['closest'] } | null | undefined;
  if (target && target.closest && target.closest('.close')) {
    hiddenLaws.value = new Set([...hiddenLaws.value, node.id]);
    if (selectedRoot.value === node.id) selectedRoot.value = null;
    return;
  }
  const isRoot = typeof node.class === 'string' && node.class.includes('root');
  if (!isRoot) return;
  selectedRoot.value = selectedRoot.value === node.id ? null : node.id;
}

// Vue Flow's MiniMap takes a flat colour string (not a CSS variable),
// so the palette token has to be resolved through getComputedStyle.
// That call is one-shot and untracked - wrapping it in a computed
// would need a fragile "touch colorScheme to register a dep" trick.
// Instead, push the resolved value into a plain ref from a watcher:
// the colorScheme dependency is now visible in the watch() argument
// list, so a future cleanup can't accidentally drop it.
//
// OS-level scheme changes while colorScheme === 'auto' aren't tracked
// (the composable doesn't expose that). Acceptable - the marker is
// decorative and the next graph re-render picks the new value up.
const { colorScheme } = useColorScheme();
const miniMapMarkerColor = ref('#ccc');

function resolveMiniMapMarkerColor() {
  miniMapMarkerColor.value =
    getComputedStyle(document.documentElement)
      .getPropertyValue('--primitives-color-coolgray-400').trim() || '#ccc';
}

resolveMiniMapMarkerColor();
watch(colorScheme, resolveMiniMapMarkerColor);

function miniMapNodeColor(node: FlowGraphNode) {
  // visibleNodes always sets `class` to a string or undefined.
  const nodeClass = node.class as string | undefined;
  if (!(nodeClass?.includes('root') && !node.hidden)) return 'transparent';
  return miniMapMarkerColor.value;
}

const currentStep = computed(() =>
  currentStepIdx.value >= 0 && currentStepIdx.value < steps.value.length
    ? steps.value[currentStepIdx.value]! // index bounds-checked above
    : null,
);
</script>

<template>
  <div class="law-graph-view">
    <div v-if="error" class="law-graph-error">
      <nldd-inline-dialog variant="alert" text="Kon de graaf niet opbouwen" :supporting-text="error"></nldd-inline-dialog>
    </div>

    <div v-else-if="!lawId" class="law-graph-empty">
      <nldd-inline-dialog text="Open een regelwerk om de graaf te zien."></nldd-inline-dialog>
    </div>

    <template v-else>
      <div class="law-graph-container">
        <div v-if="loading" class="law-graph-loading">Bezig met laden…</div>
        <div v-else-if="missingDeps.length > 0" class="law-graph-warning" :title="missingDeps.join(', ')">
          Kon {{ missingDeps.length }} afhankelijkhe{{ missingDeps.length === 1 ? 'id' : 'den' }} niet laden
        </div>
        <VueFlow
          :nodes="visibleNodes"
          :edges="visibleEdges"
          :node-types="nodeTypes"
          :nodes-connectable="false"
          :min-zoom="0.1"
          fit-view-on-init
          @node-click="handleNodeClick"
        >
          <Controls :show-lock="false" />
          <Background variant="dots" />
          <MiniMap zoomable pannable :node-color="miniMapNodeColor" />
        </VueFlow>
      </div>

      <!-- Trace stepper - shown after a scenario runs -->
      <div v-if="hasTrace" class="law-graph-trace">
        <nldd-container padding="8">
          <nldd-toolbar size="md" accessible-label="Stappen van de uitvoering">
            <nldd-toolbar-item slot="start" label="Vorige" :priority="3">
              <nldd-button
                start-icon="back"
                text="Vorige"
                appearance="secondary"
                size="md"
                :disabled="!hasPrevVisible || undefined"
                @click="prevVisible"
              ></nldd-button>
              <nldd-menu-item slot="overflow" icon="back" text="Vorige stap" :disabled="!hasPrevVisible || undefined" @select="prevVisible"></nldd-menu-item>
            </nldd-toolbar-item>
            <nldd-toolbar-item slot="start" label="Volgende" :priority="3">
              <nldd-button
                end-icon="forward"
                text="Volgende"
                appearance="secondary"
                size="md"
                :disabled="!hasNextVisible || undefined"
                @click="nextVisible"
              ></nldd-button>
              <nldd-menu-item slot="overflow" icon="forward" text="Volgende stap" :disabled="!hasNextVisible || undefined" @select="nextVisible"></nldd-menu-item>
            </nldd-toolbar-item>
            <nldd-toolbar-title slot="start" :text="`Stap ${currentStepIdx + 1} / ${steps.length}`"></nldd-toolbar-title>
            <nldd-toolbar-item slot="end" label="Filter" :priority="2">
              <nldd-segmented-control
                size="md"
                :value="filter"
                accessible-label="Filter op stappen"
                @change="onFilterChange"
              >
                <nldd-segmented-control-item value="highlights" text="Met highlights"></nldd-segmented-control-item>
                <nldd-segmented-control-item value="all" :text="`Alles (${steps.length})`"></nldd-segmented-control-item>
              </nldd-segmented-control>
              <nldd-menu-group slot="overflow" text="Filter">
                <nldd-menu-item type="radio" value="highlights" text="Met highlights" :selected="filter === 'highlights' || undefined" @select="filter = 'highlights'"></nldd-menu-item>
                <nldd-menu-item type="radio" value="all" :text="`Alles (${steps.length})`" :selected="filter === 'all' || undefined" @select="filter = 'all'"></nldd-menu-item>
              </nldd-menu-group>
            </nldd-toolbar-item>
          </nldd-toolbar>
        </nldd-container>

        <div class="law-graph-trace__body">
          <div class="law-graph-trace__list">
            <TraceStepList
              :steps="steps"
              :current-step-idx="currentStepIdx"
              :filter="filter"
              @select-step="goto"
            />
          </div>
          <div class="law-graph-trace__detail">
            <TraceStepDetail
              :step="currentStep"
              :outputs="result?.outputs || {}"
              :expectations="expectations"
            />
          </div>
        </div>
      </div>
    </template>
  </div>
</template>

<style scoped>
.law-graph-view {
  /* Fill the pane. nldd-page wraps slotted content in a flex column,
   * so flex-grow + min-height: 0 lets us claim every pixel the parent
   * gives us - works equally inside a side pane and inside a bottom
   * sheet, no chrome-height arithmetic needed. */
  display: flex;
  flex-direction: column;
  flex-grow: 1;
  min-height: 0;
}

.law-graph-container {
  flex: 1;
  position: relative;
  min-height: 0;
}

.law-graph-loading,
.law-graph-warning {
  position: absolute;
  top: 12px;
  left: 12px;
  z-index: 5;
  padding: 6px 12px;
  border-radius: 6px;
  font-size: 13px;
}

.law-graph-loading {
  background: var(--semantics-surfaces-tinted-background-color);
  color: var(--semantics-content-secondary-color);
}

.law-graph-warning {
  background: var(--primitives-color-donkergeel-100);
  color: var(--primitives-color-donkergeel-800);
  border: 1px solid var(--primitives-color-donkergeel-300);
  cursor: help;
}

.law-graph-empty,
.law-graph-error {
  padding: 16px;
}

/* --- Trace stepper panel ------------------------------------------ */

.law-graph-trace {
  display: flex;
  flex-direction: column;
  flex: 0 0 40vh;
  min-height: 220px;
  border-top: 2px solid var(--primitives-color-donkergeel-500);
  background: var(--semantics-surfaces-background-color);
  color: var(--semantics-content-color);
  font-size: 13px;
}

.law-graph-trace__body {
  display: flex;
  flex: 1;
  min-height: 0;
}
.law-graph-trace__list {
  flex: 1;
  border-right: 1px solid var(--semantics-dividers-color);
  min-width: 0;
  overflow: hidden;
}
.law-graph-trace__detail {
  flex: 1;
  min-width: 0;
  overflow: hidden;
}
</style>
