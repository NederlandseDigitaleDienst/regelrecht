<script setup>
// De andere cellen van de casus (RFC-048): wie de wet bevoegd maakt voor een
// waarde die het bestuursorgaan nodig heeft, zoals de GGD voor het
// geneeskundig advies, of wie iets voor het bestuursorgaan doet, zoals de
// drukker van de gehandicaptenparkeerkaart. Elke cel houdt haar eigen kroniek
// bij. Een cel die een feit vastlegt (een executogram, RFC-051), krijgt hier
// een formulier (Handelingen.vue). Wat een andere cel vóór het eerste
// scenario wist, komt uit de casus. De rechtbank is ook een andere cel
// (RFC-053): zij krijgt hier de beroepen en de uitspraken (RechterCel.vue).
import { state } from '../state.js';
import Handelingen from './Handelingen.vue';
import RechterCel from './RechterCel.vue';

const kroniek = (cel) => state.andere.find((a) => a.cel === cel)?.grammen ?? [];
</script>

<template>
  <template v-for="c in state.vormen.andere_cellen" :key="c.cel">
    <RechterCel v-if="c.cel === state.casus.rechter?.cel" />
    <nldd-simple-section v-else>
      <nldd-title slot="header" size="3" :text="c.cel_naam">
        <nldd-text slot="supporting-text">Een andere cel van de casus, met een eigen kroniek.</nldd-text>
      </nldd-title>
      <nldd-container gap="16" max-width="560px">
        <Handelingen :cel="c.cel" :handelingen="c.handelingen" />
        <nldd-text size="sm" color="secondary">
          {{ kroniek(c.cel).length }} chronolexogram(men) in haar kroniek; ze staan onder Kroniek.
        </nldd-text>
      </nldd-container>
    </nldd-simple-section>
  </template>
</template>
