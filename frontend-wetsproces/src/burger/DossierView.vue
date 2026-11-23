<script setup lang="ts">
// The citizen's dossier: every record about them, in every cell, as chains of
// what answers to what: the application, the decision on it, an objection to
// that decision and the decision on the objection. Derived by the server from
// the chronicles on every request (`GET /api/dossier`). Laid out as the
// editor's library: the dossier in the sidebar, a record chosen from it in
// the main pane beside it. Every row opens: a decision in BesluitPaneel.vue,
// a submission or a recorded fact in StukPaneel.vue. Under a decision that
// granted, a line says how far the acts after the grant are, in the words
// the case gives them (`na_toekenning`), such as "Uw kaart ligt klaar.".
// What each record is, and which decision holds, comes from the server
// (`stand`): a revoked decision is marked as such, and a decision that took
// a new one in the place of the decision it revoked carries the outcome of
// that new one. Under the chosen record, the timeline of the application it
// belongs to (gedeeld/TijdlijnLijst.vue, `GET /api/dossier/tijdlijn/{id}`):
// what happened, in order, and what can or must happen next.
import { computed, onMounted, ref, watch } from 'vue';
import { useRoute, useRouter } from 'vue-router';
import { leesbaar } from '@regelrecht/frontend-shared/chronolex/waarden';
import { longDate } from '../gedeeld/datum';
import { antwoordOp, besluitStand, celNaam, isBesluit, ketens, omschrijving, onderVerzoeken, plat, rolVan, toegewezen } from '../gedeeld/keten';
import { burgerStand, doorHetSysteem as doorHetSysteemTekst } from '../gedeeld/vervolg';
import { foutTekst } from '../gedeeld/http';
import TijdlijnLijst from '../gedeeld/TijdlijnLijst.vue';
import type { Casus, Gram, Tijdlijn, Vervolgaanvraag } from '../gedeeld/types';
import { wijsDeWeg } from '../gedeeld/wegwijs';
import AanvraagView from './AanvraagView.vue';
import { burgerApi } from './api';
import BesluitPaneel from './BesluitPaneel.vue';
import StukPaneel from './StukPaneel.vue';
import { useBurger } from './useBurger';

const props = defineProps<{ id?: string }>();
const route = useRoute();
const router = useRouter();
const { casus, vormen, dossier, stand: standVanDossier, vervolg, doorHetSysteem, verzoeken, fout, laden, laad } = useBurger();
onMounted(laad);
// The dossier and a decision in it are one screen, so going back to the
// dossier after an objection keeps this component: load again on what was
// submitted, or the new record would not show.
watch(() => route.query.ingediend, (soort) => soort && laad());

// The timeline of the application the chosen record belongs to, again when
// another record is chosen or the dossier loads anew. Only the answer to the
// last request counts.
const tijdlijn = ref<Tijdlijn | null>(null);
const tijdlijnFout = ref('');
let tijdlijnVraag = 0;
watch(
  () => [props.id, laden.value] as const,
  async ([id, bezig], vorige) => {
    if (id !== vorige?.[0]) tijdlijn.value = null;
    if (!id || bezig) return;
    const deze = ++tijdlijnVraag;
    tijdlijnFout.value = '';
    try {
      const t = await burgerApi.tijdlijn(id);
      if (deze === tijdlijnVraag) tijdlijn.value = t;
    } catch (e) {
      if (deze === tijdlijnVraag) tijdlijnFout.value = foutTekst(e);
    }
  },
  { immediate: true },
);
/** What the timeline under the chosen record shows; every record of it opens in the dossier. */
const tijdlijnVan = (c: Casus) => ({
  tijdlijn: tijdlijn.value,
  fout: tijdlijnFout.value,
  casus: c,
  kant: 'burger' as const,
  teksten: vormen.value?.teksten,
  huidig: props.id,
  naar: (g: Gram) => ({ name: 'stuk', params: { id: g.id } }),
});

/** What the citizen just submitted, as its kind in words, from the address. */
const ingediend = computed(() => {
  const soort = route.query.ingediend;
  // Only a kind the case knows: the address does not get to say what was submitted.
  const bekend = typeof soort === 'string' && casus.value?.indieningen.some((i) => i.soort === soort);
  return bekend ? leesbaar(soort.toLowerCase()) : '';
});

