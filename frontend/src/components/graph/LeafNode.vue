<script setup lang="ts">
import { Handle } from '@vue-flow/core';
import type { Position } from '@vue-flow/core';

withDefaults(
  defineProps<{
    data: { label: string; tooltip?: string };
    // `& string` keeps the runtime prop check at `String`: the compiler cannot
    // resolve Vue Flow's Position enum and would otherwise drop the check.
    sourcePosition?: (Position & string) | null;
    targetPosition?: (Position & string) | null;
  }>(),
  { sourcePosition: null, targetPosition: null },
);
</script>

<template>
  <div class="leaf-node" :title="data.tooltip || ''">
    {{ data.label }}
  </div>
  <Handle v-if="targetPosition" type="target" :position="targetPosition" />
  <Handle v-if="sourcePosition" type="source" :position="sourcePosition" />
</template>

<style scoped>
.leaf-node {
  padding: 4px 8px;
  font-size: 12px;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
  cursor: default;
}
</style>
