<script setup lang="ts">
import { ref } from 'vue';
import type { LawScenarios } from '../scenario/useLawScenarios';
import { STATUS_BADGE, STATUS_TEXT } from '../scenario/statusSummary';
import { scenarioFileKey } from '../scenario/openScenario';

/**
 * The open regelwerk's scenarios next to its articles, read only: what needs
 * attention one by one, the passed scenarios as one row that opens up.
 * Changing a scenario happens in its own tab; a click opens it there.
 *
 * The runs come from the editor (`useLawScenarios`), which the status in the
 * Machine pane's toolbar reads too.
 */
const props = defineProps<{
  scenarios: LawScenarios;
}>();

const emit = defineEmits<{
  /** Open a file in its tab, at a scenario. */
  open: [filename: string, scenarioName: string | null];
}>();

// The files whose passed scenarios are shown one by one, by traject,
// regelwerk and file: another regelwerk's file of the same name starts closed.
const expanded = ref(new Set<string>());
function toggle(key: string) {
  const next = new Set(expanded.value);
  if (!next.delete(key)) next.add(key);
  expanded.value = next;
}
</script>

<template>
  <nldd-simple-section data-testid="scenario-status-pane">
    <nldd-inline-dialog
      v-if="props.scenarios.listError.value"
      variant="alert"
      text="Scenario's konden niet worden geladen"
      :supporting-text="props.scenarios.listError.value.message"
    ></nldd-inline-dialog>
    <nldd-inline-dialog
      v-if="props.scenarios.environmentError.value"
      variant="alert"
      text="Afhankelijkheden konden niet worden geladen"
      :supporting-text="String(props.scenarios.environmentError.value)"
    ></nldd-inline-dialog>

    <template v-for="status in props.scenarios.files.value" :key="status.file.file.filename">
      <nldd-title size="display-5">
        <h2>{{ status.file.featureName.value || status.file.file.filename }}</h2>
      </nldd-title>
      <nldd-text v-if="status.file.featureName.value" size="sm" color="secondary">{{ status.file.file.filename }}</nldd-text>
      <nldd-spacer size="8"></nldd-spacer>
      <template v-if="status.file.dirty.value">
        <nldd-banner
          size="sm"
          variant="warning"
          :text="`Niet-opgeslagen wijzigingen in het tabblad ${status.file.file.filename}`"
          supporting-text="De status hieronder rekent met die wijzigingen."
        ></nldd-banner>
        <nldd-spacer size="8"></nldd-spacer>
      </template>
      <template v-if="status.mismatch">
        <nldd-banner
          size="sm"
          variant="neutral"
          text="Dit bestand rekent met een ander regelwerk"
        ></nldd-banner>
        <nldd-spacer size="8"></nldd-spacer>
      </template>
      <nldd-inline-dialog
        v-if="status.file.error.value"
        variant="alert"
        text="Dit bestand kon niet worden geladen"
        :supporting-text="status.file.error.value.message"
      ></nldd-inline-dialog>
      <nldd-list v-else appearance="box-tinted" :accessible-label="`Scenario's in ${status.file.file.filename}`">
        <nldd-list-item
          v-for="line in status.attention"
          :key="line.index"
          size="md"
          button
          :data-testid="`scenario-status-${status.file.file.filename}-${line.index}`"
          @click="emit('open', status.file.file.filename, line.name)"
        >
          <nldd-text-cell :text="line.name"></nldd-text-cell>
          <nldd-spacer-cell v-if="line.status" size="8"></nldd-spacer-cell>
          <nldd-cell v-if="line.status" width="fit-content">
            <nldd-badge
              size="sm"
              :color="STATUS_BADGE[line.status].color"
              :icon="STATUS_BADGE[line.status].icon"
              :accessible-label="STATUS_TEXT[line.status]"
            ></nldd-badge>
          </nldd-cell>
          <nldd-icon-cell size="20"><nldd-icon icon="chevron-right"></nldd-icon></nldd-icon-cell>
        </nldd-list-item>
        <!-- The passed scenarios: one row, opened up on a click. -->
        <nldd-list-item
          v-if="status.passed.length"
          size="md"
          button
          expandable
          :expanded="expanded.has(scenarioFileKey(status.file.file)) || undefined"
          :data-testid="`scenario-status-${status.file.file.filename}-passed`"
          @click="toggle(scenarioFileKey(status.file.file))"
        >
          <nldd-icon-cell size="20"><nldd-icon icon="success"></nldd-icon></nldd-icon-cell>
          <nldd-spacer-cell size="8"></nldd-spacer-cell>
          <nldd-text-cell :text="`${status.passed.length} geslaagd`"></nldd-text-cell>
          <nldd-icon-cell size="20" icon="chevron-right" disclosure></nldd-icon-cell>
        </nldd-list-item>
        <template v-if="expanded.has(scenarioFileKey(status.file.file))">
          <nldd-list-item
            v-for="line in status.passed"
            :key="line.index"
            size="md"
            button
            :data-testid="`scenario-status-${status.file.file.filename}-${line.index}`"
            @click="emit('open', status.file.file.filename, line.name)"
          >
            <nldd-text-cell :text="line.name"></nldd-text-cell>
            <nldd-spacer-cell size="8"></nldd-spacer-cell>
            <nldd-cell width="fit-content">
              <nldd-badge
                size="sm"
                :color="STATUS_BADGE.passed.color"
                :icon="STATUS_BADGE.passed.icon"
                :accessible-label="STATUS_TEXT.passed"
              ></nldd-badge>
            </nldd-cell>
            <nldd-icon-cell size="20"><nldd-icon icon="chevron-right"></nldd-icon></nldd-icon-cell>
          </nldd-list-item>
        </template>
      </nldd-list>
      <nldd-spacer size="24"></nldd-spacer>
    </template>

    <nldd-activity-indicator
      v-if="props.scenarios.loading.value"
      show-text
      :text="props.scenarios.loadingText.value"
    ></nldd-activity-indicator>
    <nldd-inline-dialog
      v-else-if="!props.scenarios.listError.value && !props.scenarios.files.value.length"
      text="Geen scenario's voor dit regelwerk."
    ></nldd-inline-dialog>
  </nldd-simple-section>
</template>
