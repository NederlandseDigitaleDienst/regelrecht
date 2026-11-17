<template>
  <nldd-collection layout="grid" item-min-width="170px" :max-items="12">
    <nldd-card v-for="tile in tiles" :key="tile.label">
      <nldd-container slot="header" padding="12" padding-bottom="0">
        <span class="mt-label">{{ tile.label }}</span>
      </nldd-container>
      <nldd-container padding="12">
        <span class="mt-value">{{ tile.value }}</span>
        <span v-if="tile.delta" class="mt-delta" :class="tile.richting">{{ tile.delta }} t.o.v. huidig recht</span>
        <span class="mt-sub">{{ tile.sub }}</span>
      </nldd-container>
    </nldd-card>
  </nldd-collection>
</template>

<!--
  Vier kengetallen voor één kolom: de werkversie. Bewust niet één tegel per
  kolom, want vier kolommen keer vier kengetallen is een muur van zestien
  getallen die precies herhaalt wat de postentabel erboven al toont. De tegels
  zeggen daarom expliciet over welke kolom ze gaan, met het verschil met huidig
  recht eronder; de grafiek eronder toont wél alle kolommen naast elkaar.
-->

<script setup>
import { computed } from 'vue';
import { percent, number, euroCompact } from '../../lib/format.js';

const props = defineProps({
  metrics: { type: Object, default: null },
  baselineMetrics: { type: Object, default: null },
  kolomLabel: { type: String, default: '' },
});

/** Is dit dezelfde kolom als huidig recht? Dan heeft een verschil geen zin. */
const heeftVergelijking = computed(
  () => props.metrics && props.baselineMetrics && props.metrics !== props.baselineMetrics,
);

/**
 * Verschil met huidig recht, opgemaakt. `beter` bepaalt of een stijging goed
 * nieuws is, net als in de postentabel.
 */
function verschil(waarde, basis, opmaak, beter) {
  if (!heeftVergelijking.value || waarde == null || basis == null) return { delta: '', richting: '' };
  const d = waarde - basis;
  if (Math.abs(d) < 1e-9) return { delta: '', richting: '' };
  const goed = beter === 'lager' ? d < 0 : d > 0;
  return {
    delta: `${d > 0 ? '+' : '−'}${opmaak(Math.abs(d))}`,
    richting: goed ? 'mt-goed' : 'mt-slecht',
  };
}

const tiles = computed(() => {
  const m = props.metrics;
  const b = props.baselineMetrics;
  const kolom = props.kolomLabel ? `onder ${props.kolomLabel}` : '';
  return [
    {
      label: 'Betalingsproblemen',
      value: m ? percent(m.pctBetalingsprobleem, 0) : '—',
      sub: m ? `${number(m.aantalBetalingsprobleem)} debiteuren ${kolom}`.trim() : 'nog niet berekend',
      ...verschil(m?.pctBetalingsprobleem, b?.pctBetalingsprobleem, (v) => percent(v, 1), 'lager'),
    },
    {
      label: 'Levenslang debiteur',
      value: m ? number(m.aantalLevenslang) : '—',
      sub: `bereiken het einde niet ${kolom}`.trim(),
      ...verschil(m?.aantalLevenslang, b?.aantalLevenslang, number, 'lager'),
    },
    {
      label: 'Kwijtschelding (kosten-proxy)',
      value: m ? euroCompact(m.kwijtgescholdenTotaal) : '—',
      sub: `gewogen naar cohortgrootte ${kolom}`.trim(),
      ...verschil(m?.kwijtgescholdenTotaal, b?.kwijtgescholdenTotaal, euroCompact, 'lager'),
    },
    {
      label: 'Debiteuren',
      value: m ? number(m.aantalDebiteuren) : '—',
      sub: 'gewogen totaal, in elke kolom gelijk',
    },
  ];
});
</script>

<style scoped>
.mt-label { font-size: 0.8em; color: var(--semantics-content-secondary-color); }
.mt-value { display: block; font-size: 1.5em; font-weight: 700; font-variant-numeric: tabular-nums; }
.mt-delta { display: block; font-size: 0.75em; font-variant-numeric: tabular-nums; margin-top: 2px; }
.mt-goed { color: var(--semantics-content-success-color); }
.mt-slecht { color: var(--semantics-content-critical-color); }
.mt-sub { display: block; font-size: 0.75em; color: var(--semantics-content-secondary-color); margin-top: 2px; }
</style>
