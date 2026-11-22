<script setup lang="ts">
// The chronicle of a cell the employee may read: every record, in the order
// it was recorded. Nothing in it changes or goes away. Laid out as the
// editor's library: the cells in the sidebar, the chronicle of the chosen one
// in the main pane beside it. Every record opens in a sheet with what it
// holds; a submission, or a decision on one, also leads to its panel in the
// work queue.
import { computed, onMounted, ref } from 'vue';
import { useRouter } from 'vue-router';
import { hoofdletter } from '@regelrecht/frontend-shared/chronolex/waarden';
import { longDate } from '../gedeeld/datum';
import Gegevens from '../gedeeld/Gegevens.vue';
import { artikelNaam, isBesluit, isIndiening, omschrijving, rolVan } from '../gedeeld/keten';
import type { Gram } from '../gedeeld/types';
import { useBackoffice } from './useBackoffice';

const props = defineProps<{ cel?: string }>();
const router = useRouter();
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
function naarWerkvoorraad(g: Gram) {
  const op = indieningVan(g);
  open.value = null;
  if (op && gekozen.value) void router.push({ name: 'behandelen', params: { cel: gekozen.value.cel, id: op.id } });
}
</script>

<template>
  <nldd-navigation-split-view @back="router.push('/backoffice/kroniek')">
    <nldd-split-view-pane slot="primary-sidebar" has-content>
      <nldd-page sticky-header accessible-label="Kroniek">
        <nldd-top-title-bar slot="header" text="Kroniek" heading-level="3" collapse-anchor="kroniek-titel"></nldd-top-title-bar>
        <nldd-simple-section width="full">
          <nldd-title id="kroniek-titel" size="display-3"><h3>Kroniek</h3></nldd-title>
          <nldd-text color="secondary">Alles wat een cel vastlegde, in de volgorde waarin het gebeurde.</nldd-text>
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
    </nldd-split-view-pane>

    <nldd-split-view-pane slot="main" :has-content="gekozen ? true : undefined">
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
                :supporting-text="`${longDate(g.recorded_on)} · ${artikelNaam(g.basis)}${g.refers_to ? ` · verwijst naar ${noem(g.refers_to)}` : ''}`"
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
    </nldd-split-view-pane>
  </nldd-navigation-split-view>

  <Teleport to="body">
    <nldd-sheet width="720px" :open="open ? true : undefined" @close="open = null">
      <nldd-page>
        <nldd-container slot="header" padding="16">
          <nldd-top-title-bar
            heading-level="2"
            :text="open && casus ? `${omschrijf(open)} van ${longDate(open.recorded_on)}` : ''"
            dismiss-text="Sluiten"
          ></nldd-top-title-bar>
        </nldd-container>
        <nldd-simple-section v-if="open && casus">
          <nldd-container gap="16">
            <nldd-text color="secondary">
              {{ `${artikelNaam(open.basis)}${open.refers_to ? ` · verwijst naar ${noem(open.refers_to)}` : ''}` }}
            </nldd-text>
            <nldd-title size="display-6"><h3>Gegevens</h3></nldd-title>
            <Gegevens :waarden="open.fields" :teksten="vormen?.teksten" :noem="noem" label="Gegevens van het stuk" />
            <template v-if="open.outputs">
              <nldd-title size="display-6"><h3>Uitkomst</h3></nldd-title>
              <Gegevens :waarden="open.outputs" :teksten="vormen?.teksten" label="Uitkomst van het besluit" />
            </template>
            <nldd-button-group v-if="indieningVan(open)" orientation="horizontal">
              <nldd-button appearance="primary" text="Open in de werkvoorraad" @click="naarWerkvoorraad(open)"></nldd-button>
            </nldd-button-group>
          </nldd-container>
        </nldd-simple-section>
      </nldd-page>
    </nldd-sheet>
  </Teleport>
</template>
