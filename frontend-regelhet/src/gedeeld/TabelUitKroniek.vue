<script setup lang="ts">
// A table from the chronicle: the rows of a table element of the UI spec
// (`table`), a column per key it names, and per row where it comes from.
// The back office shows it beside a submission (BehandelenPaneel.vue, `GET
// /api/backoffice/tabellen/{id}`): each row with a tag per record it comes
// from, the cell that recorded it, what it records and on which day, so an
// employee sees beside a number the record of another cell it was taken
// from. A row no record of a named fact matched says so. The application form
// shows a known value that is a list of rows with the same component,
// read-only and without sources (SpecFormulier.vue): what the system filled.
import { computed } from 'vue';
import { bronTekst, cel, kolommen, kop, watTekst } from './tabel';
import type { Tabel, Tabelrij, Veldtekst } from './types';

const props = withDefaults(
  defineProps<{
    tabel: Tabel;
    rijen: Tabelrij[];
    /** Why the table has no rows, as the server says. */
    leeg?: string;
    /** The name of a cell, for the sources. */
    celNaam?: (cel: string) => string;
    teksten?: Record<string, Veldtekst>;
    /** Spread the columns over the full width, where no column of sources takes the rest. */
    vullend?: boolean;
  }>(),
  { leeg: undefined, celNaam: (c: string) => c, teksten: () => ({}), vullend: false },
);

const kols = computed(() => kolommen(props.tabel, props.rijen));
/** Whether a row names where it comes from: only on the back office, from the server. */
const metHerkomst = computed(() => props.rijen.some((r) => r.herkomst.length || r.zonder_herkomst?.length));
const titel = computed(() => props.tabel.label ?? props.teksten[props.tabel.veld ?? '']?.label ?? props.tabel.element ?? props.tabel.naam);
const breedtes = computed(() => {
  const kolom = props.vullend && !metHerkomst.value ? 'minmax(max-content, 1fr)' : 'max-content';
  return `${kols.value.map(() => kolom).join(' ')}${metHerkomst.value ? ' 1fr' : ''}`;
});
</script>

<template>
  <nldd-container gap="8">
    <nldd-title heading-level="4" size="display-5" :text="titel" :supporting-text="tabel.help || undefined"></nldd-title>
    <nldd-inline-dialog v-if="!rijen.length" icon="info" text="Geen rijen" :supporting-text="leeg"></nldd-inline-dialog>
    <nldd-table v-else :columns="breedtes" :accessible-label="titel">
      <nldd-table-row slot="header">
        <nldd-text-cell v-for="k in kols" :key="k.name" size="sm" :text="kop(k)"></nldd-text-cell>
        <nldd-text-cell v-if="metHerkomst" size="sm" text="Herkomst"></nldd-text-cell>
      </nldd-table-row>
      <nldd-table-row v-for="(r, i) in rijen" :key="i">
        <nldd-text-cell v-for="k in kols" :key="k.name" size="sm" :text="cel(r.waarden[k.name])"></nldd-text-cell>
        <nldd-cell v-if="metHerkomst">
          <nldd-container gap="4">
            <nldd-container v-for="b in r.herkomst" :key="b.stuk" layout="row" gap="8">
              <nldd-tag size="sm" color="neutral" :text="celNaam(b.cel)" :accessible-label="bronTekst(b, celNaam)"></nldd-tag>
              <nldd-text size="sm" color="secondary"><p>{{ watTekst(b.wat) }}, {{ bronTekst(b, celNaam).split(' · ').at(-1) }}</p></nldd-text>
            </nldd-container>
            <nldd-tag v-for="f in r.zonder_herkomst ?? []" :key="f" size="sm" color="warning" :text="`Geen ${watTekst(f).toLowerCase()}`"></nldd-tag>
          </nldd-container>
        </nldd-cell>
      </nldd-table-row>
    </nldd-table>
  </nldd-container>
</template>
