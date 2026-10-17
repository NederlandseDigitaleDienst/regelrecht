<script setup>
// Eén pagina, vijf tabbladen: de scenario's, de ouder, de school, de
// kroniek en de reductie. Bovenaan de speeldatum, die voor elke stap geldt,
// en een knop die de kroniek leegmaakt. Een weigering van de cel staat in
// een banner, in het Nederlands; de melding van de cel zelf onder Details.
import { onMounted, ref, watch } from 'vue';
import { api } from './api.js';
import { isEngels, melding } from './melding.js';
import { spel, stap, verversen } from './spel.js';
import KroniekTab from './components/KroniekTab.vue';
import OuderTab from './components/OuderTab.vue';
import ReductieTab from './components/ReductieTab.vue';
import ScenarioTab from './components/ScenarioTab.vue';
import SchoolTab from './components/SchoolTab.vue';

const tabs = [
  { id: 'scenarios', tekst: "Scenario's" },
  { id: 'ouder', tekst: 'Ouder' },
  { id: 'school', tekst: 'School' },
  { id: 'kroniek', tekst: 'Kroniek' },
  { id: 'reductie', tekst: 'Reductie' },
];
const tab = ref('scenarios');
const details = ref(false);
watch(() => spel.fout, () => (details.value = false));

onMounted(verversen);
watch(() => spel.datum, verversen);

async function opnieuw() {
  await stap(() => api.reset());
}
</script>

<template>
  <nldd-app-view>
    <nldd-page sticky-header>
      <nldd-container slot="header" padding="16" gap="12">
        <nldd-top-title-bar
          text="Vakantieverlof naspelen"
          supporting-text="De cel van OBS De Klimop (fictief) · Leerplichtwet 1969 artikel 13a"
        >
          <nldd-button
            slot="toolbar"
            appearance="neutral-tinted"
            size="sm"
            text="Opnieuw beginnen"
            @click="opnieuw"
          ></nldd-button>
        </nldd-top-title-bar>
        <nldd-tab-bar accessible-label="Rol">
          <nldd-tab-bar-item
            v-for="t in tabs"
            :key="t.id"
            :text="t.tekst"
            :current="tab === t.id"
            @select="tab = t.id"
          ></nldd-tab-bar-item>
        </nldd-tab-bar>
        <nldd-form-field
          label="Speeldatum"
          label-alignment="left"
          supporting-label="De dag van elke stap; de kroniek loopt alleen vooruit."
        >
          <nldd-date-field :value="spel.datum" @change="$event.detail.value && (spel.datum = $event.detail.value)"></nldd-date-field>
        </nldd-form-field>
      </nldd-container>

      <nldd-simple-section v-if="spel.fout" padding-block="16">
        <nldd-banner
          variant="critical"
          text="Deze stap lukt niet"
          :supporting-text="melding(spel.fout)"
          dismissible
          @dismiss="spel.fout = ''"
        >
          <nldd-text v-if="details" size="sm" color="secondary" :lang="isEngels(spel.fout) ? 'en' : null">{{ spel.fout }}</nldd-text>
          <nldd-button
            slot="actions"
            appearance="neutral-tinted"
            size="sm"
            expandable
            :expanded="details"
            text="Details"
            @click="details = !details"
          ></nldd-button>
        </nldd-banner>
      </nldd-simple-section>

      <ScenarioTab v-if="tab === 'scenarios'" @start="tab = 'ouder'" />
      <OuderTab v-else-if="tab === 'ouder'" />
      <SchoolTab v-else-if="tab === 'school'" />
      <KroniekTab v-else-if="tab === 'kroniek'" />
      <ReductieTab v-else />
    </nldd-page>
  </nldd-app-view>
</template>
