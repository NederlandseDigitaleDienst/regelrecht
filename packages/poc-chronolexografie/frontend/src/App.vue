<script setup>
// Eerst de keuze van een casus (CasusKeuze.vue). Daarna één pagina met zes
// tabbladen: de scenario's, de aanvrager, het bestuursorgaan, de kroniek,
// de reductie en het logboek, en bij een casus met andere cellen (RFC-052)
// een zevende voor die cellen, de rechtbank daaronder (RFC-053). Bovenaan
// de speeldatum, die voor elke stap geldt, een knop die de kroniek terugzet
// op wat de cel vooraf vastlegde (meestal niets) en een knop terug naar de
// keuze. Een weigering van de cel
// staat in een banner, in het Nederlands; de melding van de cel zelf onder
// Details. Wat bij de casus hoort (de wet, de cel, wie beslist), komt uit
// `api.casus`.
//
// Once a scenario with a guide begins, the guide stands beside the page
// (ScenarioGuide.vue), in the inspector of the split view. Where it does not
// fit, the split view hides it and shows it as a sheet; the button "Uitleg"
// brings it back after "Verbergen".
//
// The introductory deck (presentation/) covers the page when the choice
// screen starts it or the URL carries `?presentatie`. Its closing slide, like
// the choice screen, begins one of the demo scenarios (startDemo). The button
// "Presentatie" brings it back, on the slide a scenario was last chosen on,
// or else on the first. The explanation page (presentation/UitlegPagina.vue)
// takes the place of the choice screen, from its button or `?uitleg`.
//
// The address names the case and the scenario (navigation.js), so a link
// opens a scenario and the browser's back and forward walk the choices.
import { computed, nextTick, onBeforeUnmount, onMounted, ref, watch } from 'vue';
import { api } from './api.js';
import { artikelNaam } from './logboek.js';
import { melding } from './melding.js';
import { current, followAddress, navigate, toChoice, unknown } from './navigation.js';
import { parseRoute } from './route.js';
import { beginScenario, hervatten, kiezen, state, stap, verversen } from './state.js';
import AanvragerTab from './components/AanvragerTab.vue';
import AndereCellenTab from './components/AndereCellenTab.vue';
import BestuursorgaanTab from './components/BestuursorgaanTab.vue';
import CasusKeuze from './components/CasusKeuze.vue';
import KroniekTab from './components/KroniekTab.vue';
import LogboekTab from './components/LogboekTab.vue';
import NaUitspraak from './components/NaUitspraak.vue';
import ReductieTab from './components/ReductieTab.vue';
import ScenarioGuide from './components/ScenarioGuide.vue';
import ScenarioTab from './components/ScenarioTab.vue';
import { guideFor } from './scenarioGuides.js';
import PresentationDeck from './presentation/PresentationDeck.vue';
import UitlegPagina from './presentation/UitlegPagina.vue';
import { useUitleg } from './presentation/useUitleg.js';
import { DEMO_CASUS } from './presentation/demo.js';
import { startIndex, usePresentation } from './presentation/usePresentation.js';

// Het tabblad Andere cellen staat er alleen bij een casus met andere cellen
// (RFC-052).
const tabs = computed(() => [
  { id: 'scenarios', tekst: "Scenario's" },
  { id: 'aanvrager', tekst: 'Aanvrager' },
  { id: 'bestuursorgaan', tekst: 'Bestuursorgaan' },
  ...(state.casus?.andere_cellen?.length ? [{ id: 'andere', tekst: 'Andere cellen' }] : []),
  { id: 'kroniek', tekst: 'Kroniek' },
  { id: 'reductie', tekst: 'Reductie' },
  { id: 'logboek', tekst: 'Logboek' },
]);
const tab = ref('scenarios');
// A newly chosen case opens on its scenarios, whichever way it was chosen.
// The choice screen's own `gekozen` comes too late: it is gone by then, and
// Vue drops an event from an unmounted component.
watch(
  () => state.casus?.id,
  () => (tab.value = 'scenarios'),
);
const details = ref(false);
watch(() => state.fout, () => (details.value = false));

onMounted(async () => {
  // A saved game that waits for the cell to load is not overwritten by the
  // address the page opened on; the next reload plays both.
  const resumed = await hervatten(parseRoute(window.location.pathname, import.meta.env.BASE_URL).casus);
  await followAddress(started, { first: resumed !== 'kept' });
});

const presentation = usePresentation();
const uitleg = useUitleg();
onMounted(() => {
  const i = startIndex(window.location.search, presentation.total.value);
  if (i !== null) presentation.start(i);
  // The page belongs to the choice screen; on a case's address it gives way.
  uitleg.follow({ opCasus: !!parseRoute(window.location.pathname, import.meta.env.BASE_URL).casus });
});
watch(() => state.datum, verversen);

