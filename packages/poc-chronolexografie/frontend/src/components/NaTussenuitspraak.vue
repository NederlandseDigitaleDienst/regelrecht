<script setup>
// Na een tussenuitspraak (Awb 8:51a, 8:80a): de rechter zag een gebrek in de
// beslissing op bezwaar en gaf het bestuursorgaan de gelegenheid het te
// herstellen. Het bestuursorgaan herstelt met een nieuwe beslissing op
// hetzelfde bezwaar (OpnieuwBeslissen.vue), herstelt zonder nieuw besluit,
// of laat het na. Daarna deelt het de rechtbank mee of en hoe (Awb 8:51b).
// Een nieuwe beslissing gaat met de mededeling mee (Awb 6:19, derde lid):
// het lopende beroep gaat er van rechtswege ook over. Wat de mededeling
// verder houdt, zegt de wet (`state.vormen.mededeling`). De termijn van de
// rechter toont de pagina; de cel dwingt hem niet af.
import { computed, reactive } from 'vue';
import { beroepApi, besluitNa, binnenTermijn, openTussenuitspraken } from '../beroep.js';
import { kort, leesbaar, state, stap, waarde, zonderLeeg } from '../state.js';
import { formulier, scenarioWaarden } from '../scenarioInvoer.js';
import OpnieuwBeslissen from './OpnieuwBeslissen.vue';
import OpnieuwBeslist from './OpnieuwBeslist.vue';
import VeldInvoer from './VeldInvoer.vue';

const beslissingen = reactive({});
const velden = computed(() => state.vormen?.mededeling?.velden ?? []);
const vorm = formulier(() => scenarioWaarden('mededelen'));
const rechtbank = computed(() => state.casus.rechter?.cel_naam ?? 'de rechtbank');

// Wat de tussenuitspraak zegt over het gebrek en het herstel; leeg blijft weg.
const inhoud = (o) =>
  Object.entries(o.fields ?? {})
    .filter(([, w]) => w !== null && typeof w !== 'boolean')
    .map(([naam, w]) => ({ naam, w }));

const termijn = (o) => {
  const binnen = binnenTermijn(o, state.datum, 'termijn_herstel_einddatum');
  if (binnen === null) return 'De rechter stelde geen termijn.';
  const tot = o.fields.termijn_herstel_einddatum;
  return binnen
    ? `Op de speeldatum binnen de termijn voor het herstel (t/m ${tot}).`
    : `Op de speeldatum na de termijn voor het herstel (t/m ${tot}); de cel weigert dat niet.`;
};

async function meedelen(o) {
  // Met een nieuwe beslissing houdt de mededeling die beslissing, en verder niets.
  const gegeven = besluitNa(o.id) ? {} : zonderLeeg(vorm.waarden(o.id));
  const antwoord = await stap(() => beroepApi.meedelen(o.id, gegeven, state.datum));
  if (antwoord) vorm.wis(o.id);
}
</script>

<template>
  <nldd-simple-section v-if="openTussenuitspraken().length || Object.keys(beslissingen).length">
    <nldd-title slot="header" size="3" text="Na een tussenuitspraak">
      <nldd-text slot="supporting-text">
        De rechter zag een gebrek in een beslissing op bezwaar en gaf het bestuursorgaan de
        gelegenheid het te herstellen (Awb 8:51a). Het bestuursorgaan kan het herstellen met een
        nieuwe beslissing op hetzelfde bezwaar, of zonder nieuw besluit, zoals met een aanvulling op
        de motivering. Daarna deelt het de rechtbank mee of en hoe het herstelde (Awb 8:51b). Een
        nieuwe beslissing valt onder het beroep dat al loopt (Awb 6:19).
      </nldd-text>
    </nldd-title>
    <nldd-container gap="16">
      <nldd-card v-for="o in openTussenuitspraken()" :key="o.id">
        <nldd-container padding="16" gap="12">
          <nldd-title
            size="5"
            :text="`Tussenuitspraak ontvangen op ${o.recorded_on}, over beslissing op bezwaar ${kort(o.refers_to)}`"
            :supporting-text="`ontvangen van ${o.received.from}`"
          ></nldd-title>
          <nldd-list appearance="box-base">
            <nldd-list-item v-for="r in inhoud(o)" :key="r.naam" size="sm">
              <nldd-text-cell :text="leesbaar(r.naam)"></nldd-text-cell>
              <nldd-text-cell :text="typeof r.w === 'string' ? r.w : waarde(r.w)"></nldd-text-cell>
            </nldd-list-item>
          </nldd-list>
          <nldd-text size="sm" color="secondary">{{ termijn(o) }}</nldd-text>
          <template v-if="!besluitNa(o.id)">
            <nldd-title size="6" text="Herstellen met een nieuwe beslissing"></nldd-title>
            <OpnieuwBeslissen :ontvangst="o" @genomen="beslissingen[o.id] = $event" />
          </template>
          <nldd-title size="6" :text="`Meedelen aan ${rechtbank}`"></nldd-title>
          <nldd-text v-if="besluitNa(o.id)" size="sm">
            De nieuwe beslissing {{ kort(besluitNa(o.id).id) }} gaat met de mededeling mee (Awb 6:19,
            derde lid).
          </nldd-text>
          <template v-else>
            <nldd-text size="sm" color="secondary">
              Herstelt het bestuursorgaan zonder nieuw besluit, dan zegt het hoe. Laat je het veld
              leeg, dan deelt het mee dat het de gelegenheid niet gebruikt.
            </nldd-text>
            <VeldInvoer
              v-for="v in velden"
              :key="v.naam"
              :veld="v"
              :model-value="vorm.waarden(o.id)[v.naam]"
              @update:model-value="vorm.zet(o.id, v.naam, $event)"
            />
          </template>
          <nldd-button appearance="primary" :text="`Meedelen aan ${rechtbank}`" @click="meedelen(o)"></nldd-button>
        </nldd-container>
      </nldd-card>
      <OpnieuwBeslist v-for="(b, id) in beslissingen" :key="id" :antwoord="b" />
    </nldd-container>
  </nldd-simple-section>
</template>
