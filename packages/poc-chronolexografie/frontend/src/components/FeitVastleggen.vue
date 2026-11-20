<script setup>
// De formulieren waarmee een cel van de casus een feit vastlegt (een
// executogram, RFC-051): een andere cel, zoals de GGD een advies, of de cel
// van de casus zelf terwijl zij de zaak behandelt, zoals een betaling op haar
// besluit. Welke feiten een cel vastlegt en welke velden ze hebben, zegt het
// artikel dat het feit vereist. Volg je een scenario, dan staat het formulier
// van de stap ingevuld en klappen de andere formulieren in
// (scenarioInvoer.js); die kun je zelf weer openen, tot de volgende stap.
import { computed, reactive, watch } from 'vue';
import { api } from '../api.js';
import { hoofdletter, leesbaar, state, stap } from '../state.js';
import { formulier, huidigeStap, ingeklapt, scenarioVerwijzing, scenarioWaarden } from '../scenarioInvoer.js';
import ArtikelLink from './ArtikelLink.vue';
import VeldInvoer from '@regelrecht/frontend-shared/components/chronolex/VeldInvoer.vue';

const props = defineProps({
  // De cel die vastlegt, en de feiten die zij vastlegt (`vormen`).
  cel: { type: String, required: true },
  handelingen: { type: Array, required: true },
});

// Per handeling de ingevulde velden.
const vorm = formulier((feit) => scenarioWaarden('vastleggen', { cel: props.cel, feit }));
const velden = (feit) => vorm.waarden(feit);

// The folded forms the visitor opened, until the next step.
const geopend = reactive({});
watch(
  () => huidigeStap()?.index,
  () => Object.keys(geopend).forEach((k) => delete geopend[k]),
);
// Every record form of the page: a step on any of them folds the others.
const alle = computed(() => [...(state.vormen.andere_cellen ?? []), state.vormen.eigen].filter(Boolean));
const inklapbaar = (feit) => ingeklapt(alle.value, props.cel, feit);
const zichtbaar = (feit) => !inklapbaar(feit) || !!geopend[feit];

// `beleid_x#5#geneeskundig_advies_uitgebracht` → `Geneeskundig advies uitgebracht`;
// the article that requires the fact (`beleid_x#5`) is a link below it.
const naam = (feit) => hoofdletter(leesbaar(feit.split('#')[2] ?? ''));
const artikelVan = (feit) => feit.split('#').slice(0, 2).join('#');

// Een schakelaar die uit staat, zegt nee: een feit met ja/nee-velden (zoals
// het advies van de GGD per criterium) is anders nooit heel. Verwijst de stap
// van het scenario naar een eerder record, zoals een correctie naar de
// inschrijving die ze verbetert of een betaling naar het besluit, dan gaat
// dat record mee.
async function vastleggen(handeling) {
  const feit = handeling.feit;
  const ingevuld = velden(feit);
  const gegeven = Object.fromEntries(
    handeling.velden
      .map((v) => [v.naam, ingevuld[v.naam] ?? (v.type === 'boolean' && !v.mag_leeg ? false : undefined)])
      .filter(([, v]) => v !== undefined),
  );
  const verwijstNaar = scenarioVerwijzing(props.cel, feit);
  const ok = await stap(() => api.vastleggen(props.cel, feit, gegeven, state.datum, verwijstNaar));
  if (ok) vorm.wis(feit);
}
</script>

<template>
  <nldd-card v-for="h in handelingen" :key="h.feit">
    <nldd-container padding="16" gap="12">
      <nldd-title heading-level="3" size="display-5" :text="`Vastleggen: ${naam(h.feit)}`">
        <nldd-text slot="supporting-text" size="sm" color="secondary">
          <ArtikelLink :basis="artikelVan(h.feit)" />
        </nldd-text>
        <nldd-button
          v-if="inklapbaar(h.feit)"
          slot="end"
          appearance="neutral-tinted"
          size="sm"
          :text="zichtbaar(h.feit) ? 'Inklappen' : 'Openen'"
          :accessible-label="`${zichtbaar(h.feit) ? 'Inklappen' : 'Openen'} ${naam(h.feit)}`"
          expandable
          :expanded="zichtbaar(h.feit)"
          @click="geopend[h.feit] = !geopend[h.feit]"
        ></nldd-button>
      </nldd-title>
      <template v-if="zichtbaar(h.feit)">
        <VeldInvoer
          v-for="veld in h.velden"
          :key="veld.naam"
          :model-value="velden(h.feit)[veld.naam]"
          :veld="veld"
          @update:model-value="vorm.zet(h.feit, veld.naam, $event)"
        />
        <nldd-button appearance="primary" text="Vastleggen" @click="vastleggen(h)"></nldd-button>
      </template>
    </nldd-container>
  </nldd-card>
</template>
