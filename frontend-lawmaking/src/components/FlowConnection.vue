<template>
  <path
    class="flow-connection"
    :class="{ 'flow-connection--active': isActive }"
    :d="pathData"
    fill="none"
    stroke="var(--color-connection)"
    stroke-width="3"
    :stroke-dasharray="isActive ? 'none' : dashArray"
  />
</template>

<script setup lang="ts">
import { computed } from 'vue';
import type { Connection, PlacedStage } from '../data/types';
import { connectionPath, stagePosition } from './flowLayout';

const props = withDefaults(
  defineProps<{
    connection: Connection;
    stages: PlacedStage[];
    isActive?: boolean;
    columnWidth?: number;
    rowHeight?: number;
  }>(),
  { isActive: false, columnWidth: 220, rowHeight: 80 },
);

function getPos(stageId: string): { x: number; y: number } {
  const stage = props.stages.find((s) => s.id === stageId);
  if (!stage) return { x: 0, y: 0 };
  return stagePosition(stage, props.columnWidth, props.rowHeight);
}

const dashArray = computed(() => {
  if (props.connection.type === 'main-continues') return '6 4';
  return 'none';
});

const pathData = computed(() =>
  connectionPath(getPos(props.connection.from), getPos(props.connection.to)),
);
</script>

<style scoped>
.flow-connection {
  transition: opacity 0.5s ease;
}

.flow-connection:not(.flow-connection--active) {
  opacity: 0;
}

.flow-connection--active {
  opacity: 1;
}
</style>
