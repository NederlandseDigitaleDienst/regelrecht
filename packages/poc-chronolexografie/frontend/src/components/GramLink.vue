<script setup>
// A link to a chronolexogram in the Kroniek tab, by what `doel` in
// verwijzingen.js says of it: its kind, the end of its id and its fact or
// basis. One in the chronicle of another cell names that chronicle; one no
// chronicle of the app holds is no link, and says so.
import { toonInKroniek } from '../verwijzingen.js';

defineProps({
  doel: { type: Object, required: true },
});
</script>

<template>
  <template v-if="doel.gevonden">
    <nldd-link :href="`#gram-${doel.id}`" :text="doel.tekst" @click.prevent="toonInKroniek(doel.id)"></nldd-link>
    <template v-if="doel.kroniek"> (in de kroniek van {{ doel.kroniek }})</template>
  </template>
  <template v-else>{{ doel.tekst }} (staat niet in een kroniek die de app heeft)</template>
</template>
