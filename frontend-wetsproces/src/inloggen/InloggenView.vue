<script setup lang="ts">
// Logging in. With Keycloak the button goes there and back. In test mode
// without Keycloak the server offers a test login instead: pick an account
// (a citizen with a made-up number, an employee of a cell, the admin). That
// screen says it is a test setup, because anyone who reaches it can pick any
// account.
import { onMounted, ref } from 'vue';
import { useRouter } from 'vue-router';
import type { AuthStatus } from '@regelrecht/frontend-shared';
import { foutTekst } from '../gedeeld/http';
import { laadIk, startpagina } from '../gedeeld/ik';
import { inloggenApi } from './api';
import type { Persona } from './api';
import { personaNaam, personaUitleg } from './personen';

const router = useRouter();
const status = ref<AuthStatus | null>(null);
const personen = ref<Persona[]>([]);
const fout = ref('');

onMounted(async () => {
  try {
    status.value = await inloggenApi.status();
    if (!status.value.oidc_configured) personen.value = await inloggenApi.testpersonen();
  } catch (e) {
    fout.value = foutTekst(e);
  }
});

async function kies(p: Persona) {
  fout.value = '';
  try {
    await inloggenApi.testlogin(p, personaNaam(p));
    await router.push(startpagina(await laadIk(true)));
  } catch (e) {
    fout.value = foutTekst(e);
  }
}
</script>

<template>
  <nldd-simple-section width="640px">
    <nldd-title slot="header" heading-level="1" text="Inloggen"></nldd-title>
    <nldd-container gap="16">
      <template v-if="status?.oidc_configured">
        <nldd-text>Log in met uw account. U komt daarna terug op deze pagina.</nldd-text>
        <nldd-button-group orientation="horizontal">
          <nldd-button appearance="primary" start-icon="login" text="Inloggen" @click="inloggenApi.oidc('/')"></nldd-button>
        </nldd-button-group>
      </template>
      <template v-else-if="status">
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
</template>
