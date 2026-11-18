<script setup lang="ts">
import { computed } from 'vue';
import { formatValue, formatOutputValueParts, formatMissing, normalizeForCompare, matchStatus as _matchStatus, humanize } from '../utils/outputFormat';
import type { OutputMeta } from '../utils/articleMapping';

/** The execution result; only the outputs are read here. */
interface ExecutionResult {
  outputs?: Record<string, unknown> | null;
}

const props = withDefaults(
  defineProps<{
    /** Execution result with outputs */
    result?: ExecutionResult | null;
    /** Pre-rendered box-drawing trace text */
    traceText?: string | null;
    /** Expected output values: { outputName: expectedValue } */
    expectations?: Record<string, unknown>;
    /** Error message if execution failed */
    error?: string | null;
    /** Scenario is currently executing */
    running?: boolean;
    /** Whether a re-run action is available */
    canReload?: boolean;
    /** Declared output types from buildOutputTypeMap(): name -> { type, unit } */
    outputTypes?: Map<string, OutputMeta> | null;
  }>(),
  {
    result: null,
    traceText: null,
    expectations: () => ({}),
    error: null,
    running: false,
    canReload: false,
    outputTypes: null,
  },
);

const emit = defineEmits<{ reload: [] }>();

function matchStatus(outputName: string, actualValue: unknown) {
  return _matchStatus(outputName, actualValue, props.expectations);
}

function outputParts(name: string) {
  return formatOutputValueParts(
    props.result?.outputs?.[name],
    props.outputTypes?.get(name)?.unit ?? null,
  );
}

// A collection or record output renders as JSON; de-snaking its keys would
// change the text, so humanize only a scalar.
function humanizeOutput(text: string): string {
  return /^[[{]/.test(text) ? text : humanize(text);
}

const hasContent = computed(() =>
  props.result || props.traceText || props.error,
);

const overallStatus = computed(() => {
  if (!props.result) return null;
  const keys = Object.keys(props.expectations);
  if (keys.length === 0) return null;
  for (const name of keys) {
    if (matchStatus(name, props.result.outputs?.[name]) === 'failed') return 'failed';
  }
  return 'passed';
});
</script>

<template>
  <nldd-inline-dialog v-if="running" text="Bezig met uitvoeren…"></nldd-inline-dialog>

  <template v-else-if="error && !result && !traceText">
    <nldd-inline-dialog variant="alert" text="Fout bij uitvoering" :supporting-text="error"></nldd-inline-dialog>
    <template v-if="canReload">
      <nldd-spacer size="12"></nldd-spacer>
      <nldd-button size="md" text="Opnieuw uitvoeren" @click="emit('reload')"></nldd-button>
    </template>
  </template>

  <template v-else-if="!hasContent">
    <nldd-inline-dialog text="Nog geen resultaat voor dit scenario."></nldd-inline-dialog>
    <template v-if="canReload">
      <nldd-spacer size="12"></nldd-spacer>
      <nldd-button size="md" text="Opnieuw uitvoeren" @click="emit('reload')"></nldd-button>
    </template>
  </template>

  <template v-else>
    <template v-if="result && Object.keys(expectations).length">
      <nldd-list appearance="simple">
        <nldd-list-item size="md">
          <nldd-text-cell size="md" color="secondary" text=""></nldd-text-cell>
          <nldd-text-cell
            size="md"
            color="secondary"
            horizontal-alignment="right"
            width="100px"
            text="Verwacht"
          ></nldd-text-cell>
          <nldd-text-cell
            size="md"
            color="secondary"
            horizontal-alignment="right"
            width="100px"
            text="Uitkomst"
          ></nldd-text-cell>
          <nldd-spacer-cell size="8"></nldd-spacer-cell>
          <nldd-text-cell size="md" color="secondary" horizontal-alignment="right" width="80px" text="Status"></nldd-text-cell>
        </nldd-list-item>
        <nldd-list-item v-for="name in Object.keys(expectations)" :key="name" size="md">
          <nldd-text-cell size="md" :text="humanize(name)"></nldd-text-cell>
          <nldd-text-cell
            size="md"
            horizontal-alignment="right"
            width="100px"
            :text="humanize(formatValue(normalizeForCompare(expectations[name])))"
            :supporting-text="formatMissing(expectations[name]) || undefined"
          ></nldd-text-cell>
          <nldd-text-cell
            size="md"
            horizontal-alignment="right"
            width="100px"
            :text="humanizeOutput(outputParts(name).text)"
            :supporting-text="outputParts(name).supportingText"
          ></nldd-text-cell>
          <nldd-spacer-cell size="8"></nldd-spacer-cell>
          <nldd-text-cell
            size="md"
            horizontal-alignment="right"
            width="80px"
            :text="matchStatus(name, result.outputs?.[name]) === 'passed'
              ? 'Geslaagd'
              : matchStatus(name, result.outputs?.[name]) === 'failed'
                ? 'Mislukt'
                : '—'"
          ></nldd-text-cell>
        </nldd-list-item>
      </nldd-list>
      <nldd-spacer size="16"></nldd-spacer>
    </template>

    <!-- `wrap` because the code-viewer only ever owns a *horizontal* scroller
         and grows to its full content height. Inside the trace sheet that puts
         its horizontal scrollbar thousands of pixels below the fold, so long
         lines (a DEFINITION with a dozen enum values) read as truncated with no
         hint that there is more to the right. Wrapping keeps every value
         visible; the sheet's own scroller handles the vertical axis. -->
    <template v-if="result && traceText">
      <nldd-title size="display-5" heading-level="4" class="etv-section-title"><span>Execution trace</span></nldd-title>
      <nldd-spacer size="8"></nldd-spacer>
      <nldd-code-viewer wrap>{{ traceText }}</nldd-code-viewer>
    </template>

    <template v-if="error && traceText && !result">
      <nldd-title size="display-5" heading-level="4" class="etv-section-title"><span>Partial trace (tot fout)</span></nldd-title>
      <nldd-spacer size="8"></nldd-spacer>
      <nldd-code-viewer wrap>{{ traceText }}</nldd-code-viewer>
      <template v-if="canReload">
        <nldd-spacer size="12"></nldd-spacer>
        <nldd-button size="md" text="Opnieuw uitvoeren" @click="emit('reload')"></nldd-button>
      </template>
    </template>
  </template>
</template>
