<script setup lang="ts">
// A record of the citizen's dossier that is no decision, as a page one level
// below the dossier (DossierView.vue), whose back button returns to it: a submission of the citizen (an
// application, an objection, an appeal) or a fact a cell recorded (advice
// received, the copy of a ruling). It shows what the record holds, with the
// labels of the UI spec and, for a field the system filled, where its value
// came from; what it is directed against or about; and for a submission what
// answers it now, as the server says (`stand`): its decision, a new decision
// taken in the place of a revoked one, or that it waits, also again after a
// decision on it was annulled. Under that, for an application, how far the
// acts after its grant are and what the citizen can apply for about it, such
// as a duplicate, as the dossier gives them (`vervolg`, `aanvragen`). A
// decision opens in BesluitPaneel.vue. On top, without a heading, stand how
// an application stands (`status`) and the few fields the UI spec puts in the
// summary of a record (`record.summary`): what was asked. Those are not shown
// again under what was submitted.
import { computed } from 'vue';
import { hoofdletter } from '@regelrecht/frontend-shared/chronolex/waarden';
import { useRoute, useRouter } from 'vue-router';
import { longDate } from '../gedeeld/datum';
import { herkomsten, kaalSchermveld, opVolgorde, stukWaarde } from '../gedeeld/formulier';
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
import type { Casus, Gram, Stand, Vervolgaanvraag, Vormen } from '../gedeeld/types';
import { AANVRAAG } from './routes';

const props = defineProps<{
  id: string;
  casus: Casus;
  vormen: Vormen | null;
  dossier: Gram[];
  stand: Stand;
  /** How far the acts after the grant of this application are, in the case's words. */
  vervolg?: string;
  /** What the citizen can apply for about that grant, each with the record it is about (`op`). */
  aanvragen?: (Vervolgaanvraag & { op: string })[];
  /** The name of the application as the list of applications gives it, such as `Aanvraag gehandicaptenparkeerkaart`. */
  naam?: string;
  /** How the application stands, as the list of applications says it. */
  status?: { tekst: string; kleur: string } | null;
}>();
const router = useRouter();
const route = useRoute();
const vraagAan = (a: Vervolgaanvraag & { op: string }) =>
  router.push({ path: route.path, query: { artikel: a.artikel, op: a.op }, hash: AANVRAAG });

