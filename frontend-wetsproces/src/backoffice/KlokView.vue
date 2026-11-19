<script setup lang="ts">
// The test clock, for the admin in test mode: the day every step runs on.
// Moving it lets a process run past a deadline without waiting for it. The
// server refuses it in stable mode, where the clock is the real date.
import { onMounted, ref } from 'vue';
import { longDate } from '../gedeeld/datum';
import { foutTekst } from '../gedeeld/http';
import type { Klok } from '../gedeeld/types';
import { backofficeApi } from './api';

const klok = ref<Klok | null>(null);
const datum = ref('');
const melding = ref('');
const gezet = ref(false);

onMounted(async () => {
  try {
    klok.value = await backofficeApi.klok();
    datum.value = klok.value.datum;
  } catch (e) {
    melding.value = foutTekst(e);
  }
});

const lees = (e: Event) => {
  const detail = (e as CustomEvent<{ value?: string }>).detail;
  datum.value = detail?.value ?? (e.target as HTMLInputElement | null)?.value ?? '';
};

async function zet() {
  melding.value = '';
  gezet.value = false;
  try {
    klok.value = await backofficeApi.klokZetten(datum.value);
    gezet.value = true;
  } catch (e) {
    melding.value = foutTekst(e);
  }
}
</script>

<template>
  <nldd-simple-section width="640px">
    <nldd-title slot="header" heading-level="1" text="Klok" supporting-text="Elke stap in deze testomgeving krijgt deze dag als datum."></nldd-title>
    <nldd-inline-dialog v-if="!klok && melding" icon="warning" icon-color="critical" text="De klok laadt niet" :supporting-text="melding"></nldd-inline-dialog>
    <nldd-container v-if="klok" gap="16">
      <nldd-inline-dialog v-if="!klok.bestuurbaar" icon="info" text="De klok volgt de echte datum" supporting-text="Alleen in de testmodus is de klok te zetten."></nldd-inline-dialog>
      <template v-else>
        <nldd-text>Het is nu {{ longDate(klok.datum) }}.</nldd-text>
        <nldd-form-field label="Nieuwe datum">
          <nldd-date-field :value="datum" @change="lees"></nldd-date-field>
        </nldd-form-field>
        <nldd-banner v-if="gezet" variant="success" :text="`De klok staat op ${longDate(klok.datum)}`"></nldd-banner>
        <nldd-inline-dialog v-if="melding" icon="warning" icon-color="critical" text="De klok is niet gezet" :supporting-text="melding"></nldd-inline-dialog>
        <nldd-button-group orientation="horizontal">
          <nldd-button appearance="primary" text="Klok zetten" @click="zet"></nldd-button>
        </nldd-button-group>
      </template>
    </nldd-container>
  </nldd-simple-section>
</template>
