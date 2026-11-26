<script setup lang="ts">
// The elements of the UI spec an employee at this level sees beside a
// submission (gedeeld/rollen.ts), each with its role and its fields: the
// value the submission holds, or what the decision gives, or that it is not
// known yet. A part the element adds names its fields and the facts whose
// records it shows; those show as tables from the chronicle above it
// (TabelUitKroniek.vue). A letter is shown with the decision, not here.
import { hoofdletter, leesbaar } from '@regelrecht/frontend-shared/chronolex/waarden';
import { computed } from 'vue';
import { voorMedewerker } from '../gedeeld/rollen';
import { cel } from '../gedeeld/tabel';
import type { Uielement, Veldtekst } from '../gedeeld/types';

const props = defineProps<{
  elementen?: Uielement[];
  niveau?: string | null;
  /** What is known of the submission: its fields, and the outcomes of its decision or preview. */
  waarden: Record<string, unknown>;
  teksten?: Record<string, Veldtekst>;
}>();

const zichtbaar = computed(() => (props.elementen ?? []).filter((e) => voorMedewerker(e.rol, props.niveau, e.rol_generiek)));
const naam = (n: string) => props.teksten?.[n]?.label ?? hoofdletter(leesbaar(n));
const waarde = (n: string) => (n in props.waarden && props.waarden[n] != null ? cel(props.waarden[n]) : 'Nog niet bekend');
</script>

<template>
  <nldd-container v-if="zichtbaar.length" gap="16">
    <nldd-title heading-level="4" size="display-5" text="Volgens de UI-beschrijving" supporting-text="De elementen voor uw rol, met wat over deze indiening bekend is."></nldd-title>
    <nldd-container v-for="e in zichtbaar" :key="e.id" gap="8">
      <nldd-title heading-level="5" size="display-6" :text="hoofdletter(leesbaar(e.element ?? e.id))">
        <nldd-tag slot="end" size="sm" color="neutral" :text="hoofdletter(leesbaar(e.rol ?? ''))"></nldd-tag>
      </nldd-title>
      <nldd-list v-if="e.velden.length" appearance="box-base" :accessible-label="e.element ?? e.id">
        <nldd-list-item v-for="v in e.velden" :key="v.naam" size="md">
          <nldd-text-cell :text="naam(v.naam)" :supporting-text="v.te_bouwen ? 'Het model draagt dit gegeven nog niet.' : undefined"></nldd-text-cell>
          <nldd-spacer-cell size="16"></nldd-spacer-cell>
          <nldd-text-cell :text="waarde(v.naam)" width="fit-content" horizontal-alignment="right"></nldd-text-cell>
        </nldd-list-item>
      </nldd-list>
      <nldd-text v-for="d in e.delen ?? []" :key="d.naam" size="sm" color="secondary">
        <p>{{ hoofdletter(leesbaar(d.naam)) }}{{ d.feiten.length ? `: met de vastleggingen ${d.feiten.map((f) => leesbaar(f.split('#').at(-1))).join(', ')}` : '' }}</p>
      </nldd-text>
    </nldd-container>
  </nldd-container>
</template>
