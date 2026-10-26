<script setup lang="ts">
import type { BoxNodeData } from '../../graph/lawGraph';

// A labelled box inside a law: Bronnen, Invoer or Uitvoer. Its items are
// child nodes, so this only draws the frame and the label.
const props = defineProps<{ data: BoxNodeData }>();
</script>

<template>
  <div :class="['graph-box', `graph-box--${props.data.kind}`]">
    <span class="graph-box__label">{{ props.data.label }}</span>
  </div>
</template>
