<script setup>
// Tab "Met en zonder DSO": alles wat de eerste PoC en het DSO-spoor
// vergelijkt, op één plek naast de tabs van de eerste PoC. Vier delen: op dit punt beide sporen
// naast elkaar, simuleren met het DSO-spoor, de scenario's en de kerncijfers.
import PuntPaneel from './PuntPaneel.vue';
import SimulerenTab from './SimulerenTab.vue';
import ScenarioTab from './ScenarioTab.vue';
import KerncijfersTab from './KerncijfersTab.vue';

const deel = defineModel('deel', { type: String, default: 'punt' });
const reeks = defineModel('reeks', { type: String, default: 'woontoren' });
defineProps({ data: Object, fout: String, cel: Object, index: Number, scenario: Object, weergave: String });
const emit = defineEmits(['artikel', 'scenario', 'kaart']);

const DELEN = [
  { id: 'punt', tekst: 'Op dit punt' },
  { id: 'simuleren', tekst: 'Simuleren' },
  { id: 'scenarios', tekst: "Scenario's" },
  { id: 'cijfers', tekst: 'Kerncijfers' },
];
</script>

<template>
  <nldd-container layout="stack" gap="16">
    <nldd-segmented-control :value="deel" size="sm" accessible-label="Deel van Met en zonder DSO" @change="deel = $event.target.value">
      <nldd-segmented-control-item v-for="d in DELEN" :key="d.id" :value="d.id" :text="d.tekst" :selected="deel === d.id || undefined"></nldd-segmented-control-item>
    </nldd-segmented-control>

    <nldd-banner v-if="fout" variant="critical" text="Gegevens van het DSO-spoor niet geladen" :supporting-text="fout"></nldd-banner>
    <nldd-text v-else-if="!data" size="sm" color="secondary"><p>Gegevens van het DSO-spoor laden…</p></nldd-text>
    <template v-else>
      <template v-if="deel === 'punt'">
        <PuntPaneel v-if="cel" :cel="cel" :index="index" :scenario="scenario" :weergave="weergave" :data="data" @artikel="emit('artikel', $event)" />
        <nldd-container v-else layout="stack" gap="12">
          <nldd-title size="display-3" :heading-level="2" text="Klik op een plek in de Spoorzone"></nldd-title>
          <nldd-text>
            <p>
              Je ziet dan beide sporen naast elkaar. Links zonder DSO: de eerste PoC, met eigen downloads, lagen en regels met de
              hand, alleen de gemeente. Rechts met DSO: plannen, regels en teksten uit het DSO, voor gemeente, provincie,
              waterschap en rijk.
            </p>
          </nldd-text>
          <nldd-text size="sm" color="secondary">
            <p>Of kies een plek onder Scenario's. De kaartlaag DSO kleurt waar de sporen verschillen. Verkenning, geen besluit.</p>
          </nldd-text>
        </nldd-container>
      </template>
      <SimulerenTab v-else-if="deel === 'simuleren'" :cel="cel" :index="index" :weergave="weergave" :data="data" @kaart="emit('kaart')" />
      <ScenarioTab v-else-if="deel === 'scenarios'" v-model:reeks="reeks" :data="data" :gekozen="scenario?.naam" @kies="emit('scenario', $event)" />
      <KerncijfersTab v-else :data="data" />
    </template>
  </nldd-container>
</template>
