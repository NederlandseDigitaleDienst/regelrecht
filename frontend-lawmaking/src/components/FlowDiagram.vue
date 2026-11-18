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
        :x1="80 + branch.col * columnWidth"
        :y1="50 + branch.startRow * rowHeight"
        :x2="80 + branch.col * columnWidth"
        :y2="50 + branch.endRow * rowHeight"
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
          :x="80 + branch.col * columnWidth"
          :y="50 + branch.startRow * rowHeight - 20"
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
            :y="50 + phase.startRow * rowHeight - 20"
            :width="svgWidth"
            :height="(phase.endRow - phase.startRow + 1) * rowHeight"
            :fill="phase.color || 'transparent'"
            opacity="0.04"
          />
          <text
            :x="16"
            :y="50 + phase.startRow * rowHeight - 4"
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

const SAME_COL_GAP = 1;
const CROSS_COL_GAP = 2;

/**
 * Auto-layout: compute row positions from step order and connections.
 */
const layoutStages = computed((): PlacedStage[] => {
  const sorted = [...props.stages].sort((a, b) => a.step - b.step);
  const rowMap = new Map<string, number>();
  const colLastRow = new Map<number, number>();
  let globalLastRow = -1;

  for (const stage of sorted) {
    const inbound = props.connections.filter((c) => c.to === stage.id);
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
});

/** A branch or phase whose rows are known, so it can be drawn. */
type Rows = { startRow: number; endRow: number };

const layoutBranches = computed((): (Branch & Rows)[] => {
  return props.branches.map((branch) => {
    const branchStages = layoutStages.value.filter((s) => s.branch === branch.id);
    // A branch without stages is drawn at opacity 0 (see branchOpacity), so
    // its rows only need to be numbers. No dataset has one today.
    if (branchStages.length === 0) {
      return { ...branch, startRow: branch.startRow ?? 0, endRow: branch.endRow ?? 0 };
    }
    const rows = branchStages.map((s) => s.row);
    return {
      ...branch,
      startRow: Math.min(...rows),
      endRow: Math.max(...rows),
    };
  });
});

const maxCol = computed(() => Math.max(0, ...layoutStages.value.map((s) => s.col)));
const maxRow = computed(() => Math.max(0, ...layoutStages.value.map((s) => s.row)));

const svgWidth = computed(() => 80 + (maxCol.value + 2) * columnWidth);
const svgHeight = computed(() => 50 + (maxRow.value + 2) * rowHeight);

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
    const targetY = (50 + maxActiveRow * rowHeight) * scale.value;
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

/**
 * Resolve phase row ranges from stage IDs (startStage/endStage)
 * or fall back to hardcoded startRow/endRow for legacy data.
 */
const resolvedPhases = computed(() => {
  if (!props.phases) return [];
  return props.phases.map((p) => {
    let startRow = p.startRow;
    let endRow = p.endRow;
    if (p.startStage) {
      const s = layoutStages.value.find((st) => st.id === p.startStage);
      if (s) startRow = s.row;
    }
    if (p.endStage) {
      const s = layoutStages.value.find((st) => st.id === p.endStage);
      if (s) endRow = s.row;
    }
    return { ...p, startRow, endRow };
  });
});

const visiblePhases = computed(() => {
  return resolvedPhases.value.filter((p): p is Phase & Rows => {
    const { startRow, endRow } = p;
    if (startRow === undefined || endRow === undefined) return false;
    const phaseStages = layoutStages.value.filter(
      (s) => s.row >= startRow && s.row <= endRow
    );
    if (phaseStages.length === 0) return false;
    const minStep = Math.min(...phaseStages.map((s) => s.step));
    return props.activeStep >= minStep;
  });
});

function branchOpacity(branch: Branch): number {
  const branchStages = layoutStages.value.filter((s) => s.branch === branch.id);
  if (branchStages.length === 0) return 0;
  const minStep = Math.min(...branchStages.map((s) => s.step));
  return props.activeStep >= minStep ? 1 : 0;
}

function isConnectionActive(conn: Connection): boolean {
  const fromStage = layoutStages.value.find((s) => s.id === conn.from);
  const toStage = layoutStages.value.find((s) => s.id === conn.to);
  if (!fromStage || !toStage) return false;
  return props.activeStep >= toStage.step;
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
