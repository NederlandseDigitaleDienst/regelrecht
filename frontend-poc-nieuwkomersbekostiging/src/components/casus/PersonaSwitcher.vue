<template>
  <div class="switcher">
    <img class="chip" src="/img/digid-logo.svg" alt="DigiD" />
    <nldd-dropdown size="sm" width="300px">
      <select
        ref="selectEl"
        :value="selectedBsn ?? ''"
        @change="onChange"
        aria-label="Ingelogd als schoolbestuur; kies een leerling (casus)"
      >
        <option v-if="!selectedBsn" value="" disabled>Kies een casus…</option>
        <optgroup v-for="groep in groepen" :key="groep.sector" :label="groep.label">
          <option v-for="p in groep.personas" :key="p.bsn" :value="p.bsn">
            {{ p.naam }} ({{ leeftijd(p) }} jaar{{ p.school_id ? `, ${p.school_id}` : '' }})
          </option>
        </optgroup>
      </select>
    </nldd-dropdown>
  </div>
</template>

<!--
  DigiD logo and dropdown side by side; the logo is as tall as the sm dropdown
  (the design system's control size). The login context is in the select's
  aria-label.
-->

<script setup>
import { ref, computed, watchEffect, nextTick } from 'vue';
import { leeftijdOp, sectorLabel } from '../../lib/nieuwkomerFacts.js';

const props = defineProps({
  personas: { type: Array, required: true },
  selectedBsn: { type: String, default: null },
  peildatum: { type: String, default: '2025-01-01' },
});
const emit = defineEmits(['select']);

const selectEl = ref(null);

const groepen = computed(() => {
  const by = new Map();
  for (const p of props.personas) {
    const sector = p.sector === 'vo' ? 'vo' : 'po';
    if (!by.has(sector)) by.set(sector, []);
    by.get(sector).push(p);
  }
  return [...by.entries()].map(([sector, personas]) => ({ sector, label: sectorLabel(sector), personas }));
});

// De native select zit geslot in nldd-dropdown; Vue's value-binding landt
// soms vóórdat de opties er zijn. Imperatief nasynchroniseren.
watchEffect(async () => {
  const value = props.selectedBsn ?? '';
  props.personas.length; // dependency: opties opnieuw gerenderd
  await nextTick();
  if (selectEl.value && value) {
    selectEl.value.value = value;
    selectEl.value.dispatchEvent(new Event('change', { bubbles: true }));
  }
});

function leeftijd(p) {
  return leeftijdOp(p.geboortedatum, props.peildatum) ?? '?';
}

function onChange(event) {
  const persona = props.personas.find((p) => p.bsn === event.target.value);
  if (persona) emit('select', persona);
}
</script>

<style scoped>
.switcher {
  display: flex;
  align-items: stretch;
  gap: var(--primitives-space-8);
  /* Vaste breedte: anders verspringt de kop bij een langere of kortere naam. */
  flex: 0 0 auto;
}
.switcher nldd-dropdown { inline-size: 300px; flex: 0 0 300px; }
.chip {
  block-size: var(--semantics-controls-sm-min-size);
  inline-size: var(--semantics-controls-sm-min-size);
  object-fit: contain;
  flex-shrink: 0;
}
</style>
