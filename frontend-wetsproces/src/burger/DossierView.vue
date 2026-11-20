<script setup lang="ts">
// The citizen's dossier: every record about them, in every cell, as chains of
// what answers to what: the application, the decision on it, an objection to
// that decision and the decision on the objection. Derived by the server from
// the chronicles on every request (`GET /api/dossier`). Laid out as the
// editor's library: the dossier in the sidebar, a decision chosen from it in
// the main pane beside it (BesluitPaneel.vue).
import { computed, onMounted, watch } from 'vue';
import { useRoute, useRouter } from 'vue-router';
import { leesbaar } from '@regelrecht/frontend-shared/chronolex/waarden';
import { longDate } from '../gedeeld/datum';
import { celNaam, isBesluit, ketens, omschrijving, plat, toegewezen } from '../gedeeld/keten';
import type { Gram } from '../gedeeld/types';
import BesluitPaneel from './BesluitPaneel.vue';
import { useBurger } from './useBurger';

const props = defineProps<{ id?: string }>();
const route = useRoute();
const router = useRouter();
const { casus, vormen, dossier, fout, laden, laad } = useBurger();
onMounted(laad);
// The dossier and a decision in it are one screen, so going back to the
// dossier after an objection keeps this component: load again on what was
// submitted, or the new record would not show.
watch(() => route.query.ingediend, (soort) => soort && laad());

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
  return ketens(dossier.value).map((k) => ({
    naam: omschrijving(k.gram, c),
    rijen: plat(k).map(({ gram, boven }) => ({
      gram,
      tekst: omschrijving(gram, c),
      regel: regel(gram, boven),
      stand: stand(gram),
      besluit: isBesluit(gram, c),
      gekozen: gram.id === props.id,
    })),
  }));
});

function regel(g: Gram, boven?: Gram): string {
  const c = casus.value;
  if (!c) return '';
  const delen = [longDate(g.recorded_on), celNaam(g.recording_actor, c)];
  if (boven) delen.push(`op ${omschrijving(boven, c).toLowerCase()} van ${longDate(boven.recorded_on)}`);
  return delen.join(' · ');
}
function stand(g: Gram): { tekst: string; kleur: string } | null {
  const t = casus.value ? toegewezen(g, casus.value) : undefined;
  if (t === undefined) return null;
  return t ? { tekst: 'Toegewezen', kleur: 'success' } : { tekst: 'Afgewezen', kleur: 'critical' };
}
</script>

<template>
  <nldd-navigation-split-view @back="router.push('/burger')">
    <nldd-split-view-pane slot="primary-sidebar" has-content>
      <nldd-page sticky-header accessible-label="Mijn dossier">
        <nldd-top-title-bar slot="header" text="Mijn dossier" heading-level="3" collapse-anchor="dossier-titel"></nldd-top-title-bar>
        <nldd-simple-section width="full">
          <nldd-title id="dossier-titel" size="display-3"><h3>Mijn dossier</h3></nldd-title>
          <nldd-text color="secondary">Alles wat over u is vastgelegd, per aanvraag.</nldd-text>
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
            <nldd-inline-dialog v-if="lijsten.length === 0" icon="info" text="Er is nog niets over u vastgelegd">
              <nldd-button slot="actions" appearance="primary" text="Aanvraag doen" @click="router.push('/burger/aanvraag')"></nldd-button>
            </nldd-inline-dialog>
            <template v-for="(l, i) in lijsten" :key="i">
              <nldd-spacer v-if="i > 0" size="24"></nldd-spacer>
              <nldd-title size="display-5"><h4>{{ l.naam }}</h4></nldd-title>
              <nldd-list appearance="simple" :accessible-label="l.naam">
                <nldd-list-item
                  v-for="r in l.rijen"
                  :key="r.gram.id"
                  size="md"
                  :button="r.besluit || undefined"
                  :current="r.gekozen || undefined"
                  @click="r.besluit && router.push({ name: 'besluit', params: { id: r.gram.id } })"
                >
                  <nldd-text-cell :text="r.tekst" :supporting-text="r.regel"></nldd-text-cell>
                  <nldd-cell v-if="r.stand">
                    <nldd-tag :text="r.stand.tekst" :color="r.stand.kleur" size="sm"></nldd-tag>
                  </nldd-cell>
                  <template v-if="r.besluit">
                    <nldd-spacer-cell size="8"></nldd-spacer-cell>
                    <nldd-icon-cell size="20"><nldd-icon icon="chevron-right"></nldd-icon></nldd-icon-cell>
                  </template>
                </nldd-list-item>
              </nldd-list>
            </template>
          </template>
        </nldd-simple-section>
      </nldd-page>
    </nldd-split-view-pane>

    <nldd-split-view-pane slot="main" :has-content="id || undefined">
      <BesluitPaneel v-if="id && casus" :id="id" :casus="casus" :vormen="vormen" :dossier="dossier" />
      <nldd-page v-else landmarks="page">
        <nldd-simple-section width="full">
          <nldd-inline-dialog text="Kies een besluit" supporting-text="Een besluit opent hier, naast uw dossier."></nldd-inline-dialog>
        </nldd-simple-section>
      </nldd-page>
    </nldd-split-view-pane>
  </nldd-navigation-split-view>
</template>
