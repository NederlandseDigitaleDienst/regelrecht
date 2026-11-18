<template>
  <nldd-collection layout="grid" item-min-width="170px" :max-items="12">
    <nldd-card v-for="tile in tiles" :key="tile.label">
      <nldd-container slot="header" padding="12" padding-bottom="0">
        <span class="mt-label">{{ tile.label }}</span>
      </nldd-container>
      <nldd-container padding="12">
        <span class="mt-value">{{ tile.value }}</span>
        <span class="mt-sub">{{ tile.sub }}</span>
      </nldd-container>
    </nldd-card>
  </nldd-collection>
</template>

<script setup>
import { computed } from 'vue';
import { number, euroCompact, decimal } from '../../lib/format.js';

const props = defineProps({
  metrics: { type: Object, default: null },
  title: { type: String, default: 'Huidig recht' },
});

const tiles = computed(() => {
  const m = props.metrics;
  const g = m?.totaal?.gemiddeld_per_jaar;
  return [
    {
      label: 'Regeling per jaar',
      value: m ? euroCompact(m.totaal.regeling_totaal / m.jaren.length) : '—',
      sub: m ? `po ${euroCompact(m.totaal.regeling_po.totaal / m.jaren.length)} · vo ${euroCompact(m.totaal.regeling_vo.totaal / m.jaren.length)}` : 'nog niet berekend',
    },
    {
      label: 'Leerlingen bekostigd',
      value: g ? number(g.leerlingen_bekostigd.totaal) : '—',
      sub: g ? `gemiddeld per jaar · po ${number(g.leerlingen_bekostigd.po)} · vo ${number(g.leerlingen_bekostigd.vo)}` : '',
    },
    {
      label: 'Onder de drempel',
      value: g ? number(g.leerlingen_onder_drempel) : '—',
      sub: g ? `leerlingen per jaar op ${number(g.scholen_onder_drempel)} scholen` : '',
    },
    {
      label: 'Uitvoeringslast per jaar',
      value: m ? euroCompact(m.totaal.uitvoeringslast.totaal / m.jaren.length) : '—',
      sub: m ? `school ${euroCompact(m.totaal.uitvoeringslast.school / m.jaren.length)} · DUO ${euroCompact(m.totaal.uitvoeringslast.duo / m.jaren.length)}` : '',
    },
    {
      label: 'Stabiliteit',
      value: m?.stabiliteit?.totaal != null ? decimal(m.stabiliteit.totaal, 2) : '—',
      sub: 'spreiding per school jaar op jaar (lager = stabieler)',
    },
    {
      label: 'Populatie',
      value: m ? number(m.gewichtTotaal) : '—',
      sub: m?.fouten?.aantal ? `${number(m.fouten.aantal)} engine-fouten` : 'gewogen leerlingen 2023-2028',
    },
  ];
});
</script>

<style scoped>
.mt-label { font-size: 0.8em; color: var(--semantics-content-secondary-color); }
.mt-value { display: block; font-size: 1.4em; font-weight: 700; font-variant-numeric: tabular-nums; }
.mt-sub { display: block; font-size: 0.75em; color: var(--semantics-content-secondary-color); margin-top: 2px; }
</style>
