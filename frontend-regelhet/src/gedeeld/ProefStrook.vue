<script setup lang="ts">
// The bar at the top of every screen that says what this is: a trial setup
// with a made-up case. A computed decision looks as firm as a real one, and a
// screenshot travels without the text around it, so the bar sits on every
// screen and names no case. The bar holds one line, so the rest stands in the
// sheet it opens; in test mode that sheet also says the date is the test
// clock's.
import { computed, ref } from 'vue';
import { longDate } from './datum';
import type { Klok } from './types';

const props = defineProps<{ klok: Klok | null }>();

const open = ref(false);
const testklok = computed(() => {
  const k = props.klok;
  return k?.bestuurbaar ? `De datum is die van de testklok: ${longDate(k.datum)}.` : '';
});
</script>

<template>
  <nldd-status-bar variant="warning" text="Proefopstelling met een fictieve casus" button @click="open = true"></nldd-status-bar>

  <Teleport to="body">
    <nldd-sheet width="480px" :open="open || undefined" @close="open = false">
      <nldd-page>
        <nldd-top-title-bar
          slot="header"
          heading-level="2"
          text="Proefopstelling met een fictieve casus"
          dismiss-text="Sluit"
        ></nldd-top-title-bar>
        <nldd-simple-section>
          <nldd-container gap="16">
            <nldd-text>
              <p>
                De casus, de personen en de gegevens zijn verzonnen. Een uitkomst op dit scherm is uitgerekend en is geen
                besluit van een bestuursorgaan.
              </p>
            </nldd-text>
            <nldd-text v-if="testklok"><p>{{ testklok }}</p></nldd-text>
          </nldd-container>
        </nldd-simple-section>
      </nldd-page>
    </nldd-sheet>
  </Teleport>
</template>