const artikel = computed(() => {
  const [wet, nummer] = (state.casus?.besluit ?? '#').split('#');
  return artikelNaam(wet, nummer);
});
// The notice for an address that names a case or scenario this app does not
// have (navigation.js).
const base = import.meta.env.BASE_URL;
const unknownTitle = computed(() => {
  if (unknown.value?.scenario) return 'Dit scenario bestaat niet';
  if (unknown.value?.casus) return 'Deze casus bestaat niet';
  return 'Dit adres bestaat niet';
});
const unknownText = computed(() => {
  const u = unknown.value;
  if (u?.scenario) return `De casus ${u.casus} heeft geen scenario met het adres “${u.scenario}”.`;
  if (u?.casus) return `Er is geen casus met het adres “${u.casus}”.`;
  return 'Dit adres hoort bij geen casus of scenario.';
});
const title = computed(() => `${state.casus?.titel} naspelen`);
const subtitle = computed(
  () => `De cel van ${state.casus?.cel_naam} · ${artikel.value} · bestuursorgaan: ${state.casus?.rol}`,
);

// Een link in het logboek naar een chronolexogram opent het in de kroniek.
function naarKroniek(id) {
  state.inKroniek = id;
  tab.value = 'kroniek';
}

// In a scenario, starting over begins that scenario again, as the guide's
// own button does: the play date back on its first day, the tab Aanvrager and
// the guide open.
async function opnieuw() {
  if (state.scenario) await restart();
  else await stap(() => api.reset());
}

// The guide of the scenario that began, if it has one.
const guide = computed(() => guideFor(state.casus, state.scenario));
const scenarioIndex = computed(
  () => state.casus?.scenarios.findIndex((s) => s.naam === state.scenario) ?? -1,
);
const scenario = computed(() => state.casus?.scenarios[scenarioIndex.value]);
const showGuide = ref(true);
const split = ref(null);
// Whether the split view put the inspector in a sheet, for lack of room.
const asSheet = ref(false);
let observer = null;
onMounted(() => {
  observer = new MutationObserver(() => {
    asSheet.value = split.value?.hasAttribute('inspector-auto-hidden') ?? false;
  });
  observer.observe(split.value, { attributes: true, attributeFilter: ['inspector-auto-hidden'] });
  asSheet.value = split.value.hasAttribute('inspector-auto-hidden');
});
onBeforeUnmount(() => observer?.disconnect());

// The split view decides inline or sheet after its next layout, a frame
// after the pane appears.
const frame = () => new Promise((r) => requestAnimationFrame(() => r()));
async function openGuide() {
  showGuide.value = true;
  await nextTick();
  await frame();
  await frame();
  if (split.value?.inspectorAutoHidden) await split.value.showInspectorSheet();
}
function hideGuide() {
  if (asSheet.value) split.value?.hideInspectorSheet();
  else showGuide.value = false;
}
function chooseScenario() {
  hideGuide();
  tab.value = 'scenarios';
}
async function started() {
  tab.value = 'aanvrager';
  await openGuide();
}
async function restart() {
  if (await beginScenario(state.scenario)) await started();
}
// A demo scenario of the parking card, from the choice screen or the
// closing slide: choose the case if it is not open, then begin the scenario
// as its own button does.
function startDemo(naam) {
  return navigate(async (mine) => {
    if (state.casus?.id !== DEMO_CASUS && !(await kiezen(DEMO_CASUS))) return false;
    if (!current(mine) || !(await beginScenario(naam)) || !current(mine)) return false;
    await started();
  });
}
function toTab(id) {
  tab.value = id;
  // In a sheet the guide covers the page; close it so the tab shows.
  if (asSheet.value) split.value?.hideInspectorSheet();
}
</script>