/** Per chain its name and its rows, each with what the screen shows of it. */
const lijsten = computed(() => {
  const c = casus.value;
  if (!c) return [];
  return ketens(dossier.value, onderVerzoeken(verzoeken.value)).map((k) => ({
    naam: omschrijf(k.gram),
    rijen: plat(k).map(({ gram, boven }) => ({
      gram,
      tekst: omschrijf(gram),
      regel: regel(gram, boven),
      stand: stand(gram),
      gekozen: gram.id === props.id,
      vervolg: vervolgTekst(gram),
      aanvragen: aanvragenBij(gram.id),
    })),
  }));
});

// Someone with nothing in their dossier has not applied before. The screen
// then shows nothing at all — no sidebar, no empty pane — and the shell points
// at its button instead (gedeeld/wegwijs.ts): on a screen with one thing to do,
// that one thing is the screen. A dossier that fails to load is not empty, so
// the error keeps its place.
const leeg = computed(() => !laden.value && !fout.value && lijsten.value.length === 0);
watch(leeg, (l) => (wijsDeWeg.value = l), { immediate: true });

/** A record of the dossier in words, by what the server says it is. */
const omschrijf = (g: Gram) => (casus.value ? omschrijving(g, casus.value, rolVan(g, standVanDossier.value)) : g.kind);

/** How far the acts after the grant `id` are, in the citizen's words, if `id` granted and the case names such acts. */
function vervolgTekst(g: Gram): string {
  const na = casus.value?.na_toekenning;
  if (!na || !casus.value) return '';
  const s = vervolg.value.find((v) => v.toekenning === g.id);
  if (s) return burgerStand(s, na);
  // A grant in an aggregate whose grant is now the decision on an
  // application about a grant, such as a duplicate: that one took its place.
  // Not a grant a decision on objection replaced: that is a revocation.
  const vervangers = new Set((na.aanvragen ?? []).map((a) => a.artikel));
  const vervangen = vervolg.value.some((v) => {
    const nu = dossier.value.find((x) => x.id === v.toekenning);
    return v.toekenning !== g.id && !!nu && vervangers.has(nu.basis) && hoortBij(g, v.besluit);
  });
  return vervangen && toegewezen(g, casus.value) === true ? (na.vervallen ?? '') : '';
}

/** Whether `g` is the record `wortel` or refers to it, however indirectly. */
function hoortBij(g: Gram, wortel: string): boolean {
  const gezien = new Set<string>();
  let huidig: Gram | undefined = g;
  while (huidig && !gezien.has(huidig.id)) {
    if (huidig.id === wortel) return true;
    gezien.add(huidig.id);
    const naar: string | null | undefined = huidig.refers_to;
    huidig = naar ? dossier.value.find((x) => x.id === naar) : undefined;
  }
  return false;
}

/**
 * What the citizen can apply for about the grant `id`, as the case offers it
 * (`na_toekenning.aanvragen`): only once every act after it is done, and not
 * while such an application about it waits for a decision.
 */
function aanvragenBij(id: string): Vervolgaanvraag[] {
  const na = casus.value?.na_toekenning;
  const s = vervolg.value.find((v) => v.toekenning === id);
  if (!na?.aanvragen || !s || s.volgende) return [];
  return na.aanvragen.filter(
    (a) => !dossier.value.some((g) => g.basis === a.artikel && g.refers_to === id && !antwoordOp(g.id, standVanDossier.value)),
  );
}

/** Whether the record of the address is a decision, which has a pane of its own. */
const gekozenIsBesluit = computed(() => {
  const g = dossier.value.find((x) => x.id === props.id);
  return !!g && isBesluit(g, standVanDossier.value);
});

function regel(g: Gram, boven?: Gram): string {
  const c = casus.value;
  if (!c) return '';
  const delen = [longDate(g.recorded_on), celNaam(g.recording_actor, c)];
  if (boven) delen.push(`op ${omschrijf(boven).toLowerCase()} van ${longDate(boven.recorded_on)}`);
  const systeem = doorHetSysteemTekst(g.id, doorHetSysteem.value);
  if (systeem) delen.push(systeem);
  return delen.join(' · ');
}
function stand(g: Gram): { tekst: string; kleur: string } | null {
  return casus.value ? besluitStand(g, standVanDossier.value, dossier.value, casus.value) : null;
}
</script>

