<template>
  <nldd-page>
    <nldd-simple-section>
      <div class="intro-kop">
        <nldd-title heading-level="1" size="display-1">
          <span slot="overline">Voor beleidsmakers</span>
          <span>Eén leerling door de regeling</span>
        </nldd-title>
        <div class="kop-rechts">
          <persona-switcher
            v-if="ready && personas.length"
            :personas="personas"
            :selected-bsn="selected?.bsn"
            @select="selectPersona"
          />
          <variant-switcher multiple :model-value="variantIds" @update:model-value="variantIds = $event" />
        </div>
      </div>

      <div class="meldingen">
        <nldd-banner v-if="initError" variant="critical">
          De rekenmachine kon niet worden geladen: {{ initError.message }}
        </nldd-banner>
        <nldd-banner v-else-if="!ready" variant="accent">Een moment, de regelgeving wordt geladen…</nldd-banner>
        <nldd-banner v-if="variantVanafDatum" variant="accent">
          De variant geldt vanaf {{ datumLabel(variantVanafDatum) }}; eerdere peildata zijn in beide kolommen gelijk.
        </nldd-banner>
        <nldd-banner v-else-if="!personas.length" variant="warning">
          Geen persona's gevonden in data/personas.yaml.
        </nldd-banner>
        <nldd-banner v-if="error" variant="critical">{{ error }}</nldd-banner>
      </div>
    </nldd-simple-section>

    <template v-if="ready && selected">
      <nldd-simple-section background="tinted">
        <div class="persona-kaart">
          <div>
            <nldd-title heading-level="2" size="display-3"><span>{{ selected.naam }}</span></nldd-title>
            <p class="omschrijving">{{ selected.omschrijving }}</p>
          </div>
          <dl class="feiten">
            <dt>Sector</dt><dd>{{ sectorLabel(sector) }} ({{ selected.schoolsoort }}), school {{ selected.school_id ?? '—' }}</dd>
            <dt>Geboren</dt><dd>{{ datumLabel(selected.geboortedatum) }}</dd>
            <dt>Gevestigd in Nederland</dt><dd>{{ datumLabel(selected.datum_vestiging_nederland) }}</dd>
            <dt>Eerste inschrijving</dt><dd>{{ datumLabel(selected.eerste_inschrijfdatum) }}</dd>
            <dt>Verblijfstitel</dt><dd>{{ selected.heeft_bsn === false ? 'geen BSN (onderwijsnummer)' : `code ${selected.verblijfstitel_code ?? 'onbekend'}` }}</dd>
          </dl>
        </div>
      </nldd-simple-section>

      <!-- De peildata staan boven de uitleg: dat is de uitkomst waar iemand
           voor komt, en de herleiding eronder is waar je heen gaat als je hem
           niet gelooft. -->
      <nldd-simple-section>
        <nldd-title heading-level="2" size="display-4">
          <span>Regelingen vergelijken per peildatum</span>
          <span slot="supporting-text">Per kwartaal: telt de leerling, in welke categorie en welk jaar, tegen welk bedrag (25% van het jaarbedrag). Huidig recht staat altijd links; elke gekozen regeling komt daarnaast.</span>
        </nldd-title>
        <nldd-activity-indicator v-if="loading" size="24" timing="instant"></nldd-activity-indicator>
        <peildata-tijdlijn
          v-else-if="istTimeline"
          :peildata="PERSONA_PEILDATA"
          :ist="istTimeline"
          :kolommen="variantKolommen"
          :ist-titel="istTitel"
          :geselecteerd="tracePeildatum"
          @trace="openTrace"
        />
      </nldd-simple-section>

      <nldd-simple-section>
        <nldd-title heading-level="2" size="display-4">
          <span>Waarom deze leerling meetelt, en hoe vaak</span>
          <span slot="supporting-text">Van de gegevens in ROD naar de categorie (asielzoeker of overige vreemdeling) en het aantal kwartalen dat bekostigd wordt, onder huidig recht.</span>
        </nldd-title>
        <categorie-uitleg :persona="selected" :uitkomst="uitlegUitkomst" />
      </nldd-simple-section>
    </template>

    <berekening-sheet
      :open="traceOpen"
      :titel="`${selected?.naam ?? ''} op ${datumLabel(tracePeildatum)}${traceKolom ? ` (${variantTitelVan(traceKolom)})` : ''}`"
      :trace="trace"
      :error="traceError"
      :peildata="PERSONA_PEILDATA"
      :peildatum="tracePeildatum"
      :outputs="traceOutputs"
      :output="traceOutput"
      @update:peildatum="tracePeildatum = $event"
      @update:output="traceOutput = $event"
      @close="traceOpen = false"
    />
  </nldd-page>
