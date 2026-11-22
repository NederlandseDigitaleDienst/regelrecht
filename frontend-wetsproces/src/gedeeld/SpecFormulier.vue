<script setup lang="ts">
// The fields of a form, in the order of the UI spec (`volgorde`) and under
// the headings of its groups; a field the UI spec does not name follows, in
// the order of the law (formulier.ts `geordend`).
import { computed } from 'vue';
import { geordend } from './formulier';
import type { Schermveld } from './formulier';
import SpecVeld from './SpecVeld.vue';

const props = defineProps<{ velden: Schermveld[]; waarden: Record<string, unknown>; oordeel?: boolean; volgorde?: string[] }>();
const emit = defineEmits<{ zet: [naam: string, waarde: unknown] }>();
const groepen = computed(() => geordend(props.velden, props.volgorde));
</script>

<template>
  <nldd-container gap="16">
    <nldd-container v-for="(g, i) in groepen" :key="i" gap="16">
      <nldd-title v-if="g.titel" heading-level="4" size="display-6" :text="g.titel"></nldd-title>
      <SpecVeld
        v-for="v in g.velden"
        :key="v.veld.naam"
        :veld="v"
        :oordeel="oordeel"
        :model-value="waarden[v.veld.naam]"
        @update:model-value="emit('zet', v.veld.naam, $event)"
      />
    </nldd-container>
  </nldd-container>
</template>
