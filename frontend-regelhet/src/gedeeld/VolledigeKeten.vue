<script setup lang="ts">
// The full chain of a decision in a sheet of its own, opened from a link in
// one sentence: every article the decision ran, every outcome and open term
// with who filled it in, as the chronolexografie poc shows it on its decision
// page. The chain has no address, so the link's href only makes it a link
// (focusable, announced as one); the click opens the sheet and does not
// navigate. The sentence and its sheet come together, so any screen of an
// application that has the chain can call it up where it stands, without
// unfolding it inline. The sheet rises from the bottom, with the full width
// for the chain, and its steps are md rows that fold. The steps are the shared ones (frontend-shared KetenStap.vue); this
// app names a regelwerk by the case and a field by the label of the UI spec.
import { computed, ref } from 'vue';
import KetenStap from '@regelrecht/frontend-shared/components/chronolex/KetenStap.vue';
import { genummerd, laagVanMap } from '@regelrecht/frontend-shared/chronolex/keten';
import type { Namen, Stap } from '@regelrecht/frontend-shared/chronolex/keten';
import { artikelNaam, wetNaam } from '@regelrecht/frontend-shared/chronolex/waarden';
import { kaalSchermveld } from './formulier';
import type { Casus, Vormen } from './types';

const props = defineProps<{ keten: Stap[]; casus: Casus; teksten?: Vormen['teksten'] }>();

const OMSCHRIJVING = 'Elk artikel dat het besluit uitvoerde, wat het opleverde en wie elk open begrip invulde';
const open = ref(false);
const knopen = computed(() => genummerd(props.keten));
const regelwerk = (wet: string) => props.casus.regelwerken?.find((r) => r.id === wet);
const naamVan = (wet: string) => regelwerk(wet)?.naam;
const namen: Namen = {
  wet: (wet) => wetNaam(wet, naamVan),
  artikel: (wet, artikel) => artikelNaam(wet, artikel, naamVan),
  veld: (naam) => kaalSchermveld(naam, props.teksten).label,
};
const laag = (wet: string) => laagVanMap(regelwerk(wet)?.map ?? '');
</script>

<template>
  <template v-if="knopen.length">
    <nldd-text>
      <p>
        {{ OMSCHRIJVING }}, staat in de
        <nldd-link><a href="#volledige-keten" @click.prevent="open = true">volledige keten</a></nldd-link>.
      </p>
    </nldd-text>
    <Teleport to="body">
      <nldd-sheet placement="bottom" accessible-label="Volledige keten" :open="open || undefined" @close="open = false">
        <nldd-page>
          <nldd-top-title-bar slot="header" heading-level="2" text="Volledige keten" dismiss-text="Sluit"></nldd-top-title-bar>
          <nldd-simple-section>
            <nldd-text color="secondary">
              <p>{{ OMSCHRIJVING }}.</p>
            </nldd-text>
            <nldd-spacer size="16"></nldd-spacer>
            <nldd-list type="tree" appearance="box-base" accessible-label="Stappen van het besluit">
              <KetenStap v-for="k in knopen" :key="k.nummer" :knoop="k" :namen="namen" :laag="laag" size="md" />
            </nldd-list>
          </nldd-simple-section>
        </nldd-page>
      </nldd-sheet>
    </Teleport>
  </template>
</template>
