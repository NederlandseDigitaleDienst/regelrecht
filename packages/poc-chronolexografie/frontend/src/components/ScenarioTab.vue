<script setup>
// The scenarios of the case (packages/poc-chronolexografie/corpus/casussen/). A
// scenario with a guide (scenarioGuides.js) shows one sentence here; its steps
// follow in the guide beside the page once it begins. One without (a case
// from a source, such as NAPP) shows the steps from the YAML in order and
// what you see at the end. The button puts the chronicle back on what the
// cell records before the first scenario (empty, or what the case names under
// `begin`) and the play date on the first day; then you go to the tab
// Aanvrager.
import { computed } from 'vue';
import { guideFor } from '../scenarioGuides.js';
import { beginScenario, longDate, state } from '../state.js';

const emit = defineEmits(['start']);

const scenarios = computed(() =>
  state.casus.scenarios.map((s) => ({ ...s, guide: guideFor(state.casus, s.naam) })),
);
// How many chronolexograms the cell records beforehand (RFC-051).
const vooraf = computed(() => {
  const b = state.casus.begin;
  return b?.executogrammen?.length ?? 0;
});
const eerste = computed(() => longDate(state.casus.speeldatum));

async function begin(s) {
  if (await beginScenario(s.naam)) emit('start', s);
}
</script>

<template>
  <nldd-simple-section>
    <nldd-title slot="header" size="3" text="Scenario's">
      <nldd-text slot="supporting-text">
        Zo speel je het proces na. Elk scenario begint op {{ eerste }}<template v-if="vooraf">
          met de kroniek van de cel zoals zij vooraf is vastgelegd:
          {{ vooraf === 1 ? 'één chronolexogram' : `${vooraf} chronolexogrammen` }}, zie het tabblad
          Kroniek</template><template v-else> met een lege
          kroniek</template>. De speeldatum staat bovenaan en loopt alleen vooruit.
      </nldd-text>
    </nldd-title>
    <nldd-container gap="16">
      <nldd-card v-for="(s, i) in scenarios" :key="s.naam">
        <nldd-container padding="16" gap="12">
          <nldd-title size="5" :overline="`Scenario ${i + 1}`" :text="s.naam">
            <nldd-tag v-if="state.scenario === s.naam" slot="end" color="accent" text="Bezig"></nldd-tag>
          </nldd-title>
          <nldd-text v-if="s.guide">{{ s.guide.summary }}</nldd-text>
          <nldd-rich-text v-else>
            <ol>
              <li v-for="(t, j) in s.stappen" :key="j">{{ t }}</li>
            </ol>
            <p><strong>Je ziet:</strong> {{ s.eind }}</p>
          </nldd-rich-text>
          <nldd-button appearance="secondary" text="Begin dit scenario" @click="begin(s)"></nldd-button>
        </nldd-container>
      </nldd-card>
    </nldd-container>
  </nldd-simple-section>
</template>
