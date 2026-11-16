<script setup>
// The login screen of the login step (inlogstap.js): it takes the place of
// the app, as a redirect to an accredited service would, and returns to the
// application once the service let the applicant in. It looks like no real
// service and says on every view that it is a simulation. The service checks
// its register on the play date; a login it refuses records nothing, and the
// screen says why. While a scenario is at a login step, the fields start
// from the values of that step (scenarioInvoer.js).
import { computed, ref } from 'vue';
import { api } from '../api.js';
import { melding } from '../melding.js';
import { accountTekst, weigerTekst } from '../inlogstap.js';
import { inlogScherm, zetSessie } from '../sessie.js';
import { scenarioWaarden } from '../scenarioInvoer.js';
import { loginGeweigerd, state, stap } from '../state.js';

const aanbod = computed(() => state.vormen?.aanvraag?.inloggen?.inlogstap ?? null);
const uitScenario = scenarioWaarden('inloggen');
const gebruikersnaam = ref(uitScenario?.gebruikersnaam ?? '');
const wachtwoord = ref(uitScenario ? (uitScenario.wachtwoord ?? aanbod.value?.wachtwoord ?? '') : '');
// The refusal of the last attempt, `{ gebruikersnaam, reden }`, or null.
const weigering = ref(null);
const leeg = ref(false);

const waarde = (e) => e.detail?.value ?? e.target?.value ?? '';

function terug() {
  state.fout = '';
  inlogScherm.value = false;
}

async function inloggen() {
  leeg.value = !gebruikersnaam.value;
  if (leeg.value) return;
  const antwoord = await stap(() =>
    api.inloggen(gebruikersnaam.value, wachtwoord.value, state.datum, uitScenario?.dienst ?? null),
  );
  if (!antwoord) return;
  if (antwoord.uitkomst === 'ingelogd') {
    zetSessie(state.casus.id, antwoord);
    inlogScherm.value = false;
    return;
  }
  weigering.value = antwoord;
  await loginGeweigerd(antwoord.reden);
}
</script>

<template>
  <nldd-shell>
    <nldd-page>
      <nldd-container slot="header" padding="16">
        <nldd-top-title-bar
          :text="`Inloggen bij ${aanbod.cel_naam}`"
          supporting-text="Gesimuleerd: geen echt inlogmiddel"
          dismiss-text="Annuleren"
          @dismiss="terug"
        ></nldd-top-title-bar>
      </nldd-container>
      <nldd-simple-section>
        <nldd-container gap="16" max-width="480px">
          <nldd-banner
            variant="warning"
            text="Gesimuleerd inloggen"
            supporting-text="Dit is geen echte inlogpagina en geen echt inlogmiddel. De erkende dienst, haar register en de accounts zijn verzonnen, zodat je kunt zien wat een inlog voor de aanvraag betekent."
          ></nldd-banner>
          <template v-if="!weigering">
            <nldd-title
              heading-level="2"
              size="display-4"
              text="Inloggen namens een onderneming"
              :supporting-text="`Je logt in voor de dienst ${aanbod.dienst} van ${state.casus.cel_naam}. Daarna ga je terug naar je aanvraag.`"
            ></nldd-title>
            <nldd-form-field label="Gebruikersnaam">
              <nldd-text-field
                :value="gebruikersnaam"
                autocomplete="username"
                required
                :invalid="leeg || undefined"
                @input="gebruikersnaam = waarde($event)"
                @keydown.enter="inloggen"
              ></nldd-text-field>
              <nldd-validation-list>
                <nldd-validation-item id="inlog-gebruikersnaam-verplicht" required>
                  Een gebruikersnaam
                </nldd-validation-item>
              </nldd-validation-list>
            </nldd-form-field>
            <nldd-form-field label="Wachtwoord">
              <nldd-password-field
                :value="wachtwoord"
                autocomplete="current-password"
                @input="wachtwoord = waarde($event)"
                @keydown.enter="inloggen"
              ></nldd-password-field>
              <nldd-form-field-help-text>{{ accountTekst(aanbod) }}</nldd-form-field-help-text>
            </nldd-form-field>
            <nldd-inline-dialog
              v-if="state.fout"
              variant="alert"
              text="Inloggen lukt niet"
              :supporting-text="melding(state.fout)"
            ></nldd-inline-dialog>
            <nldd-button-group orientation="horizontal">
              <nldd-button appearance="primary" text="Inloggen" @click="inloggen"></nldd-button>
              <nldd-button appearance="neutral-transparent" text="Annuleren" @click="terug"></nldd-button>
            </nldd-button-group>
          </template>
          <nldd-inline-dialog
            v-else
            variant="alert"
            horizontal-alignment="left"
            text="Inloggen geweigerd"
            :supporting-text="weigerTekst(weigering.reden)"
          >
            <nldd-button slot="actions" appearance="primary" text="Opnieuw proberen" @click="weigering = null"></nldd-button>
            <nldd-button slot="actions" appearance="secondary" text="Terug naar de aanvraag" @click="terug"></nldd-button>
          </nldd-inline-dialog>
        </nldd-container>
      </nldd-simple-section>
    </nldd-page>
  </nldd-shell>
</template>
