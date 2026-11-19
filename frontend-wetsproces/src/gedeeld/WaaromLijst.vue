<script setup lang="ts">
// Why a decision came out so, as the cell explains it: per open term of the
// decision who filled it, and per answer of a cell what it answered.
import { schermveld } from './formulier';
import { artikelNaam } from './keten';
import type { Vormen, Waarom } from './types';
import { antwoordTekst, ingevuldDoor } from './waarom';

const props = defineProps<{ waarom: Waarom; teksten?: Vormen['teksten']; celNaam: (cel: string) => string }>();
const naam = (term: string) => schermveld({ naam: term, type: null }, props.teksten).label;
</script>

<template>
  <nldd-container gap="12">
    <nldd-list v-if="waarom.ingevuld.length" appearance="box-base" accessible-label="Wie elk open begrip invulde">
      <nldd-list-item v-for="f in waarom.ingevuld" :key="`${f.law_id}#${f.article}#${f.term}`" size="sm">
        <nldd-text-cell
          :text="naam(f.term)"
          :supporting-text="`${artikelNaam(`${f.law_id}#${f.article}`)} · ${ingevuldDoor(f.filled_by, celNaam)}`"
        ></nldd-text-cell>
      </nldd-list-item>
    </nldd-list>
    <nldd-text v-else color="secondary">Dit besluit vulde geen open begrip in.</nldd-text>
    <nldd-text v-if="waarom.antwoorden.length" weight="medium">De antwoorden van cellen die het besluit gebruikte</nldd-text>
    <nldd-list v-if="waarom.antwoorden.length" appearance="box-base" accessible-label="Antwoorden van cellen">
      <nldd-list-item v-for="(a, i) in waarom.antwoorden" :key="i" size="sm">
        <nldd-text-cell
          :text="`${naam(a.begrip)}: ${antwoordTekst(a.waarde)}`"
          :supporting-text="`Antwoord van ${celNaam(a.door)}`"
        ></nldd-text-cell>
      </nldd-list-item>
    </nldd-list>
  </nldd-container>
</template>
