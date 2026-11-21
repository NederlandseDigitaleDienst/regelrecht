<script setup lang="ts">
// The test clock, for the admin in test mode: the day every step runs on.
// Moving it lets a process run past a deadline without waiting for it. The
// server refuses it in stable mode, where the clock is the real date.
import { onMounted, ref } from 'vue';
import { longDate } from '../gedeeld/datum';
import { foutTekst } from '../gedeeld/http';
import { huidigeKlok } from '../gedeeld/klok';
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
    // The trial band above shows the date too.
    huidigeKlok.value = klok.value;
    gezet.value = true;
  } catch (e) {
    melding.value = foutTekst(e);
  }
}
</script>

<template>
  <nldd-page landmarks="page" sticky-header accessible-label="Klok">
    <nldd-top-title-bar slot="header" text="Klok" collapse-anchor="klok-titel"></nldd-top-title-bar>
    <nldd-simple-section>
      <nldd-title id="klok-titel" size="display-3"><h3>Klok</h3></nldd-title>
      <nldd-text color="secondary">Elke stap in deze testomgeving krijgt deze dag als datum.</nldd-text>
      <nldd-spacer size="16"></nldd-spacer>
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
  </nldd-page>
</template>
