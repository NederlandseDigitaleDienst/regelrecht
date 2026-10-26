<script setup>
// Het keuzescherm: per casus een kaart met wat er gebeurt, bij welke
// (fictieve) uitvoerder en onder welk artikel. Kiezen begint de cel van die
// casus met een lege kroniek. Daarboven staat per bron (src/bronnen.js) wat
// er niet lukte: een bron die er niet is, of een bestand dat niet laadt.
import { onMounted, ref } from 'vue';
import { api } from '../api.js';
import { bronMeldingen, herkomst } from '../bronnen.js';
import { artikelNaam } from '../logboek.js';
import { hoofdletter, kiezen, state } from '../state.js';

const emit = defineEmits(['gekozen']);
const casussen = ref([]);
const bronnen = ref([]);

onMounted(async () => {
  try {
    const lijst = await api.casussen();
    casussen.value = lijst.casussen;
    bronnen.value = lijst.bronnen ?? [];
  } catch (e) {
    state.fout = e.message;
  }
});

const artikel = (c) => {
  const [wet, nummer] = c.besluit.split('#');
  return artikelNaam(wet, nummer);
};

async function kies(c) {
  if (await kiezen(c.id)) emit('gekozen');
}
</script>

<template>
  <nldd-simple-section>
    <nldd-title slot="header" size="3" text="Kies een casus">
      <nldd-text slot="supporting-text">
        Elke casus is een aanvraag, het besluit erop en het bezwaar ertegen, bij een verzonnen
        uitvoerder. De cel legt elke stap vast in haar kroniek en leest die terug bij een volgend
        besluit. Welke regels meelopen, volgt uit het artikel van de casus.
      </nldd-text>
    </nldd-title>
    <nldd-container v-if="bronMeldingen(bronnen).length" gap="12" padding-bottom="16">
      <nldd-banner
        v-for="m in bronMeldingen(bronnen)"
        :key="m.naam"
        :variant="m.variant"
        :text="m.tekst"
        supporting-text="De andere casussen werken gewoon."
      >
        <nldd-text v-for="r in m.regels" :key="r" size="sm" color="secondary" lang="en">{{ r }}</nldd-text>
      </nldd-banner>
    </nldd-container>
    <nldd-collection layout="grid" item-width="320px">
      <nldd-card v-for="c in casussen" :key="c.id">
        <nldd-container slot="header" padding="16">
          <nldd-title
            size="4"
            heading-level="2"
            :overline="`${hoofdletter(c.cel_naam)} (fictief)`"
            :text="c.titel"
            :supporting-text="`${artikel(c)} · beslist door ${c.rol}${herkomst(c, bronnen)}`"
          ></nldd-title>
        </nldd-container>
        <nldd-container padding="16">
          <nldd-text>{{ c.omschrijving }}</nldd-text>
        </nldd-container>
        <nldd-container slot="footer" padding="16">
          <nldd-button appearance="primary" :text="`${c.titel} naspelen`" @click="kies(c)"></nldd-button>
        </nldd-container>
      </nldd-card>
    </nldd-collection>
  </nldd-simple-section>
</template>
