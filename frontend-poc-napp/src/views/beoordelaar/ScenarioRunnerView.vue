<script setup>
import { computed, onMounted, ref } from 'vue';
import { useRouter } from 'vue-router';
import PortalHeader from '../../components/PortalHeader.vue';
import NBanner from '../../components/NBanner.vue';
import { getEngine } from '../../engine.js';
import { runFeature, runSteps } from '../../gherkin/runner.js';
import { euro } from '../../format.js';

const router = useRouter();

// Bundel alle scenario's mee als raw tekst.
const featureFiles = import.meta.glob('../../../../corpus-poc/napp/scenarios/*.feature', {
  query: '?raw',
  import: 'default',
  eager: true,
});

const resultaten = ref([]);
const bezig = ref(false);
const fout = ref('');
const klaar = ref(false);
const open = ref(new Set());
// Aangepaste Given-waarden per scenario: sleutel → { parameternaam: waarde }
const aanpassingen = ref({});

const totaal = computed(() =>
  resultaten.value.reduce((n, f) => n + f.scenarios.length, 0),
);
const geslaagd = computed(() =>
  resultaten.value.reduce((n, f) => n + f.scenarios.filter((s) => s.passed).length, 0),
);

function sleutel(feature, scenario) {
  return `${feature}::${scenario}`;
}

function toggle(feature, scenario) {
  const k = sleutel(feature, scenario);
  const next = new Set(open.value);
  if (next.has(k)) {
    next.delete(k);
  } else {
    next.add(k);
  }
  open.value = next;
}

function stapIcoon(status) {
  if (status === 'geslaagd') return 'check-mark-circle';
  if (status === 'mislukt') return 'dismiss-circle';
  return 'circle-dashed';
}

function stapKleur(status) {
  if (status === 'geslaagd') return 'success';
  if (status === 'mislukt') return 'critical';
  return 'secondary';
}

function veldWaarde(k, param, origineel) {
  return aanpassingen.value[k]?.[param] ?? origineel;
}

function zetWaarde(k, param, event) {
  const waarde = event.detail?.value ?? event.target?.value ?? '';
  aanpassingen.value = {
    ...aanpassingen.value,
    [k]: { ...(aanpassingen.value[k] ?? {}), [param]: waarde },
  };
}

function isAangepast(k) {
  return Object.keys(aanpassingen.value[k] ?? {}).length > 0;
}

function uitkomstLabel(key) {
  return key.replaceAll('_', ' ');
}

function uitkomstWaarde(key, value) {
  if (typeof value === 'boolean') return value ? 'ja' : 'nee';
  if (key.includes('bedrag')) return euro(value);
  return String(value);
}

/** Voer één scenario opnieuw uit, met de eventueel aangepaste gegevens. */
async function voerUit(featureNaam, scenarioNaam) {
  const feature = resultaten.value.find((f) => f.feature === featureNaam);
  const scenario = feature?.scenarios.find((s) => s.name === scenarioNaam);
  if (!scenario) return;
  const k = sleutel(featureNaam, scenarioNaam);
  const engine = await getEngine();
  const uitkomst = await runSteps(
    scenario.origSteps,
    engine,
    aanpassingen.value[k] ?? null,
  );
  scenario.passed = uitkomst.passed;
  scenario.steps = uitkomst.steps;
  scenario.outputs = uitkomst.outputs;
}

async function herstel(featureNaam, scenarioNaam) {
  const k = sleutel(featureNaam, scenarioNaam);
  const { [k]: _weg, ...rest } = aanpassingen.value;
  aanpassingen.value = rest;
  await voerUit(featureNaam, scenarioNaam);
}

async function run() {
  bezig.value = true;
  fout.value = '';
  resultaten.value = [];
  klaar.value = false;
  aanpassingen.value = {};
  try {
    const engine = await getEngine();
    const namen = Object.keys(featureFiles).sort();
    for (const naam of namen) {
      const uitkomst = await runFeature(featureFiles[naam], engine);
      for (const scenario of uitkomst.scenarios) {
        scenario.origSteps = scenario.steps;
      }
      resultaten.value.push(uitkomst);
    }
    klaar.value = true;
  } catch (e) {
    fout.value = String(e?.message ?? e);
  } finally {
    bezig.value = false;
  }
}