</template>

<script setup>
import { ref, computed, onMounted, watch } from 'vue';
import { useEngine } from '../engine/useEngine.js';
import { useLawStore } from '../engine/lawStore.js';
import { usePersonas, PERSONA_PEILDATA } from '../composables/usePersonas.js';
import { useColumnEngines } from '../composables/useColumnEngines.js';
import { shortTitle } from '../composables/useSimulation.js';
import { PO_LEERLING_OUTPUTS, VO_LEERLING_OUTPUTS, sectorVan } from '../sim/simulate.js';
import PersonaSwitcher from '../components/casus/PersonaSwitcher.vue';
import VariantSwitcher from '../components/VariantSwitcher.vue';
import CategorieUitleg from '../components/casus/CategorieUitleg.vue';
import PeildataTijdlijn from '../components/casus/PeildataTijdlijn.vue';
import BerekeningSheet from '../components/casus/BerekeningSheet.vue';
import { datumLabel, sectorLabel, variantVanaf } from '../lib/nieuwkomerFacts.js';
import { persistedRef } from '../composables/useBewaardeStand.js';

const { ready, initError, lawIndex, initEngine } = useEngine();
const { initStore, version, variants, werkversie, changeCount } = useLawStore();
const { personas, fetchPersonas, personaTimeline, personaTrace } = usePersonas();
const { engineFor } = useColumnEngines();

const selected = ref(null);
// Meerdere regelingen tegelijk, net als op het beleidstabblad: de impact van
// varianten op één leerling is pas te zien als ze naast elkaar staan, niet als
// je ze twee aan twee moet vergelijken. Huidig recht staat altijd links.
const variantIds = persistedRef('casussen.varianten', []);
const variantVanafDatum = computed(() => {
  // De vroegste ingangsdatum onder de gekozen varianten: daarvoor zijn alle
  // kolommen per definitie gelijk.
  const datums = variantIds.value
    .map((id) => variantVanaf(variants.value.find((x) => x.id === id)))
    .filter(Boolean)
    .sort();
  return datums[0] ?? null;
});
const istTimeline = ref(null);
/** Per gekozen variant-id de tijdlijn van deze leerling. */
const variantTimelines = ref({});
const loading = ref(false);
const error = ref(null);

const sector = computed(() => sectorVan(selected.value));
function variantTitelVan(id) {
  const v = variants.value.find((x) => x.id === id);
  const bewerkt = id === werkversie.value && changeCount.value ? ' (werkversie, bewerkt)' : '';
  return (v ? shortTitle(v) : 'Variant') + bewerkt;
}

/** Wat de tijdlijn naast huidig recht zet: [{ id, titel, timeline }]. */
const variantKolommen = computed(() => variantIds.value
  .filter((id) => variantTimelines.value[id])
  .map((id) => ({ id, titel: variantTitelVan(id), timeline: variantTimelines.value[id] })));
/** Links staat huidig recht; als dat de werkversie is met bewerkingen, zeg dat. */
const istTitel = computed(() => (werkversie.value === null && changeCount.value ? 'Huidig recht (werkversie, bewerkt)' : 'Huidig recht'));

