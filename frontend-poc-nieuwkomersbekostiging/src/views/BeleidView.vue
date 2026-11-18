<template>
  <nldd-side-by-side-split-view panes="2">
    <!-- LINKS: wat vergelijk je, wat wijzig je, hulpmiddelen, instellingen -->
    <div slot="pane-1" class="pane pane-left">
      <nldd-page landmarks="page">
        <div class="pane-inner">
          <nldd-title heading-level="1" size="display-2">
            <span slot="overline">Voor beleidsmakers</span>
            <span>Nieuwkomersbekostiging: regeling en uitvoering</span>
          </nldd-title>

          <nldd-banner v-if="mountError && !initError" variant="critical">
            Doorrekenen mislukt: {{ mountError.message }}
          </nldd-banner>
          <nldd-banner v-if="initError" variant="critical">
            De rekenmachine kon niet worden geladen: {{ initError.message }}
          </nldd-banner>
          <nldd-banner v-else-if="!ready" variant="accent">Rekenmachine wordt geladen…</nldd-banner>

          <template v-if="ready">
            <panel :title="`Regeling bijstellen`" :subtitle="`Bedragen, duur en drempel van ${werkversieLabel}. Elke wijziging rekent de werkversiekolom opnieuw.`" :summary="bijstellenSamenvatting" :badge="hasChanges ? `${changeCount} bewerkt` : ''">
              <parameter-panel />
              <details class="geavanceerd">
                <summary>Geavanceerd: YAML rechtstreeks bewerken</summary>
                <div class="geav-row">
                  <nldd-dropdown :key="`ye:${editableDocs.length}:${yamlKeuze}`" size="sm" width="420px" @change="yamlKeuze = $event.target.value ?? yamlKeuze">
                    <select :value="yamlKeuze ?? ''" aria-label="Document om als YAML te bewerken">
                      <option v-for="d in editableDocs" :key="d.entry.path" :value="d.entry.path">{{ d.entry.name }} (geldig vanaf {{ String(d.entry.valid_from ?? '').slice(0, 4) }})</option>
                    </select>
                  </nldd-dropdown>
                  <nldd-button text="Open in YAML-editor" start-icon="code" appearance="secondary" size="sm" :disabled="!yamlKeuze ? true : undefined" @click="openYaml(yamlKeuze)"></nldd-button>
                </div>
              </details>
            </panel>

            <panel title="Budgetneutraal" subtitle="Wat kan er veranderen zonder dat het meer geld kost? Zoekt het bedrag waarbij de werkversie evenveel uitgeeft als huidig recht." summary="zoekt een bedrag">
              <budgetneutraal-panel />
            </panel>

            <!-- Loopt er een gesprek, dan staat dit paneel open. Je komt terug op
                 deze pagina om te zien wat de assistent doet; dan is dichtgeklapt
                 precies het verkeerde. -->
            <panel title="Beleidsassistent" subtitle="Een instructie of doel in gewone taal; de assistent wijzigt de werkversie en rekent door." summary="instructie of doel" :badge="assistentLoopt ? 'bezig' : ''" :open="assistentLoopt">
              <assistent-panel />
            </panel>

            <panel title="Uitvoeringslastmodel" subtitle="Handelingen × minuten × tarief per partij, investeringen en budget. Bijstellen is direct zichtbaar." :summary="uitvoeringSamenvatting" :badge="hasOverrides ? 'aangepast' : ''">
              <handelingen-panel />
            </panel>

            <panel title="Populatie" subtitle="Eén vaste steekproef van synthetische leerlingen, gewogen naar de echte instroom. Alle kolommen rekenen met dezelfde leerlingen." :summary="populatieSamenvatting" :badge="popGewijzigd ? 'aangepast' : ''">
              <population-controls />
            </panel>
          </template>
        </div>
      </nldd-page>
    </div>

    <!-- RECHTS: de twee rekeningen -->
    <div slot="pane-2" class="pane pane-right">
      <nldd-page background="tinted" accessible-label="Rekeningen">
        <div class="pane-inner">
          <div class="kop-rij">
            <nldd-title heading-level="2" size="display-3">
              <span>Twee rekeningen naast elkaar</span>
              <span slot="supporting-text">Wat de regeling kost (bedragen aan scholen) en wat de uitvoering kost (handelingen bij scholen en DUO, plus investering).</span>
            </nldd-title>
            <doorreken-knop v-if="ready" class="kop-actie" />
          </div>

          <nldd-banner v-if="fouten" variant="warning">
            {{ fouten }}
          </nldd-banner>
          <nldd-banner v-if="nietInWerking" variant="accent">
            {{ nietInWerking }}
          </nldd-banner>

          <template v-if="ready">
            <section class="block">
              <kolom-tabel :columns="columns" :metrics-by-column="metricsByColumn" :jaren="jaren" :running="running" :progress="progress" />
            </section>

            <section class="block charts">
              <uitgaven-chart
                title="Regeling per jaar"
                subtitle="bedragen aan scholen"
                :columns="columns"
                :metrics-by-column="metricsByColumn"
                :jaren="jaren"
                :posten="REGELING_POSTEN"
              />
              <uitgaven-chart
                title="Uitvoering per jaar"
                subtitle="uitvoeringslast per partij en investering"
                :columns="columns"
                :metrics-by-column="metricsByColumn"
                :jaren="jaren"
                :posten="UITVOERING_POSTEN"
              />
            </section>

            <panel title="Huidig recht in cijfers" summary="kengetallen per jaar">
              <metric-tiles :metrics="istMetrics" />
            </panel>

            <panel title="Plausibiliteit" summary="simulatie naast factsheet en realisatie">
              <plausibiliteit-tabel :metrics="istMetrics" :distributions="distributions" />
            </panel>
          </template>
        </div>
      </nldd-page>
    </div>
  </nldd-side-by-side-split-view>

  <!-- Sheets buiten de split-view: daarbinnen zou de default-slot niet gerenderd worden. -->
  <yaml-editor-sheet :open="yamlOpen" :law-path="yamlPath" @close="yamlOpen = false" />
