<script setup>
// De andere cellen van de casus (RFC-052): wie de wet bevoegd maakt voor een
// waarde die het bestuursorgaan nodig heeft, zoals de GGD voor het
// geneeskundig advies. Elke cel houdt haar eigen kroniek bij en
// antwoordt daaruit. Een cel die een feit vastlegt (een executogram,
// RFC-051), krijgt hier een formulier; welke velden, zegt het artikel dat het
// feit vereist. Wat een andere cel vóór het eerste scenario wist, komt
// uit de casus. De rechtbank is ook een andere cel (RFC-053): zij krijgt
// hier de beroepen en de uitspraken (RechterCel.vue). Volg je een scenario,
// dan staat het formulier van de stap ingevuld en klappen de andere
// formulieren in (scenarioInvoer.js); die kun je zelf weer openen, tot de
// volgende stap.
import { reactive, watch } from 'vue';
import { api } from '../api.js';
import { artikelNaam } from '../logboek.js';
import { hoofdletter, leesbaar, state, stap } from '../state.js';
import { formulier, huidigeStap, ingeklapt, scenarioWaarden } from '../scenarioInvoer.js';
import RechterCel from './RechterCel.vue';
import VeldInvoer from './VeldInvoer.vue';

// Per cel en handeling de ingevulde velden.
const sleutel = (cel, basis) => `${cel} ${basis}`;
const doel = (cel, feit) => ({ cel, feit });
const vorm = formulier((k) => {
  const [cel, feit] = k.split(' ');
  return scenarioWaarden('vastleggen', doel(cel, feit));
});
const velden = (cel, basis) => vorm.waarden(sleutel(cel, basis));

// De ingeklapte formulieren die de bezoeker zelf opende, tot de volgende stap.
const geopend = reactive({});
watch(
  () => huidigeStap()?.index,
  () => Object.keys(geopend).forEach((k) => delete geopend[k]),
);
const inklapbaar = (cel, feit) => ingeklapt(state.vormen.andere_cellen, cel, feit);
const zichtbaar = (cel, feit) => !inklapbaar(cel, feit) || !!geopend[sleutel(cel, feit)];

// `beleid_x#5#geneeskundig_advies_uitgebracht` → `Geneeskundig advies uitgebracht (… artikel 5)`.
const naam = (feit) => {
  const [wet, artikel, wat] = feit.split('#');
  return `${hoofdletter(leesbaar(wat))} (${artikelNaam(wet, artikel)})`;
};

const kroniek = (cel) => state.andere.find((a) => a.cel === cel)?.grammen ?? [];

// Een schakelaar die uit staat, zegt nee: een feit met ja/nee-velden (zoals
// het advies van de GGD per criterium) is anders nooit heel.
async function vastleggen(cel, handeling) {
  const basis = handeling.feit;
  const ingevuld = velden(cel, basis);
  const gegeven = Object.fromEntries(
    handeling.velden
      .map((v) => [v.naam, ingevuld[v.naam] ?? (v.type === 'boolean' && !v.mag_leeg ? false : undefined)])
      .filter(([, v]) => v !== undefined),
  );
  const ok = await stap(() => api.vastleggen(cel, basis, gegeven, state.datum));
  if (ok) vorm.wis(sleutel(cel, basis));
}
</script>

<template>
  <template v-for="c in state.vormen.andere_cellen" :key="c.cel">
    <RechterCel v-if="c.cel === state.casus.rechter?.cel" />
    <nldd-simple-section v-else>
      <nldd-title slot="header" size="3" :text="c.cel_naam">
        <nldd-text slot="supporting-text">
          Een andere cel van de casus. De engine vraagt haar alleen naar een waarde waarvoor de wet
          haar bevoegd maakt, en zij antwoordt uit haar eigen kroniek.
        </nldd-text>
      </nldd-title>
      <nldd-container gap="16" max-width="560px">
        <nldd-card v-for="h in c.handelingen" :key="h.feit">
          <nldd-container padding="16" gap="12">
            <nldd-title size="5" :text="`Vastleggen: ${naam(h.feit)}`">
              <nldd-button
                v-if="inklapbaar(c.cel, h.feit)"
                slot="end"
                appearance="neutral-tinted"
                size="sm"
                :text="zichtbaar(c.cel, h.feit) ? 'Inklappen' : 'Openen'"
                :expanded="zichtbaar(c.cel, h.feit)"
                @click="geopend[sleutel(c.cel, h.feit)] = !geopend[sleutel(c.cel, h.feit)]"
              ></nldd-button>
            </nldd-title>
            <template v-if="zichtbaar(c.cel, h.feit)">
              <VeldInvoer
                v-for="veld in h.velden"
                :key="veld.naam"
                :model-value="velden(c.cel, h.feit)[veld.naam]"
                :veld="veld"
                @update:model-value="vorm.zet(sleutel(c.cel, h.feit), veld.naam, $event)"
              />
              <nldd-button appearance="primary" text="Vastleggen" @click="vastleggen(c.cel, h)"></nldd-button>
            </template>
          </nldd-container>
        </nldd-card>
        <nldd-text size="sm" color="secondary">
          {{ kroniek(c.cel).length }} chronolexogram(men) in haar kroniek; ze staan onder Kroniek.
        </nldd-text>
      </nldd-container>
    </nldd-simple-section>
  </template>
</template>