/** Eerste uitkomst waarin de leerling in het bestand zit, anders de eerste tellende. */
const uitlegUitkomst = computed(() => {
  const tl = istTimeline.value;
  if (!tl) return null;
  return tl.find((u) => u.telt) ?? tl.find((u) => u.in_bestand) ?? tl[tl.length - 1] ?? null;
});

onMounted(async () => {
  await fetchPersonas().catch(() => {});
  try {
    const engine = await initEngine();
    await initStore(engine, lawIndex.value);
    if (!selected.value && personas.value.length) selectPersona(personas.value[0]);
  } catch {
    // fout via initError
  }
});

function selectPersona(persona) {
  selected.value = persona;
}

let runId = 0;
async function herbereken() {
  if (!selected.value || !ready.value) return;
  const id = ++runId;
  loading.value = true;
  error.value = null;
  try {
    const ist = await engineFor(null);
    const tlIst = personaTimeline(ist, selected.value);
    const tlVar = {};
    for (const vid of variantIds.value) {
      const ve = await engineFor(vid);
      tlVar[vid] = personaTimeline(ve, selected.value);
    }
    if (id !== runId) return;
    istTimeline.value = tlIst;
    variantTimelines.value = tlVar;
    const fout = tlIst.find((u) => u.fout);
    if (fout) error.value = `De engine kon deze casus niet volledig doorrekenen: ${fout.fout}`;
    if (traceOpen.value) laadTrace();
  } catch (e) {
    if (id === runId) error.value = String(e?.message ?? e);
  } finally {
    if (id === runId) loading.value = false;
  }
}

watch([selected, variantIds, version, ready], herbereken, { deep: true });

// ---- Trace-sheet ------------------------------------------------------------
const traceOpen = ref(false);
const tracePeildatum = ref(PERSONA_PEILDATA[0]);
const traceOutput = ref('bedrag_kwartaal');
// Welke kolom de trace toont: null = huidig recht, anders een variant-id.
const traceKolom = ref(null);
const trace = ref(null);
const traceError = ref(null);

const traceOutputs = computed(() => (sector.value === 'vo' ? VO_LEERLING_OUTPUTS : PO_LEERLING_OUTPUTS));

function openTrace(peildatum) {
  tracePeildatum.value = peildatum;
  traceKolom.value = null;
  traceOpen.value = true;
  laadTrace();
}

async function laadTrace() {
  trace.value = null;
  traceError.value = null;
  try {
    const engine = await engineFor(traceKolom.value);
    trace.value = personaTrace(engine, selected.value, tracePeildatum.value, traceOutput.value);
  } catch (e) {
    traceError.value = String(e?.message ?? e);
  }
}

watch([tracePeildatum, traceOutput], () => {
  if (traceOpen.value) laadTrace();
});
</script>

<style scoped>
.intro-kop {
  display: flex;
  align-items: flex-start;
  justify-content: space-between;
  gap: var(--primitives-space-16);
  flex-wrap: wrap;
}
.kop-rechts { display: flex; gap: var(--primitives-space-12); align-items: center; flex-wrap: wrap; margin-left: auto; }
.kop-rechts > * { flex: 0 0 auto; }
.meldingen { display: flex; flex-direction: column; gap: var(--primitives-space-8); margin-top: var(--primitives-space-16); }
.meldingen:empty { display: none; }
.persona-kaart { display: grid; grid-template-columns: minmax(260px, 1.2fr) minmax(260px, 1fr); gap: var(--primitives-space-24); }
.omschrijving { margin: var(--primitives-space-8) 0 0; color: var(--semantics-content-secondary-color); }
.feiten { margin: 0; display: grid; grid-template-columns: max-content 1fr; gap: 4px 12px; font-size: 0.9em; }
.feiten dt { color: var(--semantics-content-secondary-color); }
.feiten dd { margin: 0; }
@media (max-width: 700px) {
  .persona-kaart { grid-template-columns: 1fr; }
}
</style>
