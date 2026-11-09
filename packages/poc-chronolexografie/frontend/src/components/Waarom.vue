<script setup>
// Waarom een besluit zo uitviel, zoals de cel het uitlegt (Cell::explain).
// Eerst wie elk open begrip invulde: het bestuursorgaan (een bevinding), een
// regeling, of beleid dat het begrip koppelt aan het antwoord van een cel
// (RFC-048), met waar dat antwoord vandaan kwam.
import { celNaam, gelezen, grammenVan, ingevuldDoor } from '../logboek.js';
import { leesbaar, state, waarde } from '../state.js';

defineProps({
  waarom: { type: Object, required: true },
});

// Een stil antwoord (null) laat de engine over aan de bevinding van het
// bestuursorgaan.
const jaNee = (v) =>
  v === true ? 'ja' : v === false ? 'nee' : v == null ? 'geen antwoord (het bestuursorgaan beslist)' : waarde(v);

/** De herkomst van het antwoord van de cel, leesbaar. */
const herkomst = (a) =>
  `Antwoord ${a.antwoord} van ${celNaam(a.door)}, uit reductie ${a.reductie} (geldig vanaf ${a.geldig_vanaf}). Gelezen: ` +
  gelezen({ records_read: a.gelezen, last_record: a.tot_en_met }, grammenVan(a.door, state.grammen)) +
  '.';
</script>

<template>
  <nldd-container gap="12">
    <nldd-text size="sm" color="secondary">Waarom: wie elk open begrip invulde.</nldd-text>
    <nldd-list appearance="box-base" accessible-label="Open begrippen">
      <nldd-list-item v-for="f in waarom.ingevuld" :key="`${f.law_id}#${f.article}#${f.term}`" size="sm">
        <nldd-text-cell
          :text="leesbaar(f.term)"
          :supporting-text="`${f.law_id} artikel ${f.article} · ${ingevuldDoor(f.filled_by, state.grammen)}`"
        ></nldd-text-cell>
      </nldd-list-item>
    </nldd-list>

    <nldd-inline-dialog
      v-for="(a, i) in waarom.antwoorden"
      :key="i"
      icon="info"
      :text="`${leesbaar(a.begrip)}: ${jaNee(a.waarde)}`"
      :supporting-text="herkomst(a)"
    ></nldd-inline-dialog>
  </nldd-container>
</template>
