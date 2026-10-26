<script setup>
// Waarom een besluit zo uitviel, zoals de cel het uitlegt (Cell::explain).
// Eerst wie elk open begrip invulde: het bestuursorgaan (een bevinding), een
// regeling, of beleid dat het begrip koppelt aan het antwoord van een cel
// (RFC-048). Zo'n antwoord staat in het spoor als één stap; hoe de cel ertoe
// kwam, is haar eigen verhaal en komt op verzoek (Cell::explain_answer): het
// spoor van haar reductie, met welke besluiten meetelden en welke niet.
// Daaronder het spoor van de engine.
import { reactive } from 'vue';
import { api } from '../api.js';
import { celNaam, gelezen, grammenVan, ingevuldDoor } from '../logboek.js';
import { leesbaar, state, waarde } from '../state.js';
import HoeBeantwoord from './HoeBeantwoord.vue';

const props = defineProps({
  // Het besluit waarvan dit de uitleg is: bij een beslissing op bezwaar de
  // beslissing zelf, want daarin wordt het artikel van de casus opnieuw
  // toegepast.
  besluit: { type: String, required: true },
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

// Per antwoord de uitleg van de cel, als die gevraagd is. Vroeg het besluit
// een antwoord vaker, dan geeft de cel per keer een uitleg, in de volgorde van
// het spoor; `keer` van een antwoord zegt welke.
const hoe = reactive({});
const open = reactive({});

async function toon(i) {
  const naam = props.waarom.antwoorden[i].antwoord;
  open[i] = !open[i];
  if (!open[i] || hoe[naam]) return;
  try {
    hoe[naam] = await api.hoeBeantwoord(props.besluit, naam);
  } catch (e) {
    state.fout = e.message;
    open[i] = false;
  }
}

const uitlegBij = (i) => {
  const h = hoe[props.waarom.antwoorden[i].antwoord];
  const k = props.waarom.antwoorden[i].keer;
  return h ? h.antwoorden.slice(k, k + 1) : [];
};
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

    <nldd-container v-for="(a, i) in waarom.antwoorden" :key="i" gap="8">
      <nldd-inline-dialog
        icon="info"
        :text="`${leesbaar(a.begrip)}: ${jaNee(a.waarde)}`"
        :supporting-text="herkomst(a)"
      ></nldd-inline-dialog>
      <nldd-button
        appearance="secondary"
        size="sm"
        expandable
        :expanded="!!open[i]"
        text="Hoe de cel dit beantwoordde"
        @click="toon(i)"
      ></nldd-button>
      <nldd-container v-if="open[i]" gap="8">
        <HoeBeantwoord v-for="h in uitlegBij(i)" :key="h.spoor" :uitleg="h" :besluit="besluit" />
      </nldd-container>
    </nldd-container>

    <nldd-text size="sm" color="secondary">
      Het spoor van de engine, met de waarden die het besluit gebruikte. Het antwoord van de
      cel is daarin één stap.
    </nldd-text>
    <nldd-code-viewer appearance="box-tinted">{{ waarom.spoor }}</nldd-code-viewer>
  </nldd-container>
</template>
