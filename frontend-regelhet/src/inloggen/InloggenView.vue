<script setup lang="ts">
// Logging in. With Keycloak someone without a session goes there at once and
// comes back to the screen of the link that brought them (stap.ts says when
// the screen stays instead: after logging out, after a failed login, for an
// account this server does not admit). In test mode without Keycloak the
// server offers a test login instead: pick an account (a citizen with a
// made-up number, an employee of a cell, the admin). That screen says it is
// a test setup, because anyone who reaches it can pick any account.
import { computed, onMounted, ref } from 'vue';
import { useRoute, useRouter } from 'vue-router';
import type { AuthStatus } from '@regelrecht/frontend-shared';
import { foutTekst } from '../gedeeld/http';
import { geenToegang, laadIk, startpagina } from '../gedeeld/ik';
import { ALLE_KANTEN, terugNaar } from '../gedeeld/terug';
import { inloggenApi } from './api';
import type { Persona } from './api';
import { personaNaam, personaUitleg } from './personen';
import { stap as kiesStap } from './stap';
import type { Stap } from './stap';
import { wasUitgelogd } from './uitgelogd';

/** The screens a link may lead back to after the login: the build's own sides. */
const props = withDefaults(defineProps<{ naar?: readonly string[] }>(), { naar: () => ALLE_KANTEN });
const route = useRoute();
const router = useRouter();
/** The screen of a link that brought someone here, to go back to after the login. */
const terug = computed(() => terugNaar(route.query.terug, props.naar, ''));
const status = ref<AuthStatus | null>(null);
const personen = ref<Persona[]>([]);
const fout = ref('');
const stap = ref<Stap | null>(null);

/**
 * To Keycloak, and back through this screen to the screen of the link. The
 * way back marks itself (`na=keycloak`): a browser that comes back without a
 * session then sees a message instead of going round again, also where no
 * storage keeps anything.
 */
const naarKeycloak = () =>
  inloggenApi.oidc(router.resolve({ path: route.path, query: { ...(terug.value ? { terug: terug.value } : {}), na: 'keycloak' } }).fullPath);

/** What the server said of a logged-in account: admitted, without a role here, or no answer. */
function toegangVan(wie: unknown): 'ja' | 'rol' | 'storing' {
  if (wie) return 'ja';
  return geenToegang.value?.soort === 'rol' ? 'rol' : 'storing';
}

/** The kind of failure the server put in the address, if one it knows. */
function foutUitAdres(): 'rol' | 'inloggen' | null {
  const f = route.query.fout;
  return f === 'rol' || f === 'inloggen' ? f : null;
}

onMounted(async () => {
  // Read once, whatever follows: a reload after the message goes on to Keycloak.
  const uitgelogd = wasUitgelogd();
  try {
    status.value = await inloggenApi.status();
    if (!status.value.oidc_configured) {
      personen.value = await inloggenApi.testpersonen();
      stap.value = { soort: 'testlogin' };
      return;
    }
    const wie = status.value.authenticated ? await laadIk(true) : null;
    stap.value = kiesStap({
      oidc: true,
      ingelogd: status.value.authenticated,
      toegang: toegangVan(wie),
      fout: foutUitAdres(),
      naKeycloak: route.query.na === 'keycloak',
      uitgelogd,
    });
    if (stap.value.soort === 'doorsturen') naarKeycloak();
    if (stap.value.soort === 'binnen') await router.replace(terug.value || startpagina(wie));
  } catch (e) {
    // No status, no way to tell: the screen says so and sends nobody anywhere.
    fout.value = foutTekst(e);
  }
});

/** Ask everything anew, as after a fresh visit. */
const herlaad = () => window.location.reload();

async function kies(p: Persona) {
  fout.value = '';
  try {
    // A citizen logs in as the simulated DigiD does it: by number and name.
    if (p.soort === 'burger' && p.identiteit) await inloggenApi.digid(p.identiteit, p.naam ?? personaNaam(p));
    else await inloggenApi.testlogin(p, personaNaam(p));
    // Who is in now, for the router; then back to the link's screen. The
    // router sends an account of the other side to its own start.
    const wie = await laadIk(true);
    await router.push(terug.value || startpagina(wie));
  } catch (e) {
    fout.value = foutTekst(e);
  }
}
</script>

