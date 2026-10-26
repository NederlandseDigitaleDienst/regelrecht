<script setup>
// Eén veld zoals de wet het vraagt: het type bepaalt het invoerelement, de
// basis staat eronder. Een leeg tekst- of datumveld is `undefined`: dan
// laat de aanvraag het weg, en de cel noemt het in `missing` (Awb 4:5).
import { leesbaar } from '../state.js';

const props = defineProps({
  veld: { type: Object, required: true },
  modelValue: { type: [String, Number, Boolean], default: undefined },
});
const emit = defineEmits(['update:modelValue']);

const zet = (waarde) => emit('update:modelValue', waarde === '' ? undefined : waarde);
const getal = (tekst) => {
  const n = Number(String(tekst).replace(',', '.'));
  return tekst === '' || Number.isNaN(n) ? undefined : n;
};
</script>

<template>
  <nldd-switch-field
    v-if="veld.type === 'boolean'"
    :label="leesbaar(veld.naam)"
    :checked="modelValue === true"
    @change="zet($event.detail.checked)"
  ></nldd-switch-field>
  <nldd-form-field
    v-else
    :label="leesbaar(veld.naam)"
    :supporting-label="veld.basis"
    :optional="veld.mag_leeg"
  >
    <nldd-date-field
      v-if="veld.type === 'date'"
      :value="modelValue ?? ''"
      @change="zet($event.detail.value)"
    ></nldd-date-field>
    <nldd-text-field
      v-else-if="veld.type === 'number'"
      keyboard="decimal"
      :value="modelValue ?? ''"
      @input="zet(getal($event.target.value))"
    ></nldd-text-field>
    <nldd-text-field
      v-else
      :value="modelValue ?? ''"
      @input="zet($event.target.value)"
    ></nldd-text-field>
  </nldd-form-field>
</template>
