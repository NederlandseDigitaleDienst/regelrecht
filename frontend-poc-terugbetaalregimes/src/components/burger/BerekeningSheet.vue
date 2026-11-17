<template>
  <nldd-sheet
    ref="sheetEl"
    placement="right"
    width="720px"
    accessible-label="Hoe je maandbedrag is berekend"
    @close="emit('close')"
  >
    <nldd-page>
      <nldd-container padding="24" gap="24">
        <nldd-title heading-level="2" size="display-3">
          <span slot="overline">Zo is het berekend</span>
          <span>Je maandbedrag stap voor stap</span>
        </nldd-title>

        <section class="bs-block">
          <nldd-title heading-level="3" size="display-5"><span>Verloop over de jaren</span></nldd-title>
          <p class="bs-hint">Hoeveel je per maand betaalt en hoeveel schuld je nog hebt.</p>
          <timeline-chart :timeline="timeline" />
        </section>

        <section class="bs-block">
          <nldd-title heading-level="3" size="display-5"><span>Dit komt letterlijk uit de wet</span></nldd-title>
          <p class="bs-hint">
            Deze pagina rekent met de wet zelf. Hieronder zie je per stap uit
            welk artikel je maandbedrag komt.
          </p>

          <div v-if="trace" class="bs-summary">
            <span>Uitkomst</span>
            <strong>{{ euro(trace.outputs?.te_betalen_maandbedrag) }} per maand</strong>
            <span class="bs-artikel">(artikel {{ trace.article_number }})</span>
          </div>
          <ul v-if="trace" class="bs-tree">
            <trace-node :node="trace.trace" :depth="0" />
          </ul>
        </section>
      </nldd-container>
    </nldd-page>
  </nldd-sheet>
</template>

<script setup>
import { ref, watch, nextTick } from 'vue';
import TimelineChart from './TimelineChart.vue';
import TraceNode from './TraceNode.vue';
import { euro } from '../../lib/format.js';

const props = defineProps({
  open: { type: Boolean, default: false },
  timeline: { type: Array, default: () => [] },
  trace: { type: Object, default: null },
});
const emit = defineEmits(['close']);

const sheetEl = ref(null);

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
.bs-block { display: flex; flex-direction: column; gap: var(--primitives-space-8); }
.bs-hint { margin: 0; color: var(--semantics-content-secondary-color); font-size: 0.9em; }
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
