<script setup lang="ts">
// The shell of the citizen's entry: the band that says this is a trial, the
// bar with the name of the process and who is in, and the screen of the
// route. The same for the app and the web; what differs is the transport.
import { inject, ref, watch } from 'vue';
import { useRoute, useRouter } from 'vue-router';
import { foutTekst } from '../gedeeld/http';
import ProefStrook from '../gedeeld/ProefStrook.vue';
import { VERVOER } from './injectie';
import type { Toegang } from './vervoer';

const vervoer = inject(VERVOER);
const route = useRoute();
const router = useRouter();
const toegang = ref<Toegang>({ binnen: false, naam: null });
const fout = ref('');

async function ververs() {
  if (vervoer) toegang.value = await vervoer.toegang();
}
watch(() => route.fullPath, ververs, { immediate: true });

async function afmelden() {
  if (!vervoer) return;
  try {
    await vervoer.afmelden.doe();
    await ververs();
    await router.push('/toegang');
  } catch (e) {
    fout.value = foutTekst(e);
  }
}
</script>

<template>
  <nldd-shell>
    <nldd-page>
      <nldd-container slot="header" gap="0">
        <ProefStrook :klok="null" />
        <nldd-top-navigation-bar no-logo website-title="Wetsproces">
          <nldd-menu-bar v-if="toegang.binnen" slot="utility" accessible-label="Account">
            <nldd-menu-bar-item icon="account" :text="toegang.naam ?? ''"></nldd-menu-bar-item>
            <nldd-menu-bar-item icon="logout" :text="vervoer?.afmelden.tekst" @select="afmelden"></nldd-menu-bar-item>
          </nldd-menu-bar>
        </nldd-top-navigation-bar>
      </nldd-container>
      <nldd-simple-section v-if="fout">
        <nldd-inline-dialog icon="warning" icon-color="critical" text="Afmelden lukt niet" :supporting-text="fout"></nldd-inline-dialog>
      </nldd-simple-section>
      <RouterView />
      <nldd-simple-section v-if="toegang.binnen && vervoer?.onderschrift()">
        <nldd-text color="secondary">{{ vervoer.onderschrift() }}</nldd-text>
      </nldd-simple-section>
    </nldd-page>
  </nldd-shell>
</template>
