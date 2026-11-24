<script setup lang="ts">
// Getting in on the citizen's side in test mode: a simulated DigiD step, or,
// for a case that identifies a business by its KvK number, a simulated
// eHerkenning step. The visitor picks one of the test applicants of the
// server, or types a name and a number. The server says which number the
// case asks for (`GET /auth/simulatie`), checks it and gives the account the
// citizen's role only (packages/regelhet/src/login.rs). A case with a login
// step of its own logs in with a user name of that step and the password of
// the simulation; the step's cell records the authentication. The screen says
// plainly that this is a simulation and takes nothing of the real DigiD or
// eHerkenning: no logo, no colours, none of their wording. With Keycloak the
// ordinary login screen takes over.
import { computed, onMounted, ref } from 'vue';
import { useRoute, useRouter } from 'vue-router';
import type { AuthStatus } from '@regelrecht/frontend-shared';
import { foutTekst } from '../gedeeld/http';
import { inloggenApi } from './api';
import type { Persona, Simulatie } from './api';
import InloggenView from './InloggenView.vue';
import { BURGER, terugNaar } from '../gedeeld/terug';

const route = useRoute();
const router = useRouter();
const status = ref<AuthStatus | null>(null);
const simulatie = ref<Simulatie | null>(null);
const aanvragers = ref<Persona[]>([]);
const gekozen = ref('');
const naam = ref('');
const nummer = ref('');
const gebruikersnaam = ref('');
const wachtwoord = ref('');
const fout = ref('');
const bezig = ref(false);

/** A BSN unless the server says otherwise; the screen of a case before this was asked always was. */
const identiteit = computed(
  () => simulatie.value?.identiteit ?? { claim: 'bsn', label: 'BSN', soort: 'bsn' as const, cijfers: 9 },
);
const bedrijf = computed(() => identiteit.value.soort === 'kvk');
const stap = computed(() => simulatie.value?.inlogstap ?? null);
const middel = computed(() => (bedrijf.value ? 'eHerkenning' : 'DigiD'));
const titel = computed(() => `Gesimuleerde ${middel.value}`);
const uitleg = computed(() => {
  if (stap.value) {
    return `Deze testomgeving neemt geen contact op met eHerkenning. ${stap.value.cel_naam} kijkt in haar register of de gebruiker voor deze dienst mag inloggen, en legt het inloggen vast. Elke testgebruiker heeft het wachtwoord ${stap.value.wachtwoord}.`;
  }
  if (bedrijf.value) {
    return `Deze testomgeving vraagt geen wachtwoord en neemt geen contact op met eHerkenning. U logt in namens een verzonnen bedrijf: kies een testbedrijf of vul zelf een naam en een ${identiteit.value.label} in.`;
  }
  return 'Deze testomgeving vraagt geen wachtwoord en neemt geen contact op met DigiD. U logt in als een verzonnen burger: kies een testpersoon of vul zelf een naam en een BSN uit de testreeks in.';
});
const wie = computed(() => (bedrijf.value ? 'testbedrijf' : 'testpersoon'));
const hulp = computed(() => {
  if (identiteit.value.soort === 'bsn') return 'Negen cijfers die beginnen met 99999 en de elfproef halen, zoals 999990032.';
  if (identiteit.value.cijfers) return `${identiteit.value.cijfers} cijfers.`;
  return 'Zonder spaties.';
});

onMounted(async () => {
  try {
    status.value = await inloggenApi.status();
    if (status.value.oidc_configured) return;
    // The test applicants come first: a screen without the description of
    // the case still offers them, as a BSN login.
    aanvragers.value = (await inloggenApi.testpersonen()).filter((p) => p.soort === 'burger' && p.identiteit);
    gekozen.value = aanvragers.value[0]?.identiteit ?? '';
    const s = await inloggenApi.simulatie();
    simulatie.value = s;
    gebruikersnaam.value = s.inlogstap?.gebruikersnamen[0] ?? '';
  } catch (e) {
    fout.value = foutTekst(e);
  }
});

const testpersoon = computed(() => aanvragers.value.find((p) => p.identiteit === gekozen.value));
/** The value of a field, from the event of an nldd text field or a native control. */
const waarde = (e: Event): string => {
  const detail = (e as CustomEvent<{ value?: unknown }>).detail;
  if (detail && typeof detail === 'object' && 'value' in detail) return String(detail.value ?? '');
  return (e.target as HTMLInputElement | HTMLSelectElement | null)?.value ?? '';
};

async function binnen(inloggen: () => Promise<unknown>) {
  fout.value = '';
  bezig.value = true;
  try {
    await inloggen();
    // Back to the screen of a link that brought the applicant here, or the dossier.
    await router.push(terugNaar(route.query.terug, BURGER, '/burger'));
  } catch (e) {
    fout.value = foutTekst(e);
  } finally {
    bezig.value = false;
  }
}