<template>
  <nldd-page landmarks="page" sticky-header accessible-label="Inloggen">
    <nldd-top-title-bar slot="header" text="Inloggen" collapse-anchor="inloggen-titel"></nldd-top-title-bar>
    <nldd-simple-section>
      <nldd-title id="inloggen-titel" size="display-3"><h3>Inloggen</h3></nldd-title>
      <nldd-spacer size="16"></nldd-spacer>
      <nldd-container gap="16">
        <nldd-activity-indicator v-if="stap?.soort === 'doorsturen'" text="U gaat verder naar het inloggen" show-text></nldd-activity-indicator>
        <nldd-activity-indicator v-else-if="stap?.soort === 'binnen'" text="Een moment" show-text></nldd-activity-indicator>
        <nldd-inline-dialog
          v-else-if="stap?.soort === 'uitgelogd'"
          icon="logout"
          text="U bent uitgelogd"
          supporting-text="Log opnieuw in om verder te gaan."
        >
          <nldd-button slot="actions" appearance="primary" start-icon="login" text="Opnieuw inloggen" @click="naarKeycloak"></nldd-button>
        </nldd-inline-dialog>
        <nldd-inline-dialog
          v-else-if="stap?.soort === 'mislukt'"
          icon="warning"
          icon-color="critical"
          text="Inloggen is niet gelukt"
          supporting-text="Keycloak heeft u niet ingelogd. Probeer het opnieuw. Lukt het weer niet, vraag dan na of uw account toegang heeft tot deze omgeving."
        >
          <nldd-button slot="actions" appearance="primary" text="Opnieuw proberen" @click="naarKeycloak"></nldd-button>
        </nldd-inline-dialog>
        <nldd-inline-dialog
          v-else-if="stap?.soort === 'geen_toegang'"
          icon="warning"
          icon-color="critical"
          text="Geen toegang"
          supporting-text="U bent ingelogd, maar uw account heeft geen rol in deze omgeving. Opnieuw inloggen met hetzelfde account helpt niet."
        >
          <nldd-button slot="actions" appearance="secondary" text="Met een ander account inloggen" @click="inloggenApi.uitloggen()"></nldd-button>
        </nldd-inline-dialog>
        <nldd-inline-dialog
          v-else-if="stap?.soort === 'storing'"
          icon="warning"
          icon-color="critical"
          :text="geenToegang?.kop ?? 'De server is niet bereikbaar'"
          :supporting-text="geenToegang?.reden ?? 'De server geeft nu geen antwoord. Probeer het over een moment opnieuw.'"
        >
          <nldd-button slot="actions" appearance="primary" text="Opnieuw proberen" @click="herlaad"></nldd-button>
        </nldd-inline-dialog>
        <template v-else-if="stap?.soort === 'testlogin'">
          <nldd-banner
            variant="accent"
            text="Testomgeving: kies een account"
            supporting-text="Hier logt u in zonder wachtwoord, als een van de accounts hieronder. Dat kan alleen in de testmodus zonder Keycloak. Elk account is verzonnen."
          ></nldd-banner>
          <nldd-list appearance="box-base" accessible-label="Testaccounts">
            <nldd-list-item v-for="p in personen" :key="p.sub" button @click="kies(p)">
              <nldd-text-cell :text="personaNaam(p)" :supporting-text="personaUitleg(p)"></nldd-text-cell>
              <nldd-icon-cell icon="chevron-right" color="secondary"></nldd-icon-cell>
            </nldd-list-item>
          </nldd-list>
        </template>
        <nldd-inline-dialog v-if="fout" icon="warning" icon-color="critical" text="Inloggen lukt niet" :supporting-text="fout"></nldd-inline-dialog>
      </nldd-container>
    </nldd-simple-section>
  </nldd-page>
</template>
