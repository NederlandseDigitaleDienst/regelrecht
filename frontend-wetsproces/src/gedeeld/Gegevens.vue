<script setup lang="ts">
// What a record holds, field by field: what was submitted, what the
// authority found, what came out. Empty fields stay out.
import { computed } from 'vue';
import { toon } from '@regelrecht/frontend-shared/chronolex/waarden';
import { longDate } from './datum';
import { schermveld } from './formulier';
import type { Vormen } from './types';

const props = defineProps<{ waarden?: Record<string, unknown>; teksten?: Vormen['teksten']; label: string }>();
/** A value in words; a date as a screen writes it. */
const tekst = (v: unknown) => (typeof v === 'string' && /^\d{4}-\d{2}-\d{2}$/.test(v) ? longDate(v) : toon(v));
const rijen = computed(() =>
  Object.entries(props.waarden ?? {})
    .filter(([, v]) => v !== null && v !== undefined)
    .map(([naam, v]) => ({ naam, label: schermveld({ naam, type: null }, props.teksten).label, waarde: tekst(v) })),
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
