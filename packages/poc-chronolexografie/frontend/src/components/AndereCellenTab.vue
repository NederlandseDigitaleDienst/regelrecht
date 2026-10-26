<script setup>
// De andere cellen van de sfeer (RFC-052): wie de wet bevoegd maakt voor een
// waarde die het bestuursorgaan nodig heeft, zoals de GGD voor het
// geneeskundig onderzoek. Elke cel houdt haar eigen kroniek bij en
// antwoordt daaruit. Een cel die een feit vastlegt (een executogram,
// RFC-051), krijgt hier een formulier; welke velden, zegt het artikel dat het
// feit vereist. Wat een andere cel vóór het eerste scenario wist, komt
// uit de casus. De rechtbank is ook een andere cel (RFC-053): zij krijgt
// hier de beroepen en de uitspraken (RechterCel.vue).
import { reactive } from 'vue';
import { api } from '../api.js';
import { artikelNaam } from '../logboek.js';
import { hoofdletter, leesbaar, state, stap } from '../state.js';
import RechterCel from './RechterCel.vue';
import VeldInvoer from './VeldInvoer.vue';

// Per cel en handeling de ingevulde velden.
const invoer = reactive({});
const sleutel = (cel, basis) => `${cel} ${basis}`;
const velden = (cel, basis) => (invoer[sleutel(cel, basis)] ??= {});

// `regeling_x#3#geneeskundig_onderzoek_verricht` → `Geneeskundig onderzoek verricht (… artikel 3)`.
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
  if (ok) invoer[sleutel(cel, basis)] = {};
}
</script>

<template>
  <template v-for="c in state.vormen.andere_cellen" :key="c.cel">
    <RechterCel v-if="c.cel === state.casus.rechter?.cel" />
    <nldd-simple-section v-else>
      <nldd-title slot="header" size="3" :text="c.cel_naam">
        <nldd-text slot="supporting-text">
          Een andere cel van de sfeer. De engine vraagt haar alleen naar een waarde waarvoor de wet
          haar bevoegd maakt, en zij antwoordt uit haar eigen kroniek.
        </nldd-text>
      </nldd-title>
      <nldd-container gap="16" max-width="560px">
        <nldd-card v-for="h in c.handelingen" :key="h.feit">
          <nldd-container padding="16" gap="12">
            <nldd-title size="5" :text="`Vastleggen: ${naam(h.feit)}`"></nldd-title>
            <VeldInvoer
              v-for="veld in h.velden"
              :key="veld.naam"
              v-model="velden(c.cel, h.feit)[veld.naam]"
              :veld="veld"
            />
            <nldd-button appearance="primary" text="Vastleggen" @click="vastleggen(c.cel, h)"></nldd-button>
          </nldd-container>
        </nldd-card>
        <nldd-text size="sm" color="secondary">
          {{ kroniek(c.cel).length }} chronolexogram(men) in haar kroniek; ze staan onder Kroniek.
        </nldd-text>
      </nldd-container>
    </nldd-simple-section>
  </template>
</template>
