<template>
  <div v-if="ready" class="wv" :class="{ 'wv-variant': !!werkversie, 'wv-bewerkt': changeCount > 0 }">
    <div class="wv-links">
      <span class="wv-label">Werkversie</span>
      <!-- Eigen varianten apart gegroepeerd: ze zien er in een uitkomst net zo
           stellig uit als een variant uit het dossier, terwijl ze van de
           gebruiker zelf komen en alleen in deze browser staan. -->
      <nldd-dropdown :key="`wv:${werkversie}:${variants.length}:${herteken}`" size="sm" width="360px" @change="kies($event.target.value ?? '')">
        <select :value="werkversie ?? ''" aria-label="Kies de werkversie: wat je bewerkt">
          <option value="">Huidig recht (basis)</option>
          <option v-for="v in dossierVarianten" :key="v.id" :value="v.id">{{ variantMetSoort(kortTitel(v)) }}</option>
          <optgroup v-if="eigenVarianten.length" label="Zelf bewaard (deze browser)">
            <option v-for="v in eigenVarianten" :key="v.id" :value="v.id">{{ v.title }}</option>
          </optgroup>
        </select>
      </nldd-dropdown>
      <span class="wv-status">
        <template v-if="changeCount === 0">ongewijzigd</template>
        <template v-else>{{ changeCount }} {{ changeCount === 1 ? 'document bewerkt' : 'documenten bewerkt' }}</template>
      </span>
    </div>
    <div class="wv-rechts">
      <nldd-button size="sm" text="Bekijk wijzigingen" start-icon="document" appearance="neutral-transparent" :disabled="!werkversie && changeCount === 0 ? true : undefined" @click="diffOpen = true"></nldd-button>
      <nldd-button size="sm" text="Terugzetten" start-icon="undo" appearance="neutral-transparent" :disabled="changeCount === 0 ? true : undefined" @click="resetChanges"></nldd-button>
      <!-- Werk je in je eigen variant, dan is bewaren in diezelfde variant de
           gewone handeling en aftakken de uitzondering; daarom staat die knop
           hier voorop en draagt hij de naam van de variant. -->
      <nldd-button
        v-if="eigenWerkversie"
        size="sm"
        :text="`Bewaar in ${werkversieLabel}`"
        start-icon="save"
        appearance="secondary"
        :disabled="changeCount === 0 || bezig ? true : undefined"
        @click="bewerkBij"
      ></nldd-button>
      <nldd-button size="sm" :text="eigenWerkversie ? 'Bewaar als nieuwe variant' : 'Bewaar als variant'" :start-icon="eigenWerkversie ? 'add' : 'save'" :appearance="eigenWerkversie ? 'neutral-transparent' : 'secondary'" :disabled="changeCount === 0 ? true : undefined" @click="opslaanOpen = !opslaanOpen"></nldd-button>
      <nldd-button v-if="eigenWerkversie" size="sm" text="Verwijder variant" start-icon="remove" appearance="neutral-transparent" @click="verwijderOpen = true"></nldd-button>
      <!-- "Terugzetten" hierboven maakt bewerkingen in de wet ongedaan; deze
           knop wist wat de browser onthoudt (kolommen, persona, keuzes,
           populatie-instellingen) en begint de demo schoon. -->
      <nldd-button
        v-if="ietsBewaard"
        size="sm"
        text="Begin opnieuw"
        start-icon="undo"
        appearance="neutral-transparent"
        @click="opnieuwOpen = true"
      ></nldd-button>
    </div>

    <nldd-banner v-if="opnieuwOpen" variant="warning" class="wv-breed">
      Hiermee vergeet de demo je keuzes: de gekozen kolommen, de werkversie, welke persoon je bekijkt, de
      instellingen van de populatie en de scenario's die je zelf hebt toegevoegd. De wet zelf en de opgeslagen
      varianten blijven staan.
      <span class="wv-knoppen">
        <nldd-button size="sm" text="Wis en begin opnieuw" appearance="primary" @click="resetPersistedState"></nldd-button>
        <nldd-button size="sm" text="Annuleren" appearance="neutral-transparent" @click="opnieuwOpen = false"></nldd-button>
      </span>
    </nldd-banner>

    <nldd-banner v-if="wisselNaar !== null" variant="warning" class="wv-breed">
      Je hebt {{ changeCount }} bewerkte {{ changeCount === 1 ? 'document' : 'documenten' }} in {{ werkversieLabel }}. Wisselen gooit die weg.
      <span class="wv-knoppen">
        <nldd-button size="sm" text="Toch wisselen" appearance="primary" @click="wisselBevestigd"></nldd-button>
        <nldd-button size="sm" text="Annuleren" appearance="neutral-transparent" @click="annuleerWissel"></nldd-button>
      </span>
    </nldd-banner>

    <div v-if="opslaanOpen" class="wv-breed wv-opslaan">
      <p class="wv-uitleg">
        Bewaart de bewerkte documenten
        {{ werkversie ? `(${werkversieLabel} plus jouw bewerkingen)` : '' }}
        als eigen variant en zet die als werkversie klaar. Daarna staat hij als kolom naast huidig recht,
        net als de varianten uit het dossier.
      </p>
      <p class="wv-uitleg">
        <strong>In deze browser.</strong> Een eigen variant gaat niet mee naar een ander apparaat of een
        collega, en is weg als je je browsergegevens wist. Hij overleeft wel een ververs en "Begin opnieuw".
      </p>
      <div class="wv-velden">
        <nldd-form-field label="Titel" supporting-label="korte omschrijving, bijvoorbeeld: voet naar 90% van het minimumloon">
          <nldd-text-field size="sm" :value="titel" @input="titel = $event.detail?.value ?? titel"></nldd-text-field>
        </nldd-form-field>
      </div>
      <div class="wv-knoppen">
        <nldd-button size="sm" text="Bewaar" appearance="primary" :disabled="!titel.trim() || bezig ? true : undefined" @click="opslaan"></nldd-button>
        <nldd-button size="sm" text="Annuleren" appearance="neutral-transparent" @click="opslaanOpen = false"></nldd-button>
        <span v-if="bezig" class="wv-status">bezig…</span>
      </div>
    </div>

    <nldd-banner v-if="verwijderOpen" variant="warning" class="wv-breed">
      {{ werkversieLabel }} verwijderen? Deze variant staat alleen in deze browser, dus hij is daarna weg.
      De werkversie valt terug op huidig recht.
      <span class="wv-knoppen">
        <nldd-button size="sm" text="Verwijderen" appearance="primary" @click="verwijder"></nldd-button>
        <nldd-button size="sm" text="Annuleren" appearance="neutral-transparent" @click="verwijderOpen = false"></nldd-button>
      </span>
    </nldd-banner>

    <!-- De wet is gewijzigd sinds deze variant is bewaard. De bewerking staat
         dus op een tekst die er niet meer zo staat; dat hoort de gebruiker te
         weten voordat hij een uitkomst gelooft. -->
    <nldd-banner v-if="driftPaden.length" variant="warning" class="wv-breed">
      {{ werkversieLabel }} is bewaard op een oudere versie van
      {{ driftPaden.length === 1 ? 'een wet' : `${driftPaden.length} wetten` }} in dit dossier. De variant
      rekent met de tekst van toen; huidig recht is sindsdien gewijzigd.
    </nldd-banner>
    <nldd-banner v-if="melding" :variant="meldingSoort" class="wv-breed" dismissible @dismiss="melding = ''">{{ melding }}</nldd-banner>
  </div>
  <diff-sheet :open="diffOpen" @close="diffOpen = false" />
