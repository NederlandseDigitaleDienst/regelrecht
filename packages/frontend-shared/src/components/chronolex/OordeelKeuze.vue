<script setup lang="ts">
// A judgement the authority may not have given yet: yes, no or empty. On a
// reconsideration empty is "leave what it found before"; on a term the cell
// does not know for this application it is "not judged yet". The value is
// true, false or undefined. Shared by the chronolexografie poc and the
// wetsprocessysteem; `label` names the field as a UI spec may say it.
import { computed } from 'vue';
import { leesbaar } from '../../chronolex/waarden';

const props = withDefaults(
  defineProps<{
    veld: { naam: string; basis?: string };
    // true, false or undefined: no Boolean type, as Vue makes a missing Boolean false.
    modelValue?: boolean | undefined;
    /** The text of the empty choice. */
    leeg: string;
    toelichting?: string;
    label?: string;
  }>(),
  { modelValue: undefined, toelichting: '', label: '' },
);
const emit = defineEmits<{ 'update:modelValue': [waarde: boolean | undefined] }>();

const naam = computed(() => props.label || leesbaar(props.veld.naam));
const waarde = (v: boolean | undefined) => (v === true ? 'ja' : v === false ? 'nee' : '');
const kies = (e: Event) => {
  const tekst = (e.target as HTMLSelectElement | null)?.value ?? '';
  emit('update:modelValue', tekst === '' ? undefined : tekst === 'ja');
};
</script>

<template>
  <nldd-form-field :label="naam" :supporting-label="toelichting || veld.basis">
    <nldd-dropdown>
      <select :value="waarde(modelValue)" @change="kies">
        <option value="">{{ leeg }}</option>
        <option value="ja">Ja</option>
        <option value="nee">Nee</option>
      </select>
    </nldd-dropdown>
  </nldd-form-field>
</template>
