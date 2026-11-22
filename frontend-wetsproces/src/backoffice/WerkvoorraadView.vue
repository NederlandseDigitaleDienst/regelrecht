<script setup lang="ts">
// The work queue of the executor, the one cell the employee works for: the
// submissions on which no decision was taken yet, and the facts it records
// itself. Every employee opens a submission to read it, and a decider
// decides on it there; a handler records facts. What the account may not do does not show. What an external party
// does, such as the GGD's advice, comes in through the admin panel. Laid out as the
// editor's library: the queue in the sidebar, the submission being handled
// in the main pane beside it (BehandelenPaneel.vue).
import { computed, onMounted, ref } from 'vue';
import { useRouter } from 'vue-router';
import { hoofdletter } from '@regelrecht/frontend-shared/chronolex/waarden';
import { longDate } from '../gedeeld/datum';
import { artikelNaam, omschrijving, openIndieningen } from '../gedeeld/keten';
import type { Handeling } from '../gedeeld/types';
import { backofficeApi } from './api';
import BehandelenPaneel from './BehandelenPaneel.vue';
import { feitNaam } from './feit';
import FeitVastleggen from './FeitVastleggen.vue';
import { useBackoffice } from './useBackoffice';

const props = defineProps<{ cel?: string; id?: string }>();
const router = useRouter();
const { casus, vormen, wie, kronieken, fout, laden, laad } = useBackoffice();
onMounted(laad);

/** The executor: what the account may do there, its open submissions and the facts it records. */
const cellen = computed(() =>
  (wie.value?.cellen ?? []).map((recht) => {
    const kroniek = kronieken.value.find((k) => k.cel === recht.cel);
    const eigen = vormen.value?.eigen;
    return {
      recht,
      open: casus.value && kroniek ? openIndieningen(kroniek.grammen, casus.value) : [],
      handelingen: recht.behandelen && eigen?.cel === recht.cel ? eigen.handelingen : [],
    };
  }),
);

// The fact the sheet records, in the cell `cel`.
const keuze = ref<{ cel: string; handeling: Handeling } | null>(null);
const vastgelegd = ref('');

async function klaar(naam: string) {
  vastgelegd.value = naam;
  keuze.value = null;
  await laad();
}
</script>

<template>
  <nldd-navigation-split-view @back="router.push('/backoffice')">
    <nldd-split-view-pane slot="primary-sidebar" has-content>
      <nldd-page sticky-header accessible-label="Werkvoorraad">
        <nldd-top-title-bar slot="header" text="Werkvoorraad" heading-level="3" collapse-anchor="werkvoorraad-titel"></nldd-top-title-bar>
        <nldd-simple-section width="full">
          <nldd-title id="werkvoorraad-titel" size="display-3"><h3>Werkvoorraad</h3></nldd-title>
          <nldd-text color="secondary">De indieningen zonder besluit.</nldd-text>
          <nldd-spacer size="16"></nldd-spacer>
          <nldd-activity-indicator v-if="laden" text="De werkvoorraad laden" show-text></nldd-activity-indicator>
          <nldd-inline-dialog v-else-if="fout" icon="warning" icon-color="critical" text="De werkvoorraad laadt niet" :supporting-text="fout"></nldd-inline-dialog>
          <template v-else-if="casus">
            <template v-if="vastgelegd">
              <nldd-banner variant="success" :text="`${vastgelegd} is vastgelegd`"></nldd-banner>
              <nldd-spacer size="16"></nldd-spacer>
            </template>
            <nldd-inline-dialog v-if="cellen.length === 0" icon="info" text="Geen werkvoorraad" supporting-text="Dit account mag geen kroniek lezen."></nldd-inline-dialog>
            <template v-for="(c, i) in cellen" :key="c.recht.cel">
              <nldd-spacer v-if="i > 0" size="24"></nldd-spacer>
              <nldd-title size="display-5"><h4>{{ hoofdletter(c.recht.cel_naam) }}</h4></nldd-title>
              <nldd-list v-if="c.open.length" appearance="simple" :accessible-label="`Werkvoorraad van ${c.recht.cel_naam}`">
                <nldd-list-item
                  v-for="g in c.open"
                  :key="g.id"
                  size="md"
                  button
                  :current="g.id === id || undefined"
                  @click="router.push({ name: 'behandelen', params: { cel: c.recht.cel, id: g.id } })"
                >
                  <nldd-text-cell :text="omschrijving(g, casus)" :supporting-text="`Ontvangen ${longDate(g.recorded_on)} · onder ${artikelNaam(g.basis)}`"></nldd-text-cell>
                  <nldd-spacer-cell size="8"></nldd-spacer-cell>
                  <nldd-icon-cell size="20"><nldd-icon icon="chevron-right"></nldd-icon></nldd-icon-cell>
                </nldd-list-item>
              </nldd-list>
              <nldd-text v-else color="secondary">Geen open indieningen.</nldd-text>
              <nldd-text v-if="c.open.length && !c.recht.beslissen" size="sm" color="secondary">
                Een beslisser neemt het besluit; u ziet de indieningen wel.
              </nldd-text>
              <template v-if="c.handelingen.length">
                <nldd-spacer size="8"></nldd-spacer>
                <nldd-button-group orientation="horizontal">
                  <nldd-button
                    v-for="h in c.handelingen"
                    :key="h.feit"
                    appearance="secondary"
                    start-icon="add"
                    :text="`${feitNaam(h.feit)} vastleggen`"
                    @click="keuze = { cel: c.recht.cel, handeling: h }"
                  ></nldd-button>
                </nldd-button-group>
              </template>
            </template>
          </template>
        </nldd-simple-section>
      </nldd-page>
    </nldd-split-view-pane>

    <nldd-split-view-pane slot="main" :has-content="id || undefined">
      <BehandelenPaneel
        v-if="cel && id && casus"
        :key="id"
        :cel="cel"
        :id="id"
        :casus="casus"
        :vormen="vormen"
        :wie="wie"
        :kronieken="kronieken"
        @besloten="laad"
      />
      <nldd-page v-else landmarks="page">
        <nldd-simple-section width="full">
          <nldd-inline-dialog text="Kies een indiening" supporting-text="Een indiening opent hier, naast de werkvoorraad."></nldd-inline-dialog>
        </nldd-simple-section>
      </nldd-page>
    </nldd-split-view-pane>
  </nldd-navigation-split-view>

  <FeitVastleggen :keuze="keuze" :teksten="vormen?.teksten" :vastleggen="backofficeApi.vastleggen" @sluit="keuze = null" @vastgelegd="klaar" />
</template>
