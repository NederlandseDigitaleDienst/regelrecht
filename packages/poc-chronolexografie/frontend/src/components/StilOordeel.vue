<script setup>
// Een oordeel op een begrip dat de cel voor dit geval niet weet
// (stilBegrippen in stil.js): leeg is nog niet gegeven.
import { nietGeweten } from '../stil.js';
import OordeelKeuze from './OordeelKeuze.vue';
import VeldInvoer from './VeldInvoer.vue';

defineProps({ veld: { type: Object, required: true }, modelValue: { default: undefined } });
const emit = defineEmits(['update:modelValue']);
</script>

<template>
  <OordeelKeuze
    v-if="veld.type === 'boolean'"
    :veld="veld"
    :model-value="modelValue"
    leeg="Nog niet beoordeeld"
    :toelichting="nietGeweten(veld)"
    @update:model-value="emit('update:modelValue', $event)"
  />
  <VeldInvoer
    v-else
    :veld="{ ...veld, basis: nietGeweten(veld) }"
    :model-value="modelValue"
    @update:model-value="emit('update:modelValue', $event)"
  />
</template>
