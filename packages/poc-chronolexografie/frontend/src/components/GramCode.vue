<script setup>
// One chronolexogram as the chronicle keeps it: a short heading and the gram
// itself as JSON, highlighted. The Kroniek tab and the sheet with the grams
// a reduction read both show a gram this way.
import { alsJson, kort } from '../state.js';

defineProps({
  gram: { type: Object, required: true },
  // Its place in its chronicle, from 1, when the list around it counts.
  nummer: { type: Number, default: null },
});
</script>

<template>
  <nldd-container gap="8">
    <nldd-title
      size="6"
      :overline="nummer ? `${nummer} · ${gram.recorded_on}` : gram.recorded_on"
      :text="`${gram.kind} ${kort(gram.id)}`"
      :supporting-text="gram.basis"
    ></nldd-title>
    <nldd-code-viewer language="json" appearance="box-tinted">{{ alsJson(gram) }}</nldd-code-viewer>
  </nldd-container>
</template>
