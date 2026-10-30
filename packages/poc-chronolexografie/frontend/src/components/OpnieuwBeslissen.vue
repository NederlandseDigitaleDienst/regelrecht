<script setup>
// Opnieuw beslissen op een bezwaar na de ontvangst van een uitspraak
// (RFC-053): na een uitspraak die een nieuw besluit opdroeg (NaUitspraak.vue)
// en na een tussenuitspraak, als het bestuursorgaan het gebrek herstelt met
// een nieuwe beslissing (NaTussenuitspraak.vue, Awb 8:51a). Het
// bestuursorgaan beslist opnieuw op hetzelfde bezwaar, met de ontvangst als
// `after` (Cell::decide_after). Het geeft zijn bevindingen opnieuw, zoals bij
// de eerste heroverweging. Het formulier en de voorvertoning zijn die van een
// heroverweging onder Bestuursorgaan (OordeelKeuze.vue, Voorvertoning.vue),
// ook voor een begrip dat de cel voor dit geval niet weet (stil.js): daarover
// oordeelt het bestuursorgaan zelf.
import { ref } from 'vue';
import { beroepApi } from '../beroep.js';
import { jaNee, state, stap, zonderLeeg } from '../state.js';
import { formulier, scenarioWaarden } from '../scenarioInvoer.js';
import { stilBegrippen } from '../stil.js';
import OordeelKeuze from './OordeelKeuze.vue';
import StilOordeel from './StilOordeel.vue';
import Voorvertoning from './Voorvertoning.vue';

const props = defineProps({ ontvangst: { type: Object, required: true } });
const emit = defineEmits(['genomen']);

const toets = ref(null);
const vorm = formulier(() => scenarioWaarden('opnieuw_beslissen'));
const { stil, meldStil } = stilBegrippen(vorm.eigen);
const id = () => props.ontvangst.id;
// De begrippen die de cel bij deze beslissing niet weet en die niet al onder
// de bevindingen van het bestreden besluit staan.
const stilNa = () =>
  (stil[id()] ?? []).filter((v) => !toets.value.bevindingen.some((b) => b.naam === v.naam));

async function toon() {
  const antwoord = await stap(() => beroepApi.heroverweging(id()));
  if (antwoord) {
    toets.value = antwoord;
    vorm.wis(id());
  }
}

// Wat het bestuursorgaan opnieuw vindt; 'Laat staan' geeft niets.
const gegeven = () => zonderLeeg(vorm.waarden(id()));

async function beslis() {
  const antwoord = await stap(() => beroepApi.opnieuwBeslissen(id(), gegeven(), state.datum));
  if (antwoord) {
    toets.value = null;
    emit('genomen', antwoord);
  }
}
</script>

<template>
  <nldd-button v-if="!toets" appearance="primary" text="Heroverwegen" @click="toon"></nldd-button>
  <nldd-container v-else layout="grid" gap="16">
    <nldd-container gap="12">
      <OordeelKeuze
        v-for="b in toets.bevindingen"
        :key="b.naam"
        :veld="b"
        :model-value="vorm.waarden(id())[b.naam]"
        :leeg="`Laat staan (${jaNee(b.eerder)})`"
        :toelichting="`${b.basis} · eerder: ${jaNee(b.eerder)}`"
        @update:model-value="vorm.zet(id(), b.naam, $event)"
      />
      <StilOordeel
        v-for="v in stilNa()"
        :key="v.naam"
        :veld="v"
        :model-value="vorm.waarden(id())[v.naam]"
        @update:model-value="vorm.zet(id(), v.naam, $event)"
      />
      <nldd-button appearance="primary" text="Opnieuw beslissen op bezwaar" @click="beslis"></nldd-button>
    </nldd-container>
    <Voorvertoning soort="na-uitspraak" :op="id()" :bevindingen="gegeven()" @stil="meldStil(id(), $event)" />
  </nldd-container>
</template>
