<script setup lang="ts">
import { computed } from 'vue';
import { formatOutputValue, normalizeForCompare, matchStatus as _matchStatus } from '../../utils/outputFormat';
import type { TraceStep } from '../../lib/traceEdges';

const props = withDefaults(
  defineProps<{
    step?: TraceStep | null;
    outputs?: Record<string, unknown>;
    expectations?: Record<string, unknown>;
  }>(),
  {
    step: null,
    outputs: () => ({}),
    expectations: () => ({}),
  },
);

function matchStatus(name: string, value: unknown) {
  return _matchStatus(name, value, props.expectations);
}

/** The step's result, carrying its declared unit when the law states one.
 *  Truncation wins over the unit: a value long enough to be cut is not one a
 *  currency form helps with. */
function formatResult(step: TraceStep): string {
  const shown = truncate(step.result);
  if (!step.unit || shown.endsWith('…')) return shown;
  return formatOutputValue(step.result, step.unit);
}

// No unit here (see the note above Outputs), so formatOutputValue adds
// nothing to a number; for an unknown outcome it appends the missing facts.
function fmt(v: unknown): string {
  return formatOutputValue(v, null);
}

function truncate(v: unknown): string {
  const s = fmt(v);
  return s.length > 80 ? `${s.substring(0, 77)}…` : s;
}

const outputEntries = computed(() => Object.entries(props.outputs || {}));
const expectationEntries = computed(() => Object.entries(props.expectations || {}));
</script>

<template>
  <div class="step-detail">
    <div v-if="step" class="step-detail__card">
      <div class="step-detail__head">
        <span class="step-detail__chip" :class="`node-type-${step.nodeType}`">
          {{ step.nodeType.replace(/_/g, ' ') }}
        </span>
        <span class="step-detail__name">{{ step.name }}</span>
      </div>
      <dl class="step-detail__dl">
        <div class="step-detail__row">
          <dt>Regelwerk:</dt>
          <dd class="mono">{{ step.lawId }}</dd>
        </div>
        <div v-if="step.resolveType" class="step-detail__row">
          <dt>Resolve:</dt>
          <dd class="mono indigo">{{ step.resolveType }}</dd>
        </div>
        <!-- A null result is an absence the node produced (RFC-036), shown
             as `geen`; only a node without a result has no row. -->
        <div v-if="step.result !== undefined" class="step-detail__row">
          <dt>Resultaat:</dt>
          <!-- With the unit the law declares, so an amount reads as euros
               instead of a count of cents (RFC-039). Without one it falls
               back to the plain value, which is what an intermediate result
               has. -->
          <dd class="mono emerald">{{ formatResult(step) }}</dd>
        </div>
        <div v-if="step.durationUs !== undefined" class="step-detail__row">
          <dt>Duur:</dt>
          <dd class="mono">{{ step.durationUs }}µs</dd>
        </div>
        <div v-if="step.message" class="step-detail__row">
          <dt>Bericht:</dt>
          <dd class="italic">{{ step.message }}</dd>
        </div>
      </dl>
    </div>

    <!-- Plain values, no currency form: a trace spans every law the run
         touched, so this panel has no single law's type_spec to read a unit
         from. Rendering euros here would mean guessing per name again. -->
    <h3 class="step-detail__section-title">Outputs</h3>
    <dl class="step-detail__outputs">
      <div v-for="[k, v] in outputEntries" :key="k" class="step-detail__row">
        <dt>{{ k }}:</dt>
        <dd class="mono">{{ fmt(v) }}</dd>
      </div>
      <div v-if="outputEntries.length === 0" class="step-detail__empty">Geen outputs</div>
    </dl>

    <template v-if="expectationEntries.length > 0">
      <h3 class="step-detail__section-title">Verwachtingen</h3>
      <ul class="step-detail__expectations">
        <li v-for="[name, expected] in expectationEntries" :key="name">
          <span
            class="step-detail__marker"
            :class="{
              'pass': matchStatus(name, outputs[name]) === 'passed',
              'fail': matchStatus(name, outputs[name]) === 'failed',
            }"
          >{{ matchStatus(name, outputs[name]) === 'failed' ? '✗' : '✓' }}</span>
          <span class="mono">
            {{ name }} = {{ fmt(normalizeForCompare(expected)) }}
            <span v-if="matchStatus(name, outputs[name]) === 'failed'" class="fail">
              (kreeg {{ fmt(outputs[name]) }})
            </span>
          </span>
        </li>
      </ul>
    </template>
  </div>
</template>

<style scoped>
.step-detail {
  overflow-y: auto;
  padding: 12px;
  background: var(--semantics-surfaces-tinted-background-color);
  color: var(--semantics-content-color);
  font-size: 12px;
}

.step-detail__card {
  margin-bottom: 12px;
  border: 1px solid var(--primitives-color-donkergeel-300);
  background: var(--primitives-color-donkergeel-100);
  color: var(--primitives-color-donkergeel-900);
  border-radius: 6px;
  padding: 8px;
}

.step-detail__head {
  display: flex;
  align-items: center;
  gap: 8px;
  margin-bottom: 6px;
}
.step-detail__chip {
  font-size: 10px;
  padding: 1px 4px;
  border-radius: 3px;
  text-transform: uppercase;
  letter-spacing: 0.04em;
}
.step-detail__name {
  font-family: var(--primitives-font-family-monospace);
  font-size: 12px;
  font-weight: 600;
}

.step-detail__dl,
.step-detail__outputs {
  display: flex;
  flex-direction: column;
  gap: 2px;
  font-family: var(--primitives-font-family-monospace);
  font-size: 11px;
  margin: 0;
}

.step-detail__row {
  display: flex;
  gap: 8px;
}
.step-detail__row dt {
  width: 80px;
  color: var(--semantics-content-secondary-color);
  flex-shrink: 0;
}
.step-detail__row dd {
  margin: 0;
  color: var(--semantics-content-color);
  min-width: 0;
  word-break: break-word;
}

.mono { font-family: var(--primitives-font-family-monospace); }
.indigo { color: var(--primitives-color-paars-700); }
.emerald { color: var(--primitives-color-mintgroen-700); }
.italic { font-style: italic; color: var(--semantics-content-secondary-color); }

.step-detail__section-title {
  margin: 8px 0 4px;
  font-size: 12px;
  font-weight: 600;
  color: var(--semantics-content-color);
}

.step-detail__empty {
  color: var(--semantics-content-secondary-color);
  font-style: italic;
  font-size: 11px;
}

.step-detail__expectations {
  list-style: none;
  padding: 0;
  margin: 0;
  font-family: var(--primitives-font-family-monospace);
  font-size: 11px;
  display: flex;
  flex-direction: column;
  gap: 2px;
}
.step-detail__expectations li {
  display: flex;
  gap: 6px;
}
.step-detail__marker {
  font-weight: 700;
}
.step-detail__marker.pass,
.pass { color: var(--primitives-color-mintgroen-700); }
.step-detail__marker.fail,
.fail { color: var(--primitives-color-rood-600); }
</style>
