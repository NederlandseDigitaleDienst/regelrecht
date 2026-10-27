<script setup>
// Says that a form starts from the scenario the visitor follows
// (scenarioInvoer.js), and which values the step itself gives; the rest come
// from the example values of the case. Every value can still be changed.
import { computed } from 'vue';
import { leesbaar } from '../state.js';

const props = defineProps({
  // The names of the values the step itself gives.
  namen: { type: Array, default: () => [] },
});
const tekst = computed(() => {
  const eigen = props.namen.map((n) => `"${leesbaar(n)}"`);
  const lijst = eigen.length < 2 ? eigen.join('') : `${eigen.slice(0, -1).join(', ')} en ${eigen.at(-1)}`;
  return eigen.length
    ? `Deze stap geeft ${lijst}; de rest komt uit de voorbeeldgegevens van de casus. Je kunt alles nog aanpassen.`
    : 'De waarden komen uit de voorbeeldgegevens van de casus. Je kunt ze nog aanpassen.';
});
</script>

<template>
  <nldd-inline-dialog icon="info" text="Ingevuld uit het scenario" :supporting-text="tekst"></nldd-inline-dialog>
</template>
