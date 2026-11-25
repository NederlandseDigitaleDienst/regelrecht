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
import { actieVan, gereedschapVan, menuactie, tabsVan, titelVan } from './gedeeld/schil';
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
// Where the account works, under its name in the menu: in this app one
// account acts for one cell, which the name does not say. The role does not
// go there too: the test accounts are named after it ("Beheerder"), so it
// would say the same thing twice. A citizen gets none: their identifier is a
// BSN, and a menu is not the place for it. A name that already names the
// cell ("Beslisser bij gemeente Wilgendaal") needs no line under it.
const onder = computed(() => {
  const i = ik.value;
  if (!i || i.burger || !i.niveau) return null;
  const cel = i.partij?.cel_naam ?? i.cellen[0]?.cel_naam;
  if (!cel || naam.value?.toLowerCase().includes(cel.toLowerCase())) return null;
  return hoofdletter(cel);
});
// The executor this server runs the process for,
// the same on every screen and for every employee, so it sits in the bar and
// not on a screen; a side without tabs, as a citizen's, names it under its title.
const uitvoerder = ref<string | null>(null);
watch(
  ik,
  async (i) => {
    // On the server of an external party, that party: its staff work for
    // it, not for the executor of the case.
    uitvoerder.value = i?.partij
      ? hoofdletter(i.partij.cel_naam)
      : i
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
  ...(ik.value?.partijen_invoeren && !ik.value.partij ? (['partijen_invoeren'] as const) : []),
  ...(ik.value?.partijen_invoeren && ik.value.partij ? (['partij'] as const) : []),
  ...(ik.value && !ik.value.partij ? (['uitvoerder'] as const) : []),
]);
</script>

<template>
  <Schil
    :tabs="tabsVan(route)"
    :titel="titelVan(route)"
    :uitvoerder="uitvoerder"
    :actie="actieVan(route)"
    :menu="menuactie"
    :gereedschap="gereedschapVan(route, rechten)"
    :naam="naam"
    :onder="onder"
    :klok="klok"
    :rechten="rechten"
    afmeld-tekst="Uitloggen"
    @afmelden="inloggenApi.uitloggen()"
  >
    <nldd-page v-if="geenToegang && route.name !== 'inloggen'" landmarks="page">
      <nldd-simple-section>
        <nldd-inline-dialog icon="warning" :text="geenToegang.kop" :supporting-text="geenToegang.reden">
          <nldd-button slot="actions" appearance="secondary" text="Opnieuw inloggen" @click="router.push('/inloggen')"></nldd-button>
        </nldd-inline-dialog>
      </nldd-simple-section>
    </nldd-page>
    <RouterView v-else />
  </Schil>
</template>
