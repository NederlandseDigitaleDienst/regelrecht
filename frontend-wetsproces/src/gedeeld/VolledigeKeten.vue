<script setup lang="ts">
// The full chain of a decision, folded away under one row ("Meer
// informatie"): every article the decision ran, every outcome and open term
// with who filled it in, as the chronolexografie poc shows it on its decision
// page. The steps are the shared ones (frontend-shared KetenStap.vue); this
// app names a regelwerk by the case and a field by the label of the UI spec.
import { computed, ref } from 'vue';
import KetenStap from '@regelrecht/frontend-shared/components/chronolex/KetenStap.vue';
import { genummerd, laagVanMap } from '@regelrecht/frontend-shared/chronolex/keten';
import type { Namen, Stap } from '@regelrecht/frontend-shared/chronolex/keten';
import { artikelNaam, wetNaam } from '@regelrecht/frontend-shared/chronolex/waarden';
import { kaalSchermveld } from './formulier';
import type { Casus, Vormen } from './types';

const props = defineProps<{ keten: Stap[]; casus: Casus; teksten?: Vormen['teksten'] }>();

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
  <nldd-list v-if="knopen.length" type="tree" appearance="box-base" accessible-label="Volledige keten van het besluit">
    <nldd-list-item size="md" button :expanded="open" @click.stop="open = !open">
      <nldd-icon-cell disclosure icon="chevron-right" size="16" color="secondary"></nldd-icon-cell>
      <nldd-spacer-cell size="8"></nldd-spacer-cell>
      <nldd-text-cell
        text="Meer informatie: de volledige keten"
        supporting-text="Elk artikel dat het besluit uitvoerde, elke uitkomst en wie elk open begrip invulde."
      ></nldd-text-cell>
      <KetenStap v-for="k in knopen" :key="k.nummer" slot="children" :knoop="k" :namen="namen" :laag="laag" :diepte="1" />
    </nldd-list-item>
  </nldd-list>
</template>
