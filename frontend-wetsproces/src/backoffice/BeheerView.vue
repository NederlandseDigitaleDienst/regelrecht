<script setup lang="ts">
// The admin panel: per external party of the case (the GGD that advises, the
// court that decides on an appeal) what the process allows of it now, which
// the admin enters on its behalf. Nobody works for such a party in this
// server; until there is a real link with it, this is how its acts come in.
// The panel shows of a party only what an input needs: the submissions that
// wait for its decision and its decisions not sent yet, never its chronicle.
// The facts it records and the decision forms come from the forms of the
// process (`GET /api/formulier`), as everywhere in the back office. Laid out
// as the work queue: the parties in the sidebar, a submission being decided
// in the main pane beside it (BehandelenPaneel.vue).
import { computed, onMounted, ref } from 'vue';
import { useRouter } from 'vue-router';
import { hoofdletter } from '@regelrecht/frontend-shared/chronolex/waarden';
import { longDate } from '../gedeeld/datum';
import { foutTekst, nieuweSleutel } from '../gedeeld/http';
import { artikelNaam, omschrijving } from '../gedeeld/keten';
import type { Gram, Handeling } from '../gedeeld/types';
import { beheerApi } from './api';
import type { Partijen } from './api';
import BehandelenPaneel from './BehandelenPaneel.vue';
import { feitNaam } from './feit';
import FeitVastleggen from './FeitVastleggen.vue';
import { useBackoffice } from './useBackoffice';

const props = defineProps<{ cel?: string; id?: string }>();
const router = useRouter();
const { casus, vormen, wie, kronieken, fout, laden, laad } = useBackoffice();
const panel = ref<Partijen | null>(null);

async function laadAlles() {
  const [, p] = await Promise.all([laad(), beheerApi.partijen().catch((e: unknown) => foutTekst(e))]);
  if (typeof p === 'string') fout.value = p;
  else panel.value = p;
}
onMounted(laadAlles);

/** Per external party: what waits for it, and the facts it records. */
const partijen = computed(() =>
  (panel.value?.partijen ?? []).map((p) => ({
    ...p,
    handelingen: vormen.value?.andere_cellen.find((c) => c.cel === p.cel)?.handelingen ?? [],
  })),
);
const gekozen = computed(() => partijen.value.find((p) => p.cel === props.cel));

const keuze = ref<{ cel: string; handeling: Handeling } | null>(null);
const gedaan = ref('');

async function vastgelegd(naam: string) {
  gedaan.value = `${naam} is vastgelegd`;
  keuze.value = null;
  await laadAlles();
}

const melding = ref('');
const bezig = ref('');
// One key per decision until its copy is sent, so a retry after a lost answer
// sends nothing twice.
const sleutels = new Map<string, string>();
async function toezenden(besluit: Gram) {
  bezig.value = besluit.id;
  melding.value = '';
  try {
    const sleutel = sleutels.get(besluit.id) ?? nieuweSleutel();
    sleutels.set(besluit.id, sleutel);
    await beheerApi.toezenden(besluit.id, sleutel);
    sleutels.delete(besluit.id);
    gedaan.value = 'Het afschrift is toegezonden';
    await laadAlles();
  } catch (e) {
    melding.value = foutTekst(e);
  } finally {
    bezig.value = '';
  }
}
</script>

