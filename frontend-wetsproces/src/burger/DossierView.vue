<script setup lang="ts">
// The citizen's dossier: every record about them, in every cell, as chains of
// what answers to what: the application, the decision on it, an objection to
// that decision and the decision on the objection. Derived by the server from
// the chronicles on every request (`GET /api/dossier`).
import { computed, onMounted } from 'vue';
import { useRoute, useRouter } from 'vue-router';
import { leesbaar } from '@regelrecht/frontend-shared/chronolex/waarden';
import { longDate } from '../gedeeld/datum';
import { celNaam, isBesluit, ketens, omschrijving, plat, toegewezen } from '../gedeeld/keten';
import type { Gram } from '../gedeeld/types';
import { useBurger } from './useBurger';

const route = useRoute();
const router = useRouter();
const { casus, dossier, fout, laden, laad } = useBurger();
onMounted(laad);

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
  <nldd-simple-section>
    <nldd-title slot="header" heading-level="1" text="Mijn dossier" supporting-text="Alles wat over u is vastgelegd, per aanvraag."></nldd-title>
    <nldd-activity-indicator v-if="laden"></nldd-activity-indicator>
    <nldd-inline-dialog v-else-if="fout" icon="warning" icon-color="critical" text="Het dossier laadt niet" :supporting-text="fout"></nldd-inline-dialog>
    <nldd-container v-else-if="casus" gap="16">
      <nldd-banner
        v-if="ingediend"
        variant="success"
        :text="`Uw ${ingediend} is ingediend`"
        supporting-text="Het besluit verschijnt hier zodra een medewerker klaar is."
      ></nldd-banner>
      <nldd-inline-dialog v-if="lijsten.length === 0" icon="info" text="Er is nog niets over u vastgelegd">
        <nldd-button slot="actions" appearance="primary" text="Aanvraag doen" @click="router.push('/burger/aanvraag')"></nldd-button>
      </nldd-inline-dialog>
      <nldd-list v-for="(l, i) in lijsten" :key="i" appearance="box-base" :accessible-label="l.naam">
        <nldd-list-item v-for="r in l.rijen" :key="r.gram.id">
          <nldd-text-cell :text="r.tekst" :supporting-text="r.regel"></nldd-text-cell>
          <nldd-cell v-if="r.stand">
            <nldd-tag :text="r.stand.tekst" :color="r.stand.kleur" size="sm"></nldd-tag>
          </nldd-cell>
          <nldd-cell v-if="r.besluit">
            <nldd-button
              appearance="secondary"
              size="sm"
              text="Bekijk het besluit"
              @click="router.push({ name: 'besluit', params: { id: r.gram.id } })"
            ></nldd-button>
          </nldd-cell>
        </nldd-list-item>
      </nldd-list>
    </nldd-container>
  </nldd-simple-section>
</template>
