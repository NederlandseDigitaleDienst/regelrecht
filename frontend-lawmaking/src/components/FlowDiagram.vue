<template>
  <div
    class="flow-diagram"
    ref="container"
  >
    <svg
      :viewBox="`0 0 ${svgWidth} ${svgHeight}`"
      :style="{ width: svgWidth * scale + 'px', height: svgHeight * scale + 'px' }"
      class="flow-diagram__svg"
    >
      <!-- Branch lines (background) -->
      <line
        v-for="branch in layoutBranches"
        :key="branch.id"
        :x1="ORIGIN.x + branch.col * columnWidth"
        :y1="ORIGIN.y + branch.startRow * rowHeight"
        :x2="ORIGIN.x + branch.col * columnWidth"
        :y2="ORIGIN.y + branch.endRow * rowHeight"
        :stroke="branch.color"
        stroke-width="4"
        stroke-linecap="round"
        :opacity="branchOpacity(branch)"
        class="flow-diagram__branch-line"
      />

      <!-- Branch labels -->
      <g
        v-for="branch in layoutBranches"
        :key="'label-' + branch.id"
        :opacity="branchOpacity(branch)"
      >
        <text
          :x="ORIGIN.x + branch.col * columnWidth"
          :y="ORIGIN.y + branch.startRow * rowHeight - 20"
          text-anchor="middle"
          font-size="12"
          font-weight="700"
          :fill="branch.color"
          class="flow-diagram__branch-label"
        >{{ branch.gitLabel }}</text>
      </g>

      <!-- Phase labels (advanced view) -->
      <g v-if="phases">
        <g
          v-for="phase in visiblePhases"
          :key="phase.id"
        >
          <rect
            :x="0"
            :y="ORIGIN.y + phase.startRow * rowHeight - 20"
            :width="svgWidth"
            :height="(phase.endRow - phase.startRow + 1) * rowHeight"
            :fill="phase.color || 'transparent'"
            opacity="0.04"
          />
          <text
            :x="16"
            :y="ORIGIN.y + phase.startRow * rowHeight - 4"
            font-size="11"
            font-weight="700"
            :fill="'var(--color-text-muted)'"
            text-transform="uppercase"
            class="flow-diagram__phase-label"
          >{{ phase.label }}</text>
        </g>
      </g>

      <!-- Connections -->
      <FlowConnection
        v-for="conn in connections"
        :key="conn.from + '-' + conn.to"
        :connection="conn"
        :stages="layoutStages"
        :is-active="isConnectionActive(conn)"
        :column-width="columnWidth"
        :row-height="rowHeight"
      />

      <!-- Nodes -->
      <FlowNode
        v-for="stage in layoutStages"
        :key="stage.id"
        :stage="stage"
        :is-active="stage.step <= activeStep"
        :is-selected="selectedId === stage.id"
        :column-width="columnWidth"
        :row-height="rowHeight"
        @select="$emit('select-stage', $event)"
      />
    </svg>
  </div>
</template>

<script setup lang="ts">
import { computed, ref, watch, nextTick } from 'vue';
import FlowNode from './FlowNode.vue';
import FlowConnection from './FlowConnection.vue';
import type { Branch, Connection, Phase, PlacedStage, Stage, TimelineMarker } from '../data/types';
import {
  ORIGIN,
  branchRows,
  isBranchVisible,
  isConnectionActive as isConnectionLit,
  placeStages,
  resolvePhaseRows,
  visiblePhases as visiblePhasesAt,
  type Rows,
} from './flowLayout';

const props = withDefaults(
  defineProps<{
    stages: Stage[];
    branches: Branch[];
    connections: Connection[];
    phases?: Phase[] | null;
    timeline?: TimelineMarker[] | null;
    activeStep?: number;
    selectedId?: string | null;
  }>(),
  { phases: null, timeline: null, activeStep: -1, selectedId: null },
);

defineEmits<{ 'select-stage': [id: string] }>();

const container = ref<HTMLElement | null>(null);
const scale = ref(1);

const columnWidth = 220;
const rowHeight = 80;

const layoutStages = computed((): PlacedStage[] => placeStages(props.stages, props.connections));

const layoutBranches = computed((): (Branch & Rows)[] => branchRows(props.branches, layoutStages.value));

const maxCol = computed(() => Math.max(0, ...layoutStages.value.map((s) => s.col)));
const maxRow = computed(() => Math.max(0, ...layoutStages.value.map((s) => s.row)));

const svgWidth = computed(() => ORIGIN.x + (maxCol.value + 2) * columnWidth);
const svgHeight = computed(() => ORIGIN.y + (maxRow.value + 2) * rowHeight);

// Zoom via CSS scale
function zoomIn() {
  scale.value = Math.min(3, scale.value * 1.25);
}

function zoomOut() {
  scale.value = Math.max(0.3, scale.value * 0.8);
}

function resetView() {
  scale.value = 1;
  if (container.value) {
    container.value.scrollTo({ top: 0, left: 0, behavior: 'smooth' });
  }
}

defineExpose({ zoomIn, zoomOut, resetView });

// Auto-scroll to the latest active node during playback
watch(() => props.activeStep, (step) => {
  if (step < 0 || !container.value) return;
  nextTick(() => {
    // The diagram may have unmounted before the tick.
    const el = container.value;
    if (!el) return;
    // Find the highest-row active node at this step
    const activeNodes = layoutStages.value.filter((s) => s.step === step);
    if (activeNodes.length === 0) return;
    const maxActiveRow = Math.max(...activeNodes.map((s) => s.row));
    const targetY = (ORIGIN.y + maxActiveRow * rowHeight) * scale.value;
    const containerRect = el.getBoundingClientRect();
    const scrollTarget = targetY - containerRect.height * 0.6;
    el.scrollTo({
      top: Math.max(0, scrollTarget),
      behavior: 'smooth',
    });
  });
});

// Reset scroll when data changes (view toggle)
watch([svgWidth, svgHeight], () => {
  scale.value = 1;
  if (container.value) {
    container.value.scrollTo({ top: 0, left: 0 });
  }
});

const visiblePhases = computed(() =>
  props.phases ? visiblePhasesAt(resolvePhaseRows(props.phases, layoutStages.value), layoutStages.value, props.activeStep) : [],
);

function branchOpacity(branch: Branch): number {
  return isBranchVisible(branch, layoutStages.value, props.activeStep) ? 1 : 0;
}

function isConnectionActive(conn: Connection): boolean {
  return isConnectionLit(conn, layoutStages.value, props.activeStep);
}
</script>

<style scoped>
.flow-diagram {
  overflow: auto;
  padding: var(--spacing-4);
  position: relative;
}

.flow-diagram__svg {
  display: block;
}

.flow-diagram__branch-line {
  transition: opacity 0.5s ease;
}

.flow-diagram__branch-label {
  font-family: var(--font-family-mono);
  transition: opacity 0.5s ease;
}

.flow-diagram__phase-label {
  font-family: var(--font-family);
  letter-spacing: 0.06em;
  text-transform: uppercase;
}

.flow-diagram__timeline-label {
  font-family: var(--font-family-mono);
  font-variant-numeric: tabular-nums;
}
</style>
