<script setup lang="ts">
// The work queue of the executor, the one cell the employee works for: the
// submissions on which no decision was taken yet, and the facts it records
// itself. A decider opens a submission to decide on it; a handler records
// facts. What the account may not do does not show. What an external party
// does, such as the GGD's advice, comes in through the admin panel.
import { computed, onMounted, ref } from 'vue';
import { useRouter } from 'vue-router';
import { hoofdletter } from '@regelrecht/frontend-shared/chronolex/waarden';
import { longDate } from '../gedeeld/datum';
import { omschrijving, openIndieningen } from '../gedeeld/keten';
import type { Handeling } from '../gedeeld/types';
import { backofficeApi } from './api';
import { feitNaam } from './feit';
import FeitVastleggen from './FeitVastleggen.vue';
import { useBackoffice } from './useBackoffice';

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
  <nldd-simple-section>
    <nldd-title slot="header" heading-level="1" text="Werkvoorraad" supporting-text="De indieningen zonder besluit."></nldd-title>
    <nldd-activity-indicator v-if="laden"></nldd-activity-indicator>
    <nldd-inline-dialog v-else-if="fout" icon="warning" icon-color="critical" text="De werkvoorraad laadt niet" :supporting-text="fout"></nldd-inline-dialog>
    <nldd-container v-else-if="casus" gap="32">
      <nldd-banner v-if="vastgelegd" variant="success" :text="`${vastgelegd} is vastgelegd`"></nldd-banner>
      <nldd-inline-dialog v-if="cellen.length === 0" icon="info" text="Geen werkvoorraad" supporting-text="Dit account mag geen kroniek lezen."></nldd-inline-dialog>
      <nldd-container v-for="c in cellen" :key="c.recht.cel" gap="12">
        <nldd-title heading-level="2" size="display-5" :text="hoofdletter(c.recht.cel_naam)"></nldd-title>
        <nldd-list v-if="c.open.length" appearance="box-base" :accessible-label="`Werkvoorraad van ${c.recht.cel_naam}`">
          <nldd-list-item v-for="g in c.open" :key="g.id">
            <nldd-text-cell :text="omschrijving(g, casus)" :supporting-text="`Ontvangen ${longDate(g.recorded_on)} · onder ${g.basis}`"></nldd-text-cell>
            <nldd-cell v-if="c.recht.beslissen">
              <nldd-button
                appearance="primary"
                size="sm"
                text="Behandelen"
                @click="router.push({ name: 'behandelen', params: { cel: c.recht.cel, id: g.id } })"
              ></nldd-button>
            </nldd-cell>
          </nldd-list-item>
        </nldd-list>
        <nldd-text v-else color="secondary">Geen open indieningen.</nldd-text>
        <nldd-text v-if="c.open.length && !c.recht.beslissen" size="sm" color="secondary">
          Een beslisser neemt het besluit; u ziet de indieningen wel.
        </nldd-text>
        <nldd-button-group v-if="c.handelingen.length" orientation="horizontal">
          <nldd-button
            v-for="h in c.handelingen"
            :key="h.feit"
            appearance="secondary"
            start-icon="add"
            :text="`${feitNaam(h.feit)} vastleggen`"
            @click="keuze = { cel: c.recht.cel, handeling: h }"
          ></nldd-button>
        </nldd-button-group>
      </nldd-container>
    </nldd-container>
  </nldd-simple-section>

  <FeitVastleggen :keuze="keuze" :teksten="vormen?.teksten" :vastleggen="backofficeApi.vastleggen" @sluit="keuze = null" @vastgelegd="klaar" />
</template>
