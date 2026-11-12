<template>
  <nldd-page>
    <nldd-simple-section>
      <nldd-title heading-level="1" size="display-2">
        <span slot="overline">Voor beleidsmakers</span>
        <span>Scenario's: wie gaat erop vooruit, wie achteruit</span>
        <span slot="supporting-text">
          Bij terugbetalen hangt de uitkomst af van wie je bent: regime, schuld, inkomen en of je een partner hebt.
          Het gemiddelde verbergt dat. Hier staat elk scenario onder huidig recht naast de varianten die je kiest. Je kunt er zelf een toevoegen.
        </span>
      </nldd-title>

      <!-- Dezelfde kolomkeuze als op /beleid: welke varianten je vergelijkt is
           één keuze die beide pagina's delen, maar je moet hem kunnen maken
           waar je kijkt. -->
      <kolom-kiezer class="kiezer" />

      <div class="pv-acties">
        <nldd-button
          text="Scenario toevoegen"
          start-icon="add"
          appearance="secondary"
          size="sm"
          @click="nieuwScenario"
        ></nldd-button>
      </div>

      <div class="meldingen">
        <nldd-banner v-if="initError" variant="critical">
          De rekenmachine kon niet worden geladen: {{ initError.message }}
        </nldd-banner>
        <nldd-banner v-else-if="!ready" variant="accent">Rekenmachine wordt geladen…</nldd-banner>
        <nldd-banner v-if="fout" variant="warning">{{ fout }}</nldd-banner>
      </div>
    </nldd-simple-section>

    <nldd-simple-section v-if="ready && rijen.length" background="tinted">
      <div class="pv-scroll">
        <table class="pv-tabel">
          <caption class="pv-caption">
            Alle vier de maten naast elkaar. {{ columns.length > 1 ? 'Per maat staat elke kolom voor een wetsversie; de pijl vergelijkt met huidig recht.' : 'Vink op de beleidspagina een variant aan om er kolommen naast te zetten.' }}
          </caption>
          <thead>
            <tr>
              <th scope="col" rowspan="2">Scenario</th>
              <th scope="col" rowspan="2">Situatie</th>
              <th v-for="m in MATEN" :key="m.key" scope="colgroup" :colspan="columns.length" class="num pv-maat">
                {{ m.label }}
                <span class="pv-maat-hint">{{ m.uitleg }}</span>
              </th>
            </tr>
            <tr v-if="columns.length > 1">
              <template v-for="m in MATEN" :key="m.key">
                <th
                  v-for="col in columns"
                  :key="`${m.key}:${col.key}`"
                  scope="col"
                  class="num pv-sub"
                  :title="col.titel"
                >
                  {{ col.code }}
                </th>
              </template>
            </tr>
          </thead>
          <tbody>
            <tr v-for="rij in rijen" :key="rij.persona.bsn">
              <th scope="row" class="pv-naam">
                <button type="button" class="pv-link" @click="open(rij)">{{ rij.persona.naam }}</button>
                <nldd-tag v-if="rij.persona.eigen" size="sm" color="neutral" text="zelf toegevoegd"></nldd-tag>
                <nldd-tag
                  v-if="rij.cellen[KOLOM_BASIS]?.regime"
                  :color="regimeColor(rij.cellen[KOLOM_BASIS].regime)"
                  size="sm"
                  :text="regimeLabel(rij.cellen[KOLOM_BASIS].regime)"
                ></nldd-tag>
              </th>
              <td class="pv-situatie">
                {{ situatie(rij.persona) }}
                <span v-if="rij.persona.eigen" class="pv-eigen-acties">
                  <button type="button" class="pv-mini" @click="bewerkScenario(rij.persona)">aanpassen</button>
                  <button type="button" class="pv-mini" @click="verwijderEnHerbereken(rij.persona.bsn)">verwijderen</button>
                </span>
              </td>
              <template v-for="m in MATEN" :key="m.key">
                <td
                  v-for="col in columns"
                  :key="`${m.key}:${col.key}`"
                  class="num"
                >
                  <template v-if="rij.cellen[col.key]">
                    <span>{{ toon(rij.cellen[col.key], m.key) }}</span>
                    <span v-if="deltaTekst(rij, col, m.key)" class="pv-delta" :class="deltaClass(rij, col, m.key)">
                      {{ deltaTekst(rij, col, m.key) }}
                    </span>
                  </template>
                  <span v-else class="pv-leeg">—</span>
                </td>
              </template>
            </tr>
          </tbody>
        </table>
      </div>

      <p v-if="columns.length > 1" class="pv-legenda">
        <template v-for="(col, i) in columns" :key="col.key">
          <span v-if="i" aria-hidden="true"> · </span><strong>{{ col.code }}</strong> = {{ col.titel }}
        </template>
      </p>

      <p class="pv-bron">
        De vaste scenario's staan in <code>data/personas.yaml</code> en hebben hetzelfde recordschema als de
        populatie, zodat interne DUO-data er later voor in de plaats kan. Klik op een naam voor de berekening
        met de herkomst per stap. Zelf toegevoegde scenario's blijven in je browser staan tot je ze verwijdert.
      </p>
    </nldd-simple-section>

    <scenario-formulier
      :open="formulierOpen"
      :scenario="teBewerken"
      @close="formulierOpen = false"
      @opslaan="opslaanScenario"
    />

    <berekening-sheet
      :open="sheetOpen"
      :timeline="gekozenTimeline"
      :trace="gekozenTrace"
      @close="sheetOpen = false"
    />
  </nldd-page>
