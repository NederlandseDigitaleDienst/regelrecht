<script setup>
// App-schil: nldd-shell, skip-link, bovenbalk, de knoppen "Over deze PoC" en "Presentatie",
// scrollende main met de kaart, voetregel met de engine.
import { onMounted, ref } from 'vue';
import { engineVersie, maakEngine, regelIndex } from '@eerste/engine.js';
import { laadDemo } from './data.js';
import { geladenBron } from './bron.js';
import KaartTab from './components/KaartTab.vue';
import OverPoc from './components/OverPoc.vue';
import PresentatieDeck from './components/PresentatieDeck.vue';

const overOpen = ref(false);
// De presentatie over de PoC, schermvullend.
const presentatieOpen = ref(false);
const status = ref({ geladen: false, fout: null, engine: '', commit: '', schema: '', regelingen: 0 });
const dso = ref({ geladen: false, fout: null, cellen: 0, artikelen: 0 });
// De bron: welke versie, en of het portaal een oude kopie serveert omdat ophalen mislukte.
const bron = geladenBron();

onMounted(async () => {
  laadDemo()
    .then((d) => { dso.value = { geladen: true, fout: null, cellen: d.cellen.filter((c) => !c.extra).length, artikelen: Object.keys(d.artikelen).length }; })
    .catch((e) => { dso.value = { ...dso.value, fout: String(e?.message ?? e) }; });
  try {
    const [versie, index] = await Promise.all([engineVersie(), regelIndex()]);
    const engine = await maakEngine(index.map((r) => r.path));
    status.value = {
      geladen: true,
      fout: null,
      engine: engine.version(),
      commit: (versie.commit ?? '').slice(0, 9),
      schema: versie.schema_hoogste,
      regelingen: index.length,
    };
  } catch (e) {
    status.value = { ...status.value, fout: String(e?.message ?? e) };
  }
});
</script>

<template>
  <nldd-shell class="schil">
    <div class="app-frame">
      <nldd-skip-link text="Direct naar de inhoud">
        <nldd-top-navigation-bar
          logo-title="Spoorzone Veenendaal"
          logo-subtitle="Verkenning · kan hier een woontoren van 5 lagen? Met en zonder DSO"
          website-href="./"
        >
        </nldd-top-navigation-bar>
      </nldd-skip-link>
      <div class="kop-knoppen">
        <nldd-button size="sm" appearance="neutral-tinted" start-icon="info-circle" text="Over deze PoC" @click="overOpen = true"></nldd-button>
        <nldd-button size="sm" appearance="neutral-tinted" start-icon="play" text="Presentatie" @click="presentatieOpen = true"></nldd-button>
      </div>
      <OverPoc v-model:open="overOpen" />
      <PresentatieDeck v-model:open="presentatieOpen" />

      <nldd-banner v-if="bron?.melding" variant="warning" size="sm" text="Mogelijk verouderde gegevens" :supporting-text="bron.melding"></nldd-banner>
      <main id="hoofdinhoud" class="app-main" tabindex="-1">
        <KaartTab />
      </main>

      <footer class="app-footer">
        <nldd-container padding-inline="16" padding-block="8">
          <nldd-text v-if="status.fout" size="xs" color="critical">Engine niet geladen: {{ status.fout }}</nldd-text>
          <nldd-text v-else-if="status.geladen" size="xs" color="secondary">
            RegelRecht-engine {{ status.engine }}<template v-if="status.commit">, commit {{ status.commit }}</template><template v-if="status.schema">, schema t/m {{ status.schema }}</template>,
            {{ status.regelingen }} regelingen geladen<template v-if="bron?.versie">, gegevens {{ bron.versie }}</template>.
            <template v-if="dso.geladen">DSO-spoor: {{ dso.cellen }} vakken van 40 m, {{ dso.artikelen }} artikelen, deels uit de pre-productieomgeving.</template>
            <template v-else-if="dso.fout">DSO-spoor niet geladen ({{ dso.fout }}).</template>
            Verkenning, geen besluit.
          </nldd-text>
          <nldd-text v-else size="xs" color="secondary">Engine laden…</nldd-text>
        </nldd-container>
      </footer>
    </div>
  </nldd-shell>
</template>

<style scoped>
/* nldd-shell clipt zijn inhoud en rekt zijn kind op tot zijn eigen hoogte; het kind scrollt
   zelf. Daarom staat de hoogte op de schil: het venster min de strook van het poc-portaal
   erboven (portaalStrook.js). */
.schil {
  height: calc(100dvh - var(--portaal-strook, 0px));
  min-height: 0;
}
.app-frame {
  position: relative;
  display: flex;
  flex-direction: column;
  height: 100%;
}
.app-main {
  flex: 1;
  min-height: 0;
  overflow-y: auto;
}
/* Linksboven in de kopbalk, los van het logo in het midden. */
.kop-knoppen { position: absolute; top: 16px; left: 16px; z-index: 5; display: flex; gap: 8px; }
.app-footer {
  border-top: 1px solid var(--primitives-color-neutral-200, #ddd);
}
</style>
