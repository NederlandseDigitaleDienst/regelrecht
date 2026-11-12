<script setup>
// Het keuzescherm: per casus een kaart met wat er gebeurt, bij welke
// (fictieve) uitvoerder en onder welk artikel. Kiezen begint de cel van die
// casus met een lege kroniek. Daarboven staat per bron (src/bronnen.js) wat
// er niet lukte: een bron die er niet is, of een bestand dat niet laadt.
//
// At the top: what a demo needs, the presentation and the short page that
// says what RegelRecht and chronolexografie are (presentation/kort.js), and
// right under them the demo scenarios (presentation/demo.js). The short page
// links on to the explanation page (presentation/uitleg.js).
import { nextTick, onMounted, ref } from 'vue';
import { demoGuides } from '../presentation/demo.js';
import { useKort } from '../presentation/useKort.js';
import { useUitleg } from '../presentation/useUitleg.js';
import { api } from '../api.js';
import { bronMeldingen, herkomst } from '../bronnen.js';
import { artikelNaam } from '../logboek.js';
import { navigate } from '../navigation.js';
import { hoofdletter, kiezen, state } from '../state.js';

const emit = defineEmits(['presentatie', 'kort', 'demo']);
const demo = demoGuides();
const casussen = ref([]);
const kortKnop = ref(null);
const kort = useKort();
const uitleg = useUitleg();
// Back from the short page, or from the explanation page it leads to, the
// focus returns to the button of the short page.
onMounted(async () => {
  if (!kort.terug.value && !uitleg.terug.value) return;
  kort.terug.value = false;
  uitleg.terug.value = false;
  await nextTick();
  kortKnop.value?.focus();
});
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

// A navigation (navigation.js), so the address follows and a click overtakes
// an address that is still loading.
const kies = (c) => navigate(() => kiezen(c.id));
</script>

<template>
  <nldd-simple-section>
    <nldd-title heading-level="2" slot="header" size="display-3" text="Voor de demo">
      <nldd-text slot="supporting-text">Eerst de presentatie, dan de demoscenario's.</nldd-text>
    </nldd-title>
    <nldd-container gap="16">
      <nldd-button-bar>
        <nldd-button appearance="primary" start-icon="guide" text="Presentatie starten" @click="emit('presentatie')"></nldd-button>
        <nldd-button ref="kortKnop" appearance="secondary" start-icon="link" text="In het kort" @click="emit('kort')"></nldd-button>
      </nldd-button-bar>
      <nldd-title
        size="display-4"
        heading-level="2"
        text="Demoscenario's"
        supporting-text="Van simpel naar ingewikkeld, in twee casussen."
      ></nldd-title>
      <!-- `item-min-width`, not `item-width`: @nldd/design-system 0.9.0 renamed
           the attribute (collection.d.ts, @migration), and the old name now
           does nothing. -->
      <nldd-collection layout="grid" item-min-width="320px">
        <nldd-card v-for="(d, i) in demo" :key="d.guide.name">
          <nldd-container slot="header" padding="16">
            <nldd-title size="display-5" heading-level="3" :overline="`Demo ${i + 1} · ${d.titel}`" :text="d.guide.name"></nldd-title>
          </nldd-container>
          <nldd-container padding="16">
            <nldd-text>{{ d.guide.summary }}</nldd-text>
          </nldd-container>
          <nldd-container slot="footer" padding="16">
            <nldd-button
              appearance="secondary"
              text="Begin dit scenario"
              @click="emit('demo', { casus: d.casus, naam: d.guide.name })"
            ></nldd-button>
          </nldd-container>
        </nldd-card>
      </nldd-collection>
    </nldd-container>
  </nldd-simple-section>
  <nldd-simple-section>
    <nldd-title heading-level="2" slot="header" size="display-3" text="Kies een casus">
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
    <nldd-collection layout="grid" item-min-width="320px">
      <nldd-card v-for="c in casussen" :key="c.id">
        <nldd-container slot="header" padding="16">
          <nldd-title
            size="display-4"
            heading-level="2"
            :overline="`${hoofdletter(c.cel_naam)} (fictief)`"
            :text="c.titel"
            :supporting-text="`${artikel(c)} · beslist door ${c.rol}${herkomst(c, bronnen)}`"
          ></nldd-title>
        </nldd-container>
        <nldd-container slot="footer" padding="16">
          <nldd-button appearance="primary" :text="`${c.titel} naspelen`" @click="kies(c)"></nldd-button>
        </nldd-container>
      </nldd-card>
    </nldd-collection>
  </nldd-simple-section>
</template>
