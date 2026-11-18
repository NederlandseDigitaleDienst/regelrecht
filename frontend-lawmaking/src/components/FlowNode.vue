<template>
  <g
    class="flow-node"
    :class="{ 'flow-node--active': isActive, 'flow-node--selected': isSelected }"
    :transform="`translate(${x}, ${y})`"
    @click.stop="$emit('select', stage.id)"
    role="button"
    :tabindex="isActive ? 0 : -1"
    @keydown.enter.stop="$emit('select', stage.id)"
  >
    <!-- Node circle -->
    <circle
      :r="radius"
      :fill="fillColor"
      stroke="var(--color-node-stroke)"
      stroke-width="3"
      class="flow-node__shape"
    />

    <!-- Selection ring -->
    <circle
      v-if="isSelected"
      :r="radius + 6"
      fill="none"
      stroke="var(--color-node-stroke)"
      stroke-width="2"
      opacity="0.4"
    />

    <!-- Date label (for real law views) -->
    <text
      v-if="stage.date"
      class="flow-node__date"
      :x="-20"
      y="4"
      text-anchor="end"
      font-size="9"
      fill="var(--color-text-muted)"
    >{{ stage.date }}</text>

    <!-- Tags (deploy markers, release tags) -->
    <g
      v-for="(tag, i) in (stage.tags || [])"
      :key="tag.label"
      :transform="`translate(16, ${30 + i * 22})`"
      class="flow-node__tag"
    >
      <polygon
        points="0,-7 50,-7 50,7 8,7 0,0"
        :fill="tag.color || 'var(--color-node-stroke)'"
        opacity="0.15"
        stroke="var(--color-node-stroke)"
        stroke-width="1"
      />
      <text
        x="8"
        y="4"
        font-size="9"
        font-weight="700"
        :fill="tag.color || 'var(--color-node-stroke)'"
        class="flow-node__tag-label"
      >{{ tag.label }}</text>
    </g>

    <!-- Labels to the right of the node -->
    <g :transform="`translate(20, 0)`">
      <text
        class="flow-node__git-label"
        :y="-6"
        :fill="'var(--color-text-muted)'"
        font-size="11"
      >{{ stage.gitLabel }}</text>
      <text
        class="flow-node__law-label"
        :y="10"
        :fill="'var(--color-text-primary)'"
        font-size="14"
        font-weight="700"
      >{{ stage.lawLabel }}</text>
      <text
        v-if="stage.subtitle"
        class="flow-node__subtitle"
        :y="24"
        :fill="'var(--color-text-secondary)'"
        font-size="11"
      >{{ stage.subtitle }}</text>
    </g>
  </g>
</template>

<script setup lang="ts">
import { computed } from 'vue';
import { branchColors, typeColors } from '../data/flowConstants';
import type { PlacedStage } from '../data/types';

const props = withDefaults(
  defineProps<{
    stage: PlacedStage;
    isActive?: boolean;
    isSelected?: boolean;
    columnWidth?: number;
    rowHeight?: number;
  }>(),
  { isActive: false, isSelected: false, columnWidth: 220, rowHeight: 80 },
);

defineEmits<{ select: [id: string] }>();

const radius = 10;

const x = computed(() => 80 + props.stage.col * props.columnWidth);
const y = computed(() => 50 + props.stage.row * props.rowHeight);

const fillColor = computed(() =>
  branchColors[props.stage.branch]
    || typeColors[props.stage.type]
    || 'var(--color-branch-main)',
);
</script>

<style scoped>
.flow-node {
  cursor: pointer;
  transition: opacity var(--transition-normal);
}

.flow-node:not(.flow-node--active) {
  opacity: 0;
  pointer-events: none;
}

.flow-node--active {
  opacity: 1;
  animation: nodeAppear 0.4s ease-out;
}

.flow-node__shape {
  transition: transform var(--transition-fast);
}

.flow-node:hover .flow-node__shape {
  transform: scale(1.2);
}

.flow-node__git-label {
  font-family: var(--font-family-mono);
  letter-spacing: 0.02em;
}

.flow-node__law-label {
  font-family: var(--font-family);
}

.flow-node__subtitle {
  font-family: var(--font-family);
}

.flow-node__date {
  font-family: var(--font-family-mono);
  font-variant-numeric: tabular-nums;
}

.flow-node__tag-label {
  font-family: var(--font-family-mono);
  letter-spacing: 0.02em;
}

@keyframes nodeAppear {
  from { opacity: 0; }
  to { opacity: 1; }
}
</style>
