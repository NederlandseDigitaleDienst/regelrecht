<script setup>
// The guide beside the page: the scenario the visitor began, its steps with
// how far they are, what they see now and why, and what to do next. How far
// they are comes from the chronicles (scenarioProgress.js), so free play in
// between counts too; a step that departs from the scenario gives a warning,
// not a block. On a narrow screen the split view shows this as a sheet.
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

function supporting(step) {
  return [tabText(step.tab), step.date && longDate(step.date)].filter(Boolean).join(' · ');
}

function icon(i) {
  if (p.value.done[i]) return { name: 'check-mark-circle', color: 'success' };
  if (i === p.value.current) return { name: 'arrow-right', color: 'accent' };
  return { name: 'circle', color: 'secondary' };
}
</script>

<template>
  <nldd-page sticky-header accessible-label="Uitleg bij het scenario">
    <nldd-top-title-bar
      slot="header"
      heading-level="2"
      :text="`Scenario ${number}`"
      :supporting-text="p.finished ? 'Scenario doorlopen' : `Stap ${p.current + 1} van ${total}`"
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

    <nldd-simple-section>
      <nldd-title size="4" :text="scenario.naam" :supporting-text="guide.summary"></nldd-title>
    </nldd-simple-section>

    <nldd-simple-section v-if="p.warning" padding-block="16">
      <nldd-banner
        variant="warning"
        text="Dit wijkt af van het scenario"
        :supporting-text="`${hoofdletter(p.warning)}. Klik op Opnieuw beginnen om het scenario opnieuw te spelen.`"
      >
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
      <nldd-title slot="header" size="4" text="Scenario doorlopen"></nldd-title>
      <nldd-container gap="16">
        <nldd-text>{{ scenario.eind }}</nldd-text>
        <nldd-button-group>
          <nldd-button appearance="secondary" text="Opnieuw spelen" @click="emit('restart')"></nldd-button>
          <nldd-button appearance="neutral-transparent" text="Ander scenario" @click="emit('choose')"></nldd-button>
        </nldd-button-group>
      </nldd-container>
    </nldd-simple-section>

    <template v-else>
      <nldd-simple-section v-if="seen">
        <nldd-title slot="header" size="4" text="Wat je nu ziet"></nldd-title>
        <nldd-text>{{ seen }}</nldd-text>
      </nldd-simple-section>
      <nldd-simple-section>
        <nldd-title
          slot="header"
          size="4"
          text="Wat je nu doet"
          :supporting-text="`${current.title} · ${supporting(current)}`"
        ></nldd-title>
        <nldd-container gap="16">
          <nldd-text>{{ current.todo }}</nldd-text>
          <nldd-button-group>
            <nldd-button
              v-if="laterDate"
              appearance="primary"
              :text="`Speeldatum op ${longDate(laterDate)} zetten`"
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
    </template>

    <nldd-simple-section>
      <nldd-title slot="header" size="4" text="Stappen"></nldd-title>
      <nldd-list accessible-label="Stappen van het scenario">
        <nldd-list-item
          v-for="(s, i) in guide.steps"
          :key="i"
          size="sm"
          :current="i === p.current"
        >
          <nldd-icon-cell :icon="icon(i).name" :color="icon(i).color" size="20"></nldd-icon-cell>
          <nldd-text-cell
            :text="s.title"
            :supporting-text="supporting(s)"
            :color="p.done[i] || i === p.current ? 'content' : 'secondary'"
          ></nldd-text-cell>
          <nldd-cell v-if="i === p.current" width="fit-content">
            <nldd-tag color="accent" text="Nu"></nldd-tag>
          </nldd-cell>
        </nldd-list-item>
      </nldd-list>
    </nldd-simple-section>
  </nldd-page>
</template>
