<script setup lang="ts">
// A record of the citizen's dossier that is no decision, as the main pane
// beside the dossier (DossierView.vue): a submission of the citizen (an
// application, an objection, an appeal) or a fact a cell recorded (advice
// received, the copy of a ruling). It shows what the record holds, with the
// labels of the UI spec and, for a field the system filled, where its value
// came from; what it is directed against or about; and for a submission what
// answers it now, as the server says (`stand`): its decision, a new decision
// taken in the place of a revoked one, or that it waits, also again after a
// decision on it was annulled. A decision opens in BesluitPaneel.vue.
import { computed } from 'vue';
import { hoofdletter } from '@regelrecht/frontend-shared/chronolex/waarden';
import { useRouter } from 'vue-router';
import { longDate } from '../gedeeld/datum';
import { herkomsten } from '../gedeeld/formulier';
import Gegevens from '../gedeeld/Gegevens.vue';
import {
  antwoordOp,
  artikelNaam,
  besluitNaam,
  celNaam,
  herroeping,
  herroepingTekst,
  isIndiening,
  omschrijving,
  rolVan,
  toegewezen,
  waarover,
} from '../gedeeld/keten';
import type { Casus, Gram, Stand, Vormen } from '../gedeeld/types';

const props = defineProps<{ id: string; casus: Casus; vormen: Vormen | null; dossier: Gram[]; stand: Stand }>();
const router = useRouter();

const stuk = computed(() => props.dossier.find((g) => g.id === props.id) ?? null);
const naamVanCel = (cel: string) => celNaam(cel, props.casus);
/** A record in words with its date: `besluit (beschikking) van 13 oktober 2026`. */
const omschrijf = (g: Gram) => omschrijving(g, props.casus, rolVan(g, props.stand));
const noem = (g: Gram) => `${omschrijf(g).toLowerCase()} van ${longDate(g.recorded_on)}`;
const noemId = (id: string) => {
  const g = props.dossier.find((x) => x.id === id);
  return g ? noem(g) : undefined;
};
const titel = computed(() => (stuk.value ? `${omschrijf(stuk.value)} van ${longDate(stuk.value.recorded_on)}` : 'Stuk'));
const indiening = computed(() => !!stuk.value && isIndiening(stuk.value, props.stand));
/** Where the system filled a field of this submission, by its form's UI spec. */
const herkomst = computed(() => (stuk.value ? herkomsten(props.vormen?.indieningen[stuk.value.basis]) : {}));
const over = computed(() => (stuk.value ? waarover(stuk.value, props.dossier) : undefined));
const vind = (id: string) => props.dossier.find((g) => g.id === id);
/**
 * What answers this submission now, for the screen: the decision that holds
 * (a record of its own, or the new one a decision took in the place of the
 * revoked one, which opens with the decision that holds it), and the last
 * decision on it when that was revoked.
 */
const antwoord = computed(() => {
  const a = stuk.value && indiening.value ? antwoordOp(stuk.value.id, props.stand) : undefined;
  if (!a || a.soort === 'open') return { geldend: null, vervallen: null, opnieuw: false };
  const laatste = vind(a.besluit);
  const h = laatste ? herroeping(laatste.id, props.stand, props.dossier) : undefined;
  const vervallen = laatste && h ? { besluit: laatste, woord: h.woord, tekst: herroepingTekst(h, props.casus, props.stand) } : null;
  if (a.soort === 'besloten' && laatste) {
    return { geldend: { opent: laatste, tekst: `${besluitNaam(laatste, props.casus)} van ${longDate(laatste.recorded_on)}`, uitkomst: uitkomst(laatste), regel: `Genomen door ${naamVanCel(laatste.recording_actor)}` }, vervallen, opnieuw: false };
  }
  if (a.soort === 'in_de_plaats') {
    // The new decision opens with the decision that holds it, when that is in the dossier.
    const door = vind(a.door);
    return {
      geldend: {
        opent: door,
        tekst: `Nieuw besluit van ${longDate(a.nieuw.recorded_on)}`,
        uitkomst: uitkomst(a.nieuw),
        regel: door ? `In de plaats van het eerdere besluit, genomen bij ${noem(door)}` : 'In de plaats van het eerdere besluit',
      },
      vervallen,
      opnieuw: false,
    };
  }
  // Revoked with nothing in its place: a new decision follows only after a
  // record such as the receipt of a judgment (`na`); after a withdrawal, none.
  return { geldend: null, vervallen, opnieuw: a.soort === 'vervallen' && !!a.na };
});
function uitkomst(g: Pick<Gram, 'outputs'>): string {
  const t = toegewezen(g, props.casus);
  if (t === undefined) return '';
  return t ? 'Toegewezen' : 'Afgewezen';
}
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
                :text="`${omschrijf(over)} van ${longDate(over.recorded_on)}`"
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
          <nldd-list v-if="antwoord.geldend || antwoord.vervallen" appearance="box-base" accessible-label="Stand">
            <nldd-list-item
              v-if="antwoord.geldend"
              size="md"
              :button="!!antwoord.geldend.opent || undefined"
              @click="antwoord.geldend.opent && open(antwoord.geldend.opent)"
            >
              <nldd-text-cell :text="antwoord.geldend.tekst" :supporting-text="antwoord.geldend.regel"></nldd-text-cell>
              <nldd-cell>
                <nldd-badge text="Geldt" color="success" size="sm"></nldd-badge>
              </nldd-cell>
              <nldd-cell v-if="antwoord.geldend.uitkomst">
                <nldd-tag :text="antwoord.geldend.uitkomst" :color="antwoord.geldend.uitkomst === 'Toegewezen' ? 'success' : 'critical'" size="sm"></nldd-tag>
              </nldd-cell>
              <template v-if="antwoord.geldend.opent">
                <nldd-spacer-cell size="8"></nldd-spacer-cell>
                <nldd-icon-cell size="20"><nldd-icon icon="chevron-right"></nldd-icon></nldd-icon-cell>
              </template>
            </nldd-list-item>
            <nldd-list-item v-if="antwoord.vervallen" size="md" button @click="open(antwoord.vervallen.besluit)">
              <nldd-text-cell
                :text="`${besluitNaam(antwoord.vervallen.besluit, casus)} van ${longDate(antwoord.vervallen.besluit.recorded_on)}`"
                :supporting-text="antwoord.vervallen.tekst"
              ></nldd-text-cell>
              <nldd-cell>
                <nldd-badge :text="antwoord.vervallen.woord" color="warning" size="sm"></nldd-badge>
              </nldd-cell>
              <nldd-spacer-cell size="8"></nldd-spacer-cell>
              <nldd-icon-cell size="20"><nldd-icon icon="chevron-right"></nldd-icon></nldd-icon-cell>
            </nldd-list-item>
          </nldd-list>
          <nldd-text v-if="antwoord.opnieuw">
            {{ hoofdletter(naamVanCel(stuk.recording_actor)) }} neemt een nieuw besluit. Het verschijnt hier zodra het genomen is.
          </nldd-text>
          <nldd-text v-else-if="!antwoord.geldend && !antwoord.vervallen">
            In behandeling bij {{ naamVanCel(stuk.recording_actor) }}. Het besluit verschijnt hier zodra het genomen is.
          </nldd-text>
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
          :noem="noemId"
          :herkomst="herkomst"
          :label="indiening ? 'Wat u indiende' : 'Wat er is vastgelegd'"
        />
      </template>
    </nldd-simple-section>
  </nldd-page>
</template>
