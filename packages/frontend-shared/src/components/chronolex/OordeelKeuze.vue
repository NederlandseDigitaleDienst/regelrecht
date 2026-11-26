<script setup lang="ts">
// A judgement the authority may not have given yet: yes, no or empty. On a
// reconsideration empty is "leave what it found before"; on a term the cell
// does not know for this application it is "not judged yet". The value is
// true, false or undefined. Shared by the chronolexografie poc and the
// RegelHet; `label` names the field as a UI spec may say it, and `ja` and
// `nee` may name the answers where a bare yes or no says too little. Three
// choices stand under each other as radio buttons, not in a dropdown.
import { computed, useId } from 'vue';
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
    /** The text of the choice yes. */
    ja?: string;
    /** The text of the choice no. */
    nee?: string;
    /** Name the article under the label where there is no other explanation. */
    artikel?: boolean;
    /** What the answer still lacks, once someone tried to go on without it. */
    fout?: string;
  }>(),
  { modelValue: undefined, toelichting: '', label: '', ja: 'Ja', nee: 'Nee', artikel: true, fout: '' },
);
const emit = defineEmits<{ 'update:modelValue': [waarde: boolean | undefined] }>();

const id = useId();
const ontbreekt = `${id}-ontbreekt`;
const naam = computed(() => props.label || leesbaar(props.veld.naam));
const waarde = computed(() => (props.modelValue === true ? 'ja' : props.modelValue === false ? 'nee' : 'leeg'));
const kies = (e: Event) => {
  const detail = (e as CustomEvent<{ checked: boolean; value: string } | null>).detail;
  if (detail?.checked) emit('update:modelValue', detail.value === 'leeg' ? undefined : detail.value === 'ja');
};
</script>

<template>
  <nldd-form-field :label="naam" :supporting-label="toelichting || (artikel ? veld.basis : '')">
    <nldd-radio-button-group :name="id" :invalid="!!fout || undefined" :unmet="fout ? ontbreekt : undefined" @change="kies">
      <nldd-radio-button-field value="leeg" :label="leeg" :checked="waarde === 'leeg' || undefined"></nldd-radio-button-field>
      <nldd-radio-button-field value="ja" :label="ja" :checked="waarde === 'ja' || undefined"></nldd-radio-button-field>
      <nldd-radio-button-field value="nee" :label="nee" :checked="waarde === 'nee' || undefined"></nldd-radio-button-field>
    </nldd-radio-button-group>
    <nldd-validation-list v-if="fout">
      <nldd-validation-item :id="ontbreekt">{{ fout }}</nldd-validation-item>
    </nldd-validation-list>
  </nldd-form-field>
</template>
