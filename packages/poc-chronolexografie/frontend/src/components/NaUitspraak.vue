<script setup>
// Opnieuw beslissen op een bezwaar na een uitspraak (RFC-053, Awb 8:72 lid
// 4). Droeg de rechter een nieuw besluit op, dan beslist het bestuursorgaan
// opnieuw op hetzelfde bezwaar (OpnieuwBeslissen.vue). De aanwijzingen van de
// rechter dwingt de cel niet af, de pagina toont ze. Of het bestuursorgaan
// binnen de termijn van de rechter bleef, toont de pagina ook; de cel dwingt
// het niet af.
import { reactive } from 'vue';
import { binnenTermijn, openOntvangsten } from '../beroep.js';
import { kort, leesbaar, state, waarde } from '../state.js';
import OpnieuwBeslissen from './OpnieuwBeslissen.vue';
import OpnieuwBeslist from './OpnieuwBeslist.vue';

const beslissingen = reactive({});

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
          <OpnieuwBeslissen :ontvangst="o" @genomen="beslissingen[o.id] = $event" />
        </nldd-container>
      </nldd-card>
      <OpnieuwBeslist v-for="(b, id) in beslissingen" :key="id" :antwoord="b" />
    </nldd-container>
  </nldd-simple-section>
</template>
