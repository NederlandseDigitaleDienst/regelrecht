<template>
  <nldd-sheet
    ref="sheetEl"
    placement="right"
    width="760px"
    accessible-label="Hoe de bekostiging is berekend"
    @close="emit('close')"
  >
    <nldd-page>
      <nldd-container padding="24" gap="24">
        <nldd-title heading-level="2" size="display-3">
          <span slot="overline">Dit komt letterlijk uit de regeling</span>
          <span>{{ titel }}</span>
        </nldd-title>

        <p class="bs-hint">
          De rekenmachine voert de regeling uit als code. Hieronder staat per
          artikel welke stap tot de uitkomst op deze peildatum leidt.
        </p>

        <div class="bs-controls">
          <nldd-form-field label="Peildatum">
            <nldd-dropdown :key="`k1:${peildatum}`" size="sm" @change="emit('update:peildatum', $event.target.value)">
              <select :value="peildatum">
                <option v-for="pd in peildata" :key="pd" :value="pd">{{ datumLabel(pd) }}</option>
              </select>
            </nldd-dropdown>
          </nldd-form-field>
          <nldd-form-field label="Uitkomst">
            <nldd-dropdown :key="`k2:${output}`" size="sm" @change="emit('update:output', $event.target.value)">
              <select :value="output">
                <option v-for="o in outputs" :key="o" :value="o">{{ o }}</option>
              </select>
            </nldd-dropdown>
          </nldd-form-field>
        </div>

        <nldd-banner v-if="error" variant="critical">{{ error }}</nldd-banner>

        <template v-if="trace">
          <div class="bs-summary">
            <span>Uitkomst</span>
            <strong>{{ formatOutput(trace.outputs?.[output]) }}</strong>
            <span v-if="trace.article_number" class="bs-artikel">(artikel {{ trace.article_number }})</span>
          </div>
          <ul v-if="traceWortel" class="bs-tree">
            <trace-node :node="traceWortel" :depth="0" />
          </ul>
        </template>
      </nldd-container>
    </nldd-page>
  </nldd-sheet>
</template>

<script setup>
import { ref, computed, watch, nextTick } from 'vue';
import TraceNode from './TraceNode.vue';
import { euro } from '../../lib/format.js';
import { datumLabel } from '../../lib/nieuwkomerFacts.js';

const props = defineProps({
  open: { type: Boolean, default: false },
  titel: { type: String, default: 'Bekostiging stap voor stap' },
  trace: { type: Object, default: null },
  error: { type: String, default: null },
  peildata: { type: Array, default: () => [] },
  peildatum: { type: String, default: null },
  outputs: { type: Array, default: () => [] },
  output: { type: String, default: 'bedrag_kwartaal' },
});
const emit = defineEmits(['close', 'update:peildatum', 'update:output']);

/**
 * De wortel van de tracestructuur.
 *
 * De engine geeft sinds RFC-039 een tracedocument terug (`{ trace_version,
 * root }`) in plaats van de boom zelf. Dit stond op `trace.trace`, dus het gaf
 * het document door alsof het een knoop was: dat rendert als één lege regel
 * zonder naam of type, en de hele herleiding was daarmee stil verdwenen.
 * De oude vorm blijft werken, zodat een engine van voor die wijziging het niet
 * breekt.
 */
const traceWortel = computed(() => {
  const t = props.trace?.trace;
  if (!t) return null;
  return t.root ?? (t.node_type ? t : null);
});

const sheetEl = ref(null);

function formatOutput(v) {
  if (v === null || v === undefined) return '—';
  if (typeof v === 'boolean') return v ? 'waar' : 'onwaar';
  if (typeof v === 'number' && /bedrag|toeslag|kosten/.test(props.output)) return euro(v);
  return String(v);
}

watch(
  () => props.open,
  async (open) => {
    if (!open) {
      sheetEl.value?.hide();
      return;
    }
    await nextTick();
    sheetEl.value?.show();
  },
  { immediate: true },
);
</script>

<style scoped>
.bs-hint { margin: 0; color: var(--semantics-content-secondary-color); font-size: 0.9em; }
.bs-controls { display: flex; gap: var(--primitives-space-12); flex-wrap: wrap; }
.bs-summary {
  display: flex;
  align-items: baseline;
  gap: var(--primitives-space-8);
  padding: var(--primitives-space-12);
  border-radius: var(--semantics-surfaces-corner-radius);
  background: var(--primitives-color-hemelblauw-50);
}
.bs-summary strong { font-size: 1.1em; }
.bs-artikel { color: var(--semantics-content-secondary-color); font-size: 0.9em; }
.bs-tree { margin: 0; padding: 0; }
</style>
