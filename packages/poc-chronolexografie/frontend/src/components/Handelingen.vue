<script setup>
// De feiten die een cel met de hand vastlegt (executogrammen, RFC-051), één
// formulier per feit: welke velden, zegt het artikel dat het feit vereist.
// Zo legt de GGD haar advies vast, de drukker een levering en het
// bestuursorgaan een drukopdracht. Onder elk formulier staat wat dat artikel
// nu zegt (`stand`), zoals hoeveel kaarten er nog aan te vragen zijn: het
// beleid telt dat uit de kronieken van de cellen. Volg je een scenario, dan
// staat het formulier van de stap ingevuld en klappen de andere formulieren
// in (scenarioInvoer.js); die kun je zelf weer openen, tot de volgende stap.
import { reactive, watch } from 'vue';
import { api } from '../api.js';
import { hoofdletter, leesbaar, state, stap, waarde } from '../state.js';
import { formulier, huidigeStap, ingeklapt, scenarioWaarden } from '../scenarioInvoer.js';
import ArtikelLink from './ArtikelLink.vue';
import VeldInvoer from './VeldInvoer.vue';

const props = defineProps({
  // The cell that records, and its forms (`vormen.handelingen`, or one of
  // `vormen.andere_cellen`).
  cel: { type: String, required: true },
  handelingen: { type: Array, required: true },
});

// Per cel en handeling de ingevulde velden.
const sleutel = (cel, basis) => `${cel} ${basis}`;
const vorm = formulier((k) => {
  const [cel, feit] = k.split(' ');
  return scenarioWaarden('vastleggen', { cel, feit });
});
const velden = (basis) => vorm.waarden(sleutel(props.cel, basis));

// The folded forms the visitor opened, until the next step.
const geopend = reactive({});
watch(
  () => huidigeStap()?.index,
  () => Object.keys(geopend).forEach((k) => delete geopend[k]),
);
// Every cell of the case with forms, so a form folds at a record step on
// any of them.
const alleCellen = () => [
  ...(state.vormen?.andere_cellen ?? []),
  { cel: state.casus?.cel, handelingen: state.vormen?.handelingen ?? [] },
];
const inklapbaar = (feit) => ingeklapt(alleCellen(), props.cel, feit);
const zichtbaar = (feit) => !inklapbaar(feit) || !!geopend[sleutel(props.cel, feit)];
const wissel = (feit) => {
  geopend[sleutel(props.cel, feit)] = !geopend[sleutel(props.cel, feit)];
};

// `beleid_x#5#geneeskundig_advies_uitgebracht` → `Geneeskundig advies uitgebracht`;
// the article that requires the fact (`beleid_x#5`) is a link below it.
const naam = (feit) => hoofdletter(leesbaar(feit.split('#')[2] ?? ''));
const artikelVan = (feit) => feit.split('#').slice(0, 2).join('#');

// Een schakelaar die uit staat, zegt nee: een feit met ja/nee-velden (zoals
// het advies van de GGD per criterium) is anders nooit heel.
async function vastleggen(handeling) {
  const basis = handeling.feit;
  const ingevuld = velden(basis);
  const gegeven = Object.fromEntries(
    handeling.velden
      .map((v) => [v.naam, ingevuld[v.naam] ?? (v.type === 'boolean' && !v.mag_leeg ? false : undefined)])
      .filter(([, v]) => v !== undefined),
  );
  const ok = await stap(() => api.vastleggen(props.cel, basis, gegeven, state.datum));
  if (ok) vorm.wis(sleutel(props.cel, basis));
}
</script>

<template>
  <nldd-card v-for="h in handelingen" :key="h.feit">
    <nldd-container padding="16" gap="12">
      <nldd-title size="5" :text="`Vastleggen: ${naam(h.feit)}`">
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
          @click="wissel(h.feit)"
        ></nldd-button>
      </nldd-title>
      <nldd-list v-if="h.stand?.length" appearance="box-base" accessible-label="Wat het beleid nu zegt">
        <nldd-list-item v-for="s in h.stand" :key="s.naam" size="sm">
          <nldd-text-cell :text="hoofdletter(leesbaar(s.naam))" :supporting-text="s.beschrijving ?? ''"></nldd-text-cell>
          <nldd-text-cell width="fit-content" :text="waarde(s.waarde)"></nldd-text-cell>
        </nldd-list-item>
      </nldd-list>
      <template v-if="zichtbaar(h.feit)">
        <VeldInvoer
          v-for="veld in h.velden"
          :key="veld.naam"
          :model-value="velden(h.feit)[veld.naam]"
          :veld="veld"
          @update:model-value="vorm.zet(sleutel(cel, h.feit), veld.naam, $event)"
        />
        <nldd-button appearance="primary" text="Vastleggen" @click="vastleggen(h)"></nldd-button>
      </template>
    </nldd-container>
  </nldd-card>
</template>