</template>

<!--
  De werkversiebalk staat op elke pagina onder de navigatie. Er is precies één
  werkversie (huidig recht of één variant); alles wat bewerkt werkt daarop.
-->

<script setup>
import { ref, computed, watch } from 'vue';
import { useEngine } from '../engine/useEngine.js';
import { useLawStore, kortTitel } from '../engine/lawStore.js';
import { variantMetSoort } from '../lib/regimeFacts.js';
import { resetPersistedState, hasPersistedState } from '../composables/useBewaardeStand.js';
import DiffSheet from './beleid/DiffSheet.vue';

const { ready } = useEngine();
const {
  variants, werkversie, werkversieLabel, changeCount, version,
  setWerkversie, resetChanges,
  bewaarAlsBrowserVariant, werkWerkversieBij, verwijderEigenVariant, variantDrift, isBrowserVariant,
} = useLawStore();

const diffOpen = ref(false);
const wisselNaar = ref(null); // '' = huidig recht, anders variant-id; null = geen wissel gevraagd
const herteken = ref(0); // bumpen zet de keuzelijst terug op de werkversie na annuleren
const opnieuwOpen = ref(false);
const ietsBewaard = hasPersistedState();
const opslaanOpen = ref(false);
const verwijderOpen = ref(false);
const titel = ref('');
const bezig = ref(false);
const melding = ref('');
const meldingSoort = ref('accent');

