<script setup lang="ts">
// The citizen's applications, as one list with a row per application, from
// the dossier the server derives from the chronicles on every request (`GET
// /api/dossier`). A row says what was applied for, when and where, how it
// stands (`stand`: what answers it now), and how far the acts after its grant
// are, in the words the case gives them (`na_toekenning`), such as "Uw kaart
// ligt klaar.". The rows are made in gedeeld/dossier.ts, which the list over
// every process of an organisation shares. Each row opens the application on a page one level deeper,
// whose back button returns to the list (StukPaneel.vue). What followed on it
// opens from there: its decision, and every record in the timeline of the
// application (gedeeld/TijdlijnLijst.vue, `GET /api/dossier/tijdlijn/{id}`), a
// decision in BesluitPaneel.vue, a request of the policy in a sheet over the
// page (gedeeld/VerzoekSheet.vue), anything else in StukPaneel.vue. An
// application about a grant, such as a duplicate, is a row of its own; the way
// in to one stands on the page of the application whose grant it is about.
import { computed, onMounted, ref, watch } from 'vue';
import { useRoute, useRouter } from 'vue-router';
import { leesbaar } from '@regelrecht/frontend-shared/chronolex/waarden';
import { longDate } from '../gedeeld/datum';
import { aanvraagStand, aanvragen as aanvragenVan } from '../gedeeld/dossier';
import type { Dossier } from '../gedeeld/dossier';
import { isVerzending, verzondenBericht } from '../gedeeld/bericht';
import { celNaam, isBesluit } from '../gedeeld/keten';
import { foutTekst } from '../gedeeld/http';
import TijdlijnLijst from '../gedeeld/TijdlijnLijst.vue';
import type { Casus, Gram, Tijdlijn } from '../gedeeld/types';
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
/** What the timeline under the chosen record shows; every record of it opens on its own screen in the dossier. */
const tijdlijnVan = (c: Casus) => ({
  tijdlijn: tijdlijn.value,
  fout: tijdlijnFout.value,
  casus: c,
  kant: 'burger' as const,
  teksten: vormen.value?.teksten,
  huidig: props.id,
  opent: (g: Gram) => () => void router.push({ name: 'stuk', params: { id: g.id } }),
  verzoeken: verzoeken.value,
});

/** What the citizen just submitted, as its kind in words, from the address. */
const ingediend = computed(() => {
  const soort = route.query.ingediend;
  // Only a kind the case knows: the address does not get to say what was submitted.
  const bekend = typeof soort === 'string' && casus.value?.indieningen.some((i) => i.soort === soort);
  return bekend ? leesbaar(soort.toLowerCase()) : '';
});

/** The dossier as one value, for the rows made of it (gedeeld/dossier.ts). */
const heelDossier = computed<Dossier>(() => ({
  grammen: dossier.value,
  stand: standVanDossier.value,
  vervolg: vervolg.value,
  doorHetSysteem: doorHetSysteem.value,
  verzoeken: verzoeken.value,
}));

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
  return aanvragenVan(c, heelDossier.value).map((a) => ({
    ...a,
    regel: [`${longDate(a.gram.recorded_on)} · ${celNaam(a.gram.recording_actor, c)}`, a.vervolg].filter(Boolean).join(' · '),
  }));
});
/**
 * The messages the citizen received: every fact of sending in her dossier (`verzending` in the message spec), newest
 * first, each with what it is about. The message opens from its own fact, frozen as it went (StukPaneel.vue).
 */
const berichten = computed(() =>
  dossier.value
    .filter((g) => isVerzending(g, vormen.value?.berichten) && `${g.basis}#${g.fact}` === vormen.value?.berichten?.verzending?.feit)
    .map((g) => {
      const m = verzondenBericht(g);
      return { gram: g, tekst: m?.onderwerp ?? 'Bericht', regel: `${longDate(String(g.fields?.verzonden_op ?? g.recorded_on))} · ${m?.lint ?? m?.afzender ?? ''}` };
    })
    .reverse(),
);
/** What the application of the address offers after its grant, for its own page. */
const huidige = computed(() => aanvragen.value.find((a) => a.gram.id === props.id));
/** How the record of the address stands, an objection as well as an application. */
const statusVanStuk = computed(() => {
  const g = dossier.value.find((x) => x.id === props.id);
  return g && casus.value ? aanvraagStand(g, casus.value, heelDossier.value) : null;
});

// Someone with nothing in their dossier has not applied before. The screen
// then shows nothing at all — no sidebar, no empty pane — and the shell points
// at its button instead (gedeeld/wegwijs.ts): on a screen with one thing to do,
// that one thing is the screen. A dossier that fails to load is not empty, so
// the error keeps its place.
const leeg = computed(() => !laden.value && !fout.value && aanvragen.value.length === 0);
watch(leeg, (l) => (wijsDeWeg.value = l), { immediate: true });


/** Whether the record of the address is a decision, which has a pane of its own. */
const gekozenIsBesluit = computed(() => {
  const g = dossier.value.find((x) => x.id === props.id);
  return !!g && isBesluit(g, standVanDossier.value);
});

</script>

<template>
  <template v-if="!leeg">
    <BesluitPaneel v-if="id && casus && gekozenIsBesluit" :id="id" :casus="casus" :vormen="vormen" :dossier="dossier" :stand="standVanDossier">
      <template #naast><TijdlijnLijst v-bind="tijdlijnVan(casus)" /></template>
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
      :naam="huidige?.tekst"
      :status="statusVanStuk"
    >
      <template #naast><TijdlijnLijst v-bind="tijdlijnVan(casus)" /></template>
    </StukPaneel>
    <nldd-page v-else-if="id" landmarks="page">
      <nldd-simple-section>
        <nldd-activity-indicator text="Het stuk laden" show-text></nldd-activity-indicator>
      </nldd-simple-section>
    </nldd-page>
    <nldd-page v-else landmarks="page" sticky-header accessible-label="Mijn aanvragen">
      <nldd-top-title-bar slot="header" text="Mijn aanvragen" heading-level="3" collapse-anchor="dossier-titel"></nldd-top-title-bar>
      <nldd-simple-section>
        <nldd-title id="dossier-titel" size="display-2"><h3>Mijn aanvragen</h3></nldd-title>
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
          <template v-if="berichten.length">
            <nldd-spacer size="32"></nldd-spacer>
            <nldd-title size="display-4"><h4>Mijn berichten</h4></nldd-title>
            <nldd-spacer size="8"></nldd-spacer>
            <nldd-list appearance="simple" accessible-label="Mijn berichten">
              <nldd-list-item
                v-for="b in berichten"
                :key="b.gram.id"
                size="md"
                button
                @click="router.push({ name: 'stuk', params: { id: b.gram.id } })"
              >
                <nldd-icon-cell size="20"><nldd-icon icon="envelope"></nldd-icon></nldd-icon-cell>
                <nldd-spacer-cell size="8"></nldd-spacer-cell>
                <nldd-text-cell :text="b.tekst" :supporting-text="b.regel"></nldd-text-cell>
                <nldd-spacer-cell size="8"></nldd-spacer-cell>
                <nldd-icon-cell size="20"><nldd-icon icon="chevron-right"></nldd-icon></nldd-icon-cell>
              </nldd-list-item>
            </nldd-list>
          </template>
        </template>
      </nldd-simple-section>
      <nldd-page-footer slot="footer"></nldd-page-footer>
    </nldd-page>
  </template>

  <AanvraagView
    :open="route.hash === AANVRAAG || route.hash === AANVRAAG_WETTEN"
    @sluit="router.push({ path: route.path })"
    @ingediend="laad"
  />
</template>
