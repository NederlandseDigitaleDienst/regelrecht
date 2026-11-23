<script setup lang="ts">
// The fields of a form, in the order of the UI spec (`volgorde`) and under
// the headings of its groups; a field the UI spec does not name follows, in
// the order of the law (formulier.ts `geordend`). A field with `systeem` is
// already filled, by the system or by a request: it is no question, so it
// shows as a row of a list in its place in its group, with its value and
// where that comes from, as a recorded submission shows it (Gegevens.vue).
// Fields of that kind that follow each other share one list.
import { computed } from 'vue';
import { geordend } from './formulier';
import type { Formulierveld, Veldgroep } from './formulier';
import { waardeTekst } from './keten';
import SpecVeld from './SpecVeld.vue';

const props = withDefaults(
  defineProps<{
    velden: Formulierveld[];
    waarden: Record<string, unknown>;
    oordeel?: boolean;
    volgorde?: string[];
    /** Name the article each field rests on under it. Off where a screen of its own holds them. */
    artikelen?: boolean;
    /** A caption over each list of what is already known, in the words of whoever the form speaks to. */
    bekendKop?: string;
  }>(),
  { oordeel: false, volgorde: () => [], artikelen: true, bekendKop: '' },
);
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
/** The article a whole group rests on, where every field of it names the same one. */
const groepsArtikel = (g: Veldgroep) => {
  const eerste = g.velden[0]?.veld.basis ?? '';
  return eerste && g.velden.every((v) => v.veld.basis === eerste) ? eerste : '';
};
/** A group in the order it shows: each question on its own, and each run of filled fields as one list. */
const stukken = (g: Veldgroep) =>
  g.velden.reduce<{ bekend: boolean; velden: Formulierveld[] }[]>((uit, v) => {
    const laatste = uit[uit.length - 1];
    if (v.systeem && laatste?.bekend) laatste.velden.push(v);
    else uit.push({ bekend: !!v.systeem, velden: [v] });
    return uit;
  }, []);
/** What a filled field holds, in words. */
const bekend = (v: Formulierveld) => (v.systeem?.waarde == null ? '' : waardeTekst(v.systeem.waarde));
/** The line under a field: what the UI spec says, the article it rests on, and who filled it. */
const onder = (v: Formulierveld) =>
  [v.help, props.artikelen && v.veld.basis ? `Volgens ${v.veld.basis}.` : '', v.systeem?.herkomst]
    .filter(Boolean)
    .join(' ');
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
            kaal
            :artikel="artikelen"
            :model-value="waarden[v.veld.naam]"
            @update:model-value="emit('zet', v.veld.naam, $event)"
          />
        </nldd-checkbox-group>
        <nldd-form-field-help-text v-if="artikelen && groepsArtikel(g)">Volgens {{ groepsArtikel(g) }}.</nldd-form-field-help-text>
      </nldd-form-field>
      <template v-else>
        <nldd-title v-if="g.titel" heading-level="4" size="display-6" :text="g.titel"></nldd-title>
        <template v-for="s in stukken(g)" :key="s.velden[0]!.veld.naam">
          <nldd-container v-if="s.bekend" gap="8">
            <nldd-text v-if="bekendKop"><p>{{ bekendKop }}</p></nldd-text>
            <nldd-list appearance="box-base" :accessible-label="bekendKop || g.titel || 'Wat al bekend is'">
              <nldd-list-item v-for="v in s.velden" :key="v.veld.naam">
                <nldd-text-cell :text="v.label" :supporting-text="onder(v)"></nldd-text-cell>
                <nldd-spacer-cell size="16"></nldd-spacer-cell>
                <nldd-text-cell :text="bekend(v)" width="fit-content" horizontal-alignment="right"></nldd-text-cell>
              </nldd-list-item>
            </nldd-list>
          </nldd-container>
          <SpecVeld
            v-else
            :veld="s.velden[0]!"
            :oordeel="oordeel"
            :artikel="artikelen"
            :model-value="waarden[s.velden[0]!.veld.naam]"
            @update:model-value="emit('zet', s.velden[0]!.veld.naam, $event)"
          />
        </template>
      </template>
    </nldd-container>
  </nldd-container>
</template>
