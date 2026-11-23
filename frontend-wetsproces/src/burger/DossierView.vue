<script setup lang="ts">
// The citizen's applications, as one list with a row per application, from
// the dossier the server derives from the chronicles on every request (`GET
// /api/dossier`). A row says what was applied for, when and where, how it
// stands (`stand`: what answers it now), and how far the acts after its grant
// are, in the words the case gives them (`na_toekenning`), such as "Uw kaart
// ligt klaar.". Each row opens the application on a page one level deeper,
// whose back button returns to the list (StukPaneel.vue). What followed on it
// opens from there: its decision, and every record in the timeline of the
// application (gedeeld/TijdlijnLijst.vue, `GET /api/dossier/tijdlijn/{id}`), a
// decision in BesluitPaneel.vue, anything else in StukPaneel.vue. An
// application about a grant, such as a duplicate, is a row of its own; the way
// in to one stands on the page of the application whose grant it is about.
import { computed, onMounted, ref, watch } from 'vue';
import { useRoute, useRouter } from 'vue-router';
import { leesbaar } from '@regelrecht/frontend-shared/chronolex/waarden';
import { longDate } from '../gedeeld/datum';
import { antwoordOp, celNaam, herroeping, isBesluit, isIndiening, ketens, omschrijving, onderVerzoeken, rolVan, toegewezen } from '../gedeeld/keten';
import type { Knoop } from '../gedeeld/keten';
import { burgerStand, doorHetSysteem as doorHetSysteemTekst } from '../gedeeld/vervolg';
import { foutTekst } from '../gedeeld/http';
import TijdlijnLijst from '../gedeeld/TijdlijnLijst.vue';
import type { Casus, Gram, Tijdlijn, Vervolgaanvraag } from '../gedeeld/types';
import { wijsDeWeg } from '../gedeeld/wegwijs';
import AanvraagView from './AanvraagView.vue';
import { AANVRAAG, AANVRAAG_WETTEN } from './routes';
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

/** The articles of an application: the case's own, and those it offers about a grant, such as a duplicate. */
const aanvraagArtikelen = computed(
  () => new Set([casus.value?.artikel, ...(casus.value?.na_toekenning?.aanvragen ?? []).map((a) => a.artikel)]),
);
const isAanvraag = (g: Gram) => isIndiening(g, standVanDossier.value) && aanvraagArtikelen.value.has(g.basis);
/** `k` and every record under it. */
const alleKnopen = (k: Knoop): Knoop[] => [k, ...k.onder.flatMap(alleKnopen)];
/** The records of the application `k`: it and what follows on it, without another application under it. */
const vanAanvraag = (k: Knoop): Gram[] => [k.gram, ...k.onder.filter((o) => !isAanvraag(o.gram)).flatMap(vanAanvraag)];

/**
 * Every application in the dossier, as one row each: what was applied for,
 * when and where, how it stands, and how far the acts after its grant are.
 * What followed on it (an investigation, the decision, an objection) opens
 * from the application's own page; an application about its grant, such as
 * a duplicate, is a row of its own.
 */
const aanvragen = computed(() => {
  const c = casus.value;
  if (!c) return [];
  return ketens(dossier.value, onderVerzoeken(verzoeken.value))
    .flatMap(alleKnopen)
    .filter((k) => isAanvraag(k.gram))
    .map((k) => {
      const stukken = vanAanvraag(k);
      const vervolg = stukken.map(vervolgTekst).filter(Boolean).at(-1) ?? '';
      return {
        gram: k.gram,
        tekst: aanvraagNaam(k.gram, c),
        regel: [`${longDate(k.gram.recorded_on)} · ${celNaam(k.gram.recording_actor, c)}`, vervolg].filter(Boolean).join(' · '),
        stand: aanvraagStand(k.gram),
        vervolg,
        aanvragen: stukken.flatMap((g) => aanvragenBij(g.id).map((a) => ({ ...a, op: g.id }))),
      };
    });
});
/** What the application of the address offers after its grant, for its own page. */
const huidige = computed(() => aanvragen.value.find((a) => a.gram.id === props.id));

/** What an application asks, in words: the case's own after its title, one about a grant by the title of its form. */
function aanvraagNaam(g: Gram, c: Casus): string {
  const over = c.na_toekenning?.aanvragen?.find((a) => a.artikel === g.basis);
  if (over) return over.titel;
  return c.titel ? `${omschrijf(g)} ${c.titel.toLowerCase()}` : omschrijf(g);
}