</template>

<!--
  Persona-overzicht voor beleidsmakers. Waar /burger één persoon toont zoals
  die het zelf ziet, staan hier alle persona's naast elkaar onder huidig recht
  en onder elke gekozen variant. Dat is precies wat bij deze casus wél
  interessant is en bij nieuwkomersbekostiging niet: daar krijgt iedereen die
  aan de definitie voldoet hetzelfde bedrag, hier hangt alles af van wie je bent.
-->

<script setup>
import { ref, computed, watch, onMounted } from 'vue';
import { useEngine } from '../engine/useEngine.js';
import { useLawStore } from '../engine/lawStore.js';
import { usePersonas } from '../composables/usePersonas.js';
import { usePopulation, KOLOM_BASIS } from '../composables/usePopulation.js';
import { useKolomEngines } from '../composables/useKolomEngines.js';
import { defaultChoices, runSimulation, traceOutput } from '../composables/useSimulation.js';
import { simulate } from '../sim/simulate.js';
import { euro, euroWhole, regimeLabel, regimeColor } from '../lib/format.js';
import BerekeningSheet from '../components/burger/BerekeningSheet.vue';
import KolomKiezer from '../components/beleid/KolomKiezer.vue';
import ScenarioFormulier from '../components/scenario/ScenarioFormulier.vue';

const { ready, initError, lawIndex, initEngine } = useEngine();
const { initStore, version } = useLawStore();
const { personas, fetchPersonas, bewaarScenario, verwijderScenario } = usePersonas();
const { columns } = usePopulation();
const { engineFor, pruneEngines } = useKolomEngines();

const uitkomsten = ref({}); // kolomkey -> { bsn -> samenvatting }
const fout = ref('');
const sheetOpen = ref(false);
const formulierOpen = ref(false);
const teBewerken = ref(null);
const gekozenTimeline = ref([]);
const gekozenTrace = ref(null);

/**
 * De vier maten staan naast elkaar in één tabel. `beterLager` bepaalt de
 * kleur van de pijl: een lager maandbedrag is goed voor de debiteur, meer
 * kwijtschelding kost de begroting geld, en meer betalen is niet gunstig
 * voor wie het betaalt.
 */