const stuk = computed(() => props.dossier.find((g) => g.id === props.id) ?? null);
const naamVanCel = (cel: string) => celNaam(cel, props.casus);
/** A record in words with its date: `besluit (beschikking) van 13 oktober 2026`. */
const omschrijf = (g: Gram) => omschrijving(g, props.casus, rolVan(g, props.stand));
const noem = (g: Gram) => `${omschrijf(g).toLowerCase()} van ${longDate(g.recorded_on)}`;
const noemId = (id: string) => {
  const g = props.dossier.find((x) => x.id === id);
  return g ? noem(g) : undefined;
};
// What it is goes in the title; when and where stand under it.
const titel = computed(() => props.naam ?? (stuk.value ? omschrijf(stuk.value) : 'Stuk'));
const indiening = computed(() => !!stuk.value && isIndiening(stuk.value, props.stand));
/** Where the system filled a field of this submission, by its form's UI spec. */
const herkomst = computed(() => (stuk.value ? herkomsten(props.vormen?.indieningen[stuk.value.basis]) : {}));
const over = computed(() => (stuk.value ? waarover(stuk.value, props.dossier) : undefined));
/** The fields of this record the UI spec puts in its summary, in the order of the UI spec. */
const samenvatting = computed(() => {
  const teksten = props.vormen?.teksten ?? {};
  const velden = stuk.value?.fields ?? {};
  const namen = Object.keys(teksten).filter((n) => teksten[n]?.record?.summary && velden[n] !== null && velden[n] !== undefined);
  return opVolgorde(namen, props.vormen?.volgorde, (n) => n).map((n) => ({
    naam: n,
    label: kaalSchermveld(n, teksten).label,
    waarde: stukWaarde(n, velden[n], teksten),
  }));
});
/** What was submitted, without what the summary already shows. */
const rest = computed(() => {
  const weg = new Set(samenvatting.value.map((s) => s.naam));
  return Object.fromEntries(Object.entries(stuk.value?.fields ?? {}).filter(([n]) => !weg.has(n)));
});
const vind = (id: string) => props.dossier.find((g) => g.id === id);
/** Whether the section on how it stands has anything to say: what answered it, or what followed. Only that it waits says the status on top. */
const metStand = computed(
  () => indiening.value && !!(antwoord.value.geldend || antwoord.value.vervallen || antwoord.value.opnieuw || props.vervolg || props.aanvragen?.length),
);
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
    <nldd-top-title-bar slot="header" :text="titel" back-text="Mijn aanvragen" collapse-anchor="stuk-titel" @back="router.push({ name: 'dossier' })"></nldd-top-title-bar>
    <nldd-two-thirds-one-third-section>
      <nldd-title
        v-if="stuk"
        id="stuk-titel"
        slot="header"
        size="display-2"
        :supporting-text="`${longDate(stuk.recorded_on)} · ${naamVanCel(stuk.recording_actor)}`"
      ><h3>{{ titel }}</h3></nldd-title>
      <nldd-inline-dialog
        v-if="!stuk"
        icon="info"
        text="Dit stuk staat niet in uw dossier"
        supporting-text="Kies een stuk uit de lijst. U ziet alleen wat over u is vastgelegd."
      ></nldd-inline-dialog>
      <template v-else>
        <template v-if="(status && indiening) || samenvatting.length">
          <nldd-list appearance="box-base" accessible-label="Samenvatting">
            <nldd-list-item v-if="status && indiening" size="md">
              <nldd-text-cell text="Status"></nldd-text-cell>
              <nldd-spacer-cell size="16"></nldd-spacer-cell>
              <!-- The word says it, so the dot beside it is decorative. -->
              <nldd-cell><nldd-badge :color="status.kleur" size="sm" decorative></nldd-badge></nldd-cell>
              <nldd-spacer-cell size="8"></nldd-spacer-cell>
              <nldd-text-cell :text="status.tekst" width="fit-content" horizontal-alignment="right"></nldd-text-cell>
            </nldd-list-item>
            <nldd-list-item v-for="s in samenvatting" :key="s.naam" size="md">
              <nldd-text-cell :text="s.label"></nldd-text-cell>
              <nldd-spacer-cell size="16"></nldd-spacer-cell>
              <nldd-text-cell :text="s.waarde" width="fit-content" horizontal-alignment="right"></nldd-text-cell>
            </nldd-list-item>
          </nldd-list>
          <nldd-spacer size="24"></nldd-spacer>
        </template>

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

        <!-- That it waits says the status on top; this says what answered it, and what followed. -->
        <template v-if="metStand">
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
            <p>
              {{ hoofdletter(naamVanCel(stuk.recording_actor)) }} neemt een nieuw besluit. Het verschijnt hier zodra het genomen is.
            </p>
          </nldd-text>
          <template v-if="vervolg || aanvragen?.length">
            <nldd-spacer size="8"></nldd-spacer>
            <nldd-list appearance="box-base" accessible-label="Na de toekenning">
              <nldd-list-item v-if="vervolg" size="md">
                <nldd-icon-cell size="20"><nldd-icon icon="info"></nldd-icon></nldd-icon-cell>
                <nldd-spacer-cell size="8"></nldd-spacer-cell>
                <nldd-text-cell :text="vervolg" supporting-text="Na de toekenning"></nldd-text-cell>
              </nldd-list-item>
              <nldd-list-item v-for="a in aanvragen" :key="`${a.op}-${a.artikel}`" size="md" button @click="vraagAan(a)">
                <nldd-icon-cell size="20"><nldd-icon icon="add"></nldd-icon></nldd-icon-cell>
                <nldd-spacer-cell size="8"></nldd-spacer-cell>
                <nldd-text-cell :text="a.knop" :supporting-text="a.titel"></nldd-text-cell>
                <nldd-icon-cell size="20"><nldd-icon icon="chevron-right"></nldd-icon></nldd-icon-cell>
              </nldd-list-item>
            </nldd-list>
          </template>
          <nldd-spacer size="24"></nldd-spacer>
        </template>

        <!-- Right under the summary the record needs no heading; after another section it does, or it reads as part of that one. -->
        <template v-if="over || metStand">
          <nldd-title size="display-5"><h4>{{ indiening ? 'Wat u indiende' : 'Wat er is vastgelegd' }}</h4></nldd-title>
          <nldd-spacer size="8"></nldd-spacer>
        </template>
        <!-- When and where stand under the title of the page; the article it rests on closes the record. -->
        <Gegevens
          :waarden="rest"
          :teksten="vormen?.teksten"
          :volgorde="vormen?.volgorde"
          :noem="noemId"
          :herkomst="herkomst"
          :label="indiening ? 'Wat u indiende' : 'Wat er is vastgelegd'"
          :bij="[{ naam: 'grondslag', label: 'Op grond van', waarde: artikelNaam(stuk.basis) }]"
        />
      </template>
      <!-- What the dossier puts beside the record, such as the timeline of its application. -->
      <nldd-container v-if="$slots.naast" slot="right">
        <slot name="naast"></slot>
      </nldd-container>
    </nldd-two-thirds-one-third-section>
    <nldd-page-footer slot="footer"></nldd-page-footer>
  </nldd-page>
</template>
