<script setup lang="ts">
// The chronicle of a cell the employee may read: every record, in the order
// it was recorded. Nothing in it changes or goes away. Laid out as the
// editor's library: the cells in the sidebar, the chronicle of the chosen one
// in the main pane beside it. Every record opens in a sheet with what it
// holds; a submission, or a decision on one, also leads to its panel in the
// work queue. A decision of the executor shows the full chain of why it came
// out so, as the server explains it (`GET /api/backoffice/waarom/{id}`).
// A record named in the address (`?stuk=<id>`), as the timeline of an
// application links to it, opens in that sheet.
import { computed, onMounted, ref, watch } from 'vue';
import { useRoute, useRouter } from 'vue-router';
import { hoofdletter } from '@regelrecht/frontend-shared/chronolex/waarden';
import { longDate } from '../gedeeld/datum';
import Gegevens from '../gedeeld/Gegevens.vue';
import { foutTekst } from '../gedeeld/http';
import { artikelNaam, isBesluit, isIndiening, omschrijving, rolVan } from '../gedeeld/keten';
import type { Gram, Waarom } from '../gedeeld/types';
import { doorHetSysteem } from '../gedeeld/vervolg';
import VolledigeKeten from '../gedeeld/VolledigeKeten.vue';
import { backofficeApi } from './api';
import { useBackoffice } from './useBackoffice';

const props = defineProps<{ cel?: string }>();
const router = useRouter();
const route = useRoute();
const { casus, vormen, kronieken, fout, laden, laad } = useBackoffice();
onMounted(laad);

const gekozen = computed(() => kronieken.value.find((k) => k.cel === props.cel));
const kies = (cel: string) => router.push({ name: 'kroniek', params: { cel } });
/** The record `id` of the chosen chronicle, in words, by its number in the chronicle. */
const noem = (id: string) => {
  const i = gekozen.value?.grammen.findIndex((g) => g.id === id) ?? -1;
  const g = gekozen.value?.grammen[i];
  if (!g) return 'een stuk in een andere kroniek';
  return casus.value ? `nr. ${i + 1}, ${omschrijf(g).toLowerCase()}` : `nr. ${i + 1}`;
};

/** A record of the chosen chronicle in words, by what the server says it is. */
const omschrijf = (g: Gram) =>
  casus.value && gekozen.value ? omschrijving(g, casus.value, rolVan(g, gekozen.value.stand)) : g.kind;

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
/** The submission a record is or answers to, in this chronicle: what its panel in the work queue shows. */
const indieningVan = (g: Gram): Gram | undefined => {
  const k = gekozen.value;
  if (!k) return undefined;
  if (isIndiening(g, k.stand)) return g;
  // A decision leads to the submission it answers; a recorded fact to nothing.
  if (!isBesluit(g, k.stand)) return undefined;
  const op = k.grammen.find((x) => x.id === g.refers_to);
  return op && isIndiening(op, k.stand) ? op : undefined;
};
/** Why the decision in the sheet came out so, once loaded; for a decision of the executor only. */
const waarom = ref<Waarom | null>(null);
const waaromFout = ref('');
watch(open, async (g) => {
  waarom.value = null;
  waaromFout.value = '';
  const k = gekozen.value;
  if (!g || !k || !isBesluit(g, k.stand) || g.recording_actor !== casus.value?.cel) return;
  try {
    const uitleg = await backofficeApi.waarom(g.id);
    if (open.value?.id === g.id) waarom.value = uitleg;
  } catch (e) {
    if (open.value?.id === g.id) waaromFout.value = foutTekst(e);
  }
});
/** Close the sheet, and drop the record from the address so it does not open again. */
function sluit() {
  open.value = null;
  if (route.query.stuk) void router.replace({ query: {} });
}
function naarWerkvoorraad(g: Gram) {
  const op = indieningVan(g);
  open.value = null;
  if (op && gekozen.value) void router.push({ name: 'behandelen', params: { cel: gekozen.value.cel, id: op.id } });
}
</script>