/** How an application stands, by what answers it now (`stand`). */
function aanvraagStand(g: Gram): { tekst: string; kleur: string } | null {
  const c = casus.value;
  const a = antwoordOp(g.id, standVanDossier.value);
  if (!c || !a) return null;
  // Revoked with a new decision to follow, after a ruling: it waits again.
  if (a.soort === 'open' || (a.soort === 'vervallen' && a.na)) return { tekst: 'In behandeling', kleur: 'accent' };
  if (a.soort === 'vervallen') {
    return { tekst: herroeping(a.besluit, standVanDossier.value, dossier.value)?.woord ?? 'Herroepen', kleur: 'warning' };
  }
  const t = toegewezen(a.soort === 'in_de_plaats' ? a.nieuw : (dossier.value.find((x) => x.id === a.besluit) ?? {}), c);
  if (t === undefined) return null;
  return t ? { tekst: 'Toegewezen', kleur: 'success' } : { tekst: 'Afgewezen', kleur: 'critical' };
}

// Someone with nothing in their dossier has not applied before. The screen
// then shows nothing at all — no sidebar, no empty pane — and the shell points
// at its button instead (gedeeld/wegwijs.ts): on a screen with one thing to do,
// that one thing is the screen. A dossier that fails to load is not empty, so
// the error keeps its place.
const leeg = computed(() => !laden.value && !fout.value && aanvragen.value.length === 0);
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

</script>

<template>
  <template v-if="!leeg">
    <BesluitPaneel v-if="id && casus && gekozenIsBesluit" :id="id" :casus="casus" :vormen="vormen" :dossier="dossier" :stand="standVanDossier">
      <template #onder><TijdlijnLijst v-bind="tijdlijnVan(casus)" /></template>
    </BesluitPaneel>
    <StukPaneel
      v-else-if="id && casus && !laden"
      :id="id"
      :casus="casus"
      :vormen="vormen"
      :dossier="dossier"
      :stand="standVanDossier"
      :vervolg="huidige?.vervolg"
      :aanvragen="huidige?.aanvragen"
    >
      <template #onder><TijdlijnLijst v-bind="tijdlijnVan(casus)" /></template>
    </StukPaneel>
    <nldd-page v-else-if="id" landmarks="page">
      <nldd-simple-section>
        <nldd-activity-indicator text="Het stuk laden" show-text></nldd-activity-indicator>
      </nldd-simple-section>
    </nldd-page>
    <nldd-page v-else landmarks="page" sticky-header accessible-label="Mijn aanvragen">
      <nldd-top-title-bar slot="header" text="Mijn aanvragen" heading-level="3" collapse-anchor="dossier-titel"></nldd-top-title-bar>
      <nldd-simple-section>
        <nldd-title id="dossier-titel" size="display-3"><h3>Mijn aanvragen</h3></nldd-title>
        <nldd-spacer size="16"></nldd-spacer>
        <nldd-activity-indicator v-if="laden" text="Uw aanvragen laden" show-text></nldd-activity-indicator>
        <nldd-inline-dialog v-else-if="fout" icon="warning" icon-color="critical" text="Uw aanvragen laden niet" :supporting-text="fout"></nldd-inline-dialog>
        <template v-else-if="casus">
          <template v-if="ingediend">
            <nldd-banner
              variant="success"
              :text="`Uw ${ingediend} is ingediend`"
              supporting-text="Het besluit verschijnt hier zodra een medewerker klaar is."
            ></nldd-banner>
            <nldd-spacer size="16"></nldd-spacer>
          </template>
          <nldd-list appearance="simple" accessible-label="Mijn aanvragen">
            <nldd-list-item
              v-for="r in aanvragen"
              :key="r.gram.id"
              size="md"
              button
              @click="router.push({ name: 'stuk', params: { id: r.gram.id } })"
            >
              <nldd-text-cell :text="r.tekst" :supporting-text="r.regel"></nldd-text-cell>
              <template v-if="r.stand">
                <nldd-spacer-cell size="16"></nldd-spacer-cell>
                <!-- A dot beside the status word: the word says it, so the dot is decorative. -->
                <nldd-cell><nldd-badge :color="r.stand.kleur" size="sm" decorative></nldd-badge></nldd-cell>
                <nldd-spacer-cell size="8"></nldd-spacer-cell>
                <nldd-cell><nldd-text size="sm"><p>{{ r.stand.tekst }}</p></nldd-text></nldd-cell>
              </template>
              <nldd-spacer-cell size="8"></nldd-spacer-cell>
              <nldd-icon-cell size="20"><nldd-icon icon="chevron-right"></nldd-icon></nldd-icon-cell>
            </nldd-list-item>
          </nldd-list>
        </template>
      </nldd-simple-section>
    </nldd-page>
  </template>

  <AanvraagView
    :open="route.hash === AANVRAAG || route.hash === AANVRAAG_WETTEN"
    @sluit="router.push({ path: route.path })"
    @ingediend="laad"
  />
</template>
