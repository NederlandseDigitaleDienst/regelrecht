<script setup>
// Kerncijfers: hoe vaak de sporen hetzelfde zeggen, de scenario's, en wat niet meetelt.
import { computed } from 'vue';
import { kortId } from '../data.js';

const props = defineProps({ data: Object });
const raster = computed(() => props.data.cellen.filter((c) => !c.extra));
const vergelijkbaar = computed(() => raster.value.filter((c) => c.oud && !['bron', 'open'].includes(c.oud.geldend)));
// Zelfde oordeel: beide toegestaan of beide niet toegestaan (zoals in het verslag, paragraaf 3).
const gelijk = computed(() => vergelijkbaar.value.filter((c) => c.oud.geldend === c.dso.geldend && ['toegestaan', 'niet'].includes(c.oud.geldend)).length);
const naam = { toegestaan: 'Toegestaan', niet: 'Niet toegestaan', onthouding: 'Onthouding', bron: 'Alleen scan (Het Ambacht 2009)', open: 'Rekent hier niet' };
const overgang = computed(() => {
  const m = {};
  for (const c of raster.value) {
    const k = `${naam[c.oud?.geldend] ?? 'geen vak'} → ${naam[c.dso.geldend] ?? 'geen'}`;
    m[k] = (m[k] ?? 0) + 1;
  }
  return Object.entries(m).sort((a, b) => b[1] - a[1]);
});
const sc = computed(() => props.data.scenarios);
const be = computed(() => props.data.besluiten ?? []);
const goed = (lijst, s) => lijst.filter((r) => r[s].uitkomst_klopt).length;
const som = (f) => sc.value.reduce((a, r) => a + f(r), 0);
const CIJFERS = computed(() => [
  { waarde: `${gelijk.value} van ${vergelijkbaar.value.length}`, tekst: 'vakken met hetzelfde oordeel van de gemeente (geldend recht)' },
  { waarde: `${goed(sc.value, 'oud')} / ${goed(sc.value, 'dso')}`, tekst: `van ${sc.value.length} woontoren-scenario's goed, vóór / met DSO` },
  { waarde: `${goed(be.value, 'oud')} / ${goed(be.value, 'dso')}`, tekst: `van ${be.value.length} besluit-scenario's goed, vóór / met DSO` },
  { waarde: `${som((r) => r.oud.artikelen.gerekend)} / ${som((r) => r.dso.artikelen.in_lijst)}`, tekst: `van ${som((r) => r.dso.artikelen.verwacht)} verwachte gemeentelijke artikelen, vóór (gerekend) / met DSO (gevonden)` },
  { waarde: `0 / ${som((r) => r.dso.andere.gevonden)}`, tekst: `van ${som((r) => r.dso.andere.verwacht)} verwachte regels van andere overheden, vóór / met DSO` },
]);
</script>

<template>
  <nldd-container layout="stack" gap="16">
    <nldd-title size="display-5" :heading-level="2" text="Kerncijfers" supporting-text="Vóór DSO is de eerste PoC; met DSO is het DSO-spoor. Zelfde bouwplan, zelfde engine."></nldd-title>
    <div class="cijfers">
      <div v-for="c in CIJFERS" :key="c.tekst">
        <nldd-text size="display-3">{{ c.waarde }}</nldd-text>
        <nldd-text size="xs" color="secondary">{{ c.tekst }}</nldd-text>
      </div>
    </div>

    <nldd-title size="display-6" :heading-level="3" text="Overgang per vak, geldend recht" supporting-text="Oordeel van de gemeente vóór DSO → met DSO."></nldd-title>
    <nldd-list appearance="simple" accessible-label="Overgang per vak">
      <nldd-list-item v-for="[k, n] in overgang" :key="k" size="sm">
        <nldd-text-cell :text="k"></nldd-text-cell>
        <nldd-cell width="fit-content"><nldd-text size="sm">{{ n }}</nldd-text></nldd-cell>
      </nldd-list-item>
    </nldd-list>

    <nldd-title size="display-6" :heading-level="3" text="Testdata in de pre-productieomgeving, telt niet mee"></nldd-title>
    <nldd-list appearance="simple" accessible-label="Uitgesloten regelingen">
      <nldd-list-item v-for="(v, k) in data.uitgesloten" :key="k" size="sm">
        <nldd-text-cell :text="kortId(k)" :supporting-text="v"></nldd-text-cell>
      </nldd-list-item>
    </nldd-list>
  </nldd-container>
</template>

<style scoped>
.cijfers { display: grid; grid-template-columns: repeat(auto-fit, minmax(170px, 1fr)); gap: 8px; }
.cijfers > div { display: flex; flex-direction: column; gap: 2px; border: 1px solid color-mix(in srgb, var(--primitives-color-neutral-400, #999) 40%, transparent); border-radius: 8px; padding: 10px 12px; }
</style>
