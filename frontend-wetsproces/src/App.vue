<script setup lang="ts">
// The shell of the web builds with the login (the combined build and the back
// office): the editor's layout (gedeeld/Schil.vue) with the tabs of the side
// the screen belongs to, the account of the login, and the trial band with the
// date of the clock.
import { computed, onMounted, ref, watch } from 'vue';
import { useRoute, useRouter } from 'vue-router';
import { gedeeldApi } from './gedeeld/api';
import { geenToegang, ik } from './gedeeld/ik';
import Schil from './gedeeld/Schil.vue';
import { tabsVan } from './gedeeld/schil';
import type { Tab } from './gedeeld/schil';
import type { Klok } from './gedeeld/types';
import { inloggenApi } from './inloggen/api';

const route = useRoute();
const router = useRouter();
const klok = ref<Klok | null>(null);

async function laadKlok() {
  klok.value = ik.value ? await gedeeldApi.klok().catch(() => null) : null;
}
onMounted(laadKlok);
watch(ik, laadKlok);
// The admin sets the clock on its own screen; the band follows.
watch(() => route.fullPath, laadKlok);

const naam = computed(() => (ik.value ? ik.value.naam || ik.value.sub : null));
const rechten = computed<Tab['recht'][]>(() => [
  ...(ik.value?.klok_zetten ? (['klok_zetten'] as const) : []),
  ...(ik.value?.partijen_invoeren ? (['partijen_invoeren'] as const) : []),
]);
</script>

<template>
  <Schil
    :tabs="tabsVan(route)"
    :naam="naam"
    :klok="klok"
    :rechten="rechten"
    afmeld-tekst="Uitloggen"
    @afmelden="inloggenApi.uitloggen()"
  >
    <nldd-page v-if="geenToegang && route.name !== 'inloggen'" landmarks="page">
      <nldd-simple-section width="full">
        <nldd-inline-dialog icon="warning" :text="geenToegang.kop" :supporting-text="geenToegang.reden">
          <nldd-button slot="actions" appearance="secondary" text="Opnieuw inloggen" @click="router.push('/inloggen')"></nldd-button>
        </nldd-inline-dialog>
      </nldd-simple-section>
    </nldd-page>
    <RouterView v-else />
  </Schil>
</template>
