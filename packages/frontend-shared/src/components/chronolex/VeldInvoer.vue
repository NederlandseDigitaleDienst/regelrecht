<script setup lang="ts">
// One field as the law asks it: its type picks the input, and the article it
// rests on stands in the line under it ("Volgens Awb artikel 4:2."), where a
// reader meets it as a sentence rather than as a second label. An amount (`amount`) is a number in the unit the law counts it in,
// such as eurocent. An empty text or date field is `undefined`: the submission
// then leaves it out, and the cell names it in `missing` (Awb 4:5).
// `herkomst` is the line under a prefilled field: which cell it comes from, or
// that the applicant changed it (voorinvulling.ts). A checkbox (yes or no)
// carries its own label; with a line under it, it stands in an
// nldd-form-field without a label of its own, which holds that line. A table
// (an array with `kolommen`, schema v0.8.20) is a TabelInvoer. A field the
// system fills (`alleenLezen`) shows its value, read-only.
// `label` names the field on the screen, as a UI spec may say it; without it
// the field goes by its name in the regelwerk. `help` is a line under it.
// A field the screen requires (`verplicht`) loses its "Optioneel" badge where
// the law lets it be empty, and the form refuses to submit it empty; the
// validation list under it then says so, as the browser judges it.
// Shared by the chronolexografie poc and the wetsprocessysteem.
import { computed } from 'vue';
import { leesbaar } from '../../chronolex/waarden';
import type { Kolom } from '../../chronolex/tabel';
import TabelInvoer from './TabelInvoer.vue';

/** A field of a form, as the kern derives it from the law. */
export interface InvoerVeld {
  naam: string;
  type: string | null;
  mag_leeg?: boolean;
  basis?: string;
  kolommen?: Kolom[];
}

const props = withDefaults(
  defineProps<{
    veld: InvoerVeld;
    modelValue?: unknown;
    herkomst?: string;
    alleenLezen?: boolean;
    label?: string;
    help?: string;
    /** Draw the control bare: a group around it carries the line under it. */
    kaal?: boolean;
    /** The browser's autofill hint, for an input that answers part of a field. */
    autocomplete?: string;
    /** The on-screen keyboard a phone raises for it. */
    keyboard?: string;
    /** How wide the input is, as a CSS length; without it, as wide as the form. */
    width?: string;
    /** Name the article the field rests on under it. Off where a screen of its own holds them. */
    artikel?: boolean;
    /** The form does not submit it empty, whatever the law lets be empty. */
    verplicht?: boolean;
  }>(),
  {
    modelValue: undefined,
    herkomst: '',
    alleenLezen: false,
    label: '',
    help: '',
    kaal: false,
    autocomplete: '',
    keyboard: '',
    width: '',
    artikel: true,
    verplicht: false,
  },
);
const emit = defineEmits<{ 'update:modelValue': [waarde: unknown] }>();

const naam = computed(() => props.label || leesbaar(props.veld.naam));
/** The article the field rests on, as a sentence. */
const volgens = computed(() => (props.artikel && props.veld.basis ? `Volgens ${props.veld.basis}.` : ''));
const onder = computed(() => (props.kaal ? '' : [props.help, volgens.value, props.herkomst].filter(Boolean).join(' ')));
const tekst = computed(() => (props.modelValue === undefined || props.modelValue === null ? '' : String(props.modelValue)));

const zet = (waarde: unknown) => emit('update:modelValue', waarde === '' ? undefined : waarde);
const getal = (invoer: string): number | undefined => {
  const n = Number(String(invoer).replace(',', '.'));
  return invoer === '' || Number.isNaN(n) ? undefined : n;
};
/** The value an nldd input event carries: on `detail`, else on the native target. */
const uit = (e: Event): string => {
  const detail = (e as CustomEvent<{ value?: unknown; checked?: unknown }>).detail;
  if (detail && typeof detail === 'object' && 'value' in detail) return String(detail.value ?? '');
  return (e.target as HTMLInputElement | null)?.value ?? '';
};
const aan = (e: Event): boolean => Boolean((e as CustomEvent<{ checked?: unknown }>).detail?.checked);
</script>

<template>
  <TabelInvoer
    v-if="veld.type === 'array' && veld.kolommen"
    :veld="{ ...veld, kolommen: veld.kolommen }"
    :model-value="Array.isArray(modelValue) ? modelValue : undefined"
    :herkomst="herkomst"
    :alleen-lezen="alleenLezen"
    :label="label"
    @update:model-value="emit('update:modelValue', $event)"
  />
  <nldd-form-field v-else-if="veld.type === 'boolean' && onder">
    <nldd-checkbox-field
      :label="naam"
      :checked="modelValue === true"
      :disabled="alleenLezen || undefined"
      @change="zet(aan($event))"
    ></nldd-checkbox-field>
    <nldd-form-field-help-text>{{ onder }}</nldd-form-field-help-text>
  </nldd-form-field>
  <nldd-checkbox-field
    v-else-if="veld.type === 'boolean'"
    :label="naam"
    :checked="modelValue === true"
    :disabled="alleenLezen || undefined"
    @change="zet(aan($event))"
  ></nldd-checkbox-field>
  <nldd-form-field v-else :label="naam" :optional="(veld.mag_leeg && !verplicht) || undefined">
    <nldd-date-field
      v-if="veld.type === 'date'"
      :value="tekst"
      :required="verplicht || undefined"
      :readonly="alleenLezen || undefined"
      @change="zet(uit($event))"
    ></nldd-date-field>
    <nldd-text-field
      v-else-if="veld.type === 'number' || veld.type === 'amount'"
      keyboard="decimal"
      :value="tekst"
      :required="verplicht || undefined"
      :readonly="alleenLezen || undefined"
      @input="zet(getal(uit($event)))"
    ></nldd-text-field>
    <nldd-text-field
      v-else
      :value="tekst"
      :autocomplete="autocomplete || undefined"
      :keyboard="keyboard || undefined"
      :width="width || undefined"
      :required="verplicht || undefined"
      :readonly="alleenLezen || undefined"
      @input="zet(uit($event))"
    ></nldd-text-field>
    <nldd-validation-list v-if="verplicht"></nldd-validation-list>
    <nldd-form-field-help-text v-if="onder">{{ onder }}</nldd-form-field-help-text>
  </nldd-form-field>
</template>
