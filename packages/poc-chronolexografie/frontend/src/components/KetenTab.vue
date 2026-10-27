<script setup>
// The chain of the case (tab Keten): from the article that decides it to
// what the cells read from their chronicles, built by the game from the
// regelwerken the case loads (`api.keten`, src/keten.rs) on the play date.
// Each step opens (KetenSchakel.vue); the YAML of an article opens in a
// sheet, with its explanation, its file and its source.
import { onMounted, ref, watch } from 'vue';
import { api } from '../api.js';
import { state } from '../state.js';
import { artikelNaam } from '../logboek.js';
import { bestandPad, korteUitleg, laagNaam } from '../keten.js';
import KetenSchakel from './KetenSchakel.vue';

const keten = ref(null);
const yaml = ref(null);

// Only fetch, no step: an error of the last step stays where it is.
async function laden() {
  try {
    keten.value = await api.keten(state.datum);
  } catch (e) {
    state.fout = e.message;
  }
}

onMounted(laden);
watch(() => state.datum, laden);
</script>

<template>
  <nldd-simple-section>
    <nldd-title slot="header" size="3" :text="`Keten van ${state.casus.titel.toLowerCase()}`">
      <nldd-text slot="supporting-text">
        Van het artikel dat over de aanvraag beslist naar wat de cellen uit hun kroniek lezen. Het
        spel leidt deze keten af uit de regelwerken die de casus laadt, op de speeldatum. Een open
        begrip staat onder het artikel dat het openlaat, met het beleid dat het invult. Neemt dat
        beleid een antwoord van een cel, dan volgen de cel en haar reductie. Klap een stap open voor
        uitleg en klik op YAML bekijken voor het artikel zoals het in de corpus staat. Wat de PoC
        verzon, zoals de verordening, het beleid en de reducties, draagt het label fictief.
      </nldd-text>
    </nldd-title>
    <nldd-list v-if="keten" type="tree" appearance="box-base" accessible-label="Keten van de casus">
      <KetenSchakel :schakel="keten" open @yaml="yaml = $event" />
    </nldd-list>
    <nldd-text v-else size="sm" color="secondary">Even geduld…</nldd-text>
  </nldd-simple-section>

  <Teleport to="body">
    <nldd-sheet width="720px" :open="yaml ? true : undefined" @close="yaml = null">
      <nldd-page>
        <nldd-container slot="header" padding="16">
          <nldd-top-title-bar
            heading-level="2"
            :text="yaml ? artikelNaam(yaml.wet, yaml.artikel) : ''"
            :supporting-text="yaml ? `${laagNaam(yaml.laag)} · ${bestandPad(yaml.bestand, state.casus?.bron)}` : ''"
            dismiss-text="Sluiten"
          ></nldd-top-title-bar>
        </nldd-container>
        <nldd-simple-section v-if="yaml">
          <nldd-container gap="8">
            <nldd-text v-if="korteUitleg(yaml.tekst)">{{ korteUitleg(yaml.tekst) }}</nldd-text>
            <nldd-link v-if="yaml.url" :href="yaml.url" target="_blank" text="Bron van de tekst"></nldd-link>
            <nldd-code-viewer language="yaml" appearance="box-tinted">{{ yaml.yaml }}</nldd-code-viewer>
          </nldd-container>
        </nldd-simple-section>
      </nldd-page>
    </nldd-sheet>
  </Teleport>
</template>
