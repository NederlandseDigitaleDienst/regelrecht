<script setup lang="ts">
import ExecutionTraceView from './ExecutionTraceView.vue';
import type { ScenarioRun } from '../scenario/run';
import type { buildOutputTypeMap } from '../utils/articleMapping';

/**
 * The outcome of one scenario run: the trace with its expected outcomes. Used
 * in the scenario screen's result pane and, when that pane does not fit, in a
 * sheet. The law graph opens in a sheet of its own, which needs the full width.
 */
withDefaults(defineProps<{
  run: ScenarioRun | null;
  /** Declared output types of the executed regelwerk, so money shows as money. */
  outputTypes?: ReturnType<typeof buildOutputTypeMap> | null;
}>(), {
  outputTypes: null,
});

const emit = defineEmits<{
  rerun: [];
}>();
</script>

<template>
  <nldd-simple-section width="full">
    <ExecutionTraceView
      :result="run?.result ?? null"
      :trace-text="run?.traceText ?? null"
      :error="run?.error ?? null"
      :diagnostics="run?.diagnostics ?? []"
      :running="!!run?.pending"
      :expectations="run?.expectations ?? {}"
      :output-types="outputTypes"
      can-reload
      @reload="emit('rerun')"
    />
  </nldd-simple-section>
</template>
