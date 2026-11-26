<script setup lang="ts">
// What a decision rests on, as one list: per open term of the decision who
// filled it, then per answer of a cell what it answered.
// `beslisser` is the name of the cell that took the decision: a finding is
// its finding, whether it is a municipality or a court.
import { kaalSchermveld } from './formulier';
import { artikelNaam } from './keten';
import type { Vormen, Waarom } from './types';
import { antwoordTekst, ingevuldDoor } from './waarom';

const props = defineProps<{
  waarom: Waarom;
  teksten?: Vormen['teksten'];
  celNaam: (cel: string) => string;
  beslisser: string;
}>();
const naam = (term: string) => kaalSchermveld(term, props.teksten).label;
</script>

<template>
  <nldd-list
    v-if="waarom.ingevuld.length || waarom.antwoorden.length"
    appearance="box-base"
    accessible-label="Waarop het besluit rust"
  >
    <nldd-list-item v-for="f in waarom.ingevuld" :key="`${f.law_id}#${f.article}#${f.term}`">
      <nldd-text-cell
        :text="naam(f.term)"
        :supporting-text="`${artikelNaam(`${f.law_id}#${f.article}`)} · ${ingevuldDoor(f.filled_by, celNaam, beslisser)}`"
      ></nldd-text-cell>
    </nldd-list-item>
    <nldd-list-item v-for="(a, i) in waarom.antwoorden" :key="i">
      <nldd-text-cell
        :text="`${naam(a.begrip)}: ${antwoordTekst(a.waarde, beslisser)}`"
        :supporting-text="`Antwoord van ${celNaam(a.door)}`"
      ></nldd-text-cell>
    </nldd-list-item>
  </nldd-list>
  <nldd-text v-else color="secondary"><p>Dit besluit vulde geen open begrip in en gebruikte geen antwoord van een cel.</p></nldd-text>
</template>