<template>
  <nldd-navigation-split-view @back="router.push('/backoffice/beheer')">
    <nldd-split-view-pane slot="primary-sidebar" has-content>
      <nldd-page sticky-header accessible-label="Beheerpaneel">
        <nldd-top-title-bar slot="header" text="Beheerpaneel" heading-level="3" collapse-anchor="beheer-titel"></nldd-top-title-bar>
        <nldd-simple-section width="full">
          <nldd-title id="beheer-titel" size="display-3"><h3>Beheerpaneel</h3></nldd-title>
          <nldd-text color="secondary">
            Wat een externe partij in het proces doet, voert u hier namens haar in. De kroniek legt vast dat het via het beheerpaneel binnenkwam, en door welk account.
          </nldd-text>
          <nldd-spacer size="16"></nldd-spacer>
          <nldd-activity-indicator v-if="laden || (!panel && !fout)" text="Het beheerpaneel laden" show-text></nldd-activity-indicator>
          <nldd-inline-dialog v-else-if="fout" icon="warning" icon-color="critical" text="Het beheerpaneel laadt niet" :supporting-text="fout"></nldd-inline-dialog>
          <template v-else-if="casus && panel">
            <template v-if="gedaan">
              <nldd-banner variant="success" :text="gedaan"></nldd-banner>
              <nldd-spacer size="16"></nldd-spacer>
            </template>
            <template v-if="melding">
              <nldd-inline-dialog icon="warning" icon-color="critical" text="Dat lukt niet" :supporting-text="melding"></nldd-inline-dialog>
              <nldd-spacer size="16"></nldd-spacer>
            </template>
            <nldd-inline-dialog v-if="partijen.length === 0" icon="info" text="Deze casus kent geen externe partijen"></nldd-inline-dialog>
            <template v-for="(p, i) in partijen" :key="p.cel">
              <nldd-spacer v-if="i > 0" size="24"></nldd-spacer>
              <nldd-title size="display-5"><h4>{{ hoofdletter(p.cel_naam) }}</h4></nldd-title>
              <nldd-list v-if="p.open.length" appearance="simple" :accessible-label="`Wat wacht op ${p.cel_naam}`">
                <nldd-list-item
                  v-for="g in p.open"
                  :key="g.id"
                  size="md"
                  button
                  :current="g.id === id || undefined"
                  @click="router.push({ name: 'beheer-behandelen', params: { cel: p.cel, id: g.id } })"
                >
                  <nldd-text-cell :text="omschrijving(g, casus, 'indiening')" :supporting-text="`Ontvangen ${longDate(g.recorded_on)} · onder ${artikelNaam(g.basis)}`"></nldd-text-cell>
                  <nldd-spacer-cell size="8"></nldd-spacer-cell>
                  <nldd-icon-cell size="20"><nldd-icon icon="chevron-right"></nldd-icon></nldd-icon-cell>
                </nldd-list-item>
              </nldd-list>
              <nldd-list v-if="p.toe_te_zenden.length" appearance="simple" :accessible-label="`Besluiten van ${p.cel_naam} die nog niet zijn toegezonden`">
                <nldd-list-item v-for="g in p.toe_te_zenden" :key="g.id" size="md">
                  <nldd-text-cell :text="omschrijving(g, casus, 'besluit')" :supporting-text="`Genomen ${longDate(g.recorded_on)} · nog niet toegezonden`"></nldd-text-cell>
                  <nldd-cell>
                    <nldd-button
                      appearance="secondary"
                      size="sm"
                      start-icon="send"
                      text="Afschrift toezenden"
                      :loading="bezig === g.id || undefined"
                      @click="toezenden(g)"
                    ></nldd-button>
                  </nldd-cell>
                </nldd-list-item>
              </nldd-list>
              <template v-if="p.handelingen.length">
                <nldd-spacer size="8"></nldd-spacer>
                <nldd-button-group orientation="horizontal">
                  <nldd-button
                    v-for="h in p.handelingen"
                    :key="h.feit"
                    appearance="secondary"
                    start-icon="add"
                    :text="`${feitNaam(h.feit)} vastleggen`"
                    @click="keuze = { cel: p.cel, handeling: h }"
                  ></nldd-button>
                </nldd-button-group>
              </template>
              <nldd-text v-if="!p.open.length && !p.toe_te_zenden.length && !p.handelingen.length" color="secondary">
                Op dit moment vraagt het proces niets van {{ p.cel_naam }}.
              </nldd-text>
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
        namens
        :open="gekozen?.open ?? []"
        @besloten="laadAlles"
      />
      <nldd-page v-else landmarks="page">
        <nldd-simple-section width="full">
          <nldd-inline-dialog text="Kies wat op een partij wacht" supporting-text="Een indiening opent hier, naast het beheerpaneel."></nldd-inline-dialog>
        </nldd-simple-section>
      </nldd-page>
    </nldd-split-view-pane>
  </nldd-navigation-split-view>

  <FeitVastleggen :keuze="keuze" :teksten="vormen?.teksten" :vastleggen="beheerApi.vastleggen" @sluit="keuze = null" @vastgelegd="vastgelegd">
    <nldd-banner
      v-if="keuze"
      variant="neutral"
      :text="`U legt dit vast namens ${partijen.find((p) => p.cel === keuze?.cel)?.cel_naam ?? keuze.cel}`"
    ></nldd-banner>
  </FeitVastleggen>
</template>
