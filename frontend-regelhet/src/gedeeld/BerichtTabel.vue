<script setup lang="ts">
// A table of a message as one NLDD block (`nldd-table`): a specification (its
// rows and total), or an annex in columns with its rows grouped, a sum per
// group and the total. A block of its own, so a generic table from the
// chronicle can take its place without touching the rest of the message
// (Bericht.vue).
import { computed } from 'vue';
import type { Berichtbijlage, Berichtonderdeel } from './bericht';

const props = defineProps<{
  tabel?: Berichtonderdeel['tabel'];
  bijlage?: Berichtbijlage | null;
  label: string;
}>();

/** The columns of the grid: the first wide, the rest as wide as they need. */
const kolommen = computed(() =>
  props.bijlage ? ['minmax(180px,2fr)', ...props.bijlage.kolommen.slice(1).map(() => 'auto')].join(' ') : 'minmax(160px,1fr) auto',
);
const leeg = computed(() => (props.bijlage ? props.bijlage.kolommen.slice(2).map((_, i) => i) : []));
</script>

<template>
  <nldd-table v-if="bijlage" :columns="kolommen" :accessible-label="label">
    <nldd-table-row slot="header">
      <nldd-text-cell v-for="(k, i) in bijlage.kolommen" :key="i" :text="k" :horizontal-alignment="i > 0 ? 'right' : undefined"></nldd-text-cell>
    </nldd-table-row>
    <template v-for="(g, gi) in bijlage.groepen" :key="gi">
      <nldd-table-row v-if="g.titel">
        <nldd-text-cell :text="g.titel" row-header size="sm" color="secondary"></nldd-text-cell>
        <nldd-cell v-for="i in bijlage.kolommen.length - 1" :key="i"></nldd-cell>
      </nldd-table-row>
      <nldd-table-row v-for="(r, ri) in g.rijen" :key="ri">
        <nldd-text-cell
          v-for="(c, ci) in r.cellen"
          :key="ci"
          :text="c.tekst"
          :supporting-text="c.onder"
          :row-header="ci === 0"
          :horizontal-alignment="ci > 0 ? 'right' : undefined"
        ></nldd-text-cell>
      </nldd-table-row>
      <nldd-table-row v-if="g.subtotaal">
        <nldd-text-cell :text="g.subtotaal.label" row-header></nldd-text-cell>
        <nldd-cell v-for="i in leeg" :key="i"></nldd-cell>
        <nldd-text-cell :text="g.subtotaal.waarde" horizontal-alignment="right"></nldd-text-cell>
      </nldd-table-row>
    </template>
    <nldd-table-row v-if="bijlage.totaal">
      <nldd-text-cell :text="bijlage.totaal.label" row-header></nldd-text-cell>
      <nldd-cell v-for="i in leeg" :key="i"></nldd-cell>
      <nldd-text-cell :text="bijlage.totaal.waarde" horizontal-alignment="right"></nldd-text-cell>
    </nldd-table-row>
  </nldd-table>
  <nldd-table v-else-if="tabel" columns="minmax(160px,1fr) auto" :accessible-label="label">
    <nldd-table-row v-for="(r, i) in tabel.rijen" :key="i">
      <nldd-text-cell :text="r.label" row-header></nldd-text-cell>
      <nldd-text-cell :text="r.waarde" horizontal-alignment="right"></nldd-text-cell>
    </nldd-table-row>
    <nldd-table-row v-if="tabel.totaal">
      <nldd-text-cell :text="tabel.totaal.label" row-header></nldd-text-cell>
      <nldd-text-cell :text="tabel.totaal.waarde" horizontal-alignment="right"></nldd-text-cell>
    </nldd-table-row>
  </nldd-table>
</template>
