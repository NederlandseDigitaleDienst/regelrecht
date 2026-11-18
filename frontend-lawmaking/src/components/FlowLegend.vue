<template>
  <div class="flow-legend">
    <div
      v-for="item in legend"
      :key="item.label"
      class="flow-legend__item"
    >
      <svg width="20" height="20" viewBox="0 0 20 20">
        <circle
          cx="10" cy="10" r="7"
          :fill="item.color"
          stroke="var(--color-node-stroke)"
          stroke-width="2"
        />
      </svg>
      <span class="flow-legend__label">{{ item.label }}</span>
    </div>
  </div>
</template>

<script setup lang="ts">
import { legend } from '../data/flowConstants';
</script>

<style scoped>
.flow-legend {
  display: flex;
  align-items: center;
  justify-content: center;
  gap: var(--spacing-4);
  padding: var(--spacing-2) var(--spacing-4);
  font-size: var(--font-size-xs);
  color: var(--color-text-secondary);
}

.flow-legend__item {
  display: flex;
  align-items: center;
  gap: var(--spacing-1);
}

.flow-legend__label {
  white-space: nowrap;
}
</style>
