<script setup lang="ts">
// The shell of the app: the band that says this is a trial, the bar with the
// name of the process and who is logged in, and the screen of the route.
import { computed, onMounted, ref, watch } from 'vue';
import { useRoute, useRouter } from 'vue-router';
import { gedeeldApi } from './gedeeld/api';
import { geenToegang, ik, startpagina } from './gedeeld/ik';
import ProefStrook from './gedeeld/ProefStrook.vue';
import type { Casus, Klok } from './gedeeld/types';
import { inloggenApi } from './inloggen/api';

const route = useRoute();
const router = useRouter();
const klok = ref<Klok | null>(null);
const casus = ref<Casus | null>(null);

async function laad() {
  if (!ik.value) {
    klok.value = null;
    casus.value = null;
    return;
  }
  [klok.value, casus.value] = await Promise.all([gedeeldApi.klok(), gedeeldApi.casus()]).catch(() => [null, null]);
}
onMounted(laad);
watch(ik, laad);
// The admin sets the clock on its own screen; the band follows.
watch(
  () => route.fullPath,
  () => ik.value && gedeeldApi.klok().then((k) => (klok.value = k)).catch(() => undefined),
);

const titel = computed(() => (casus.value?.titel ? `Wetsproces · ${casus.value.titel}` : 'Wetsproces'));
const wie = computed(() => ik.value?.naam || ik.value?.sub || '');
const thuis = computed(() => startpagina(ik.value));
</script>

<template>
  <nldd-shell>
    <nldd-page>
      <nldd-container slot="header" gap="0">
        <ProefStrook :klok="klok" />
        <nldd-top-navigation-bar no-logo :website-title="titel" :website-href="thuis">
          <nldd-menu-bar v-if="ik" slot="utility" accessible-label="Account">
            <nldd-menu-bar-item icon="account" :text="wie"></nldd-menu-bar-item>
            <nldd-menu-bar-item icon="logout" text="Uitloggen" @select="inloggenApi.uitloggen()"></nldd-menu-bar-item>
          </nldd-menu-bar>
        </nldd-top-navigation-bar>
      </nldd-container>
      <nldd-simple-section v-if="geenToegang && route.name !== 'inloggen'">
        <nldd-inline-dialog icon="warning" text="Geen toegang" :supporting-text="geenToegang">
          <nldd-button slot="actions" appearance="secondary" text="Opnieuw inloggen" @click="router.push('/inloggen')"></nldd-button>
        </nldd-inline-dialog>
      </nldd-simple-section>
      <RouterView v-else />
    </nldd-page>
  </nldd-shell>
</template>
