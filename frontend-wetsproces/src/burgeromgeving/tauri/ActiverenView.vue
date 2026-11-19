<script setup lang="ts">
// Activating the app in the demo: pick a made-up citizen number, and the core
// makes a key, asks the test Activatiedienst of the server a credential for
// it, checks the Stelseldocument and starts a session. Nothing of that comes
// here; the screen gets the core's status back.
import { computed, ref } from 'vue';
import { useRoute, useRouter } from 'vue-router';
import { invoke } from '../vervoer/tauri';
import { melding } from './kern';
import type { KernFout } from './kern';
import { appStatus, laadStatus, opslagTekst } from './status';

const route = useRoute();
const router = useRouter();
const bezig = ref('');
const fout = ref('');

const reden = computed(() => {
  if (route.query.reden === 'credential_expired' || appStatus.value?.credential_expired) {
    return 'De activering van deze app is verlopen. Activeer de app opnieuw om verder te gaan.';
  }
  if (route.query.reden === 'credential_refused') {
    return 'De server neemt de activering van deze app niet meer aan. Activeer de app opnieuw om verder te gaan.';
  }
  return '';
});

async function activeer(bsn: string) {
  bezig.value = bsn;
  fout.value = '';
  try {
    appStatus.value = await invoke('activeren', { bsn });
    await router.push('/burger');
  } catch (e) {
    fout.value = melding(e as KernFout);
    await laadStatus(invoke).catch(() => undefined);
  } finally {
    bezig.value = '';
  }
}
</script>

<template>
  <nldd-simple-section width="640px">
    <nldd-title slot="header" heading-level="1" text="App activeren"></nldd-title>
    <nldd-container gap="16">
      <nldd-inline-dialog v-if="reden" icon="warning" text="Opnieuw activeren" :supporting-text="reden"></nldd-inline-dialog>
      <nldd-inline-dialog
        v-if="appStatus?.config_problem"
        icon="warning"
        icon-color="critical"
        text="Geen server ingesteld"
        :supporting-text="melding({ kind: 'not_configured', detail: appStatus.config_problem })"
      ></nldd-inline-dialog>
      <template v-else-if="appStatus">
        <nldd-banner
          variant="accent"
          text="Testomgeving: kies een verzonnen burger"
          supporting-text="In de testmodus geeft de demo-Activatiedienst van de server deze app een bewijs voor een fictief BSN uit de testreeks. Niemand controleert wie u bent. Een echte app krijgt dat bewijs pas nadat u zich bij een Activatiedienst hebt geïdentificeerd."
        ></nldd-banner>
        <nldd-text color="secondary">Server: {{ appStatus.server }}</nldd-text>
        <nldd-list appearance="box-base" accessible-label="Fictieve burgers">
          <nldd-list-item v-for="bsn in appStatus.test_bsns" :key="bsn" button @click="activeer(bsn)">
            <nldd-text-cell :text="`Burger met BSN ${bsn}`" supporting-text="Het BSN is fictief."></nldd-text-cell>
            <nldd-icon-cell icon="chevron-right" color="secondary"></nldd-icon-cell>
          </nldd-list-item>
        </nldd-list>
        <nldd-activity-indicator v-if="bezig"></nldd-activity-indicator>
        <nldd-text color="secondary">{{ opslagTekst(appStatus) }}</nldd-text>
      </template>
      <nldd-inline-dialog v-if="fout" icon="warning" icon-color="critical" text="Activeren lukt niet" :supporting-text="fout"></nldd-inline-dialog>
    </nldd-container>
  </nldd-simple-section>
</template>
