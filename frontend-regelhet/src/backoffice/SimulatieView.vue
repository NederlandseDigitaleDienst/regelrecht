<script setup lang="ts">
// The simulation, for the admin in test mode, in two parts. The clock: the
// day every step runs on; moving it lets a process run past a deadline
// without waiting for it, and the server refuses it in stable mode, where the
// clock is the real date. Emptying the test setup, after a confirmation,
// which the server offers only in test mode.
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
const bevestigen = ref(false);
const leeg = ref(false);
const bezig = ref(false);

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

async function leegmaken() {
  bevestigen.value = false;
  melding.value = '';
  gezet.value = false;
  bezig.value = true;
  try {
    klok.value = await backofficeApi.leegmaken();
    datum.value = klok.value.datum;
    huidigeKlok.value = klok.value;
    leeg.value = true;
  } catch (e) {
    melding.value = foutTekst(e);
  } finally {
    bezig.value = false;
  }
}

async function zet() {
  leeg.value = false;
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
  <nldd-page landmarks="page" sticky-header accessible-label="Simulatie">
    <nldd-top-title-bar slot="header" text="Simulatie" collapse-anchor="simulatie-titel"></nldd-top-title-bar>
    <nldd-simple-section>
      <nldd-title id="simulatie-titel" size="display-3"><h3>Simulatie</h3></nldd-title>
      <nldd-text color="secondary"><p>De testomgeving bijsturen: de datum waarop alles gebeurt, en een lege start.</p></nldd-text>
      <nldd-spacer size="24"></nldd-spacer>
      <nldd-inline-dialog v-if="!klok && melding" icon="warning" icon-color="critical" text="De klok laadt niet" :supporting-text="melding"></nldd-inline-dialog>
      <nldd-container v-if="klok" gap="16">
        <nldd-inline-dialog v-if="!klok.bestuurbaar" icon="info" text="De klok volgt de echte datum" supporting-text="Alleen in de testmodus is de klok te zetten."></nldd-inline-dialog>
        <template v-else>
          <nldd-title size="display-5"><h4>Klok</h4></nldd-title>
          <nldd-text color="secondary"><p>Elke stap in deze testomgeving krijgt deze dag als datum.</p></nldd-text>
          <nldd-text><p>Het is nu {{ longDate(klok.datum) }}.</p></nldd-text>
          <nldd-form-field label="Nieuwe datum">
            <nldd-date-field :value="datum" @change="lees"></nldd-date-field>
          </nldd-form-field>
          <nldd-banner v-if="gezet" variant="success" :text="`De klok staat op ${longDate(klok.datum)}`"></nldd-banner>
          <nldd-inline-dialog v-if="melding" icon="warning" icon-color="critical" text="De klok is niet gezet" :supporting-text="melding"></nldd-inline-dialog>
          <nldd-button-group>
            <nldd-button appearance="primary" text="Klok zetten" @click="zet"></nldd-button>
          </nldd-button-group>

          <nldd-spacer size="24"></nldd-spacer>
          <nldd-title size="display-5"><h4>Testomgeving leegmaken</h4></nldd-title>
          <nldd-text color="secondary">
            <p>
              Begin opnieuw met een lege testomgeving: alle kronieken zijn daarna leeg en de klok staat weer op de
              begindatum van de casus. Wie is ingelogd, blijft ingelogd.
            </p>
          </nldd-text>
          <nldd-banner v-if="leeg" variant="success" :text="`De testomgeving is leeg; de klok staat op ${longDate(klok.datum)}`"></nldd-banner>
          <nldd-button-group>
            <nldd-button appearance="critical-tinted" start-icon="delete" text="Testomgeving leegmaken" :loading="bezig || undefined" @click="bevestigen = true"></nldd-button>
          </nldd-button-group>
        </template>
      </nldd-container>
    </nldd-simple-section>
  </nldd-page>

  <nldd-modal-dialog
    variant="alert"
    :open="bevestigen || undefined"
    text="Testomgeving leegmaken?"
    supporting-text="Alles wat in deze testomgeving is vastgelegd verdwijnt: de kronieken van alle cellen, met de aanvragen, besluiten, bezwaren en beroepen, en wat in het beheerpaneel namens externe partijen is ingevoerd. De klok gaat terug naar de begindatum. Wie is ingelogd, blijft ingelogd. Dit is niet terug te draaien."
    @close="bevestigen = false"
  >
    <nldd-button slot="actions" appearance="primary" text="Annuleren" @click="bevestigen = false"></nldd-button>
    <nldd-button slot="actions" appearance="destructive" text="Leegmaken" @click="leegmaken"></nldd-button>
  </nldd-modal-dialog>
</template>
