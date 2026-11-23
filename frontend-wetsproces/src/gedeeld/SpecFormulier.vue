<script setup lang="ts">
// The fields of a form, in the order of the UI spec (`volgorde`) and under
// the headings of its groups; a field the UI spec does not name follows, in
// the order of the law (formulier.ts `geordend`). A field with `systeem` is
// already filled, by the system or by a request: it shows read-only in its
// place in its group, with where its value comes from.
import { computed } from 'vue';
import { geordend } from './formulier';
import type { Formulierveld, Veldgroep } from './formulier';
import { waardeTekst } from './keten';
import SpecVeld from './SpecVeld.vue';

const props = defineProps<{ velden: Formulierveld[]; waarden: Record<string, unknown>; oordeel?: boolean; volgorde?: string[] }>();
const emit = defineEmits<{ zet: [naam: string, waarde: unknown] }>();
const groepen = computed(() => geordend(props.velden, props.volgorde));

/** Whether this field draws a bare checkbox: a plain yes or no, with no help under it. */
const isVinkje = (v: Formulierveld) =>
  !v.systeem && v.veld.type === 'boolean' && !props.oordeel && !v.veld.mag_leeg && !v.help;
// A heading whose every field is a bare checkbox is one question with several
// answers, so it becomes a checkbox group inside an nldd-form-field: the field
// carries the heading as the caption over the group, which is what a caption
// over several controls is for, and the group makes a screen reader announce
// them as one. A field with something under it draws its own form field, so a
// group with one of those stays a list of fields.
const vinkjesGroep = (g: Veldgroep) => !!g.titel && g.velden.length > 1 && g.velden.every(isVinkje);
</script>

<template>
  <nldd-container gap="16">
    <nldd-container v-for="(g, i) in groepen" :key="i" gap="16">
      <nldd-form-field v-if="vinkjesGroep(g)" :label="g.titel">
        <nldd-checkbox-group>
          <SpecVeld
            v-for="v in g.velden"
            :key="v.veld.naam"
            :veld="v"
            :model-value="waarden[v.veld.naam]"
            @update:model-value="emit('zet', v.veld.naam, $event)"
          />
        </nldd-checkbox-group>
      </nldd-form-field>
      <template v-else>
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
      </template>
    </nldd-container>
  </nldd-container>
</template>
