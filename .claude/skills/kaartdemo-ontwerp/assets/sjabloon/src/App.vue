<script setup>
// App-schil: nldd-shell, skip-link, bovenbalk, knoppen linksboven, scrollende main met de kaart,
// voetregel met de engine. Vul de engine-status in zodra er een engine is.
import { ref } from 'vue';
import KaartTab from './components/KaartTab.vue';
import OverPoc from './components/OverPoc.vue';

const overOpen = ref(false);
// { geladen, fout, engine, commit, schema, regelingen }
const status = ref({ geladen: false, fout: null, engine: '', commit: '', schema: '', regelingen: 0 });
// Zet op een tekst als de gegevens een oude kopie zijn; dan verschijnt een waarschuwing.
const melding = ref(null);
</script>

<template>
  <nldd-shell>
    <div class="app-frame">
      <nldd-skip-link text="Direct naar de inhoud">
        <nldd-top-navigation-bar
          logo-title="%%GEBIED%%"
          logo-subtitle="Verkenning · %%VRAAG%%"
          website-href="./"
        ></nldd-top-navigation-bar>
      </nldd-skip-link>
      <div class="kop-knoppen">
        <nldd-button size="sm" appearance="neutral-tinted" start-icon="info-circle" text="Over deze PoC" @click="overOpen = true"></nldd-button>
        <!-- Optioneel: <nldd-button size="sm" appearance="neutral-tinted" start-icon="play" text="Presentatie" …> -->
      </div>
      <OverPoc v-model:open="overOpen" />

      <nldd-banner v-if="melding" variant="warning" size="sm" text="Mogelijk verouderde gegevens" :supporting-text="melding"></nldd-banner>
      <main id="hoofdinhoud" class="app-main" tabindex="-1">
        <KaartTab />
      </main>

      <footer class="app-footer">
        <nldd-container padding-inline="16" padding-block="8">
          <nldd-text v-if="status.fout" size="xs" color="critical"><p>Engine niet geladen: {{ status.fout }}</p></nldd-text>
          <nldd-text v-else-if="status.geladen" size="xs" color="secondary">
            <p>
              RegelRecht-engine {{ status.engine }}<template v-if="status.commit">, commit {{ status.commit }}</template><template v-if="status.schema">, schema t/m {{ status.schema }}</template>,
              {{ status.regelingen }} regelingen geladen. Verkenning, geen besluit.
            </p>
          </nldd-text>
          <nldd-text v-else size="xs" color="secondary"><p>Verkenning, geen besluit.</p></nldd-text>
        </nldd-container>
      </footer>
    </div>
  </nldd-shell>
</template>

<style scoped>
.app-frame {
  position: relative;
  display: flex;
  flex-direction: column;
  /* nldd-shell knipt zijn inhoud af; vaste hoogte, de main scrollt zelf. */
  height: 100vh;
}
.app-main { flex: 1; min-height: 0; overflow-y: auto; }
/* Linksboven in de kopbalk, los van het logo in het midden. */
.kop-knoppen { position: absolute; top: 16px; left: 16px; z-index: 5; display: flex; gap: 8px; }
.app-footer { border-top: 1px solid var(--primitives-color-neutral-200, #ddd); }
</style>
