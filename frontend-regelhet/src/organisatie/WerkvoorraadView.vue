<script setup lang="ts">
// The work queue of an organisation's front: what waits for a decision in
// every process of the organisation, in one list, the longest waiting first,
// each row with the process it is from. A segmented control narrows it to one
// process. A row opens the submission in the handling screen of its process,
// handed over so the employee does not log in again. A process that does not
// answer is named above the list, and the rows of the others stay.
import { computed, onMounted, ref } from 'vue';
import { foutTekst } from '../gedeeld/http';
import { organisatieApi } from './api';
import type { Proces } from './api';
import { ALLE, behandelAdres, filter, laadWerkvoorraad, procesNaam, samenvoegen } from './werkvoorraad';
import type { Samengevoegd } from './werkvoorraad';

const processen = ref<Proces[]>([]);
const stand = ref<Samengevoegd | null>(null);
const fout = ref('');
const gekozen = ref(ALLE);

onMounted(async () => {
  try {
    processen.value = (await organisatieApi.overzicht()).processen;
    stand.value = samenvoegen(await laadWerkvoorraad(processen.value));
  } catch (e) {
    fout.value = foutTekst(e);
  }
});

const zichtbaar = computed(() => (stand.value ? filter(stand.value.rijen, gekozen.value) : []));
/** The value the segmented control reports, from its `change` event. */
const kies = (e: Event) => {
  const waarde = (e as CustomEvent<{ value?: unknown }>).detail?.value;
  gekozen.value = typeof waarde === 'string' ? waarde : ALLE;
};
</script>

<template>
  <nldd-page landmarks="page" sticky-header accessible-label="Werkvoorraad">
    <nldd-top-title-bar slot="header" text="Werkvoorraad" collapse-anchor="organisatie-werkvoorraad-titel"></nldd-top-title-bar>
    <nldd-simple-section>
      <nldd-container gap="16">
        <nldd-title
          id="organisatie-werkvoorraad-titel"
          size="display-3"
          heading-level="1"
          text="Werkvoorraad"
          supporting-text="De aanvragen die op een besluit wachten, van alle regelingen samen. Een aanvraag opent in de regeling zelf, zonder opnieuw in te loggen."
        ></nldd-title>
        <nldd-activity-indicator v-if="!stand && !fout" text="De werkvoorraad wordt geladen" show-text></nldd-activity-indicator>
        <nldd-inline-dialog v-if="fout" icon="warning" icon-color="critical" text="De werkvoorraad laadt niet" :supporting-text="fout"></nldd-inline-dialog>
        <template v-else-if="stand">
          <nldd-inline-dialog
            v-for="o in stand.onbereikbaar"
            :key="o.proces.id"
            icon="warning"
            icon-color="warning"
            :text="`${procesNaam(o.proces)} is nu niet bereikbaar`"
            :supporting-text="`Wat daar wacht, staat niet in deze lijst. ${o.fout}`"
          ></nldd-inline-dialog>
          <nldd-segmented-control
            v-if="processen.length > 1"
            accessible-label="Regeling"
            width="fit-content"
            :value="gekozen"
            @change="kies"
          >
            <nldd-segmented-control-item :value="ALLE" text="Alle"></nldd-segmented-control-item>
            <nldd-segmented-control-item v-for="p in processen" :key="p.id" :value="p.id" :text="procesNaam(p)"></nldd-segmented-control-item>
          </nldd-segmented-control>
          <nldd-list v-if="zichtbaar.length" appearance="box-base" accessible-label="Werkvoorraad">
            <nldd-list-item v-for="r in zichtbaar" :key="`${r.proces.id}/${r.stuk.id}`" size="md" :href="behandelAdres(r)">
              <nldd-text-cell :text="r.tekst" :supporting-text="r.regel"></nldd-text-cell>
              <nldd-cell>
                <nldd-tag :text="procesNaam(r.proces)" size="sm"></nldd-tag>
              </nldd-cell>
              <nldd-cell v-if="r.opnieuw">
                <nldd-badge text="Opnieuw beslissen" color="warning" size="sm"></nldd-badge>
              </nldd-cell>
              <nldd-spacer-cell size="8"></nldd-spacer-cell>
              <nldd-icon-cell size="20"><nldd-icon icon="chevron-right"></nldd-icon></nldd-icon-cell>
            </nldd-list-item>
          </nldd-list>
          <nldd-text v-else color="secondary"><p>Er wacht niets op een besluit.</p></nldd-text>
        </template>
      </nldd-container>
    </nldd-simple-section>
  </nldd-page>
</template>
