<script setup>
// One step of the chain of a decision: the shared step
// (frontend-shared/components/chronolex/KetenStap.vue), with the names of
// this app and its sheet. A click on a step with an origin opens the article
// or the chronolexograms the reduction read (HerkomstSheet.vue), in the
// version of the decision's day.
import GedeeldeKetenStap from '@regelrecht/frontend-shared/components/chronolex/KetenStap.vue';
import { laag, toonHerkomst } from '../herkomst.js';
import { NAMEN } from '../logboek.js';

defineProps({
  // A step from `genummerd()`: { nummer, stap, kinderen }.
  knoop: { type: Object, required: true },
  diepte: { type: Number, default: 0 },
  // The day of the decision, for the version of an article it opens; without
  // it, the play date.
  datum: { type: String, default: null },
});
</script>

<template>
  <GedeeldeKetenStap
    :knoop="knoop"
    :namen="NAMEN"
    :laag="laag"
    :diepte="diepte"
    kan-openen
    @kies="(h) => toonHerkomst({ ...h, datum })"
  />
</template>
