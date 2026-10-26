<script setup>
// Hoe de cel tot een antwoord kwam (RFC-048), één keer dat het besluit het
// vroeg (Cell::explain_answer): de reductie opnieuw gedraaid op
// de kroniek zoals het besluit haar las, wat haar uitvoer werd, wat zij uit
// andere regelgeving las (zoals de regio van een school uit de regeling), en
// haar eigen spoor, met welke besluiten meetelden en welke niet. Gaf de cel
// binnen die reductie zelf weer een antwoord (zoals de plaats van
// vestiging), dan staat dat eronder, met op verzoek hoe zij dat deed.
// Gedeeld door de uitleg bij een besluit (Waarom) en het logboek.
import { reactive } from 'vue';
import { api } from '../api.js';
import { kort, leesbaar, rol, state } from '../state.js';
import { toon } from '../logboek.js';

const props = defineProps({
  // Eén antwoord uit `api.hoeBeantwoord`.
  uitleg: { type: Object, required: true },
  // Het besluit waarin de cel antwoordde; nodig om een begrip binnen de
  // reductie uit te leggen.
  besluit: { type: String, default: null },
});

/** Een rij uit de kroniek (met haar `id`) als het chronolexogram: `executogram 8f958fef`. */
const rij = (v) => {
  const g = state.grammen.find((x) => x.id === v.id);
  return `${g ? rol(g).toLowerCase() : 'chronolexogram'} ${kort(v.id)}`;
};

const isRij = (v) => v && typeof v === 'object' && typeof v.id === 'string';

/**
 * Een uitvoer van de reductie: een lijst ids als besluiten, een lijst rijen
 * (zoals de geldende besluiten van de Awb-reductie) en een rij uit de kroniek
 * (zoals de vestiging) als die chronolexogrammen, de rest als ja/nee of
 * waarde.
 */
const toonWaarde = (v) => {
  if (Array.isArray(v))
    return v.length ? v.map((x) => (isRij(x) ? rij(x) : `besluit ${kort(x)}`)).join(', ') : 'geen';
  if (isRij(v)) return rij(v);
  return toon(v);
};

// Per begrip binnen de reductie de uitleg van de cel, als die gevraagd is.
const genest = reactive({});
const open = reactive({});

async function uitklappen(a, i) {
  open[i] = !open[i];
  if (!open[i] || genest[i]) return;
  try {
    const antwoord = await api.hoeBeantwoord(props.besluit, a.antwoord);
    const uitleg = antwoord.antwoorden[a.keer];
    if (!uitleg) throw new Error(`De cel gaf geen uitleg bij ${leesbaar(a.begrip)}.`);
    genest[i] = uitleg;
  } catch (e) {
    state.fout = e.message;
    open[i] = false;
  }
}
</script>

<template>
  <nldd-container gap="8">
    <nldd-text size="sm" color="secondary">
      De reductie {{ uitleg.reductie }} (geldig vanaf {{ uitleg.geldig_vanaf }}) opnieuw
      gedraaid, op de kroniek zoals het besluit haar las.
    </nldd-text>
    <nldd-list appearance="box-base" accessible-label="Uitkomst van de reductie">
      <nldd-list-item v-for="(v, naam) in uitleg.uitvoer" :key="naam" size="sm">
        <nldd-text-cell :text="leesbaar(naam)"></nldd-text-cell>
        <nldd-text-cell width="fit-content" :text="toonWaarde(v)"></nldd-text-cell>
      </nldd-list-item>
    </nldd-list>
    <template v-if="uitleg.uit_regelgeving?.length">
      <nldd-text size="sm" color="secondary">Gelezen uit andere regelgeving:</nldd-text>
      <nldd-list appearance="box-base" accessible-label="Gelezen uit andere regelgeving">
        <nldd-list-item v-for="g in uitleg.uit_regelgeving" :key="g.naam" size="sm">
          <nldd-text-cell :text="leesbaar(g.naam)" :supporting-text="g.regelwerk"></nldd-text-cell>
          <nldd-text-cell width="fit-content" :text="toon(g.waarde)"></nldd-text-cell>
        </nldd-list-item>
      </nldd-list>
    </template>
    <nldd-container v-for="(a, i) in uitleg.antwoorden ?? []" :key="`${a.begrip}#${a.keer}`" gap="8">
      <nldd-inline-dialog
        icon="info"
        :text="`${leesbaar(a.begrip)}: ${toon(a.waarde)}`"
        :supporting-text="`Binnen deze reductie ingevuld met antwoord ${a.antwoord} van de cel, uit reductie ${a.reductie} (geldig vanaf ${a.geldig_vanaf}).`"
      ></nldd-inline-dialog>
      <template v-if="besluit">
        <nldd-button
          appearance="secondary"
          size="sm"
          expandable
          :expanded="!!open[i]"
          text="Hoe de cel dit beantwoordde"
          @click="uitklappen(a, i)"
        ></nldd-button>
        <HoeBeantwoord v-if="open[i] && genest[i]" :uitleg="genest[i]" :besluit="besluit" />
      </template>
    </nldd-container>
    <nldd-text size="sm" color="secondary">
      Het spoor van de reductie, met daarin het beleid en de regeling die zij aanroept: bij
      FOREACH staat hoeveel besluiten meetelden en hoeveel het filter oversloeg.
    </nldd-text>
    <nldd-code-viewer appearance="box-tinted">{{ uitleg.spoor }}</nldd-code-viewer>
  </nldd-container>
</template>
