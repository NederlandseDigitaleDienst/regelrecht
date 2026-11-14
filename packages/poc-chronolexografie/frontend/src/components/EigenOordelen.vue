<script setup>
// De oordelen die het beslissende artikel zelf aan het bestuursorgaan vraagt
// (oordelen.js), één veld per open begrip. Bij een eerste beslissing op
// bezwaar en bij opnieuw beslissen na een uitspraak hetzelfde formulier.
import VeldInvoer from './VeldInvoer.vue';

defineProps({
  velden: { type: Array, required: true },
  waarden: { type: Object, required: true },
});
const emit = defineEmits(['zet']);
</script>

<template>
  <VeldInvoer
    v-for="v in velden"
    :key="v.naam"
    :model-value="waarden[v.naam]"
    :veld="v"
    @update:model-value="emit('zet', v.naam, $event)"
  />
</template>
