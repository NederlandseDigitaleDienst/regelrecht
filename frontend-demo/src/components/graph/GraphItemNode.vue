<script setup lang="ts">
import { Handle, Position } from '@vue-flow/core';
import { humanize } from '../../data/format';
import type { ItemNodeData } from '../../graph/lawGraph';

// One source, input or output of a law, with the persona's value when the
// engine produced one. De lijn loopt van leverancier naar afnemer, mee met de
// layout: de uitvoer draagt het handle waar hij vertrekt (rechts), de invoer
// dat waar hij aankomt (links).
const props = defineProps<{ data: ItemNodeData }>();
</script>

<template>
  <div :class="['graph-item', `graph-item--${props.data.kind}`]" :title="props.data.ref ? `${props.data.name} ← ${props.data.ref.regulation}.${props.data.ref.output}` : props.data.name">
    <Handle v-if="props.data.kind === 'inputs'" type="target" :position="Position.Left" />
    <span class="graph-item__name">{{ humanize(props.data.name) }}</span>
    <span v-if="props.data.value !== undefined" class="graph-item__value">{{ props.data.value }}</span>
    <Handle v-if="props.data.kind === 'outputs'" type="source" :position="Position.Right" />
  </div>
</template>
