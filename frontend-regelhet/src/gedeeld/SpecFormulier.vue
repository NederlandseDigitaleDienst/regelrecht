<script setup lang="ts">
// The fields of a form, in the order of the UI spec (`volgorde`) and under
// the headings of its groups; a field the UI spec does not name follows, in
// the order of the law (formulier.ts `geordend`). A field with `systeem` is
// already filled, by the system or by a request: it is no question, so it
// stands apart, under the questions, in one list of what is already known,
// with its value and where that comes from, as a recorded submission shows it
// (Gegevens.vue). Whoever fills in the form starts with what is asked.
import { computed } from 'vue';
import { geordend } from './formulier';
import type { Formulierveld, Veldgroep } from './formulier';
import { waardeTekst } from './keten';
import SpecVeld from './SpecVeld.vue';
import TabelUitKroniek from './TabelUitKroniek.vue';
import { alsRijen } from './tabel';
import type { Tabel } from './types';

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
    /** The tables of the UI spec: a known value that is a list of rows takes the columns of the one on its field. */
    tabellen?: Tabel[];
    /** Per field, what its answer still lacks. */
    fouten?: Record<string, string>;
  }>(),
  { oordeel: false, volgorde: () => [], artikelen: true, bekendKop: '', tabellen: () => [], fouten: () => ({}) },
);
const emit = defineEmits<{ zet: [naam: string, waarde: unknown]; bewerk: [naam: string] }>();
/** The questions, in their groups. */
const groepen = computed(() =>
  geordend(
    props.velden.filter((v) => !v.systeem),
    props.volgorde,
  ),
);
/** What is already known, in the order of the UI spec, under the questions. */
const bekendeVelden = computed(() => geordend(props.velden.filter((v) => v.systeem), props.volgorde).flatMap((g) => g.velden));

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
/** A known value that is a list of rows: it shows as a table under the list, read-only, not as one line. */
const alsTabel = (v: Formulierveld) => alsRijen(v.systeem?.waarde);
const tabelVan = (v: Formulierveld): Tabel =>
  props.tabellen.find((t) => t.veld === v.veld.naam) ?? { naam: v.veld.naam, kolommen: [] };
/** What a filled field holds, in words. */
const bekend = (v: Formulierveld) => (v.systeem?.waarde == null ? '' : waardeTekst(v.systeem.waarde));
/** The line under a field: what the UI spec says, the article it rests on, and who filled it. */
const onder = (v: Formulierveld) =>
  [v.help, props.artikelen && v.veld.basis ? `Volgens ${v.veld.basis}.` : '', v.systeem?.herkomst]
    .filter(Boolean)
    .join(' ');
</script>

<template>
  <nldd-container>
    <template v-for="(g, i) in groepen" :key="i">
      <nldd-spacer v-if="i > 0" size="24"></nldd-spacer>
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
        <template v-if="g.titel">
          <nldd-title heading-level="4" size="display-6" :text="g.titel"></nldd-title>
          <nldd-spacer size="8"></nldd-spacer>
        </template>
        <template v-for="(v, j) in g.velden" :key="v.veld.naam">
          <nldd-spacer v-if="j > 0" size="16"></nldd-spacer>
          <SpecVeld
            :veld="v"
            :oordeel="oordeel"
            :artikel="artikelen"
            :fout="fouten[v.veld.naam]"
            :model-value="waarden[v.veld.naam]"
            @update:model-value="emit('zet', v.veld.naam, $event)"
          />
        </template>
      </template>
    </template>
    <template v-if="bekendeVelden.length">
      <nldd-spacer v-if="groepen.length" size="24"></nldd-spacer>
      <nldd-title heading-level="4" size="display-6" text="Wat al bekend is"></nldd-title>
      <nldd-spacer size="8"></nldd-spacer>
      <template v-if="bekendKop">
        <nldd-text><p>{{ bekendKop }}</p></nldd-text>
        <nldd-spacer size="8"></nldd-spacer>
      </template>
      <nldd-list appearance="box-base" accessible-label="Wat al bekend is">
        <nldd-list-item v-for="v in bekendeVelden.filter((b) => !alsTabel(b))" :key="v.veld.naam">
          <nldd-text-cell :text="v.label" :supporting-text="onder(v)"></nldd-text-cell>
          <nldd-spacer-cell size="16"></nldd-spacer-cell>
          <nldd-text-cell :text="bekend(v)" width="fit-content" horizontal-alignment="right"></nldd-text-cell>
          <nldd-cell v-if="v.systeem?.aanpasbaar" width="fit-content">
            <nldd-button size="sm" appearance="secondary" text="Aanpassen" :accessible-label="`${v.label} aanpassen`" @click="emit('bewerk', v.veld.naam)"></nldd-button>
          </nldd-cell>
        </nldd-list-item>
      </nldd-list>
      <template v-for="v in bekendeVelden.filter((b) => alsTabel(b))" :key="v.veld.naam">
        <nldd-spacer size="16"></nldd-spacer>
        <TabelUitKroniek :tabel="{ ...tabelVan(v), label: v.label, help: onder(v) }" :rijen="alsTabel(v) ?? []" />
      </template>
    </template>
  </nldd-container>
</template>
