<script setup lang="ts">
// What a record holds, field by field: what was submitted, what the
// authority found, what came out. Empty fields stay out. The fields stand
// in the order of the UI spec (`volgorde`) and under the headings of its
// groups, with its label and help, as on the forms (formulier.ts). `noem`
// names a record a field refers to, such as the contested decision of an
// objection. `herkomst` says per field where its value came from, where the
// system filled it, such as the rule of a policy.
import { computed } from 'vue';
import { geordend, kaalSchermveld } from './formulier';
import { waardeTekst } from './keten';
import type { Vormen } from './types';

const props = defineProps<{
  waarden?: Record<string, unknown>;
  teksten?: Vormen['teksten'];
  volgorde?: string[];
  label: string;
  noem?: (id: string) => string | undefined;
  herkomst?: Record<string, string>;
}>();
const groepen = computed(() => {
  const waarden = props.waarden ?? {};
  const velden = Object.keys(waarden)
    .filter((naam) => waarden[naam] !== null && waarden[naam] !== undefined)
    .map((naam) => kaalSchermveld(naam, props.teksten));
  return geordend(velden, props.volgorde).map((g) => ({
    titel: g.titel,
    rijen: g.velden.map((s) => ({
      naam: s.veld.naam,
      label: s.label,
      waarde: [waardeTekst(waarden[s.veld.naam], props.noem), props.herkomst?.[s.veld.naam], s.help]
        .filter(Boolean)
        .join(' · '),
    })),
  }));
});
</script>

<template>
  <nldd-container v-if="groepen.length" gap="8">
    <template v-for="(g, i) in groepen" :key="i">
      <nldd-text v-if="g.titel" weight="medium">{{ g.titel }}</nldd-text>
      <nldd-list appearance="box-base" :accessible-label="g.titel || label">
        <nldd-list-item v-for="r in g.rijen" :key="r.naam" size="sm">
          <nldd-text-cell :text="r.label" :supporting-text="r.waarde"></nldd-text-cell>
        </nldd-list-item>
      </nldd-list>
    </template>
  </nldd-container>
  <nldd-text v-else color="secondary">Geen gegevens.</nldd-text>
</template>
