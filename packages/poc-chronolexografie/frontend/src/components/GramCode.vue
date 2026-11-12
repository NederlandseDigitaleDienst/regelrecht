<script setup>
// One chronolexogram of a chronicle: its fact and main fields in view, the
// gram itself as highlighted JSON behind a button (UitklapCode.vue). The
// Kroniek tab and the sheet with the grams a reduction read both show a gram
// this way.
import { computed } from 'vue';
import { alsJson, hoofdvelden, kort } from '../state.js';
import UitklapCode from './UitklapCode.vue';

const props = defineProps({
  gram: { type: Object, required: true },
  // Its place in its chronicle, from 1, when the list around it counts.
  nummer: { type: Number, default: null },
  // The title's level in the outline around it (see UitklapCode.vue).
  headingLevel: { type: Number, default: 3 },
});

const labels = computed(() => hoofdvelden(props.gram));
const naam = computed(() => `${props.gram.kind} ${kort(props.gram.id)}`);
const kop = computed(() => props.gram.fact ?? naam.value);
const boven = computed(() =>
  [props.nummer, props.gram.recorded_on, props.gram.fact && naam.value].filter(Boolean).join(' · '),
);
</script>

<template>
  <UitklapCode
    :overline="boven"
    :text="kop"
    :supporting-text="gram.basis"
    :naam="naam"
    language="json"
    :code="alsJson(gram)"
    :heading-level="headingLevel"
  >
    <nldd-container v-if="labels.length" layout="wrap" gap="4">
      <nldd-tag v-for="w in labels" :key="w" size="sm" :text="w"></nldd-tag>
    </nldd-container>
  </UitklapCode>
</template>
