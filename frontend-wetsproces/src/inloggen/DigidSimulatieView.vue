<script setup lang="ts">
// Getting in on the citizen's side in test mode: a simulated DigiD step. The
// visitor picks one of the test citizens of the server, or types a name and a
// citizen service number of the test range. The server checks the number and
// gives the account the citizen's role only (packages/wetsproces/src/login.rs).
// The screen says plainly that this is a simulation and takes nothing of the
// real DigiD: no logo, no colours, none of its wording. With Keycloak the
// ordinary login screen takes over.
import { computed, onMounted, ref } from 'vue';
import { useRoute, useRouter } from 'vue-router';
import type { AuthStatus } from '@regelrecht/frontend-shared';
import { foutTekst } from '../gedeeld/http';
import { inloggenApi } from './api';
import type { Persona } from './api';
import InloggenView from './InloggenView.vue';
import { BURGER, terugNaar } from '../gedeeld/terug';

const route = useRoute();
const router = useRouter();
const status = ref<AuthStatus | null>(null);
const burgers = ref<Persona[]>([]);
const gekozen = ref('');
const naam = ref('');
const bsn = ref('');
const fout = ref('');
const bezig = ref(false);

onMounted(async () => {
  try {
    status.value = await inloggenApi.status();
    if (status.value.oidc_configured) return;
    burgers.value = (await inloggenApi.testpersonen()).filter((p) => p.soort === 'burger' && p.identiteit);
    gekozen.value = burgers.value[0]?.identiteit ?? '';
  } catch (e) {
    fout.value = foutTekst(e);
  }
});

const testpersoon = computed(() => burgers.value.find((p) => p.identiteit === gekozen.value));
/** The value of a field, from the event of an nldd text field or a native control. */
const waarde = (e: Event): string => {
  const detail = (e as CustomEvent<{ value?: unknown }>).detail;
  if (detail && typeof detail === 'object' && 'value' in detail) return String(detail.value ?? '');
  return (e.target as HTMLInputElement | HTMLSelectElement | null)?.value ?? '';
};

async function inloggen(nummer: string, wie: string) {
  fout.value = '';
  bezig.value = true;
  try {
    await inloggenApi.digid(nummer, wie);
    // Back to the screen of a link that brought the citizen here, or the dossier.
    await router.push(terugNaar(route.query.terug, BURGER, '/burger'));
  } catch (e) {
    fout.value = foutTekst(e);
  } finally {
    bezig.value = false;
  }
}
</script>

<template>
  <InloggenView v-if="status?.oidc_configured" :naar="BURGER" />
  <nldd-page v-else landmarks="page" sticky-header accessible-label="Gesimuleerde DigiD">
    <nldd-top-title-bar slot="header" text="Gesimuleerde DigiD" collapse-anchor="digid-titel"></nldd-top-title-bar>
    <nldd-simple-section>
      <nldd-title id="digid-titel" size="display-3"><h3>Inloggen met de gesimuleerde DigiD</h3></nldd-title>
      <nldd-spacer size="16"></nldd-spacer>
      <nldd-container gap="24">
        <nldd-banner
          variant="accent"
          text="Dit is een simulatie, geen echte DigiD"
          supporting-text="Deze testomgeving vraagt geen wachtwoord en neemt geen contact op met DigiD. U logt in als een verzonnen burger: kies een testpersoon of vul zelf een naam en een BSN uit de testreeks in."
        ></nldd-banner>

        <nldd-container gap="16">
          <nldd-title size="display-5"><h4>Kies een testpersoon</h4></nldd-title>
          <nldd-form-field label="Testpersoon">
            <nldd-dropdown accessible-label="Testpersoon">
              <select :value="gekozen" @change="gekozen = waarde($event)">
                <option v-for="p in burgers" :key="p.sub" :value="p.identiteit">{{ p.naam }}, BSN {{ p.identiteit }}</option>
              </select>
            </nldd-dropdown>
          </nldd-form-field>
          <nldd-button-group>
            <nldd-button
              appearance="primary"
              start-icon="login"
              text="Inloggen als deze testpersoon"
              :disabled="!testpersoon || bezig || undefined"
              @click="testpersoon && inloggen(testpersoon.identiteit ?? '', testpersoon.naam ?? '')"
            ></nldd-button>
          </nldd-button-group>
        </nldd-container>

        <nldd-container gap="16">
          <nldd-title size="display-5"><h4>Of vul zelf een naam en een BSN in</h4></nldd-title>
          <nldd-form-field label="Naam">
            <nldd-text-field :value="naam" autocomplete="off" maxlength="100" @input="naam = waarde($event)"></nldd-text-field>
          </nldd-form-field>
          <nldd-form-field label="BSN">
            <nldd-text-field :value="bsn" keyboard="numeric" autocomplete="off" maxlength="9" @input="bsn = waarde($event)"></nldd-text-field>
            <nldd-form-field-help-text>Negen cijfers die beginnen met 99999 en de elfproef halen, zoals 999990032.</nldd-form-field-help-text>
          </nldd-form-field>
          <nldd-button-group>
            <nldd-button appearance="secondary" start-icon="login" text="Inloggen" :disabled="bezig || undefined" @click="inloggen(bsn, naam)"></nldd-button>
          </nldd-button-group>
        </nldd-container>

        <nldd-inline-dialog v-if="fout" icon="warning" icon-color="critical" text="Inloggen lukt niet" :supporting-text="fout"></nldd-inline-dialog>
      </nldd-container>
    </nldd-simple-section>
  </nldd-page>
</template>
