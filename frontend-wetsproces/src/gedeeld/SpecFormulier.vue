<script setup lang="ts">
// The fields of a form, one after the other, in the order the law gives them.
import type { Schermveld } from './formulier';
import SpecVeld from './SpecVeld.vue';

defineProps<{ velden: Schermveld[]; waarden: Record<string, unknown>; oordeel?: boolean }>();
const emit = defineEmits<{ zet: [naam: string, waarde: unknown] }>();
</script>

<template>
  <nldd-container gap="16">
    <SpecVeld
      v-for="v in velden"
      :key="v.veld.naam"
      :veld="v"
      :oordeel="oordeel"
      :model-value="waarden[v.veld.naam]"
      @update:model-value="emit('zet', v.veld.naam, $event)"
    />
  </nldd-container>
</template>
