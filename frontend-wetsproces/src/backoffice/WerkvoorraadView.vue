<script setup lang="ts">
// The work queue: per cell the employee works for, the submissions on which
// no decision was taken yet, and the facts the cell records, such as an
// advice. A decider opens a submission to decide on it; a handler records
// facts. What the account may not do does not show. Laid out as the
// editor's library: the queue in the sidebar, the submission being handled
// in the main pane beside it (BehandelenPaneel.vue).
import { computed, onMounted, ref } from 'vue';
import { useRouter } from 'vue-router';
import { hoofdletter, leesbaar, zonderLeeg } from '@regelrecht/frontend-shared/chronolex/waarden';
import { longDate } from '../gedeeld/datum';
import { beginwaarden, schermveld } from '../gedeeld/formulier';
import { foutTekst, nieuweSleutel } from '../gedeeld/http';
import { omschrijving, openIndieningen } from '../gedeeld/keten';
import SpecFormulier from '../gedeeld/SpecFormulier.vue';
import type { Handeling } from '../gedeeld/types';
import { backofficeApi } from './api';
import BehandelenPaneel from './BehandelenPaneel.vue';
import { useBackoffice } from './useBackoffice';

const props = defineProps<{ cel?: string; id?: string }>();
const router = useRouter();
const { casus, vormen, wie, kronieken, fout, laden, laad } = useBackoffice();
onMounted(laad);

/** Per cell: what the account may do there, its open submissions and the facts it records. */
const cellen = computed(() =>
  (wie.value?.cellen ?? []).map((recht) => {
    const kroniek = kronieken.value.find((k) => k.cel === recht.cel);
    const v = vormen.value;
    const handelingen =
      v?.eigen.cel === recht.cel ? v.eigen.handelingen : (v?.andere_cellen.find((c) => c.cel === recht.cel)?.handelingen ?? []);
    return {
      recht,
      open: casus.value && kroniek ? openIndieningen(kroniek.grammen, casus.value) : [],
      handelingen: recht.behandelen ? handelingen : [],
    };
  }),
);

/** The name of a fact: the last part of `<regelwerk>#<article>#<fact>`. */
const feitNaam = (feit: string) => hoofdletter(leesbaar(feit.split('#').at(-1) ?? feit));

// The fact the sheet records, in the cell `cel`.
const open = ref(false);
const keuze = ref<{ cel: string; handeling: Handeling } | null>(null);
const waarden = ref<Record<string, unknown>>({});
const melding = ref('');
const bezig = ref(false);
const vastgelegd = ref('');
let sleutel = nieuweSleutel();
const velden = computed(() => (keuze.value?.handeling.velden ?? []).map((v) => schermveld(v, vormen.value?.teksten)));

function start(cel: string, handeling: Handeling) {
  keuze.value = { cel, handeling };
  waarden.value = beginwaarden(velden.value);
  melding.value = '';
  sleutel = nieuweSleutel();
  open.value = true;
}

async function vastleggen() {
  if (!keuze.value) return;
  bezig.value = true;
  melding.value = '';
  try {
    const { cel, handeling } = keuze.value;
    await backofficeApi.vastleggen({ cel, feit: handeling.feit, velden: zonderLeeg(waarden.value) }, sleutel);
    vastgelegd.value = feitNaam(handeling.feit);
    open.value = false;
    await laad();
  } catch (e) {
    melding.value = foutTekst(e);
  } finally {
    bezig.value = false;
  }
}
</script>

