<script setup>
// One chronolexogram of a chronicle: its fact and the values it records in
// view, the gram itself as highlighted JSON behind a button. The Kroniek tab
// and the sheet with the grams a reduction read both show a gram this way.
import { computed, ref } from 'vue';
import { alsJson, eigenWaarden, kort } from '../state.js';

const props = defineProps({
  gram: { type: Object, required: true },
  // Its place in its chronicle, from 1, when the list around it counts.
  nummer: { type: Number, default: null },
});

const open = ref(false);
const naam = computed(() => `${props.gram.kind} ${kort(props.gram.id)}`);
const kop = computed(() => props.gram.fact ?? naam.value);
const boven = computed(() =>
  [props.nummer, props.gram.recorded_on, props.gram.fact && naam.value].filter(Boolean).join(' · '),
);
</script>

<template>
  <nldd-container gap="8">
    <nldd-title size="6" :overline="boven" :text="kop" :supporting-text="gram.basis">
      <nldd-button
        slot="end"
        appearance="neutral-tinted"
        size="sm"
        text="Code"
        expandable
        :expanded="open"
        :accessible-label="`Code van ${naam}`"
        @click="open = !open"
      ></nldd-button>
    </nldd-title>
    <nldd-container layout="wrap" gap="4">
      <nldd-tag v-for="w in eigenWaarden(gram)" :key="w" size="sm" :text="w"></nldd-tag>
    </nldd-container>
    <nldd-code-viewer v-if="open" language="json" appearance="box-tinted">{{ alsJson(gram) }}</nldd-code-viewer>
  </nldd-container>
</template>
