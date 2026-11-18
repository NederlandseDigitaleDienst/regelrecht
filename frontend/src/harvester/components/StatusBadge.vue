<script setup lang="ts">
import { computed } from 'vue';
import { STATUS_BADGE_MAP } from '../constants';
import { formatStatus } from '../formatters';

const props = withDefaults(
  defineProps<{
    status: string;
    size?: string;
  }>(),
  {
    size: 'sm',
  },
);

// Colour stays keyed on the raw enum value; only the visible label is humanised.
const variant = computed(() => STATUS_BADGE_MAP[props.status] || 'neutral');
const label = computed(() => formatStatus(props.status));
</script>

<template>
  <nldd-tag :color="variant" :text="label" :size="size" />
</template>
