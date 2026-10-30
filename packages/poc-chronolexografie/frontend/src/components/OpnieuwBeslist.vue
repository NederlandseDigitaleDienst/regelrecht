<script setup>
// Wat het bestuursorgaan opnieuw besliste op een bezwaar na de ontvangst van
// een uitspraak (OpnieuwBeslissen.vue): de beslissing, het nieuwe besluit als
// zij het bestreden besluit herriep, en waarom.
import { alsJson, kort } from '../state.js';
import Uitkomst from './Uitkomst.vue';
import Waarom from './Waarom.vue';

defineProps({ antwoord: { type: Object, required: true } });
</script>

<template>
  <nldd-container gap="12">
    <nldd-title
      size="5"
      :text="`Opnieuw beslist op bezwaar ${kort(antwoord.beslissing.refers_to)}`"
      :supporting-text="antwoord.beslissing.outputs.bestreden_besluit_herroepen ? 'Gegrond: het besluit is herroepen, er is een nieuw besluit' : 'Ongegrond: het besluit blijft'"
    ></nldd-title>
    <nldd-code-viewer language="json" appearance="box-tinted">{{ alsJson(antwoord.beslissing.outputs) }}</nldd-code-viewer>
    <Uitkomst
      v-if="antwoord.nieuw_besluit"
      titel="Nieuw besluit"
      :besluit="antwoord.nieuw_besluit"
      :waarom="antwoord.waarom"
    />
    <Waarom v-else :waarom="antwoord.waarom" />
  </nldd-container>
</template>
