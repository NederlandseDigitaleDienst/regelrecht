<script setup>
// The guide beside the page, short enough to read at a glance: how far the
// visitor is in the scenario, what the last step left behind, what happens
// in the case now, and the one thing to do, with the values it takes. How far they are comes from the
// chronicles (scenarioProgress.js), so free play in between counts too; a
// step that departs from the scenario gives a warning, not a block. On a
// narrow screen the split view shows this as a sheet.
import { computed } from 'vue';
import { counts, progress } from '../scenarioProgress.js';
import { hoofdletter, longDate, state } from '../state.js';

const props = defineProps({
  // The guide (scenarioGuides.js) and the scenario of the case YAML it follows.
  guide: { type: Object, required: true },
  scenario: { type: Object, required: true },
  // Its number on the tab Scenario's.
  number: { type: Number, required: true },
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

const tabText = (id) => props.tabs.find((t) => t.id === id)?.tekst ?? id;
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
  <nldd-page sticky-header accessible-label="Uitleg bij het scenario">
    <nldd-top-title-bar
      slot="header"
      heading-level="2"
      :text="`Scenario ${number}`"
      :supporting-text="p.finished ? 'Doorlopen' : `Stap ${p.current + 1} van ${total}`"
    >
      <!-- Not dismiss-text: the split view hides that button while the guide stands beside the page. -->
      <nldd-button slot="toolbar" appearance="neutral-tinted" size="sm" text="Verbergen" @click="emit('hide')"></nldd-button>
    </nldd-top-title-bar>

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

    <nldd-simple-section v-if="p.finished">
      <nldd-container gap="16">
        <nldd-text>{{ scenario.eind }}</nldd-text>
        <nldd-button-group size="sm">
          <nldd-button appearance="secondary" text="Opnieuw spelen" @click="emit('restart')"></nldd-button>
          <nldd-button appearance="neutral-transparent" text="Ander scenario" @click="emit('choose')"></nldd-button>
        </nldd-button-group>
      </nldd-container>
    </nldd-simple-section>

    <nldd-simple-section v-else>
      <nldd-container gap="12">
        <nldd-text v-if="seen" size="sm" color="secondary">{{ seen }}</nldd-text>
        <nldd-title size="4" heading-level="3" :text="current.title" :supporting-text="where"></nldd-title>
        <nldd-text v-if="current.story">{{ current.story }}</nldd-text>
        <nldd-text>{{ current.todo }}</nldd-text>
        <nldd-list v-if="current.values.length" dividers="never" accessible-label="Waarden uit het scenario">
          <nldd-list-item v-for="v in current.values" :key="v" size="sm">
            <nldd-icon-cell icon="circle-filled-extra-small" color="secondary" size="16"></nldd-icon-cell>
            <nldd-text-cell :text="v"></nldd-text-cell>
          </nldd-list-item>
        </nldd-list>
        <nldd-button-group size="sm">
          <nldd-button
            v-if="laterDate"
            appearance="primary"
            :text="`Speeldatum ${longDate(laterDate)}`"
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
