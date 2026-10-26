<script setup>
// Een oordeel van het bestuursorgaan dat het ook nog niet kan hebben
// gegeven: ja, nee of leeg. Bij een heroverweging is leeg "laat staan wat
// het eerder vond"; bij een begrip dat de cel voor deze aanvraag niet weet,
// is leeg "nog niet beoordeeld". De waarde is true, false of undefined.
import { leesbaar } from '../state.js';

defineProps({
  veld: { type: Object, required: true },
  // true, false of undefined; geen Boolean-type, want Vue maakt van een
  // ontbrekende Boolean false.
  modelValue: { default: undefined },
  // De tekst bij de lege keuze.
  leeg: { type: String, required: true },
  toelichting: { type: String, default: '' },
});
const emit = defineEmits(['update:modelValue']);

const waarde = (v) => (v === true ? 'ja' : v === false ? 'nee' : '');
const kies = (tekst) => emit('update:modelValue', tekst === '' ? undefined : tekst === 'ja');
</script>

<template>
  <nldd-form-field :label="leesbaar(veld.naam)" :supporting-label="toelichting || veld.basis">
    <nldd-dropdown>
      <select :value="waarde(modelValue)" @change="kies($event.target.value)">
        <option value="">{{ leeg }}</option>
        <option value="ja">Ja</option>
        <option value="nee">Nee</option>
      </select>
    </nldd-dropdown>
  </nldd-form-field>
</template>
