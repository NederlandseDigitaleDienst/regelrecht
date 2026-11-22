<script setup lang="ts">
// The fields of a form, in the order of the UI spec (`volgorde`) and under
// the headings of its groups; a field the UI spec does not name follows, in
// the order of the law (formulier.ts `geordend`). A field with `systeem` is
// already filled, by the system or by a request: it shows read-only in its
// place in its group, with where its value comes from.
import { computed } from 'vue';
import { geordend } from './formulier';
import type { Formulierveld } from './formulier';
import { waardeTekst } from './keten';
import SpecVeld from './SpecVeld.vue';

const props = defineProps<{ velden: Formulierveld[]; waarden: Record<string, unknown>; oordeel?: boolean; volgorde?: string[] }>();
const emit = defineEmits<{ zet: [naam: string, waarde: unknown] }>();
const groepen = computed(() => geordend(props.velden, props.volgorde));
</script>

<template>
  <nldd-container gap="16">
    <nldd-container v-for="(g, i) in groepen" :key="i" gap="16">
      <nldd-title v-if="g.titel" heading-level="4" size="display-6" :text="g.titel"></nldd-title>
      <template v-for="v in g.velden" :key="v.veld.naam">
        <nldd-form-field v-if="v.systeem" :label="v.label" :supporting-label="v.veld.basis">
          <nldd-text-field :value="v.systeem.waarde == null ? '' : waardeTekst(v.systeem.waarde)" readonly></nldd-text-field>
          <nldd-form-field-help-text>{{ [v.help, v.systeem.herkomst].filter(Boolean).join(' ') }}</nldd-form-field-help-text>
        </nldd-form-field>
        <SpecVeld
          v-else
          :veld="v"
          :oordeel="oordeel"
          :model-value="waarden[v.veld.naam]"
          @update:model-value="emit('zet', v.veld.naam, $event)"
        />
      </template>
    </nldd-container>
  </nldd-container>
</template>
