<script setup lang="ts">
// A record of the citizen's dossier that is no decision, as the main pane
// beside the dossier (DossierView.vue): a submission of the citizen (an
// application, an objection, an appeal) or a fact a cell recorded (advice
// received, the copy of a ruling). It shows what the record holds, with the
// labels of the UI spec and, for a field the system filled, where its value
// came from; what it is directed against or about; and for a submission what
// answered it, or that it waits. A decision opens in BesluitPaneel.vue.
import { computed } from 'vue';
import { useRouter } from 'vue-router';
import { longDate } from '../gedeeld/datum';
import { herkomsten } from '../gedeeld/formulier';
import Gegevens from '../gedeeld/Gegevens.vue';
import {
  artikelNaam,
  besluitNaam,
  besluitenOp,
  celNaam,
  isIndiening,
  omschrijving,
  toegewezen,
  waarover,
} from '../gedeeld/keten';
import type { Casus, Gram, Vormen } from '../gedeeld/types';

const props = defineProps<{ id: string; casus: Casus; vormen: Vormen | null; dossier: Gram[] }>();
const router = useRouter();

const stuk = computed(() => props.dossier.find((g) => g.id === props.id) ?? null);
const naamVanCel = (cel: string) => celNaam(cel, props.casus);
/** A record in words with its date: `besluit (beschikking) van 13 oktober 2026`. */
const noem = (g: Gram) => `${omschrijving(g, props.casus).toLowerCase()} van ${longDate(g.recorded_on)}`;
const noemId = (id: string) => {
  const g = props.dossier.find((x) => x.id === id);
  return g ? noem(g) : undefined;
};
const titel = computed(() => (stuk.value ? `${omschrijving(stuk.value, props.casus)} van ${longDate(stuk.value.recorded_on)}` : 'Stuk'));
const indiening = computed(() => !!stuk.value && isIndiening(stuk.value, props.casus));
/** Where the system filled a field of this submission, by its form's UI spec. */
const herkomst = computed(() => (stuk.value ? herkomsten(props.vormen?.indieningen[stuk.value.basis]) : {}));
const over = computed(() => (stuk.value ? waarover(stuk.value, props.dossier) : undefined));
/** The latest decision on this submission, if one was taken. */
const antwoord = computed(() => (stuk.value && indiening.value ? besluitenOp(stuk.value.id, props.dossier, props.casus).at(-1) : undefined));
const uitkomst = computed(() => {
  const t = antwoord.value ? toegewezen(antwoord.value, props.casus) : undefined;
  if (t === undefined) return '';
  return t ? 'Toegewezen' : 'Afgewezen';
});
const open = (g: Gram) => router.push({ name: 'stuk', params: { id: g.id } });
</script>

<template>
  <nldd-page landmarks="page" sticky-header :accessible-label="titel">
    <nldd-top-title-bar slot="header" :text="titel" back-text="Mijn dossier" collapse-anchor="stuk-titel"></nldd-top-title-bar>
    <nldd-simple-section width="full">
      <nldd-inline-dialog
        v-if="!stuk"
        icon="info"
        text="Dit stuk staat niet in uw dossier"
        supporting-text="Kies een stuk uit de lijst. U ziet alleen wat over u is vastgelegd."
      ></nldd-inline-dialog>
      <template v-else>
        <nldd-title id="stuk-titel" size="display-3"><h3>{{ titel }}</h3></nldd-title>
        <nldd-text color="secondary">{{ naamVanCel(stuk.recording_actor) }} · onder {{ artikelNaam(stuk.basis) }}</nldd-text>
        <nldd-spacer size="24"></nldd-spacer>

        <template v-if="over">
          <nldd-title size="display-5"><h4>{{ indiening ? 'Gericht tegen' : 'Over' }}</h4></nldd-title>
          <nldd-spacer size="8"></nldd-spacer>
          <nldd-list appearance="box-base" :accessible-label="indiening ? 'Gericht tegen' : 'Over'">
            <nldd-list-item size="md" button @click="open(over)">
              <nldd-text-cell
                :text="`${omschrijving(over, casus)} van ${longDate(over.recorded_on)}`"
                :supporting-text="naamVanCel(over.recording_actor)"
              ></nldd-text-cell>
              <nldd-spacer-cell size="8"></nldd-spacer-cell>
              <nldd-icon-cell size="20"><nldd-icon icon="chevron-right"></nldd-icon></nldd-icon-cell>
            </nldd-list-item>
          </nldd-list>
          <nldd-spacer size="24"></nldd-spacer>
        </template>

        <template v-if="indiening">
          <nldd-title size="display-5"><h4>Stand</h4></nldd-title>
          <nldd-spacer size="8"></nldd-spacer>
          <nldd-list v-if="antwoord" appearance="box-base" accessible-label="Stand">
            <nldd-list-item size="md" button @click="open(antwoord)">
              <nldd-text-cell
                :text="`${besluitNaam(antwoord, casus)} van ${longDate(antwoord.recorded_on)}`"
                :supporting-text="`Genomen door ${naamVanCel(antwoord.recording_actor)}`"
              ></nldd-text-cell>
              <nldd-cell v-if="uitkomst">
                <nldd-tag :text="uitkomst" :color="uitkomst === 'Toegewezen' ? 'success' : 'critical'" size="sm"></nldd-tag>
              </nldd-cell>
              <nldd-spacer-cell size="8"></nldd-spacer-cell>
              <nldd-icon-cell size="20"><nldd-icon icon="chevron-right"></nldd-icon></nldd-icon-cell>
            </nldd-list-item>
          </nldd-list>
          <nldd-text v-else>In behandeling bij {{ naamVanCel(stuk.recording_actor) }}. Het besluit verschijnt hier zodra het genomen is.</nldd-text>
          <nldd-spacer size="24"></nldd-spacer>
        </template>

        <nldd-title size="display-5"><h4>{{ indiening ? 'Wat u indiende' : 'Wat er is vastgelegd' }}</h4></nldd-title>
        <nldd-text color="secondary">
          {{ indiening ? `Ingediend op ${longDate(stuk.recorded_on)}.` : `Vastgelegd op ${longDate(stuk.recorded_on)} door ${naamVanCel(stuk.recording_actor)}.` }}
        </nldd-text>
        <nldd-spacer size="8"></nldd-spacer>
        <Gegevens
          :waarden="stuk.fields"
          :teksten="vormen?.teksten"
          :volgorde="vormen?.volgorde"
          :noem="noemId"
          :herkomst="herkomst"
          :label="indiening ? 'Wat u indiende' : 'Wat er is vastgelegd'"
        />
      </template>
    </nldd-simple-section>
  </nldd-page>
</template>
