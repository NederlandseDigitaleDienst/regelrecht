<script setup lang="ts">
// The tools of the test setup, for the admin in test mode, over whatever
// screen of the back office is open; the account menu opens them by hash.
// Setting the clock is a sheet (`#klok`): the day every step runs on, which
// lets a process run past a deadline without waiting for it; the server
// refuses it in stable mode, where the clock is the real date. Emptying the
// test setup is a modal dialog (`#testomgeving-legen`), since it cannot be
// undone; once done it reloads the work queue, which shows the setup empty,
// since every screen still shows what was there.
import { computed, ref, watch } from 'vue';
import { useRoute, useRouter } from 'vue-router';
import { longDate } from '../gedeeld/datum';
import { foutTekst } from '../gedeeld/http';
import { huidigeKlok } from '../gedeeld/klok';
import type { Klok } from '../gedeeld/types';
import { backofficeApi } from './api';
import { KLOK, LEGEN } from './routes';

const route = useRoute();
const router = useRouter();
const klokOpen = computed(() => route.hash === KLOK);
const legenOpen = computed(() => route.hash === LEGEN);

const klok = ref<Klok | null>(null);
const datum = ref('');
const melding = ref('');
const gezet = ref(false);

// Each time the sheet opens it reads the clock afresh and says nothing of before.
watch(
  klokOpen,
  async (o) => {
    if (!o) return;
    melding.value = '';
    gezet.value = false;
    try {
      klok.value = await backofficeApi.klok();
      datum.value = klok.value.datum;
    } catch (e) {
      melding.value = foutTekst(e);
    }
  },
  { immediate: true },
);

const sluit = () => void router.push({ path: route.path, query: route.query });

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

const bezig = ref(false);
const legenFout = ref('');

watch(legenOpen, (o) => {
  if (o) legenFout.value = '';
});

async function leegmaken() {
  legenFout.value = '';
  bezig.value = true;
  try {
    huidigeKlok.value = await backofficeApi.leegmaken();
  } catch (e) {
    legenFout.value = foutTekst(e);
    return;
  } finally {
    bezig.value = false;
  }
  // Every screen still shows what was there: start over at the work queue.
  await router.push('/backoffice');
  router.go(0);
}

const LEGEN_UITLEG =
  'Alles wat in deze testomgeving is vastgelegd verdwijnt: de kronieken van alle cellen, met de aanvragen, besluiten, bezwaren en beroepen, en wat in het beheerpaneel namens externe partijen is ingevoerd. De klok gaat terug naar de begindatum. Wie is ingelogd, blijft ingelogd. Dit is niet terug te draaien.';
</script>

<template>
  <Teleport to="body">
    <nldd-sheet width="720px" accessible-label="Klok instellen" :open="klokOpen || undefined" @close="klokOpen && sluit()">
      <nldd-page sticky-header>
        <nldd-top-title-bar slot="header" heading-level="2" text="Klok instellen" dismiss-text="Sluit" collapse-anchor="klok-titel"></nldd-top-title-bar>
        <nldd-simple-section>
          <nldd-container>
            <nldd-title id="klok-titel" size="display-2" heading-level="2" text="Klok instellen"></nldd-title>
            <nldd-spacer size="8"></nldd-spacer>
            <nldd-inline-dialog v-if="!klok && melding" icon="warning" icon-color="critical" text="De klok laadt niet" :supporting-text="melding"></nldd-inline-dialog>
            <template v-if="klok">
              <nldd-inline-dialog v-if="!klok.bestuurbaar" icon="info" text="De klok volgt de echte datum" supporting-text="Alleen in de testmodus is de klok te zetten."></nldd-inline-dialog>
              <template v-else>
                <nldd-text><p>Elke stap in deze testomgeving krijgt deze dag als datum. Het is nu {{ longDate(klok.datum) }}.</p></nldd-text>
                <nldd-spacer size="24"></nldd-spacer>
                <nldd-form-field label="Nieuwe datum">
                  <nldd-date-field :value="datum" @change="lees"></nldd-date-field>
                </nldd-form-field>
                <template v-if="gezet">
                  <nldd-spacer size="16"></nldd-spacer>
                  <nldd-banner variant="success" :text="`De klok staat op ${longDate(klok.datum)}`"></nldd-banner>
                </template>
                <template v-if="melding">
                  <nldd-spacer size="16"></nldd-spacer>
                  <nldd-inline-dialog icon="warning" icon-color="critical" text="De klok is niet gezet" :supporting-text="melding"></nldd-inline-dialog>
                </template>
                <nldd-spacer size="24"></nldd-spacer>
                <nldd-button-group orientation="vertical">
                  <nldd-button appearance="primary" text="Zet klok" @click="zet"></nldd-button>
                </nldd-button-group>
              </template>
            </template>
          </nldd-container>
        </nldd-simple-section>
      </nldd-page>
    </nldd-sheet>

    <nldd-modal-dialog
      variant="alert"
      :open="legenOpen || undefined"
      text="Testomgeving legen?"
      :supporting-text="legenFout ? `Het legen lukte niet: ${legenFout}` : LEGEN_UITLEG"
      @close="legenOpen && sluit()"
    >
      <nldd-button slot="actions" appearance="primary" text="Behoud gegevens" @click="sluit"></nldd-button>
      <nldd-button slot="actions" appearance="destructive" text="Leeg testomgeving" :loading="bezig || undefined" @click="leegmaken"></nldd-button>
    </nldd-modal-dialog>
  </Teleport>
</template>
