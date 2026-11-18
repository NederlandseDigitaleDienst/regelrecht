<script setup>
// The guide beside the page, short enough to read at a glance: how far the
// visitor is in the scenario, what the last step left behind, what happens
// in the case now, the instructions the scenario gives as text before it,
// and the one thing to do, with the values it takes. How far they are comes from the
// chronicles (scenarioProgress.js), so free play in between counts too; a
// step that departs from the scenario gives a warning, not a block. On a
// narrow screen the split view shows this as a sheet. Where the last step
// done expects answers of a cell (antwoorden.js), the guide shows what the
// cell answers beside what the scenario expects.
import { computed, ref, watch } from 'vue';
import { toetsAntwoorden } from '../antwoorden.js';
import { loginHint } from '../inloggen.js';
import { recordMetKenmerk, scenarioWaarden } from '../scenarioInvoer.js';
import { counts, progress } from '../scenarioProgress.js';
import { hoofdletter, leesbaar, longDate, state } from '../state.js';
import { soortVan, toonWaarde } from '../stand.js';

const props = defineProps({
  // The guide (scenarioGuides.js) and the scenario of the case YAML it follows.
  guide: { type: Object, required: true },
  scenario: { type: Object, required: true },
  // The tabs of the page, `{ id, tekst }`, to name the tab of a step, and
  // the one that is open.
  tabs: { type: Array, required: true },
  tab: { type: String, required: true },
});
const emit = defineEmits(['tab', 'hide', 'choose', 'restart']);

const p = computed(() => progress(props.guide, counts()));
const total = computed(() => props.guide.steps.length);
const current = computed(() => props.guide.steps[p.value.current] ?? null);
// What the visitor sees now: what the last step done left behind.
const seen = computed(() => (p.value.current > 0 ? props.guide.steps[p.value.current - 1].seen : null));

// The answers of the last step done, asked again whenever a chronicle
// changes; a later answer wins over one still on its way.
const vorige = computed(() => props.guide.steps[p.value.current - 1] ?? null);
const antwoorden = ref([]);
let vraag = 0;
watch(
  [
    vorige,
    () => state.datum,
    // The last record of every chronicle: a chronicle played anew to the
    // same length is another chronicle.
    () => [state.grammen, ...state.andere.map((a) => a.grammen)].map((g) => `${g.length}:${g.at(-1)?.id}`).join(),
  ],
  async ([step, datum]) => {
    const deze = ++vraag;
    const uit = step?.antwoorden?.length ? await toetsAntwoorden(step, datum, undefined, recordMetKenmerk) : [];
    if (deze === vraag) antwoorden.value = uit;
  },
  { immediate: true },
);
// `beleid#1#betaalde_leges` → `Betaalde leges: € 50,00`, with what the scenario
// expects where the cell answers otherwise. A source a view of the UI spec
// shows is written in its unit (stand.js).
const antwoordTekst = (a) => {
  const naam = hoofdletter(leesbaar(a.bron.split('#').at(-1)));
  const soort = soortVan(a.bron, state.vormen?.per_besluit);
  const kreeg = a.fout ? `geen antwoord (${a.fout})` : toonWaarde(a.kreeg, soort);
  return a.klopt ? `${naam}: ${kreeg}` : `${naam}: ${kreeg}; het scenario verwacht ${toonWaarde(a.verwacht, soort)}`;
};

const tabText = (id) => props.tabs.find((t) => t.id === id)?.tekst ?? id;
// Who to log in as, for an application of a case with a login: from the
// choices the cell offers on the play date, as the form shows them.
// Null at any other step, where the form of the application starts empty.
const login = computed(() => loginHint(state.vormen?.aanvraag?.inloggen, scenarioWaarden('aanvraag')));
// The play date only moves forward: offer the step's date when it is later.
const laterDate = computed(() => {
  const d = current.value?.date;
  return d && d > state.datum ? d : null;
});
const where = computed(() =>
  [current.value && tabText(current.value.tab), current.value?.date && longDate(current.value.date)]
    .filter(Boolean)
    .join(' · '),
);
</script>

