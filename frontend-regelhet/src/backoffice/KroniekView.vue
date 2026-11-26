<script setup lang="ts">
// The chronicle of a cell the employee may read: every record, in the order
// it was recorded. Nothing in it changes or goes away. Laid out as the
// editor's library: the cells in the sidebar, the chronicle of the chosen one
// in the main pane beside it. Every record opens in a sheet with what it
// holds (StukSheet.vue). A record named in the address (`?stuk=<id>`) opens
// in that sheet.
import { computed, onMounted, ref, watch } from 'vue';
import { useRoute, useRouter } from 'vue-router';
import { hoofdletter } from '@regelrecht/frontend-shared/chronolex/waarden';
import { longDate } from '../gedeeld/datum';
import { artikelNaam } from '../gedeeld/keten';
import type { Gram } from '../gedeeld/types';
import { doorHetSysteem } from '../gedeeld/vervolg';
import { noemStuk, omschrijfStuk } from './kroniekStuk';
import StukSheet from './StukSheet.vue';
import { useBackoffice } from './useBackoffice';

const props = defineProps<{ cel?: string }>();
const router = useRouter();
const route = useRoute();
const { casus, vormen, kronieken, fout, laden, laad } = useBackoffice();
onMounted(laad);

const gekozen = computed(() => kronieken.value.find((k) => k.cel === props.cel));
const kies = (cel: string) => router.push({ name: 'kroniek', params: { cel } });
const noem = (id: string) => noemStuk(id, casus.value, gekozen.value);
const omschrijf = (g: Gram) => omschrijfStuk(g, casus.value, gekozen.value);
// Newest first; each keeps its number in the chronicle, which a later record
// names when it refers back ("verwijst naar nr. 1").
const nieuwsteEerst = computed(() => (gekozen.value?.grammen ?? []).map((g, i) => ({ g, nummer: i + 1 })).reverse());

/** The record open in the sheet. */
const open = ref<Gram | null>(null);
// The record the address names, once the chronicle it is in has loaded.
watch(
  () => [route.query.stuk, gekozen.value] as const,
  ([stuk, k]) => {
    const g = typeof stuk === 'string' ? k?.grammen.find((x) => x.id === stuk) : undefined;
    if (g) open.value = g;
  },
  { immediate: true },
);
/** Close the sheet, and drop the record from the address so it does not open again. */
function sluit() {
  open.value = null;
  if (route.query.stuk) void router.replace({ query: {} });
}
</script>

<template>
  <nldd-navigation-split-view @back="router.push('/backoffice/kroniek')">
    <nldd-page slot="primary-sidebar" sticky-header accessible-label="Kroniek">
        <nldd-top-title-bar slot="header" text="Kroniek" heading-level="3" collapse-anchor="kroniek-titel"></nldd-top-title-bar>
        <nldd-simple-section width="full">
          <nldd-title id="kroniek-titel" size="display-3">
            <h3>Kroniek</h3>
            <nldd-icon-button slot="end" appearance="neutral-base" size="sm" icon="info" tooltip-timing="never" text="Over Kroniek">
              <nldd-popover slot="popup" role="region" accessible-label="Over Kroniek">
                <nldd-container padding="16">
                  <nldd-text><p>Alles wat een cel vastlegde, in de volgorde waarin het gebeurde.</p></nldd-text>
                </nldd-container>
              </nldd-popover>
            </nldd-icon-button>
          </nldd-title>
          <nldd-spacer size="16"></nldd-spacer>
          <nldd-activity-indicator v-if="laden" text="De kronieken laden" show-text></nldd-activity-indicator>
          <nldd-inline-dialog v-else-if="fout" icon="warning" icon-color="critical" text="De kroniek laadt niet" :supporting-text="fout"></nldd-inline-dialog>
          <nldd-inline-dialog v-else-if="kronieken.length === 0" text="U mag geen kroniek lezen"></nldd-inline-dialog>
          <nldd-list v-else appearance="simple" accessible-label="Cellen">
            <nldd-list-item
              v-for="k in kronieken"
              :key="k.cel"
              size="md"
              button
              :current="k.cel === gekozen?.cel || undefined"
              @click="kies(k.cel)"
            >
              <nldd-text-cell :text="hoofdletter(k.cel_naam)"></nldd-text-cell>
              <nldd-spacer-cell size="8"></nldd-spacer-cell>
              <nldd-text-cell :text="String(k.grammen.length)" color="secondary" width="fit-content" horizontal-alignment="right"></nldd-text-cell>
              <nldd-spacer-cell size="2"></nldd-spacer-cell>
              <nldd-icon-cell size="20"><nldd-icon icon="chevron-right"></nldd-icon></nldd-icon-cell>
            </nldd-list-item>
          </nldd-list>
        </nldd-simple-section>
      </nldd-page>

    <div slot="main" :empty="gekozen ? undefined : true">
      <nldd-page v-if="gekozen && casus" landmarks="page" sticky-header :accessible-label="`Kroniek van ${gekozen.cel_naam}`">
        <nldd-top-title-bar
          slot="header"
          :text="hoofdletter(gekozen.cel_naam)"
          back-text="Kroniek"
          collapse-anchor="cel-kroniek-titel"
        ></nldd-top-title-bar>
        <nldd-simple-section>
          <nldd-title id="cel-kroniek-titel" size="display-3"><h3>{{ hoofdletter(gekozen.cel_naam) }}</h3></nldd-title>
          <nldd-spacer size="16"></nldd-spacer>
          <nldd-list v-if="gekozen.grammen.length" appearance="simple" :accessible-label="`Kroniek van ${gekozen.cel_naam}`">
            <nldd-list-item v-for="{ g, nummer } in nieuwsteEerst" :key="g.id" button @click="open = g">
              <nldd-text-cell
                :text="`${nummer}: ${omschrijf(g)}`"
                :supporting-text="[longDate(g.recorded_on), artikelNaam(g.basis), g.refers_to ? `verwijst naar ${noem(g.refers_to)}` : '', doorHetSysteem(g.id, gekozen.door_het_systeem)].filter(Boolean).join(' · ')"
              ></nldd-text-cell>
              <nldd-spacer-cell size="8"></nldd-spacer-cell>
              <nldd-icon-cell size="20"><nldd-icon icon="chevron-right"></nldd-icon></nldd-icon-cell>
            </nldd-list-item>
          </nldd-list>
          <nldd-inline-dialog v-else text="De kroniek is leeg"></nldd-inline-dialog>
        </nldd-simple-section>
      </nldd-page>
      <nldd-page v-else landmarks="page">
        <nldd-simple-section>
          <nldd-inline-dialog text="Kies een cel" supporting-text="De kroniek van een cel opent hier."></nldd-inline-dialog>
        </nldd-simple-section>
      </nldd-page>
    </div>
  </nldd-navigation-split-view>

  <StukSheet :stuk="open" :kroniek="gekozen" :casus="casus" :vormen="vormen" @sluit="sluit" />
</template>
