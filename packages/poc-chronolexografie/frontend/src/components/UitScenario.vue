<script setup>
// Says that the form of `soort` starts from the scenario the visitor follows
// (scenarioInvoer.js): which values the step gives, and whether the rest
// comes from the example values of the case. Shows nothing when the form
// starts empty. Every value can still be changed.
import { computed } from 'vue';
import { heeftVoorbeeld, ingevuld, uitScenario } from '../scenarioInvoer.js';
import { leesbaar } from '../state.js';

const props = defineProps({
  soort: { type: String, required: true },
  // What the form acts on (scenarioInvoer.js, `doel`).
  doel: { type: Object, default: () => ({}) },
});
const tekst = computed(() => {
  const namen = uitScenario(props.soort, props.doel).map((n) => `"${leesbaar(n)}"`);
  const lijst = namen.length < 2 ? namen.join('') : `${namen.slice(0, -1).join(', ')} en ${namen.at(-1)}`;
  const zinnen = [];
  if (namen.length) zinnen.push(`Uit deze stap van het scenario: ${lijst}.`);
  if (heeftVoorbeeld(props.soort)) {
    zinnen.push(namen.length ? 'De andere velden komen uit de voorbeeldgegevens van de casus.' : 'De velden komen uit de voorbeeldgegevens van de casus.');
  }
  zinnen.push('Je kunt alles nog aanpassen.');
  return zinnen.join(' ');
});
</script>

<template>
  <nldd-inline-dialog
    v-if="ingevuld(soort, doel)"
    icon="info"
    text="Ingevuld uit het scenario"
    :supporting-text="tekst"
  ></nldd-inline-dialog>
</template>