<template>
  <nldd-navigation-split-view @back="router.push('/backoffice')">
    <nldd-split-view-pane slot="primary-sidebar" has-content>
      <nldd-page sticky-header accessible-label="Werkvoorraad">
        <nldd-top-title-bar slot="header" text="Werkvoorraad" heading-level="3" collapse-anchor="werkvoorraad-titel"></nldd-top-title-bar>
        <nldd-simple-section width="full">
          <nldd-title id="werkvoorraad-titel" size="display-3"><h3>Werkvoorraad</h3></nldd-title>
          <nldd-text color="secondary">Per cel waarvoor u werkt: de indieningen zonder besluit.</nldd-text>
          <nldd-spacer size="16"></nldd-spacer>
          <nldd-activity-indicator v-if="laden" text="De werkvoorraad laden" show-text></nldd-activity-indicator>
          <nldd-inline-dialog v-else-if="fout" icon="warning" icon-color="critical" text="De werkvoorraad laadt niet" :supporting-text="fout"></nldd-inline-dialog>
          <template v-else-if="casus">
            <template v-if="vastgelegd">
              <nldd-banner variant="success" :text="`${vastgelegd} is vastgelegd`"></nldd-banner>
              <nldd-spacer size="16"></nldd-spacer>
            </template>
            <nldd-inline-dialog v-if="cellen.length === 0" icon="info" text="U werkt voor geen cel" supporting-text="Dit account mag geen kroniek lezen."></nldd-inline-dialog>
            <template v-for="(c, i) in cellen" :key="c.recht.cel">
              <nldd-spacer v-if="i > 0" size="24"></nldd-spacer>
              <nldd-title size="display-5"><h4>{{ hoofdletter(c.recht.cel_naam) }}</h4></nldd-title>
              <nldd-list v-if="c.open.length" appearance="simple" :accessible-label="`Werkvoorraad van ${c.recht.cel_naam}`">
                <nldd-list-item
                  v-for="g in c.open"
                  :key="g.id"
                  size="md"
                  :button="c.recht.beslissen || undefined"
                  :current="g.id === id || undefined"
                  @click="c.recht.beslissen && router.push({ name: 'behandelen', params: { cel: c.recht.cel, id: g.id } })"
                >
                  <nldd-text-cell :text="omschrijving(g, casus)" :supporting-text="`Ontvangen ${longDate(g.recorded_on)} · onder ${g.basis}`"></nldd-text-cell>
                  <template v-if="c.recht.beslissen">
                    <nldd-spacer-cell size="8"></nldd-spacer-cell>
                    <nldd-icon-cell size="20"><nldd-icon icon="chevron-right"></nldd-icon></nldd-icon-cell>
                  </template>
                </nldd-list-item>
              </nldd-list>
              <nldd-text v-else color="secondary">Geen open indieningen.</nldd-text>
              <nldd-text v-if="c.open.length && !c.recht.beslissen" size="sm" color="secondary">
                Een beslisser van deze cel neemt het besluit; u ziet de indieningen wel.
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
                    @click="start(c.recht.cel, h)"
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

  <Teleport to="body">
    <nldd-sheet width="720px" :open="open || undefined" @close="open = false">
      <nldd-page>
        <nldd-container slot="header" padding="16">
          <nldd-top-title-bar heading-level="2" :text="keuze ? `${feitNaam(keuze.handeling.feit)} vastleggen` : ''" dismiss-text="Sluiten"></nldd-top-title-bar>
        </nldd-container>
        <nldd-simple-section v-if="keuze">
          <nldd-container gap="16">
            <nldd-text color="secondary">De gegevens komen uit {{ keuze.handeling.feit.split('#').slice(0, 2).join(' artikel ') }}.</nldd-text>
            <SpecFormulier :velden="velden" :waarden="waarden" @zet="(n, v) => (waarden[n] = v)" />
            <nldd-inline-dialog v-if="melding" icon="warning" icon-color="critical" text="Niet vastgelegd" :supporting-text="melding"></nldd-inline-dialog>
            <nldd-button-group orientation="horizontal">
              <nldd-button appearance="primary" text="Vastleggen" :loading="bezig || undefined" @click="vastleggen"></nldd-button>
              <nldd-button appearance="neutral-transparent" text="Annuleren" @click="open = false"></nldd-button>
            </nldd-button-group>
          </nldd-container>
        </nldd-simple-section>
      </nldd-page>
    </nldd-sheet>
  </Teleport>
</template>
