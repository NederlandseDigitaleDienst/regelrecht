<template>
  <div class="switcher">
    <img class="chip" src="/img/digid-logo.svg" alt="DigiD" />
    <nldd-dropdown size="sm" width="230px">
      <select
        ref="selectEl"
        :value="selectedBsn ?? ''"
        @change="onChange"
        aria-label="Ingelogd met DigiD als voorbeeldpersoon; kies een persoon"
      >
        <option v-if="!selectedBsn" value="" disabled>Kies een persoon…</option>
        <option v-for="p in personas" :key="p.bsn" :value="p.bsn">
          {{ p.naam }} ({{ leeftijd(p) }} jaar)
        </option>
      </select>
    </nldd-dropdown>
  </div>
</template>

<!--
  Alleen het DigiD-logo + de dropdown naast elkaar, zonder label; het logo is
  even hoog als de dropdown via align-items: stretch. De inlogcontext staat in
  het aria-label van de select (toegankelijk, niet zichtbaar).
-->

<script setup>
import { ref, watchEffect, nextTick } from 'vue';

const props = defineProps({
  personas: { type: Array, required: true },
  selectedBsn: { type: String, default: null },
  startJaar: { type: Number, default: 2026 },
});
const emit = defineEmits(['select']);

const selectEl = ref(null);

// De native select zit geslot in nldd-dropdown; Vue's value-binding landt
// soms vóórdat de opties er zijn. Imperatief nasynchroniseren.
watchEffect(async () => {
  const value = props.selectedBsn ?? '';
  props.personas.length; // dependency: opties opnieuw gerenderd
  await nextTick();
  if (selectEl.value && value) {
    selectEl.value.value = value;
    // Altijd een event: de nldd-dropdown-wrapper leest zijn weergavetekst
    // alleen bij events, niet bij programmatische toewijzing. onChange emit
    // hetzelfde persona en is idempotent.
    selectEl.value.dispatchEvent(new Event('change', { bubbles: true }));
  }
});

function leeftijd(p) {
  return props.startJaar - p.geboortejaar;
}

function onChange(event) {
  const persona = props.personas.find((p) => p.bsn === event.target.value);
  if (persona) emit('select', persona);
}
</script>

<style scoped>
.switcher {
  display: flex;
  /* stretch: het logo wordt even hoog als de dropdown. */
  align-items: stretch;
  gap: var(--primitives-space-8);
}
.chip {
  /* DigiD logo as a login hint for the demo (example persons, no real
     login). File: public/img/digid-logo.svg. As tall as the sm dropdown next
     to it, through the design system's control size (stretch does not work
     here: the logo's intrinsic size would set the row height itself). */
  block-size: var(--semantics-controls-sm-min-size);
  inline-size: var(--semantics-controls-sm-min-size);
  object-fit: contain;
  flex-shrink: 0;
}
</style>
