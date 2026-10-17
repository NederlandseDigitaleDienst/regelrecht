<script setup>
// Waarom een besluit zo uitviel, zoals de cel het uitlegt (Cell::explain).
// Eerst wie elk open begrip invulde: het hoofd (een bevinding), een regeling
// (het beleid van de school) of de school zelf uit haar kroniek. Een begrip
// dat de school beantwoordde, staat in het spoor als één stap; hoe zij tot
// dat antwoord kwam, is haar eigen verhaal en komt op verzoek
// (Cell::explain_answer): het spoor van haar reductie, met welke besluiten
// meetelden en welke niet. Daaronder het spoor van de engine.
import { reactive } from 'vue';
import { api } from '../api.js';
import { kort, leesbaar, spel, waarde } from '../spel.js';

const props = defineProps({
  // Het besluit waarvan dit de uitleg is: bij een beslissing op bezwaar de
  // beslissing zelf, want daarin wordt artikel 13a opnieuw toegepast.
  besluit: { type: String, required: true },
  waarom: { type: Object, required: true },
});

// Een stil antwoord (null) laat de engine over aan de bevinding van het hoofd.
const jaNee = (v) =>
  v === true ? 'ja' : v === false ? 'nee' : v == null ? 'geen antwoord (het hoofd beslist)' : waarde(v);

/** Wie het open begrip invulde, in een zin. */
function door(f) {
  const by = f.filled_by;
  if (by === 'finding') return 'bevinding van het hoofd';
  if (by?.regulation) return `regeling ${by.regulation.law_id}, artikel ${by.regulation.article}`;
  if (by?.answer) return `de school uit haar kroniek (reductie ${by.answer.law_id})`;
  return JSON.stringify(by);
}

/** De herkomst van het antwoord van de school, leesbaar. */
const herkomst = (a) =>
  `Ingevuld door de school met reductie ${a.reductie} (geldig vanaf ${a.geldig_vanaf}). ` +
  (a.tot_en_met
    ? `Gelezen: ${a.gelezen} chronolexogram(men), tot en met ${soortVan(a.tot_en_met)} ${kort(a.tot_en_met)}.`
    : 'Nog niets uit de kroniek gelezen.');

const soortVan = (id) => {
  const g = spel.grammen.find((x) => x.id === id);
  return g ? g.kind.toLowerCase() : 'chronolexogram';
};

// Per begrip de uitleg van de school, als die gevraagd is. Vroeg het besluit
// een begrip vaker, dan geeft de cel per keer een uitleg, in de volgorde van
// het spoor; het i-de antwoord hoort bij de i-de keer (`keer`).
const hoe = reactive({});
const open = reactive({});

const keer = (i) =>
  props.waarom.antwoorden.slice(0, i).filter((a) => a.begrip === props.waarom.antwoorden[i].begrip)
    .length;

async function toon(i) {
  const begrip = props.waarom.antwoorden[i].begrip;
  open[i] = !open[i];
  if (!open[i] || hoe[begrip]) return;
  try {
    hoe[begrip] = await api.hoeBeantwoord(props.besluit, begrip);
  } catch (e) {
    spel.fout = e.message;
    open[i] = false;
  }
}

const uitlegBij = (i) => {
  const h = hoe[props.waarom.antwoorden[i].begrip];
  return h ? h.antwoorden.slice(keer(i), keer(i) + 1) : [];
};

/** Een uitvoer van de reductie: een lijst ids als besluiten, de rest als ja/nee of waarde. */
const toonWaarde = (v) =>
  Array.isArray(v) ? (v.length ? v.map((id) => `besluit ${kort(id)}`).join(', ') : 'geen') : jaNee(v);
</script>

<template>
  <nldd-container gap="12">
    <nldd-text size="sm" color="secondary">Waarom: wie elk open begrip invulde.</nldd-text>
    <nldd-list appearance="box-base" accessible-label="Open begrippen">
      <nldd-list-item v-for="f in waarom.ingevuld" :key="`${f.law_id}#${f.article}#${f.term}`" size="sm">
        <nldd-text-cell
          :text="leesbaar(f.term)"
          :supporting-text="`${f.law_id} artikel ${f.article} · ${door(f)}`"
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
        text="Hoe de school dit beantwoordde"
        @click="toon(i)"
      ></nldd-button>
      <nldd-container v-if="open[i]" gap="8">
        <nldd-container v-for="h in uitlegBij(i)" :key="h.spoor" gap="8">
          <nldd-text size="sm" color="secondary">
            De reductie {{ h.reductie }} (geldig vanaf {{ h.geldig_vanaf }}) opnieuw gedraaid, op de
            kroniek zoals het besluit haar las.
          </nldd-text>
          <nldd-list appearance="box-base" accessible-label="Uitkomst van de reductie">
            <nldd-list-item v-for="(v, naam) in h.uitvoer" :key="naam" size="sm">
              <nldd-text-cell :text="leesbaar(naam)"></nldd-text-cell>
              <nldd-text-cell width="fit-content" :text="toonWaarde(v)"></nldd-text-cell>
            </nldd-list-item>
          </nldd-list>
          <nldd-text size="sm" color="secondary">
            Het spoor van de reductie: bij FOREACH staat hoeveel besluiten meetelden en hoeveel
            het filter oversloeg.
          </nldd-text>
          <nldd-code-viewer appearance="box-tinted">{{ h.spoor }}</nldd-code-viewer>
        </nldd-container>
      </nldd-container>
    </nldd-container>

    <nldd-text size="sm" color="secondary">
      Het spoor van de engine, met de waarden die het besluit gebruikte. Het antwoord van de
      school is daarin één stap.
    </nldd-text>
    <nldd-code-viewer appearance="box-tinted">{{ waarom.spoor }}</nldd-code-viewer>
  </nldd-container>
</template>