<template>
  <nldd-navigation-split-view @back="router.push('/backoffice/kroniek')">
    <nldd-page slot="primary-sidebar" sticky-header accessible-label="Kroniek">
        <nldd-top-title-bar slot="header" text="Kroniek" heading-level="3" collapse-anchor="kroniek-titel"></nldd-top-title-bar>
        <nldd-simple-section width="full">
          <nldd-title id="kroniek-titel" size="display-3"><h3>Kroniek</h3></nldd-title>
          <nldd-text color="secondary"><p>Alles wat een cel vastlegde, in de volgorde waarin het gebeurde.</p></nldd-text>
          <nldd-spacer size="16"></nldd-spacer>
          <nldd-activity-indicator v-if="laden" text="De kronieken laden" show-text></nldd-activity-indicator>
          <nldd-inline-dialog v-else-if="fout" icon="warning" icon-color="critical" text="De kroniek laadt niet" :supporting-text="fout"></nldd-inline-dialog>
          <nldd-inline-dialog v-else-if="kronieken.length === 0" icon="info" text="U mag geen kroniek lezen"></nldd-inline-dialog>
          <nldd-list v-else appearance="simple" accessible-label="Cellen">
            <nldd-list-item
              v-for="k in kronieken"
              :key="k.cel"
              size="md"
              button
              :current="k.cel === gekozen?.cel || undefined"
              @click="kies(k.cel)"
            >
              <nldd-icon-cell size="20"><nldd-icon icon="history"></nldd-icon></nldd-icon-cell>
              <nldd-spacer-cell size="8"></nldd-spacer-cell>
              <nldd-text-cell :text="hoofdletter(k.cel_naam)" :supporting-text="`${k.grammen.length} vastgelegd`"></nldd-text-cell>
              <nldd-spacer-cell size="8"></nldd-spacer-cell>
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
        <nldd-simple-section width="full">
          <nldd-title id="cel-kroniek-titel" size="display-3"><h3>{{ hoofdletter(gekozen.cel_naam) }}</h3></nldd-title>
          <nldd-spacer size="16"></nldd-spacer>
          <nldd-list v-if="gekozen.grammen.length" appearance="box-base" :accessible-label="`Kroniek van ${gekozen.cel_naam}`">
            <nldd-list-item v-for="(g, i) in gekozen.grammen" :key="g.id" size="sm" button @click="open = g">
              <nldd-text-cell
                :overline="`${i + 1}`"
                :text="omschrijf(g)"
                :supporting-text="[longDate(g.recorded_on), artikelNaam(g.basis), g.refers_to ? `verwijst naar ${noem(g.refers_to)}` : '', doorHetSysteem(g.id, gekozen.door_het_systeem)].filter(Boolean).join(' · ')"
              ></nldd-text-cell>
              <nldd-spacer-cell size="8"></nldd-spacer-cell>
              <nldd-icon-cell size="20"><nldd-icon icon="chevron-right"></nldd-icon></nldd-icon-cell>
            </nldd-list-item>
          </nldd-list>
          <nldd-inline-dialog v-else icon="info" text="De kroniek is leeg"></nldd-inline-dialog>
        </nldd-simple-section>
      </nldd-page>
      <nldd-page v-else landmarks="page">
        <nldd-simple-section width="full">
          <nldd-inline-dialog text="Kies een cel" supporting-text="De kroniek van een cel opent hier."></nldd-inline-dialog>
        </nldd-simple-section>
      </nldd-page>
    </div>
  </nldd-navigation-split-view>

  <Teleport to="body">
    <nldd-sheet width="720px" :open="open ? true : undefined" @close="sluit">
      <nldd-page>
        <nldd-top-title-bar
          slot="header"
          heading-level="2"
          :text="open && casus ? `${omschrijf(open)} van ${longDate(open.recorded_on)}` : ''"
          dismiss-text="Sluit"
        ></nldd-top-title-bar>
        <nldd-simple-section v-if="open && casus">
          <nldd-container gap="16">
            <nldd-text color="secondary">
              <p>
                {{ `${artikelNaam(open.basis)}${open.refers_to ? ` · verwijst naar ${noem(open.refers_to)}` : ''}` }}
              </p>
            </nldd-text>
            <nldd-title size="display-6"><h3>Gegevens</h3></nldd-title>
            <Gegevens :waarden="open.fields" :teksten="vormen?.teksten" :volgorde="vormen?.volgorde" :noem="noem" label="Gegevens van het stuk" />
            <template v-if="open.outputs">
              <nldd-title size="display-6"><h3>Uitkomst</h3></nldd-title>
              <Gegevens :waarden="open.outputs" :teksten="vormen?.teksten" :volgorde="vormen?.volgorde" label="Uitkomst van het besluit" />
            </template>
            <VolledigeKeten v-if="waarom?.keten" :keten="waarom.keten" :casus="casus" :teksten="vormen?.teksten" />
            <nldd-inline-dialog v-if="waaromFout" icon="warning" text="De keten van dit besluit laadt niet" :supporting-text="waaromFout"></nldd-inline-dialog>
            <nldd-button-group v-if="indieningVan(open)">
              <nldd-button appearance="primary" text="Open in de werkvoorraad" @click="naarWerkvoorraad(open)"></nldd-button>
            </nldd-button-group>
          </nldd-container>
        </nldd-simple-section>
      </nldd-page>
    </nldd-sheet>
  </Teleport>
</template>
