<script setup lang="ts">
// What a record holds, field by field: what was submitted, what the
// authority found, what came out. Empty fields stay out. `noem` names a
// record a field refers to, such as the contested decision of an objection.
// `herkomst` says per field where its value came from, where the system
// filled it, such as the rule of a policy.
import { computed } from 'vue';
import { schermveld } from './formulier';
import { waardeTekst } from './keten';
import type { Vormen } from './types';

const props = defineProps<{
  waarden?: Record<string, unknown>;
  teksten?: Vormen['teksten'];
  label: string;
  noem?: (id: string) => string | undefined;
  herkomst?: Record<string, string>;
}>();
const rijen = computed(() =>
  Object.entries(props.waarden ?? {})
    .filter(([, v]) => v !== null && v !== undefined)
    .map(([naam, v]) => ({ naam, label: schermveld({ naam, type: null }, props.teksten).label, waarde: [waardeTekst(v, props.noem), props.herkomst?.[naam]].filter(Boolean).join(' · ') })),
);
</script>

<template>
  <nldd-list v-if="rijen.length" appearance="box-base" :accessible-label="label">
    <nldd-list-item v-for="r in rijen" :key="r.naam" size="sm">
      <nldd-text-cell :text="r.label" :supporting-text="r.waarde"></nldd-text-cell>
    </nldd-list-item>
  </nldd-list>
  <nldd-text v-else color="secondary">Geen gegevens.</nldd-text>
</template>
