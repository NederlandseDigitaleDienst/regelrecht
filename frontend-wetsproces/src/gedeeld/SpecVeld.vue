<script setup lang="ts">
// One field of a form that follows from the law and the UI spec. A yes or no
// that may stay empty, and every judgement of the authority (`oordeel`),
// asks ja, nee or empty; a checkbox knows only on and off. Every other field
// is the shared VeldInvoer of the chronolexografie poc.
import OordeelKeuze from '@regelrecht/frontend-shared/components/chronolex/OordeelKeuze.vue';
import VeldInvoer from '@regelrecht/frontend-shared/components/chronolex/VeldInvoer.vue';
import type { Schermveld } from './formulier';

const props = withDefaults(defineProps<{ veld: Schermveld; modelValue?: unknown; oordeel?: boolean }>(), {
  modelValue: undefined,
  oordeel: false,
});
const emit = defineEmits<{ 'update:modelValue': [waarde: unknown] }>();

const keuze = () => props.veld.veld.type === 'boolean' && (props.oordeel || !!props.veld.veld.mag_leeg);
const alsKeuze = (v: unknown): boolean | undefined => (typeof v === 'boolean' ? v : undefined);
</script>

<template>
  <OordeelKeuze
    v-if="keuze()"
    :veld="veld.veld"
    :label="veld.label"
    :model-value="alsKeuze(modelValue)"
    :leeg="oordeel ? 'Nog niet beoordeeld' : 'Niet opgegeven'"
    :toelichting="veld.help || veld.veld.basis"
    @update:model-value="emit('update:modelValue', $event)"
  />
  <VeldInvoer
    v-else
    :veld="veld.veld"
    :label="veld.label"
    :help="veld.help"
    :model-value="modelValue"
    @update:model-value="emit('update:modelValue', $event)"
  />
</template>
