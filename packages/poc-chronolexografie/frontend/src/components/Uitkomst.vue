<script setup>
// Een besluit op een aanvraag: toegewezen of afgewezen (volgens de uitvoer
// die de casus noemt), de uitvoer, en
// desgewenst waaróm (Waarom.vue). `waaromVan` is het besluit waarvan die
// uitleg is: het besluit zelf, of de beslissing op bezwaar die het nam.
import { computed } from 'vue';
import { leesbaar, toegewezen, waarde } from '../state.js';
import Waarom from './Waarom.vue';

const props = defineProps({
  besluit: { type: Object, required: true },
  waarom: { type: Object, default: null },
  waaromVan: { type: String, default: '' },
  titel: { type: String, default: 'Besluit' },
});

const verleend = computed(() => toegewezen(props.besluit) === true);
const uitvoer = computed(() =>
  Object.entries(props.besluit.outputs ?? {}).map(([naam, waarde]) => ({ naam, waarde })),
);
</script>

<template>
  <nldd-container gap="12">
    <nldd-title size="5" :text="titel">
      <nldd-tag
        slot="end"
        :color="verleend ? 'success' : 'critical'"
        :text="verleend ? 'Toegewezen' : 'Afgewezen'"
      ></nldd-tag>
    </nldd-title>
    <nldd-list appearance="box-base">
      <nldd-list-item v-for="u in uitvoer" :key="u.naam" size="sm">
        <nldd-text-cell :text="leesbaar(u.naam)"></nldd-text-cell>
        <nldd-text-cell width="fit-content" :text="waarde(u.waarde)"></nldd-text-cell>
      </nldd-list-item>
    </nldd-list>
    <Waarom v-if="waarom" :besluit="waaromVan || besluit.id" :waarom="waarom" />
  </nldd-container>
</template>
