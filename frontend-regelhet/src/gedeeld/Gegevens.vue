<script setup lang="ts">
// What a record holds, field by field: what was submitted, what the
// authority found, what came out. Empty fields stay out. A row names the
// field with its origin and help under it, and has the value on its right, as
// the form shows what is already known (SpecFormulier.vue). The fields stand
// in the order of the UI spec (`volgorde`) and under the headings of its
// groups, with its label and help, as on the forms (formulier.ts). `noem`
// names a record a field refers to, such as the contested decision of an
// objection. `herkomst` says per field where its value came from, where the
// system filled it, such as the rule of a policy. `bij` adds rows that are no
// field of the record, such as the article it rests on, at the end of its
// last group. It renders no wrapper of its own, so where it stands in a
// container with a gap the caller gives it a container without one.
import { computed } from 'vue';
import { geordend, kaalSchermveld, stukWaarde } from './formulier';
import type { Vormen } from './types';

const props = defineProps<{
  waarden?: Record<string, unknown>;
  teksten?: Vormen['teksten'];
  volgorde?: string[];
  label: string;
  noem?: (id: string) => string | undefined;
  herkomst?: Record<string, string>;
  bij?: { naam: string; label: string; waarde: string }[];
}>();
const groepen = computed(() => {
  const waarden = props.waarden ?? {};
  const velden = Object.keys(waarden)
    .filter((naam) => waarden[naam] !== null && waarden[naam] !== undefined)
    .map((naam) => kaalSchermveld(naam, props.teksten));
  const groepen = geordend(velden, props.volgorde).map((g) => ({
    titel: g.titel,
    rijen: g.velden.map((s) => ({
      naam: s.veld.naam,
      label: s.label,
      waarde: stukWaarde(s.veld.naam, waarden[s.veld.naam], props.teksten, props.noem),
      onder: [props.herkomst?.[s.veld.naam], s.help].filter(Boolean).join(' · '),
    })),
  }));
  const bij = (props.bij ?? []).map((b) => ({ ...b, onder: '' }));
  if (!bij.length) return groepen;
  const laatste = groepen.at(-1);
  return laatste ? [...groepen.slice(0, -1), { ...laatste, rijen: [...laatste.rijen, ...bij] }] : [{ titel: '', rijen: bij }];
});
</script>

<template>
  <!-- No wrapper: the lists, headings and spacers stand in the slot of the section around it. -->
  <template v-if="groepen.length">
    <template v-for="(g, i) in groepen" :key="i">
      <nldd-spacer v-if="i > 0" size="24"></nldd-spacer>
      <template v-if="g.titel">
        <nldd-text size="lg" weight="medium"><p>{{ g.titel }}</p></nldd-text>
        <nldd-spacer size="8"></nldd-spacer>
      </template>
      <nldd-list appearance="box-base" :accessible-label="g.titel || label">
        <nldd-list-item v-for="r in g.rijen" :key="r.naam" size="md">
          <nldd-text-cell :text="r.label" :supporting-text="r.onder || undefined"></nldd-text-cell>
          <nldd-spacer-cell size="16"></nldd-spacer-cell>
          <nldd-text-cell :text="r.waarde" width="fit-content" horizontal-alignment="right"></nldd-text-cell>
        </nldd-list-item>
      </nldd-list>
    </template>
  </template>
  <nldd-text v-else color="secondary"><p>Geen gegevens.</p></nldd-text>
</template>