</template>

<script setup>
import { ref, computed, watch, onMounted } from 'vue';
import { useEngine } from '../engine/useEngine.js';
import { useLawStore } from '../engine/lawStore.js';
import { useSimulation } from '../composables/useSimulation.js';
import { useHandelingen } from '../composables/useHandelingen.js';
import { number } from '../lib/format.js';
import Panel from '@regelrecht/frontend-shared/components/Panel.vue';
import ParameterPanel from '../components/beleid/ParameterPanel.vue';
import HandelingenPanel from '../components/beleid/HandelingenPanel.vue';
import BudgetneutraalPanel from '../components/beleid/BudgetneutraalPanel.vue';
import PopulationControls from '../components/beleid/PopulationControls.vue';
import { usePopulatie } from '../composables/usePopulatie.js';
import DoorrekenKnop from '../components/beleid/DoorrekenKnop.vue';
import KolomTabel from '../components/beleid/KolomTabel.vue';
import UitgavenChart from '../components/beleid/UitgavenChart.vue';
import MetricTiles from '../components/beleid/MetricTiles.vue';
import PlausibiliteitTabel from '../components/beleid/PlausibiliteitTabel.vue';
import YamlEditorSheet from '../components/beleid/YamlEditorSheet.vue';
import AssistentPanel from '../components/beleid/AssistentPanel.vue';
import { useAssistent } from '../composables/useAssistent.js';

const { ready, initError, lawIndex, initEngine } = useEngine();
const { initStore, hasChanges, changeCount, editableDocs, werkversieLabel } = useLawStore();
const { columns, metricsByColumn, istMetrics, jaren, simVersion, distributions, recompute, running, progress, n } = useSimulation();
const { fetchHandelingen, base, overrides, hasOverrides, alleHandelingen } = useHandelingen();
const { instroomRijen, instroomTotaal, hasOverrides: popGewijzigd } = usePopulatie();

const REGELING_POSTEN = [
  { key: 'po', label: 'Regeling po', pick: (m) => m.regeling_po.totaal },
  { key: 'vo', label: 'Regeling vo', pick: (m) => m.regeling_vo.totaal },
];
const UITVOERING_POSTEN = [
  { key: 'school', label: 'Uitvoeringslast school', pick: (m) => m.uitvoeringslast.school },
  { key: 'duo', label: 'Uitvoeringslast DUO', pick: (m) => m.uitvoeringslast.duo },
  { key: 'overig', label: 'Uitvoeringslast overig', pick: (m) => m.uitvoeringslast.overig },
  { key: 'inv', label: 'Investering (afschrijving)', pick: (m) => m.investering },
];