const MATEN = [
  { key: 'maandbedrag', label: 'Maandbedrag', uitleg: 'eerste jaar', beterLager: true },
  { key: 'einde', label: 'Klaar met betalen', uitleg: 'jaar, of levenslang', beterLager: true },
  { key: 'kwijt', label: 'Kwijtschelding', uitleg: 'kosten OCW-begroting', beterLager: true },
  { key: 'betaald', label: 'Totaal betaald', uitleg: 'aflossing en rente', beterLager: true },
];

function samenvatting(r) {
  if (!r) return null;
  return {
    maandbedrag: r.timeline.length ? r.timeline[0].maandbedrag : null,
    einde: r.totals.levenslang ? 'levenslang' : String(r.totals.eindejaar),
    eindejaar: r.totals.levenslang ? Infinity : r.totals.eindejaar,
    kwijt: r.totals.kwijtgescholden,
    betaald: r.totals.totaalBetaald,
    regime: r.totals.regime,
  };
}

/** Draai alle persona's door alle kolommen; per kolom één engine. */
async function herbereken() {
  if (!ready.value || !personas.value.length) return;
  fout.value = '';
  const cols = columns.value;
  pruneEngines(cols.map((c) => c.key));
  const out = {};
  for (const col of cols) {
    try {
      const engine = await engineFor(col);
      const perBsn = {};
      for (const persona of personas.value) {
        try {
          perBsn[persona.bsn] = samenvatting(
            simulate(engine, persona, defaultChoices(persona), { startJaar: 2026 }),
          );
        } catch (e) {
          console.warn('Persona mislukt', persona.naam, col.key, e);
          perBsn[persona.bsn] = null;
        } finally {
          engine.clearDataSources?.();
        }
      }
      out[col.key] = perBsn;
    } catch (e) {
      fout.value = `Kolom ${col.titel} kon niet worden doorgerekend: ${String(e?.message ?? e)}`;
    }
  }
  uitkomsten.value = out;
}

const rijen = computed(() => personas.value.map((persona) => ({
  persona,
  cellen: Object.fromEntries(
    columns.value
      .map((col) => [col.key, uitkomsten.value[col.key]?.[persona.bsn] ?? null])
      .filter(([, v]) => v !== null),
  ),
})));

/** Eén regel context per persona, zodat de getallen te plaatsen zijn. */
function nieuwScenario() {
  teBewerken.value = null;
  formulierOpen.value = true;
}

function bewerkScenario(persona) {
  teBewerken.value = persona;
  formulierOpen.value = true;
}

async function opslaanScenario(record) {
  bewaarScenario(record);
  formulierOpen.value = false;
  await herbereken();
}

async function verwijderEnHerbereken(bsn) {
  verwijderScenario(bsn);
  await herbereken();
}

function situatie(p) {
  const delen = [`schuld ${euroWhole(p.schuld)}`, `inkomen ${euroWhole(p.inkomen)}`];
  if (p.heeft_partner) delen.push(`partner ${euroWhole(p.partnerinkomen)}`);
  return delen.join(' · ');
}

function toon(cel, maat) {
  if (!cel) return '—';
  if (maat === 'maandbedrag') return cel.maandbedrag === null ? '—' : euro(cel.maandbedrag);
  if (maat === 'einde') return cel.einde;
  if (maat === 'kwijt') return euroWhole(cel.kwijt);
  return euroWhole(cel.betaald);
}

function waarde(cel, maat) {
  if (!cel) return null;
  if (maat === 'maandbedrag') return cel.maandbedrag;
  if (maat === 'einde') return cel.eindejaar;
  if (maat === 'kwijt') return cel.kwijt;
  return cel.betaald;
}

function deltaTekst(rij, col, maat) {
  if (col.key === KOLOM_BASIS) return '';
  const a = waarde(rij.cellen[col.key], maat);
  const b = waarde(rij.cellen[KOLOM_BASIS], maat);
  if (a === null || b === null || a === b) return '';
  if (!Number.isFinite(a) || !Number.isFinite(b)) return a > b ? '▲' : '▼';
  return a < b ? '▼' : '▲';
}

