<script setup>
// Opnieuw beslissen op een bezwaar na een uitspraak (RFC-053, Awb 8:72 lid
// 4). Droeg de rechter een nieuw besluit op, dan beslist het bestuursorgaan
// opnieuw op hetzelfde bezwaar, met de ontvangst van de uitspraak als
// `after` (Cell::decide_after). Het geeft zijn bevindingen opnieuw, zoals
// bij de eerste heroverweging; de aanwijzingen van de rechter dwingt de cel
// niet af, de pagina toont ze. Of het bestuursorgaan binnen de termijn van de
// rechter bleef, toont de pagina ook; de cel dwingt het niet af. Het
// formulier en de voorvertoning zijn die van een heroverweging onder
// Bestuursorgaan (OordeelKeuze.vue, Voorvertoning.vue), ook voor een begrip
// dat de cel voor dit geval niet weet (stil.js): daarover oordeelt het
// bestuursorgaan zelf.
import { reactive } from 'vue';
import { beroepApi, binnenTermijn, openOntvangsten } from '../beroep.js';
import { alsJson, jaNee, kort, leesbaar, state, stap, waarde, zonderLeeg } from '../state.js';
import { formulier, scenarioWaarden } from '../scenarioInvoer.js';
import { stilBegrippen } from '../stil.js';
import OordeelKeuze from './OordeelKeuze.vue';
import StilOordeel from './StilOordeel.vue';
import Uitkomst from './Uitkomst.vue';
import Voorvertoning from './Voorvertoning.vue';
import Waarom from './Waarom.vue';

const toetsen = reactive({});
const vorm = formulier(() => scenarioWaarden('opnieuw_beslissen'));
const opnieuw = vorm.eigen;
const beslissingen = reactive({});
const { stil, meldStil } = stilBegrippen(opnieuw);
// De begrippen die de cel bij deze beslissing niet weet en die niet al onder
// de bevindingen van het bestreden besluit staan.
const stilNa = (o) =>
  (stil[o.id] ?? []).filter((v) => !toetsen[o.id].bevindingen.some((b) => b.naam === v.naam));

async function toon(o) {
  const antwoord = await stap(() => beroepApi.heroverweging(o.id));
  if (antwoord) {
    toetsen[o.id] = antwoord;
    vorm.wis(o.id);
  }
}

// Wat het bestuursorgaan opnieuw vindt; 'Laat staan' geeft niets.
const gegeven = (o) => zonderLeeg(vorm.waarden(o.id));

async function beslis(o) {
  const antwoord = await stap(() => beroepApi.opnieuwBeslissen(o.id, gegeven(o), state.datum));
  if (antwoord) {
    beslissingen[o.id] = antwoord;
    delete toetsen[o.id];
  }
}

const termijn = (o) => {
  const binnen = binnenTermijn(o, state.datum);
  if (binnen === null) return 'De rechter stelde geen termijn.';
  return binnen
    ? `Op de speeldatum binnen de termijn van de rechter (t/m ${o.fields.termijn_nieuw_besluit_einddatum}).`
    : `Op de speeldatum na de termijn van de rechter (t/m ${o.fields.termijn_nieuw_besluit_einddatum}); de cel weigert dat niet.`;
};
</script>

<template>
  <nldd-simple-section v-if="openOntvangsten().length || Object.keys(beslissingen).length">
    <nldd-title slot="header" size="3" text="Na een uitspraak">
      <nldd-text slot="supporting-text">
        De rechter vernietigde een beslissing op bezwaar en droeg een nieuw besluit op. Het
        bestuursorgaan beslist dan opnieuw op hetzelfde bezwaar: het heroverweegt het bestreden
        besluit zoals de eerste keer, en de nieuwe beslissing noemt de ontvangst van de uitspraak.
      </nldd-text>
    </nldd-title>
    <nldd-container gap="16">
      <nldd-card v-for="o in openOntvangsten()" :key="o.id">
        <nldd-container padding="16" gap="12">
          <nldd-title
            size="5"
            :text="`Uitspraak ontvangen op ${o.recorded_on}, over beslissing op bezwaar ${kort(o.refers_to)}`"
            :supporting-text="`${o.fields.uitspraak_strekt_tot} · ontvangen van ${o.received.from}`"
          ></nldd-title>
          <nldd-list appearance="box-base">
            <nldd-list-item v-for="(w, naam) in o.fields" :key="naam" size="sm">
              <nldd-text-cell :text="leesbaar(naam)"></nldd-text-cell>
              <nldd-text-cell width="fit-content" :text="waarde(w)"></nldd-text-cell>
            </nldd-list-item>
          </nldd-list>
          <nldd-text size="sm" color="secondary">{{ termijn(o) }}</nldd-text>
          <nldd-button
            v-if="!toetsen[o.id]"
            appearance="primary"
            text="Heroverwegen"
            @click="toon(o)"
          ></nldd-button>
          <template v-else>
            <nldd-container layout="grid" gap="16">
              <nldd-container gap="12">
                <OordeelKeuze
                  v-for="b in toetsen[o.id].bevindingen"
                  :key="b.naam"
                  :veld="b"
                  :model-value="vorm.waarden(o.id)[b.naam]"
                  :leeg="`Laat staan (${jaNee(b.eerder)})`"
                  :toelichting="`${b.basis} · eerder: ${jaNee(b.eerder)}`"
                  @update:model-value="vorm.zet(o.id, b.naam, $event)"
                />
                <StilOordeel
                  v-for="v in stilNa(o)"
                  :key="v.naam"
                  :veld="v"
                  :model-value="vorm.waarden(o.id)[v.naam]"
                  @update:model-value="vorm.zet(o.id, v.naam, $event)"
                />
                <nldd-button appearance="primary" text="Opnieuw beslissen op bezwaar" @click="beslis(o)"></nldd-button>
              </nldd-container>
              <Voorvertoning
                soort="na-uitspraak"
                :op="o.id"
                :bevindingen="gegeven(o)"
                @stil="meldStil(o.id, $event)"
              />
            </nldd-container>
          </template>
        </nldd-container>
      </nldd-card>
      <nldd-container v-for="(b, id) in beslissingen" :key="id" gap="12">
        <nldd-title
          size="5"
          :text="`Opnieuw beslist op bezwaar ${kort(b.beslissing.refers_to)}`"
          :supporting-text="b.beslissing.outputs.bestreden_besluit_herroepen ? 'Gegrond: het besluit is herroepen, er is een nieuw besluit' : 'Ongegrond: het besluit blijft'"
        ></nldd-title>
        <nldd-code-viewer language="json" appearance="box-tinted">{{ alsJson(b.beslissing.outputs) }}</nldd-code-viewer>
        <Uitkomst
          v-if="b.nieuw_besluit"
          titel="Nieuw besluit"
          :besluit="b.nieuw_besluit"
          :waarom="b.waarom"
        />
        <Waarom v-else :waarom="b.waarom" />
      </nldd-container>
    </nldd-container>
  </nldd-simple-section>
</template>
