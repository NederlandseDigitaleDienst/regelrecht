<script setup lang="ts">
import { defineAsyncComponent } from 'vue';
import ExecutionTraceView from './ExecutionTraceView.vue';
import type { ScenarioRun } from '../scenario/run';
import type { buildOutputTypeMap } from '../utils/articleMapping';

// The graph pulls in Vue Flow; load it only when someone opens it.
const LawGraphView = defineAsyncComponent(() => import('./LawGraphView.vue'));

/** Which view of a run is shown. */
export type ResultView = 'trace' | 'graph';

/**
 * The outcome of one scenario run: the trace with its expected outcomes, or
 * the law graph with the trace laid over it. Used in the scenario screen's
 * result pane and, when that pane does not fit, in a sheet.
 */
withDefaults(defineProps<{
  run: ScenarioRun | null;
  /** The regelwerk the scenario executes, for the graph. */
  lawId: string | null;
  view: ResultView;
  /** Declared output types of the executed regelwerk, so money shows as money. */
  outputTypes?: ReturnType<typeof buildOutputTypeMap> | null;
}>(), {
  outputTypes: null,
});

const emit = defineEmits<{
  'update:view': [view: ResultView];
  rerun: [];
}>();

function onViewChange(e: Event) {
  const value = (e as CustomEvent<{ value: string }>).detail.value;
  emit('update:view', value === 'graph' ? 'graph' : 'trace');
}
</script>

<template>
  <nldd-container padding-inline="16" padding-top="8" padding-bottom="8">
    <nldd-segmented-control
      size="md"
      width="fit-content"
      accessible-label="Weergave van het resultaat"
      :value="view"
      @change="onViewChange"
    >
      <nldd-segmented-control-item value="trace" text="Resultaat"></nldd-segmented-control-item>
      <nldd-segmented-control-item value="graph" text="Graaf"></nldd-segmented-control-item>
    </nldd-segmented-control>
  </nldd-container>
  <nldd-simple-section v-if="view === 'trace'" width="full">
    <ExecutionTraceView
      :result="run?.result ?? null"
      :trace-text="run?.traceText ?? null"
      :error="run?.error ?? null"
      :running="!!run?.pending"
      :expectations="run?.expectations ?? {}"
      :output-types="outputTypes"
      can-reload
      @reload="emit('rerun')"
    />
  </nldd-simple-section>
  <LawGraphView
    v-else
    :law-id="lawId"
    :result="run?.result ?? null"
    :output-name="run?.outputName ?? null"
    :expectations="run?.expectations ?? {}"
  />
</template>