<template>
  <!-- The layout of the editor (frontend/src/AppShell.vue): the bars across
       the full width in an nldd-bar-split-view, the content under them in its
       main pane, here a navigation split view with the guide as inspector. -->
  <nldd-shell>
    <nldd-bar-split-view>
      <nldd-split-view-pane slot="title-bar">
        <nldd-container padding="8">
          <nldd-toolbar accessible-label="Casus">
            <nldd-toolbar-title
              slot="start"
              max-width="640px"
              :text="state.casus ? title : 'Een uitvoeringsproces naspelen'"
              :supporting-text="state.casus ? subtitle : 'Kies een casus om te beginnen'"
            ></nldd-toolbar-title>
            <template v-if="state.casus">
              <nldd-toolbar-item v-if="guide && (!showGuide || asSheet)" slot="end" :priority="2">
                <nldd-button appearance="neutral-tinted" text="Uitleg" @click="openGuide"></nldd-button>
                <nldd-menu-item slot="overflow" text="Uitleg" @select="openGuide"></nldd-menu-item>
              </nldd-toolbar-item>
              <nldd-toolbar-item slot="end" :priority="2">
                <nldd-button appearance="neutral-tinted" text="Presentatie" @click="presentation.resume()"></nldd-button>
                <nldd-menu-item slot="overflow" text="Presentatie" @select="presentation.resume()"></nldd-menu-item>
              </nldd-toolbar-item>
              <nldd-toolbar-item slot="end" :priority="1">
                <nldd-button appearance="neutral-tinted" text="Andere casus" @click="toChoice"></nldd-button>
                <nldd-menu-item slot="overflow" text="Andere casus" @select="toChoice"></nldd-menu-item>
              </nldd-toolbar-item>
              <nldd-toolbar-item slot="end" :priority="1">
                <nldd-button appearance="neutral-tinted" text="Opnieuw beginnen" @click="opnieuw"></nldd-button>
                <nldd-menu-item slot="overflow" text="Opnieuw beginnen" @select="opnieuw"></nldd-menu-item>
              </nldd-toolbar-item>
            </template>
          </nldd-toolbar>
        </nldd-container>
      </nldd-split-view-pane>
      <nldd-split-view-pane v-if="state.casus" slot="play-bar">
        <nldd-container padding="16" padding-top="0" gap="12">
          <nldd-tab-bar accessible-label="Rol">
            <nldd-tab-bar-item
              v-for="t in tabs"
              :key="t.id"
              :text="t.tekst"
              :current="tab === t.id"
              @select="tab = t.id"
            ></nldd-tab-bar-item>
          </nldd-tab-bar>
          <nldd-form-field
            label="Speeldatum"
            label-alignment="left"
            supporting-label="De dag van elke stap; de kroniek loopt alleen vooruit."
          >
            <nldd-date-field :value="state.datum" @change="$event.detail.value && (state.datum = $event.detail.value)"></nldd-date-field>
          </nldd-form-field>
        </nldd-container>
      </nldd-split-view-pane>

      <nldd-split-view-pane slot="main">
        <nldd-navigation-split-view ref="split" inspector-accessible-label="Uitleg bij het scenario">
          <nldd-split-view-pane slot="main" has-content>
            <!-- One page per tab, as the editor gets one per route: a new tab
                 starts at its top instead of where the last one was scrolled. -->
            <nldd-page :key="uitleg.active.value && !state.casus ? 'uitleg' : tab" landmarks="page">
              <nldd-simple-section v-if="state.fout" padding-block="16">
                <nldd-banner
                  variant="critical"
                  text="Deze stap lukt niet"
                  :supporting-text="melding(state.fout)"
                  dismissible
                  @dismiss="state.fout = ''"
                >
                  <nldd-text v-if="details" size="sm" color="secondary" lang="en">{{ state.fout }}</nldd-text>
                  <nldd-button
                    slot="actions"
                    appearance="neutral-tinted"
                    size="sm"
                    expandable
                    :expanded="details"
                    text="Details"
                    @click="details = !details"
                  ></nldd-button>
                </nldd-banner>
              </nldd-simple-section>

              <nldd-simple-section v-if="unknown" padding-block="16">
                <nldd-banner
                  variant="warning"
                  :text="unknownTitle"
                  :supporting-text="unknownText"
                >
                  <nldd-button
                    slot="actions"
                    appearance="neutral-tinted"
                    size="sm"
                    :href="base"
                    text="Naar de casuskeuze"
                    @click.prevent="toChoice"
                  ></nldd-button>
                </nldd-banner>
              </nldd-simple-section>
              <UitlegPagina
                v-else-if="uitleg.active.value && !state.casus"
                @terug="uitleg.close()"
                @casus="(id) => uitleg.naarCasus(() => navigate(() => kiezen(id)))"
              />
              <CasusKeuze
                v-else-if="!state.casus"
                @presentatie="presentation.start()"
                @uitleg="uitleg.open()"
                @demo="startDemo"
              />
              <template v-else-if="state.vormen">
                <ScenarioTab v-if="tab === 'scenarios'" @start="started" />
                <AanvragerTab v-else-if="tab === 'aanvrager'" />
                <template v-else-if="tab === 'bestuursorgaan'">
                  <NaUitspraak v-if="state.casus.beroep" />
                  <BestuursorgaanTab />
                </template>
                <AndereCellenTab v-else-if="tab === 'andere'" />
                <KroniekTab v-else-if="tab === 'kroniek'" />
                <ReductieTab v-else-if="tab === 'reductie'" />
                <LogboekTab v-else @kroniek="naarKroniek" />
              </template>
            </nldd-page>
          </nldd-split-view-pane>
          <nldd-split-view-pane v-if="guide && scenario && showGuide" slot="inspector">
            <ScenarioGuide
              :guide="guide"
              :scenario="scenario"
              :number="scenarioIndex + 1"
              :tabs="tabs"
              :tab="tab"
              @tab="toTab"
              @hide="hideGuide"
              @choose="chooseScenario"
              @restart="restart"
            />
          </nldd-split-view-pane>
        </nldd-navigation-split-view>
      </nldd-split-view-pane>
    </nldd-bar-split-view>
  </nldd-shell>
  <PresentationDeck @scenario="startDemo" />
</template>
