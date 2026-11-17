<script setup lang="ts">
import type { LawScenarios } from '../scenario/useLawScenarios';
import { STATUS_TEXT } from '../scenario/statusSummary';
import type { ScenarioStatus } from '../scenario/run';

/**
 * The open regelwerk's scenarios as menu items, per file: what needs attention
 * one by one, the passed scenarios in a submenu. Choosing one opens it in its
 * tab. Rendered inside an nldd-menu, behind the status in the Machine pane's
 * toolbar and in that toolbar's overflow menu.
 */
const props = defineProps<{
  scenarios: LawScenarios;
}>();

const emit = defineEmits<{
  open: [filename: string, scenarioName: string];
}>();

const STATUS_ICON: Record<ScenarioStatus, string> = {
  passed: 'success',
  failed: 'error',
  'no-expectations': 'info',
};
</script>

<template>
  <!-- Only before anything is listed: an item that comes and goes on every
       reload changes the toolbar's overflow copy, and the toolbar rebuilds. -->
  <nldd-menu-item
    v-if="props.scenarios.loading.value && !props.scenarios.files.value.length"
    disabled
    :text="props.scenarios.loadingText.value"
  ></nldd-menu-item>
  <nldd-menu-item
    v-if="props.scenarios.listError.value"
    disabled
    icon="error"
    text="Scenario's konden niet worden geladen"
  ></nldd-menu-item>
  <nldd-menu-item
    v-if="props.scenarios.environmentError.value"
    disabled
    icon="error"
    text="Afhankelijkheden konden niet worden geladen"
  ></nldd-menu-item>
  <nldd-menu-group
    v-for="status in props.scenarios.files.value"
    :key="status.file.file.filename"
    :text="status.file.featureName.value || status.file.file.filename"
  >
    <nldd-menu-item
      v-if="status.file.dirty.value"
      disabled
      icon="warning"
      text="Rekent met niet-opgeslagen wijzigingen"
    ></nldd-menu-item>
    <nldd-menu-item
      v-if="status.mismatch"
      disabled
      icon="info"
      text="Rekent met een ander regelwerk"
    ></nldd-menu-item>
    <nldd-menu-item
      v-if="status.file.error.value"
      disabled
      icon="error"
      text="Dit bestand kon niet worden geladen"
    ></nldd-menu-item>
    <nldd-menu-item
      v-for="line in status.attention"
      :key="line.index"
      :icon="line.status ? STATUS_ICON[line.status] : undefined"
      :text="line.name"
      :details="line.status ? STATUS_TEXT[line.status] : undefined"
      :data-testid="`scenario-menu-${status.file.file.filename}-${line.index}`"
      @select="emit('open', status.file.file.filename, line.name)"
    ></nldd-menu-item>
    <nldd-menu-item
      v-if="status.passed.length"
      icon="success"
      :text="`${status.passed.length} geslaagd`"
      :data-testid="`scenario-menu-${status.file.file.filename}-passed`"
    >
      <!-- A fixed width: sized to its names, the submenu runs off the
           screen when it has to open to the left (design-system 0.22). -->
      <nldd-menu width="400px">
        <nldd-menu-item
          v-for="line in status.passed"
          :key="line.index"
          :text="line.name"
          :details="STATUS_TEXT.passed"
          @select="emit('open', status.file.file.filename, line.name)"
        ></nldd-menu-item>
      </nldd-menu>
    </nldd-menu-item>
  </nldd-menu-group>
  <nldd-menu-item
    v-if="!props.scenarios.loading.value && !props.scenarios.listError.value && !props.scenarios.files.value.length"
    disabled
    text="Geen scenario's voor dit regelwerk"
  ></nldd-menu-item>
</template>
