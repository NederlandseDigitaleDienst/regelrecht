<script setup lang="ts">
// One field of a form that follows from the law and the UI spec. A yes or no
// that may stay empty, and every judgement of the authority (`oordeel`),
// asks ja, nee or empty; a checkbox knows only on and off. Where the UI spec
// words the answers (`when_true`, `when_false`), the choices say them, so a
// decider scanning the findings reads the answer without the question:
// "Onderzoek nodig" rather than "Nee". An empty judgement is "Niet
// beoordeeld". Every other field
// is the shared VeldInvoer of the chronolexografie poc.
import OordeelKeuze from '@regelrecht/frontend-shared/components/chronolex/OordeelKeuze.vue';
import VeldInvoer from '@regelrecht/frontend-shared/components/chronolex/VeldInvoer.vue';
import type { Formulierveld } from './formulier';

const props = withDefaults(
  defineProps<{ veld: Formulierveld; modelValue?: unknown; oordeel?: boolean; kaal?: boolean; artikel?: boolean; fout?: string }>(),
  { modelValue: undefined, oordeel: false, kaal: false, artikel: true, fout: '' },
);
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
    :leeg="oordeel ? 'Niet beoordeeld' : 'Niet opgegeven'"
    :ja="veld.ja ?? 'Ja'"
    :nee="veld.nee ?? 'Nee'"
    :toelichting="veld.help || (artikel ? veld.veld.basis : '')"
    :artikel="artikel"
    :fout="fout"
    @update:model-value="emit('update:modelValue', $event)"
  />
  <VeldInvoer
    v-else
    :kaal="kaal"
    :artikel="artikel"
    :autocomplete="veld.deel?.autocomplete"
    :keyboard="veld.deel?.keyboard"
    :width="veld.deel?.width"
    :verplicht="veld.verplicht"
    :veld="veld.veld"
    :label="veld.label"
    :help="veld.help"
    :model-value="modelValue"
    @update:model-value="emit('update:modelValue', $event)"
  />
</template>