function deltaClass(rij, col, maat) {
  const t = deltaTekst(rij, col, maat);
  if (!t) return '';
  const beterLager = MATEN.find((m) => m.key === maat)?.beterLager ?? true;
  return (t === '▼') === beterLager ? 'pv-goed' : 'pv-slecht';
}

/**
 * De berekening tonen zoals de werkversie hem maakt: dat is de wetsversie
 * waarop je bewerkt, en dus waar de trace over gaat. De kolommen hierboven
 * zetten de uitkomsten naast elkaar; de sheet legt er één uit.
 */
function open(rij) {
  const keuzes = defaultChoices(rij.persona);
  const sim = runSimulation(rij.persona, keuzes);
  gekozenTimeline.value = sim?.timeline ?? [];
  gekozenTrace.value = traceOutput(rij.persona, keuzes);
  sheetOpen.value = true;
}

onMounted(async () => {
  const engine = await initEngine();
  await initStore(engine, lawIndex.value);
  await fetchPersonas().catch(() => {});
  await herbereken();
});

watch([version, columns, personas, ready], herbereken);
</script>

<style scoped>
.kiezer { display: block; margin-top: var(--primitives-space-16); }
.meldingen { display: flex; flex-direction: column; gap: var(--primitives-space-8); margin-top: var(--primitives-space-16); }
.meldingen:empty { display: none; }
.pv-acties { display: flex; justify-content: flex-end; margin-top: var(--primitives-space-12); }
.pv-eigen-acties { display: inline-flex; gap: var(--primitives-space-8); margin-left: var(--primitives-space-8); }
.pv-mini {
  background: none;
  border: none;
  padding: 0;
  font: inherit;
  font-size: 0.9em;
  color: var(--semantics-content-accent-color);
  cursor: pointer;
  text-decoration: underline;
}
.pv-scroll { overflow-x: auto; }
.pv-tabel { width: 100%; border-collapse: collapse; font-size: 0.9em; }
.pv-caption { text-align: left; font-size: 0.85em; color: var(--semantics-content-secondary-color); padding-bottom: var(--primitives-space-8); }
.pv-tabel th, .pv-tabel td { text-align: left; padding: var(--primitives-space-8); border-bottom: 1px solid var(--semantics-dividers-color); vertical-align: top; }
.pv-tabel thead th { font-weight: 600; }
.pv-maat-hint { display: block; font-size: 0.8em; font-weight: 400; color: var(--semantics-content-secondary-color); }
.pv-sub { font-size: 0.85em; font-weight: 400; color: var(--semantics-content-secondary-color); }
.pv-legenda { margin: var(--primitives-space-8) 0 0; font-size: 0.85em; color: var(--semantics-content-secondary-color); }

.pv-tabel .num { text-align: right; font-variant-numeric: tabular-nums; white-space: nowrap; }
.pv-naam { display: flex; align-items: center; gap: var(--primitives-space-8); flex-wrap: wrap; }
.pv-link {
  background: none;
  border: none;
  padding: 0;
  font: inherit;
  font-weight: 600;
  color: var(--semantics-content-accent-color);
  cursor: pointer;
  text-decoration: underline;
}
/* Niet midden in een bedrag afbreken; liever een bredere kolom. */
.pv-situatie {
  font-size: 0.85em;
  color: var(--semantics-content-secondary-color);
  min-width: 190px;
}
.pv-delta { margin-left: var(--primitives-space-4); font-size: 0.85em; }
.pv-goed { color: var(--semantics-content-success-color); }
.pv-slecht { color: var(--semantics-content-critical-color); }
.pv-leeg { color: var(--semantics-content-secondary-color); }
.pv-bron { margin: var(--primitives-space-8) 0 0; font-size: 0.8em; color: var(--semantics-content-secondary-color); }
</style>
