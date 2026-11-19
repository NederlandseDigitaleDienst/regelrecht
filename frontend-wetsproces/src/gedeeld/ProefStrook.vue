<script setup lang="ts">
// The band on every screen that says what this is: a trial setup with a
// made-up case. A computed decision looks as firm as a real one, and a
// screenshot travels without the text around it, so the band sits on every
// screen and names no case. In test mode it also says the date is the test
// clock's.
import { computed } from 'vue';
import { longDate } from './datum';
import type { Klok } from './types';

const props = defineProps<{ klok: Klok | null }>();

const tekst = computed(() => {
  const basis =
    'De casus, de personen en de gegevens zijn verzonnen. Een uitkomst op dit scherm is uitgerekend en is geen besluit van een bestuursorgaan.';
  const k = props.klok;
  return k?.bestuurbaar ? `${basis} De datum is die van de testklok: ${longDate(k.datum)}.` : basis;
});
</script>

<template>
  <nldd-banner variant="warning" size="sm" text="Proefopstelling met een fictieve casus" :supporting-text="tekst"></nldd-banner>
</template>
