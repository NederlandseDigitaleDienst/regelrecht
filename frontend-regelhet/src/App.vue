<script setup lang="ts">
// The shell of the web builds with the login (the combined build and the back
// office): the editor's layout (gedeeld/Schil.vue) with the tabs of the side
// the screen belongs to, the account of the login, and the trial band with the
// date of the clock.
import { computed, onMounted, ref, watch } from 'vue';
import { useRoute, useRouter } from 'vue-router';
import { hoofdletter } from '@regelrecht/frontend-shared/chronolex/waarden';
import { gedeeldApi } from './gedeeld/api';
import { geenToegang, ik } from './gedeeld/ik';
import Schil from './gedeeld/Schil.vue';
import { huidigeKlok as klok } from './gedeeld/klok';
import { actieVan, tabsVan, titelVan } from './gedeeld/schil';
import type { Tab } from './gedeeld/schil';
import { inloggenApi } from './inloggen/api';

const route = useRoute();
const router = useRouter();

async function laadKlok() {
  klok.value = ik.value ? await gedeeldApi.klok().catch(() => null) : null;
}
onMounted(laadKlok);
watch(ik, laadKlok);
// Another account or another tab may have moved the clock meanwhile.
watch(() => route.fullPath, laadKlok);

const naam = computed(() => (ik.value ? ik.value.naam || ik.value.sub : null));
// What the account is here, under its name in the menu: in this app one
// account acts for one cell, and which cell decides what it may do, so the
// role plus that cell is the context the menu is missing. A citizen gets
// none: their identifier is a BSN, and a menu is not the place for it. The
// test accounts are named after their own role ("Beslisser bij gemeente
// Wilgendaal"), and a line that repeats the name above it says nothing, so
// it drops out.
const onder = computed(() => {
  const i = ik.value;
  if (!i || i.burger || !i.niveau) return null;
  const cel = i.cellen[0]?.cel_naam;
  const rol = cel ? `${hoofdletter(i.niveau)} bij ${cel}` : hoofdletter(i.niveau);
  return rol.toLowerCase() === naam.value?.trim().toLowerCase() ? null : rol;
});
// The organization this server runs the process for: the case's executor,
// the same on every screen and for every employee, so it sits in the bar and
// not on a screen. A citizen's side names its service there instead.
const organisatie = ref<string | null>(null);
watch(
  ik,
  async (i) => {
    organisatie.value =
      i && !i.burger
        ? await gedeeldApi
            .casus()
            .then((c) => hoofdletter(c.cel_naam))
            .catch(() => null)
        : null;
  },
  { immediate: true },
);
const rechten = computed<Tab['recht'][]>(() => [
  ...(ik.value?.klok_zetten ? (['klok_zetten'] as const) : []),
  ...(ik.value?.partijen_invoeren ? (['partijen_invoeren'] as const) : []),
]);
</script>

<template>
  <Schil
    :tabs="tabsVan(route)"
    :titel="titelVan(route)"
    :organisatie="organisatie"
    :actie="actieVan(route)"
    :naam="naam"
    :onder="onder"
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
