<script setup lang="ts">
// "Mijn aanvragen" on the citizens' side of an organisation's front: the
// citizen's applications in every process of the organisation, as one list,
// newest first (mijnAanvragen.ts). A row says what was applied for, in which
// process (a tag), when, and how it stands; it opens the application in its
// process through the front's hand-off, so the citizen arrives there logged
// in. A process that did not answer gets a notice above the list, and the
// applications of the others stay.
import { computed, onMounted, ref } from 'vue';
import { longDate } from '../gedeeld/datum';
import { foutTekst } from '../gedeeld/http';
import { organisatieApi, tegelNaam } from './api';
import { mijnAanvragen } from './mijnAanvragen';
import type { MijnAanvraag, MijnAanvragen } from './mijnAanvragen';

const KOP = 'Mijn aanvragen';
const lijst = ref<MijnAanvragen | null>(null);
const fout = ref('');

onMounted(async () => {
  try {
    const organisatie = await organisatieApi.overzicht();
    lijst.value = await mijnAanvragen(organisatie.processen);
  } catch (e) {
    fout.value = foutTekst(e);
  }
});

/** The line under a row: the day it was applied for, and how far the acts after its grant are. */
const regel = (a: MijnAanvraag): string => [longDate(a.datum), a.vervolg].filter(Boolean).join(' · ');
/** What a screen reader hears of a row: all of it, the process and the status included. */
const label = (a: MijnAanvraag): string =>
  [a.tekst, tegelNaam(a.proces), regel(a), a.stand?.tekst].filter(Boolean).join(', ');
const leeg = computed(() => !!lijst.value && !lijst.value.aanvragen.length && !lijst.value.nietBereikbaar.length);
</script>

<template>
  <nldd-page landmarks="page" sticky-header :accessible-label="KOP">
    <nldd-top-title-bar slot="header" :text="KOP" collapse-anchor="mijn-aanvragen-titel"></nldd-top-title-bar>
    <nldd-simple-section>
      <nldd-container gap="16">
        <nldd-title
          id="mijn-aanvragen-titel"
          size="display-3"
          heading-level="1"
          :text="KOP"
          supporting-text="Uw aanvragen bij alle regelingen van deze organisatie. Kies een aanvraag om die te openen; u bent daar al ingelogd."
        ></nldd-title>
        <nldd-activity-indicator v-if="!lijst && !fout" text="Uw aanvragen worden geladen" show-text></nldd-activity-indicator>
        <nldd-inline-dialog
          v-if="fout"
          icon="warning"
          icon-color="critical"
          text="Uw aanvragen zijn niet te laden"
          :supporting-text="fout"
        ></nldd-inline-dialog>
        <template v-else-if="lijst">
          <nldd-inline-dialog
            v-for="p in lijst.nietBereikbaar"
            :key="p.id"
            icon="warning"
            icon-color="warning"
            :text="`${tegelNaam(p)} is nu niet bereikbaar`"
            supporting-text="Uw aanvragen daar staan nu niet in deze lijst. Probeer het later opnieuw."
          ></nldd-inline-dialog>
          <nldd-inline-dialog
            v-if="leeg"
            icon="info"
            text="U hebt nog niets aangevraagd"
            supporting-text="Kies bij Aanvragen wat u wilt aanvragen."
          ></nldd-inline-dialog>
          <nldd-list v-if="lijst.aanvragen.length" appearance="simple" :accessible-label="KOP">
            <nldd-list-item v-for="a in lijst.aanvragen" :key="`${a.proces.id}/${a.id}`" size="md" :href="a.href" :accessible-label="label(a)">
              <nldd-text-cell :text="a.tekst" :supporting-text="regel(a)"></nldd-text-cell>
              <nldd-spacer-cell size="16"></nldd-spacer-cell>
              <nldd-cell><nldd-tag :text="tegelNaam(a.proces)" size="sm"></nldd-tag></nldd-cell>
              <template v-if="a.stand">
                <nldd-spacer-cell size="16"></nldd-spacer-cell>
                <!-- A dot beside the status word: the word says it, so the dot is decorative. -->
                <nldd-cell><nldd-badge :color="a.stand.kleur" size="sm" decorative></nldd-badge></nldd-cell>
                <nldd-spacer-cell size="8"></nldd-spacer-cell>
                <nldd-cell><nldd-text size="sm"><p>{{ a.stand.tekst }}</p></nldd-text></nldd-cell>
              </template>
              <nldd-spacer-cell size="8"></nldd-spacer-cell>
              <nldd-icon-cell size="20"><nldd-icon icon="chevron-right"></nldd-icon></nldd-icon-cell>
            </nldd-list-item>
          </nldd-list>
        </template>
      </nldd-container>
    </nldd-simple-section>
  </nldd-page>
</template>
