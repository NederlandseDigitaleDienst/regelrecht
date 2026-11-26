<script setup lang="ts">
// The shell of the citizen's entry: the editor's layout (gedeeld/Schil.vue)
// with the citizen's tabs, who is in and how to leave, and the screen of the
// route. The same for the app and the web; what differs is the transport.
import { inject, ref, watch } from 'vue';
import { useRoute, useRouter } from 'vue-router';
import { foutTekst } from '../gedeeld/http';
import Schil from '../gedeeld/Schil.vue';
import { actieVan, tabsVan, titelVan } from '../gedeeld/schil';
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
  <Schil
    :tabs="tabsVan(route)"
    :titel="titelVan(route)"
    :actie="actieVan(route)"
    :naam="toegang.binnen ? toegang.naam || 'Burger' : null"
    :afmeld-tekst="vervoer?.afmelden.tekst"
    @afmelden="afmelden"
  >
    <RouterView />
    <template v-if="fout || (toegang.binnen && vervoer?.onderschrift())" #voet>
      <nldd-inline-dialog v-if="fout" icon="warning" icon-color="critical" text="Afmelden lukt niet" :supporting-text="fout"></nldd-inline-dialog>
      <nldd-text v-else color="secondary"><p>{{ vervoer?.onderschrift() }}</p></nldd-text>
    </template>
  </Schil>
</template>
