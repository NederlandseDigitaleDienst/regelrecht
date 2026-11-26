<script setup>
// Invoer van de aannames (toets/aannames.js): gegevens die de kaart niet kent. Per gegeven
// onbekend, ja of nee, of een getal. Leeg is onbekend: dan geeft de toets een onthouding.
// Elke groep klapt in en uit, standaard dicht; groepen die op het
// gekozen punt meetellen staan bovenaan, met "speelt hier".
import { computed, reactive } from 'vue';
import Uitklap from './Uitklap.vue';
import { GROEPEN, aannames, aantalIngevuld, allesGunstig, allesWissen } from '../toets/aannames.js';
import { bouwplan } from '../toets/bouwplan.js';

const props = defineProps({
  // Namen van parameters die op het punt ontbreken of als aanname zijn gelezen.
  spelen: { type: Object, default: () => new Set() },
});
const emit = defineEmits(['uitleg']);

const speelt = (v) => v.namen.some((n) => props.spelen.has(n));
const groepen = computed(() => GROEPEN
  .map((g) => ({ ...g, hier: g.velden.some(speelt), ingevuld: g.velden.filter((v) => aannames[v.id] !== undefined).length }))
  .sort((a, b) => Number(b.hier) - Number(a.hier)));
const ingevuld = computed(() => aantalIngevuld());
const open = reactive(Object.fromEntries(GROEPEN.map((g) => [g.id, false])));
const uitlegOpen = reactive({ v: false });
const alleOpen = computed(() => GROEPEN.every((g) => open[g.id]));
function allesKlappen() {
  const naar = !alleOpen.value;
  for (const g of GROEPEN) open[g.id] = naar;
}

function jaNee(v, w) {
  if (w === 'onbekend') delete aannames[v.id];
  else aannames[v.id] = w === 'ja';
}
const jaNeeWaarde = (v) => (aannames[v.id] === true ? 'ja' : aannames[v.id] === false ? 'nee' : 'onbekend');
function wis(v) { delete aannames[v.id]; }
function getal(v, e) {
  // Een geleegd veld geeft null, en Number(null) is 0: leeg is geen antwoord.
  if (e.target.value === null || e.target.value === '') return;
  const n = Number(e.target.value);
  if (Number.isFinite(n) && n >= 0) aannames[v.id] = n;
}
</script>

<template>
  <nldd-container layout="stack" gap="8" class="aannames">
    <div class="knoppen">
      <nldd-button size="sm" appearance="neutral-tinted" text="Alles gunstig" @click="allesGunstig(aannames, bouwplan)"></nldd-button>
      <nldd-button size="sm" appearance="neutral-transparent" text="Alles wissen" :disabled="!ingevuld || undefined" @click="allesWissen()"></nldd-button>
      <span class="ruimte"></span>
      <nldd-button size="sm" appearance="neutral-transparent" :start-icon="alleOpen ? 'chevron-up' : 'chevron-down'" :text="alleOpen ? 'Alles inklappen' : 'Alles openklappen'" @click="allesKlappen"></nldd-button>
    </div>

    <Uitklap v-model:open="uitlegOpen.v" soort="groep" titel="Hoe werkt dit?">
      <nldd-text size="xs" color="secondary">
        <p>
          Leeg is onbekend; dan geeft de toets een
          <a href="#" @click.prevent="emit('uitleg', 'onthouding')">onthouding</a>.
          Wat je invult is een aanname, geen feit. Toets, Graaf en de kaartlaag Uitkomst rekenen er ook mee.
        </p>
      </nldd-text>
      <nldd-text size="xs" color="secondary">
        <p>
          "Alles gunstig" vult ja, nee en getallen in met de waarde die de regel laat slagen; de grens staat bij elk getal.
          Getallen die van het bouwplan afhangen (bestaande woningen, bestaande hoogte, sociale woningen) volgen het bouwplan
          van dit moment. Een paar blijven leeg: daar volstaat een andere route al.
        </p>
      </nldd-text>
      <nldd-text size="xs" color="secondary">
        <p>
          "Speelt hier": daar vroeg de engine op dit punt om. Sommige vragen komen pas na een antwoord:
          bodemkwaliteit telt alleen bij een bodemgevoelige locatie.
        </p>
      </nldd-text>
    </Uitklap>

    <Uitklap v-for="g in groepen" :key="g.id" v-model:open="open[g.id]" soort="groep" :titel="g.kop" :sub="g.bron">
      <template #kop>
        <nldd-tag v-if="g.hier" size="sm" color="accent" text="speelt hier"></nldd-tag>
        <nldd-text size="xs" color="secondary"><p>{{ g.ingevuld }}/{{ g.velden.length }}</p></nldd-text>
      </template>
      <div v-for="v in g.velden" :key="v.id" class="veld" :class="{ hier: speelt(v) }">
        <div class="label">
          <nldd-text size="sm"><p>{{ v.tekst }}</p></nldd-text>
          <nldd-text v-if="v.uitleg" size="xs" color="secondary"><p>{{ v.uitleg }}</p></nldd-text>
        </div>
        <nldd-segmented-control
          v-if="v.soort === 'ja-nee'"
          size="sm" :value="jaNeeWaarde(v)" :accessible-label="v.tekst"
          @change="jaNee(v, $event.target.value)"
        >
          <nldd-segmented-control-item value="onbekend" text="?" :selected="jaNeeWaarde(v) === 'onbekend' || undefined"></nldd-segmented-control-item>
          <nldd-segmented-control-item value="ja" text="Ja" :selected="jaNeeWaarde(v) === 'ja' || undefined"></nldd-segmented-control-item>
          <nldd-segmented-control-item value="nee" text="Nee" :selected="jaNeeWaarde(v) === 'nee' || undefined"></nldd-segmented-control-item>
        </nldd-segmented-control>
        <div v-else class="getal">
          <template v-if="aannames[v.id] !== undefined">
            <nldd-number-field :value="aannames[v.id]" min="0" max="100000" step="1" size="sm" width="96px" :accessible-label="v.tekst" @change="getal(v, $event)"></nldd-number-field>
            <nldd-text size="xs" color="secondary"><p>{{ v.eenheid }}</p></nldd-text>
            <nldd-icon-button size="sm" appearance="neutral-transparent" icon="close-small" text="Wissen, terug naar onbekend" @click="wis(v)"></nldd-icon-button>
          </template>
          <nldd-button v-else size="sm" appearance="neutral-transparent" start-icon="plus-small" text="Invullen" @click="aannames[v.id] = 0"></nldd-button>
        </div>
      </div>
    </Uitklap>
  </nldd-container>
</template>

<style scoped>
.knoppen { display: flex; flex-wrap: wrap; gap: 6px; align-items: center; }
.ruimte { flex: 1; }
.veld {
  display: grid; grid-template-columns: 1fr auto; gap: 4px 12px; align-items: center;
  padding: 6px 0; border-top: 1px solid color-mix(in srgb, var(--primitives-color-neutral-400, #999) 22%, transparent);
}
.veld.hier .label { border-left: 3px solid #154273; padding-left: 6px; }
@media (prefers-color-scheme: dark) { .veld.hier .label { border-left-color: #8fb8de; } }
.label { display: flex; flex-direction: column; }
.getal { display: inline-flex; align-items: center; gap: 4px; }
</style>