<template>
  <nldd-navigation-split-view v-if="!leeg" @back="router.push('/burger')">
    <nldd-page slot="primary-sidebar" sticky-header accessible-label="Mijn dossier">
        <nldd-top-title-bar slot="header" text="Mijn dossier" heading-level="3" collapse-anchor="dossier-titel"></nldd-top-title-bar>
        <nldd-simple-section width="full">
          <nldd-title id="dossier-titel" size="display-3"><h3>Mijn dossier</h3></nldd-title>
          <nldd-text color="secondary"><p>Alles wat over u is vastgelegd, per aanvraag.</p></nldd-text>
          <nldd-spacer size="16"></nldd-spacer>
          <nldd-activity-indicator v-if="laden" text="Het dossier laden" show-text></nldd-activity-indicator>
          <nldd-inline-dialog v-else-if="fout" icon="warning" icon-color="critical" text="Het dossier laadt niet" :supporting-text="fout"></nldd-inline-dialog>
          <template v-else-if="casus">
            <template v-if="ingediend">
              <nldd-banner
                variant="success"
                :text="`Uw ${ingediend} is ingediend`"
                supporting-text="Het besluit verschijnt hier zodra een medewerker klaar is."
              ></nldd-banner>
              <nldd-spacer size="16"></nldd-spacer>
            </template>
            <template v-for="(l, i) in lijsten" :key="i">
              <nldd-spacer v-if="i > 0" size="24"></nldd-spacer>
              <nldd-title size="display-5"><h4>{{ l.naam }}</h4></nldd-title>
              <nldd-list appearance="simple" :accessible-label="l.naam">
                <template v-for="r in l.rijen" :key="r.gram.id">
                  <nldd-list-item
                    size="md"
                    button
                    :current="r.gekozen || undefined"
                    @click="router.push({ name: 'stuk', params: { id: r.gram.id } })"
                  >
                    <nldd-text-cell :text="r.tekst" :supporting-text="r.regel"></nldd-text-cell>
                    <nldd-cell v-if="r.stand">
                      <nldd-tag :text="r.stand.tekst" :color="r.stand.kleur" size="sm"></nldd-tag>
                    </nldd-cell>
                    <nldd-spacer-cell size="8"></nldd-spacer-cell>
                    <nldd-icon-cell size="20"><nldd-icon icon="chevron-right"></nldd-icon></nldd-icon-cell>
                  </nldd-list-item>
                  <nldd-list-item v-if="r.vervolg" size="md">
                    <nldd-icon-cell size="20"><nldd-icon icon="info"></nldd-icon></nldd-icon-cell>
                    <nldd-spacer-cell size="8"></nldd-spacer-cell>
                    <nldd-text-cell :text="r.vervolg" supporting-text="Na de toekenning"></nldd-text-cell>
                  </nldd-list-item>
                  <nldd-list-item
                    v-for="a in r.aanvragen"
                    :key="`${r.gram.id}-${a.artikel}`"
                    size="md"
                    button
                    @click="router.push({ name: 'aanvraag', query: { artikel: a.artikel, op: r.gram.id } })"
                  >
                    <nldd-icon-cell size="20"><nldd-icon icon="add"></nldd-icon></nldd-icon-cell>
                    <nldd-spacer-cell size="8"></nldd-spacer-cell>
                    <nldd-text-cell :text="a.knop" :supporting-text="a.titel"></nldd-text-cell>
                    <nldd-icon-cell size="20"><nldd-icon icon="chevron-right"></nldd-icon></nldd-icon-cell>
                  </nldd-list-item>
                </template>
              </nldd-list>
            </template>
          </template>
        </nldd-simple-section>
      </nldd-page>

    <div slot="main" :empty="id ? undefined : true">
      <BesluitPaneel v-if="id && casus && gekozenIsBesluit" :id="id" :casus="casus" :vormen="vormen" :dossier="dossier" :stand="standVanDossier">
        <template #onder><TijdlijnLijst v-bind="tijdlijnVan(casus)" /></template>
      </BesluitPaneel>
      <StukPaneel v-else-if="id && casus && !laden" :id="id" :casus="casus" :vormen="vormen" :dossier="dossier" :stand="standVanDossier">
        <template #onder><TijdlijnLijst v-bind="tijdlijnVan(casus)" /></template>
      </StukPaneel>
      <nldd-page v-else-if="id && laden" landmarks="page">
        <nldd-simple-section width="full">
          <nldd-activity-indicator text="Het stuk laden" show-text></nldd-activity-indicator>
        </nldd-simple-section>
      </nldd-page>
      <nldd-page v-else landmarks="page">
        <nldd-simple-section width="full">
          <nldd-inline-dialog text="Kies een stuk" supporting-text="Wat u kiest, opent hier, naast uw dossier."></nldd-inline-dialog>
        </nldd-simple-section>
      </nldd-page>
    </div>
  </nldd-navigation-split-view>

  <AanvraagView :open="route.name === 'aanvraag'" @sluit="router.push('/burger')" />
</template>