onMounted(run);
</script>

<template>
  <nldd-page>
    <PortalHeader
      slot="header"
      portal="beoordelaar"
      :items="[{ text: 'Werkvoorraad', to: '/' }, { text: 'Partijregister', to: '/partijregister' }, { text: 'Scenario’s', to: '/scenarios' }]"
    />

    <nldd-simple-section width="860px">
      <nldd-button
        appearance="neutral-transparent"
        size="sm"
        text="Terug naar de werkvoorraad"
        start-icon="chevron-left"
        @click="router.push('/')"
      ></nldd-button>
      <nldd-spacer size="16"></nldd-spacer>

      <nldd-title size="2">
        <span slot="overline">Beoordelingsomgeving</span>
        <h2>Scenario's</h2>
        <div slot="actions">
          <nldd-button
            appearance="secondary"
            text="Alles opnieuw uitvoeren"
            start-icon="arrow-2-counter-clockwise"
            :disabled="bezig || undefined"
            @click="run"
          ></nldd-button>
        </div>
      </nldd-title>
      <nldd-spacer size="12"></nldd-spacer>
      <nldd-rich-text>
        <p>
          Deze scenario's beschrijven hoe de Wet op de politieke partijen hoort te
          werken. Ze worden live uitgevoerd door dezelfde wet-engine die de
          besluiten neemt — in uw browser. Klik op een scenario voor de stappen;
          u kunt de gegevens aanpassen en het scenario opnieuw uitvoeren om te
          zien wat de wet dan doet.
        </p>
      </nldd-rich-text>
      <nldd-spacer size="24"></nldd-spacer>

      <NBanner v-if="fout" variant="critical" text="Uitvoeren mislukt" :supporting-text="fout" />

      <NBanner
        v-else-if="klaar"
        :variant="geslaagd === totaal ? 'success' : 'critical'"
        :text="`${geslaagd} van ${totaal} scenario's geslaagd`"
        :supporting-text="geslaagd === totaal
          ? 'De wet gedraagt zich in alle beschreven situaties zoals bedoeld.'
          : 'Niet alle scenario\'s slagen — bij aangepaste gegevens betekent dit dat de uitkomst afwijkt van het vastgelegde scenario.'"
      />

      <nldd-activity-indicator v-else-if="bezig" show-text text="Scenario's uitvoeren"></nldd-activity-indicator>

      <template v-for="feature in resultaten" :key="feature.feature">
        <nldd-spacer size="32"></nldd-spacer>
        <nldd-title size="4"><h3>{{ feature.feature }}</h3></nldd-title>
        <nldd-spacer size="12"></nldd-spacer>
        <nldd-list appearance="box-tinted">
          <template v-for="scenario in feature.scenarios" :key="scenario.name">
            <nldd-list-item
              size="md"
              button
              @click="toggle(feature.feature, scenario.name)"
            >
              <nldd-icon-cell
                :icon="scenario.passed ? 'check-mark-circle' : 'dismiss-circle'"
                :color="scenario.passed ? 'success' : 'critical'"
                size="20"
              ></nldd-icon-cell>
              <nldd-spacer-cell size="8"></nldd-spacer-cell>
              <nldd-text-cell :text="scenario.name">
                <span v-if="!scenario.passed" slot="supporting-text">
                  {{ scenario.steps.find((s) => s.status === 'mislukt')?.error }}
                </span>
              </nldd-text-cell>
              <template v-if="isAangepast(sleutel(feature.feature, scenario.name))">
                <nldd-cell width="fit-content">
                  <nldd-tag color="accent" text="Aangepast"></nldd-tag>
                </nldd-cell>
                <nldd-spacer-cell size="8"></nldd-spacer-cell>
              </template>
              <nldd-cell width="fit-content">
                <nldd-tag
                  :color="scenario.passed ? 'success' : 'critical'"
                  :text="scenario.passed ? 'Geslaagd' : 'Mislukt'"
                ></nldd-tag>
              </nldd-cell>
              <nldd-spacer-cell size="8"></nldd-spacer-cell>
              <nldd-icon-cell
                :icon="open.has(sleutel(feature.feature, scenario.name)) ? 'chevron-up' : 'chevron-down'"
                size="16"
                color="secondary"
              ></nldd-icon-cell>
            </nldd-list-item>

            <template v-if="open.has(sleutel(feature.feature, scenario.name))">
              <template v-for="(step, si) in scenario.steps" :key="si">
                <nldd-list-item size="sm">
                  <nldd-spacer-cell size="24"></nldd-spacer-cell>
                  <nldd-icon-cell
                    :icon="stapIcoon(step.status)"
                    :color="stapKleur(step.status)"
                    size="16"
                  ></nldd-icon-cell>
                  <nldd-spacer-cell size="8"></nldd-spacer-cell>
                  <nldd-text-cell size="sm" :text="`**${step.keyword}** ${step.text}`">
                    <span v-if="step.error" slot="supporting-text">{{ step.error }}</span>
                  </nldd-text-cell>
                </nldd-list-item>
                <template v-if="step.dataTable">
                  <nldd-list-item v-for="rij in step.dataTable" :key="rij[0]" size="sm">
                    <nldd-spacer-cell size="48"></nldd-spacer-cell>
                    <nldd-text-cell size="sm" :text="rij[0]" color="secondary"></nldd-text-cell>
                    <nldd-cell width="220px">
                      <nldd-text-field
                        size="sm"
                        :value="veldWaarde(sleutel(feature.feature, scenario.name), rij[0], rij[1])"
                        :accessible-label="`Waarde voor ${rij[0]}`"
                        @input="zetWaarde(sleutel(feature.feature, scenario.name), rij[0], $event)"
                      ></nldd-text-field>
                    </nldd-cell>
                    <nldd-spacer-cell size="16"></nldd-spacer-cell>
                  </nldd-list-item>
                </template>
              </template>

              <nldd-list-item size="md">
                <nldd-spacer-cell size="40"></nldd-spacer-cell>
                <nldd-cell width="full">
                  <nldd-button-group orientation="horizontal">
                    <nldd-button
                      appearance="primary"
                      size="sm"
                      text="Uitvoeren met deze gegevens"
                      start-icon="arrow-right"
                      @click="voerUit(feature.feature, scenario.name)"
                    ></nldd-button>
                    <nldd-button
                      v-if="isAangepast(sleutel(feature.feature, scenario.name))"
                      appearance="secondary"
                      size="sm"
                      text="Herstel origineel"
                      start-icon="arrow-2-counter-clockwise"
                      @click="herstel(feature.feature, scenario.name)"
                    ></nldd-button>
                  </nldd-button-group>
                </nldd-cell>
              </nldd-list-item>

              <template v-if="scenario.outputs">
                <nldd-list-item size="sm">
                  <nldd-spacer-cell size="40"></nldd-spacer-cell>
                  <nldd-text-cell size="sm" text="**Uitkomst van de wet**" color="secondary"></nldd-text-cell>
                </nldd-list-item>
                <nldd-list-item v-for="(waarde, key) in scenario.outputs" :key="key" size="sm">
                  <nldd-spacer-cell size="48"></nldd-spacer-cell>
                  <nldd-text-cell size="sm" :text="uitkomstLabel(key)" color="secondary"></nldd-text-cell>
                  <nldd-text-cell
                    size="sm"
                    :text="uitkomstWaarde(key, waarde)"
                    width="fit-content"
                    horizontal-alignment="right"
                  ></nldd-text-cell>
                  <nldd-spacer-cell size="16"></nldd-spacer-cell>
                </nldd-list-item>
              </template>
            </template>
          </template>
        </nldd-list>
      </template>
    </nldd-simple-section>
  </nldd-page>
</template>