<template>
  <nldd-page accessible-label="Uitleg bij het scenario">
    <!-- nldd-title, not nldd-top-title-bar: that bar cuts its title off on one
         line, and a scenario name is too long for the pane. -->
    <nldd-container slot="header" padding-inline="16" padding-block="8">
      <nldd-title
        size="display-5"
        heading-level="2"
        :text="scenario.naam"
        :supporting-text="p.finished ? 'Doorlopen' : `Stap ${p.current + 1} van ${total}`"
      >
        <nldd-button slot="end" appearance="neutral-tinted" size="sm" text="Verbergen" @click="emit('hide')"></nldd-button>
      </nldd-title>
    </nldd-container>

    <nldd-container padding-inline="16">
      <nldd-progress-bar
        :value="p.done.filter(Boolean).length"
        :max="total"
        size="sm"
        value-display="none"
        :accessible-label="`${p.done.filter(Boolean).length} van ${total} stappen gedaan`"
      ></nldd-progress-bar>
    </nldd-container>

    <nldd-simple-section v-if="p.warning" padding-block="16">
      <nldd-banner variant="warning" text="Wijkt af van het scenario" :supporting-text="`${hoofdletter(p.warning)}.`">
        <nldd-button
          slot="actions"
          appearance="neutral-tinted"
          size="sm"
          text="Opnieuw beginnen"
          @click="emit('restart')"
        ></nldd-button>
      </nldd-banner>
    </nldd-simple-section>

    <nldd-simple-section v-if="antwoorden.length" padding-block="8">
      <nldd-list dividers="never" accessible-label="Antwoorden van de cel na de vorige stap">
        <nldd-list-item v-for="(a, i) in antwoorden" :key="i" size="sm">
          <nldd-icon-cell
            :icon="a.klopt ? 'success' : 'error'"
            :color="a.klopt ? 'success' : 'critical'"
            size="16"
          ></nldd-icon-cell>
          <nldd-text-cell :text="antwoordTekst(a)"></nldd-text-cell>
        </nldd-list-item>
      </nldd-list>
    </nldd-simple-section>

    <nldd-simple-section v-if="p.finished">
      <nldd-container gap="16">
        <nldd-text v-for="(n, i) in guide.after" :key="i">{{ n }}</nldd-text>
        <nldd-text>{{ scenario.eind }}</nldd-text>
        <nldd-button-group size="sm">
          <nldd-button appearance="secondary" text="Opnieuw spelen" @click="emit('restart')"></nldd-button>
          <nldd-button appearance="neutral-transparent" text="Ander scenario" @click="emit('choose')"></nldd-button>
        </nldd-button-group>
      </nldd-container>
    </nldd-simple-section>

    <nldd-simple-section v-else padding-block="8">
      <nldd-container gap="8">
        <nldd-text v-if="seen" size="sm" color="secondary">{{ seen }}</nldd-text>
        <nldd-title size="display-4" heading-level="3" :text="current.title" :supporting-text="where"></nldd-title>
        <nldd-text v-if="current.story">{{ current.story }}</nldd-text>
        <nldd-text v-for="(n, i) in current.notes" :key="i">{{ n }}</nldd-text>
        <nldd-text v-if="login">{{ login }}</nldd-text>
        <nldd-text>{{ current.todo }}</nldd-text>
        <nldd-list v-if="current.values.length" dividers="never" accessible-label="Waarden uit het scenario">
          <nldd-list-item v-for="v in current.values" :key="v" size="sm">
            <nldd-icon-cell icon="circle-filled-extra-small" color="secondary" size="16"></nldd-icon-cell>
            <nldd-text-cell :text="v"></nldd-text-cell>
          </nldd-list-item>
        </nldd-list>
        <nldd-button-group size="sm" orientation="horizontal">
          <nldd-button
            v-if="laterDate"
            appearance="primary"
            start-icon="calendar-event"
            :text="laterDate.split('-').reverse().join('-')"
            :accessible-label="`Speeldatum op ${longDate(laterDate)} zetten`"
            @click="state.datum = laterDate"
          ></nldd-button>
          <nldd-button
            v-if="current.tab !== tab"
            appearance="secondary"
            :text="`Naar ${tabText(current.tab)}`"
            @click="emit('tab', current.tab)"
          ></nldd-button>
        </nldd-button-group>
      </nldd-container>
    </nldd-simple-section>
  </nldd-page>
</template>