const mountError = ref(null);
/**
 * Loopt er een gesprek, of staat er een antwoord dat nog niet gezien is? Dan
 * staat het assistentpaneel open: je komt terug op deze pagina om te zien wat
 * hij doet, en dan is dichtgeklapt precies het verkeerde.
 */
const { streaming: assistentStreamt, gesprekId: assistentGesprek } = useAssistent();
const assistentLoopt = computed(() => assistentStreamt.value || !!assistentGesprek.value);

const yamlOpen = ref(false);
const yamlPath = ref(null);
const yamlKeuze = ref(null);

// Standaard het nieuwste bewerkbare document in de YAML-keuzelijst.
watch(editableDocs, (docs) => {
  if (!docs.some((d) => d.entry.path === yamlKeuze.value)) yamlKeuze.value = docs[docs.length - 1]?.entry.path ?? null;
}, { immediate: true });

// Samenvattingen op de dichtgeklapte panelen: de huidige stand in één regel.
// Dicht zegt het paneel waar het over gaat en of eraan gewerkt is; open zou het
// als eerste paneel de hele linkerkolom vullen voordat iemand iets gekozen heeft.
const bijstellenSamenvatting = computed(() => (hasChanges.value ? `${werkversieLabel.value} bewerkt` : `bedragen en drempel van ${werkversieLabel.value}`));
const populatieSamenvatting = computed(() => {
  const instroom = instroomTotaal.value
    ? `${number(instroomTotaal.value)} nieuwkomers over ${instroomRijen.value.length} instroomjaren`
    : 'instroom uit CBS en de OCW-factsheet';
  return `${instroom} · ${number(n.value)} records · in elke kolom dezelfde leerlingen`;
});
const uitvoeringSamenvatting = computed(() => {
  const b = base.value?.budget ?? {};
  const mln = (key) => Math.round(((overrides.budget?.[key] ?? b[key] ?? 0) / 1e8) * 10) / 10;
  return `${alleHandelingen.value.length} handelingen · budget po ${mln('regeling_po')} / vo ${mln('regeling_vo')} / uitvoering ${mln('uitvoering')} mln`;
});

const fouten = computed(() => {
  const m = istMetrics.value;
  if (!m?.fouten?.aantal) return null;
  return `De engine gaf ${m.fouten.aantal} fouten bij huidig recht (corpus in aanbouw?). Eerste: ${m.fouten.voorbeelden[0]}`;
});

const nietInWerking = computed(() => {
  const m = istMetrics.value;
  if (!m?.niet_in_werking) return null;
  const delen = [];
  for (const [sector, lijst] of Object.entries(m.niet_in_werking)) {
    if (lijst.length) delen.push(`${sector}: geen regeling in werking op ${lijst.length} peildata vanaf ${lijst[0]} (uitgaven daar 0)`);
  }
  return delen.length ? delen.join(' · ') : null;
});

function openYaml(path) {
  if (!path) return;
  yamlPath.value = path;
  yamlOpen.value = true;
}

onMounted(async () => {
  try {
    const engine = await initEngine();
    await initStore(engine, lawIndex.value);
    await fetchHandelingen();
    if (simVersion.value === -1) await recompute();
  } catch (e) {
    // initError toont engine-fouten; alles daarna hoort hier zichtbaar te zijn.
    console.error('Beleidsview initialisatie mislukt', e);
    mountError.value = e;
  }
});
</script>

<style scoped>
.pane { height: 100%; }
.pane-inner {
  padding: var(--primitives-space-24);
  display: flex;
  flex-direction: column;
  gap: var(--primitives-space-16);
}
.block { display: flex; flex-direction: column; gap: var(--primitives-space-12); }
.charts { display: grid; grid-template-columns: repeat(auto-fit, minmax(360px, 1fr)); gap: var(--primitives-space-16); }
.kop-rij { display: flex; justify-content: space-between; align-items: flex-start; gap: var(--primitives-space-16); flex-wrap: wrap; }
.kop-actie { flex: 0 0 auto; }
.geavanceerd summary { cursor: pointer; font-size: 0.9em; color: var(--semantics-content-secondary-color); }
.geav-row { display: flex; gap: var(--primitives-space-8); align-items: center; flex-wrap: wrap; margin-top: var(--primitives-space-8); }
</style>