const inloggen = (n: string, w: string) => binnen(() => inloggenApi.digid(n, w));
const viaStap = () => binnen(() => inloggenApi.inlogstap(gebruikersnaam.value, wachtwoord.value));
</script>

<template>
  <InloggenView v-if="status?.oidc_configured" :naar="BURGER" />
  <nldd-page v-else landmarks="page" sticky-header :accessible-label="titel">
    <nldd-top-title-bar slot="header" :text="titel" collapse-anchor="digid-titel"></nldd-top-title-bar>
    <nldd-simple-section>
      <nldd-title id="digid-titel" size="display-3"><h3>Inloggen met de gesimuleerde {{ middel }}</h3></nldd-title>
      <nldd-spacer size="16"></nldd-spacer>
      <nldd-container gap="24">
        <nldd-banner variant="accent" :text="`Dit is een simulatie, geen echte ${middel}`" :supporting-text="uitleg"></nldd-banner>

        <nldd-container v-if="stap" gap="16">
          <nldd-title size="display-5"><h4>Log in met een testgebruiker</h4></nldd-title>
          <nldd-form-field label="Gebruikersnaam">
            <nldd-dropdown accessible-label="Gebruikersnaam">
              <select :value="gebruikersnaam" @change="gebruikersnaam = waarde($event)">
                <option v-for="g in stap.gebruikersnamen" :key="g" :value="g">{{ g }}</option>
              </select>
            </nldd-dropdown>
          </nldd-form-field>
          <nldd-form-field label="Wachtwoord">
            <nldd-password-field :value="wachtwoord" @input="wachtwoord = waarde($event)"></nldd-password-field>
            <nldd-form-field-help-text>In deze simulatie is het wachtwoord {{ stap.wachtwoord }}.</nldd-form-field-help-text>
          </nldd-form-field>
          <nldd-button-group>
            <nldd-button
              appearance="primary"
              start-icon="login"
              text="Inloggen"
              :disabled="!gebruikersnaam || bezig || undefined"
              @click="viaStap()"
            ></nldd-button>
          </nldd-button-group>
        </nldd-container>

        <template v-else>
          <nldd-container v-if="aanvragers.length" gap="16">
            <nldd-title size="display-5"><h4>Kies een {{ wie }}</h4></nldd-title>
            <nldd-form-field :label="bedrijf ? 'Testbedrijf' : 'Testpersoon'">
              <nldd-dropdown :accessible-label="bedrijf ? 'Testbedrijf' : 'Testpersoon'">
                <select :value="gekozen" @change="gekozen = waarde($event)">
                  <option v-for="p in aanvragers" :key="p.sub" :value="p.identiteit">
                    {{ p.naam ? `${p.naam}, ` : '' }}{{ p.label ?? identiteit.label }} {{ p.identiteit }}
                  </option>
                </select>
              </nldd-dropdown>
            </nldd-form-field>
            <nldd-button-group>
              <nldd-button
                appearance="primary"
                start-icon="login"
                :text="bedrijf ? 'Inloggen als dit testbedrijf' : 'Inloggen als deze testpersoon'"
                :disabled="!testpersoon || bezig || undefined"
                @click="testpersoon && inloggen(testpersoon.identiteit ?? '', testpersoon.naam ?? testpersoon.identiteit ?? '')"
              ></nldd-button>
            </nldd-button-group>
          </nldd-container>

          <nldd-container gap="16">
            <nldd-title size="display-5"><h4>Of vul zelf een naam en een {{ identiteit.label }} in</h4></nldd-title>
            <nldd-form-field label="Naam">
              <nldd-text-field :value="naam" autocomplete="off" maxlength="100" @input="naam = waarde($event)"></nldd-text-field>
            </nldd-form-field>
            <nldd-form-field :label="identiteit.label">
              <nldd-text-field
                :value="nummer"
                :keyboard="identiteit.cijfers ? 'numeric' : undefined"
                autocomplete="off"
                :maxlength="identiteit.cijfers ?? 64"
                @input="nummer = waarde($event)"
              ></nldd-text-field>
              <nldd-form-field-help-text>{{ hulp }}</nldd-form-field-help-text>
            </nldd-form-field>
            <nldd-button-group>
              <nldd-button appearance="secondary" start-icon="login" text="Inloggen" :disabled="bezig || undefined" @click="inloggen(nummer, naam)"></nldd-button>
            </nldd-button-group>
          </nldd-container>
        </template>

        <nldd-inline-dialog v-if="fout" icon="warning" icon-color="critical" text="Inloggen lukt niet" :supporting-text="fout"></nldd-inline-dialog>
      </nldd-container>
    </nldd-simple-section>
  </nldd-page>
</template>