/** Is de werkversie een variant die de gebruiker zelf bewaarde? */
const eigenWerkversie = computed(() => isBrowserVariant(werkversie.value));

const dossierVarianten = computed(() => variants.value.filter((v) => !isBrowserVariant(v.id)));
const eigenVarianten = computed(() => variants.value.filter((v) => isBrowserVariant(v.id)));

/**
 * Wetten waarvan de basis is gewijzigd sinds deze eigen variant is bewaard.
 * Hangt aan `version` zodat de melding meebeweegt met een wissel van werkversie.
 */
const driftPaden = computed(() => {
  version.value;
  return werkversie.value ? variantDrift(werkversie.value).paths : [];
});

function kies(value) {
  const doel = value === '' ? null : value;
  if (doel === (werkversie.value ?? null)) return;
  if (changeCount.value > 0) {
    wisselNaar.value = value;
    return;
  }
  doeWissel(doel);
}

async function doeWissel(doel) {
  await setWerkversie(doel);
}

function annuleerWissel() {
  wisselNaar.value = null;
  herteken.value++;
}

async function wisselBevestigd() {
  const value = wisselNaar.value;
  wisselNaar.value = null;
  await doeWissel(value === '' ? null : value);
}

async function opslaan() {
  bezig.value = true;
  melding.value = '';
  try {
    await bewaarAlsBrowserVariant(titel.value);
    opslaanOpen.value = false;
    titel.value = '';
    meldingSoort.value = 'success';
    melding.value = 'Bewaard als eigen variant; hij is nu de werkversie en staat in de keuzelijst.';
  } catch (e) {
    meldingSoort.value = 'critical';
    melding.value = `Bewaren mislukt: ${e?.message ?? e}`;
  } finally {
    bezig.value = false;
  }
}

/**
 * Bewaar de bewerkingen in de variant waar je nu in werkt. Geen dialoog: de
 * variant heeft al een naam, en er valt niets te kiezen.
 */
async function bewerkBij() {
  bezig.value = true;
  melding.value = '';
  const naam = werkversieLabel.value;
  try {
    await werkWerkversieBij();
    meldingSoort.value = 'success';
    melding.value = `De bewerkingen staan nu in ${naam}.`;
  } catch (e) {
    meldingSoort.value = 'critical';
    melding.value = `Bewaren in ${naam} mislukt: ${e?.message ?? e}`;
  } finally {
    bezig.value = false;
  }
}

async function verwijder() {
  verwijderOpen.value = false;
  const naam = werkversieLabel.value;
  try {
    await verwijderEigenVariant(werkversie.value);
    meldingSoort.value = 'accent';
    melding.value = `${naam} is verwijderd; de werkversie is weer huidig recht.`;
  } catch (e) {
    meldingSoort.value = 'critical';
    melding.value = `Verwijderen mislukt: ${e?.message ?? e}`;
  }
}
</script>

<style scoped>
.wv {
  display: flex; flex-wrap: wrap; align-items: center; gap: var(--primitives-space-12);
  padding: var(--primitives-space-8) var(--primitives-space-24);
  border-bottom: 1px solid var(--semantics-dividers-color);
  background: var(--semantics-surfaces-tinted-background-color);
}
.wv-variant { background: var(--primitives-color-lintblauw-50); }
.wv-links, .wv-rechts, .wv-knoppen { display: flex; align-items: center; gap: var(--primitives-space-8); flex-wrap: wrap; }
.wv-rechts { margin-left: auto; }
.wv-label { font-weight: 600; }
.wv-status { font-size: 0.9em; color: var(--semantics-content-secondary-color); }
.wv-bewerkt .wv-status { color: var(--semantics-content-warning-color); font-weight: 600; }
.wv-breed { flex-basis: 100%; }
.wv-opslaan { display: flex; flex-direction: column; gap: var(--primitives-space-8); padding: var(--primitives-space-8) 0; }
.wv-uitleg { margin: 0; font-size: 0.9em; color: var(--semantics-content-secondary-color); }
.wv-velden { display: flex; gap: var(--primitives-space-12); flex-wrap: wrap; }
.wv-velden nldd-form-field { min-width: 280px; }
</style>
