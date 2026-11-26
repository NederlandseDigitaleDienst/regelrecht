<template>
  <nldd-table
    accessible-label="Vergelijking terugbetaalregimes"
    columns="minmax(110px,1.2fr) minmax(130px,1.3fr) minmax(130px,1.3fr) minmax(90px,1fr) minmax(110px,1.1fr) minmax(120px,1.2fr)"
  >
    <nldd-table-row slot="header">
      <nldd-text-cell text="Regels"></nldd-text-cell>
      <nldd-text-cell text="Wat je mag houden"></nldd-text-cell>
      <nldd-text-cell text="Deel dat je betaalt"></nldd-text-cell>
      <nldd-text-cell text="Hoe lang"></nldd-text-cell>
      <nldd-text-cell text="Pauze mogelijk"></nldd-text-cell>
      <nldd-text-cell text="Kijkt DUO naar je inkomen?"></nldd-text-cell>
    </nldd-table-row>

    <nldd-table-row
      v-for="rij in facts"
      :key="rij.regime"
      :selected="rij.regime === highlight ? true : undefined"
    >
      <nldd-text-cell>
        <p><nldd-tag :color="regimeColor(rij.regime)" size="sm" :text="regimeLabel(rij.regime)"></nldd-tag></p>
      </nldd-text-cell>
      <nldd-text-cell :text="`${percent(rij.voetRatio, 0)} WML`"></nldd-text-cell>
      <nldd-text-cell :text="rij.draagkrachtLabel ?? percent(rij.draagkrachtRatio, 0)"></nldd-text-cell>
      <nldd-text-cell :text="`${jaren(rij.periodeMaanden)} jaar`"></nldd-text-cell>
      <nldd-text-cell :text="rij.aflosvrij ? 'Ja (jokerjaren)' : 'Nee'"></nldd-text-cell>
      <nldd-text-cell :text="rij.draagkrachtAutomatisch ? 'Automatisch' : 'Op aanvraag'"></nldd-text-cell>
    </nldd-table-row>
  </nldd-table>
</template>

<script setup>
import { computed } from 'vue';
import { regimeFacts } from '../../lib/regimeFacts.js';
import { useLawStore } from '../../engine/lawStore.js';
import { percent, regimeLabel, regimeColor } from '../../lib/format.js';

defineProps({
  highlight: { type: String, default: null },
});

const { version } = useLawStore();

// Herbereken de feiten zodra de wet-versie verandert (beleid-edit of variant).
const facts = computed(() => {
  version.value;
  return regimeFacts();
});

function jaren(maanden) {
  if (maanden === null || maanden === undefined) return '—';
  return Math.round(maanden / 12);
}
</script>
