<script setup lang="ts">
// The timeline of one application, on the citizen's screen and the
// employee's alike: every record that happened about it, in order, and under
// it what can or must happen next and who is to act. The server derives both
// (`GET /api/dossier/tijdlijn/{id}`, `GET /api/backoffice/tijdlijn/{id}`) and
// keeps them to what the reader may see; this names them (tijdlijn.ts). A
// row opens the record's own screen where the side has one (`naar`).
// Built as the design system draws steps under each other: an `nldd-list`
// with an `nldd-step-cell` per row, the past done and the next steps current.
import { computed } from 'vue';
import type { RouteLocationRaw } from 'vue-router';
import { useRouter } from 'vue-router';
import { regels, stap } from './tijdlijn';
import type { Kant } from './tijdlijn';
import type { Casus, Gram, Tijdlijn, Veldtekst } from './types';

const props = defineProps<{
  tijdlijn: Tijdlijn | null;
  casus: Casus;
  kant: Kant;
  teksten?: Record<string, Veldtekst>;
  /** The record on screen beside it, marked as current. */
  huidig?: string;
  /** Where a record opens, if this side has a screen for it. */
  naar?: (g: Gram) => RouteLocationRaw | undefined;
  /** Why the timeline did not load. */
  fout?: string;
}>();
const router = useRouter();

const rijen = computed(() => (props.tijdlijn ? regels(props.tijdlijn, props.casus).map((r) => ({ ...r, naar: props.naar?.(r.gram) })) : []));
const volgende = computed(() => {
  const t = props.tijdlijn;
  return t ? t.volgende.map((s) => stap(s, t, props.casus, props.kant, props.teksten)) : [];
});
/** Where row `i` of all rows stands, for the track of the step cells. */
function positie(i: number): 'first' | 'between' | 'last' | 'only' {
  const n = rijen.value.length + volgende.value.length;
  if (n === 1) return 'only';
  if (i === 0) return 'first';
  return i === n - 1 ? 'last' : 'between';
}
</script>

<template>
  <nldd-container gap="8">
    <nldd-title
      heading-level="4"
      size="display-5"
      text="Tijdlijn"
      supporting-text="Wat er bij deze aanvraag is gebeurd, op volgorde, en wat er nu kan of moet gebeuren."
    ></nldd-title>
    <nldd-inline-dialog v-if="fout" icon="warning" icon-color="critical" text="De tijdlijn laadt niet" :supporting-text="fout"></nldd-inline-dialog>
    <nldd-inline-dialog v-else-if="!tijdlijn" variant="loading" text="De tijdlijn laden"></nldd-inline-dialog>
    <nldd-list v-else appearance="simple" dividers="never" accessible-label="Tijdlijn van de aanvraag">
      <nldd-list-item
        v-for="(r, i) in rijen"
        :key="r.gram.id"
        size="md"
        :button="r.naar ? true : undefined"
        :current="r.gram.id === huidig || undefined"
        @click="r.naar && router.push(r.naar)"
      >
        <nldd-step-cell status="past" :position="positie(i)"></nldd-step-cell>
        <nldd-spacer-cell size="8"></nldd-spacer-cell>
        <nldd-text-cell :text="r.tekst" :supporting-text="r.regel"></nldd-text-cell>
        <nldd-cell v-if="r.stand">
          <nldd-tag :text="r.stand.tekst" :color="r.stand.kleur" size="sm"></nldd-tag>
        </nldd-cell>
        <template v-if="r.naar">
          <nldd-spacer-cell size="8"></nldd-spacer-cell>
          <nldd-icon-cell size="20"><nldd-icon icon="chevron-right"></nldd-icon></nldd-icon-cell>
        </template>
      </nldd-list-item>
      <nldd-list-item v-for="(s, j) in volgende" :key="`volgende-${j}`" size="md">
        <nldd-step-cell status="current" :position="positie(rijen.length + j)"></nldd-step-cell>
        <nldd-spacer-cell size="8"></nldd-spacer-cell>
        <nldd-text-cell overline="Volgende stap" :text="s.tekst" :supporting-text="s.regel"></nldd-text-cell>
      </nldd-list-item>
    </nldd-list>
    <nldd-text v-if="tijdlijn && !volgende.length" size="sm" color="secondary"><p>Er staat nu niets open bij deze aanvraag.</p></nldd-text>
  </nldd-container>
</template>
