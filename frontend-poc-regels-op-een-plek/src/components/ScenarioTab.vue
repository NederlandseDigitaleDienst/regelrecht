<script setup>
// Scenario's: twee reeksen, als lijst in de stijl van Testpunten in de eerste PoC. Klik gaat naar de plek.
import { computed } from 'vue';
import { REEKSEN, TAG, TEKST, UIT_KLASSE } from '../data.js';

const reeks = defineModel('reeks', { type: String, default: 'woontoren' });
const props = defineProps({ data: Object, gekozen: String });
const emit = defineEmits(['kies']);

const r = computed(() => REEKSEN.find((x) => x.id === reeks.value));
const lijst = computed(() => (reeks.value === 'besluiten' ? props.data.besluiten : props.data.scenarios) ?? []);
const tel = (spoor) => lijst.value.filter((s) => s[spoor].uitkomst_klopt).length;
const buiten = computed(() => lijst.value.filter((s) => s.buiten_bereik).length);
const k = (u) => UIT_KLASSE[u] ?? u;
const SOORT = { verleend: 'verleend', BOPA: 'BOPA', geweigerd: 'geweigerd' };
const soort = (s) => (s.besluit?.match(/\((verleend|BOPA|geweigerd)/)?.[1]) ?? null;
</script>

<template>
  <nldd-container layout="stack" gap="12">
    <nldd-segmented-control :value="reeks" size="sm" accessible-label="Reeks scenario's" @change="reeks = $event.target.value">
      <nldd-segmented-control-item v-for="x in REEKSEN" :key="x.id" :value="x.id" :text="`${x.kort} (${(x.id === 'besluiten' ? data.besluiten : data.scenarios)?.length ?? 0})`" :selected="reeks === x.id || undefined"></nldd-segmented-control-item>
    </nldd-segmented-control>
    <nldd-title size="display-5" :heading-level="2" :text="r.titel" :supporting-text="r.uitleg"></nldd-title>

    <div class="cijfers">
      <div><nldd-text size="xs" color="secondary"><p>Vóór DSO goed</p></nldd-text><nldd-text size="display-3"><p>{{ tel('oud') }} van {{ lijst.length }}</p></nldd-text></div>
      <div><nldd-text size="xs" color="secondary"><p>Met DSO goed</p></nldd-text><nldd-text size="display-3"><p>{{ tel('dso') }} van {{ lijst.length }}</p></nldd-text></div>
      <div v-if="buiten"><nldd-text size="xs" color="secondary"><p>Buiten bereik van beide</p></nldd-text><nldd-text size="display-3"><p>{{ buiten }}</p></nldd-text></div>
    </div>

    <nldd-list appearance="simple" accessible-label="Scenario's">
      <nldd-list-item v-for="(s, i) in lijst" :key="s.naam" button :selected="s.naam === gekozen || undefined" @click="emit('kies', s)">
        <nldd-cell width="fit-content"><span class="nr">{{ r.voorvoegsel }}{{ i + 1 }}</span></nldd-cell>
        <nldd-title-cell size="display-6" :text="s.naam" :supporting-text="`Verwacht: ${s.verwacht.uitkomst}${soort(s) ? ` · besluit: ${SOORT[soort(s)]}` : ''}`"></nldd-title-cell>
        <nldd-cell width="fit-content">
          <div class="sporen">
            <span class="spoor"><nldd-text size="xs" color="secondary"><span>vóór</span></nldd-text>
              <nldd-tag :color="s.oud.uitkomst_klopt ? 'success' : (TAG[k(s.oud.uitkomst)] === 'critical' ? 'critical' : 'warning')" :text="TEKST[k(s.oud.uitkomst)] ?? s.oud.uitkomst ?? s.oud.status ?? 'geen'"></nldd-tag></span>
            <span class="spoor"><nldd-text size="xs" color="secondary"><span>met DSO</span></nldd-text>
              <nldd-tag :color="s.dso.uitkomst_klopt ? 'success' : (TAG[k(s.dso.uitkomst)] === 'critical' ? 'critical' : 'warning')" :text="TEKST[k(s.dso.uitkomst)] ?? s.dso.uitkomst ?? s.dso.status ?? 'geen'"></nldd-tag></span>
          </div>
        </nldd-cell>
      </nldd-list-item>
    </nldd-list>
    <nldd-text size="sm" color="secondary">
      <p>
        Groen: zoals verwacht. Anders oranje of rood. Klik op een scenario om naar de plek te gaan; het nummer staat ook op de kaart.
        <template v-if="reeks === 'woontoren'"> Het verbod van de provincie (3.51) telde in de eerste run niet mee door een fout; na de reparatie geeft "alle overheden" bij een diepe fundering nee.</template>
        <template v-else> Een BOPA betekent: buiten het plan afgeweken, dus volgens het plan niet toegestaan. Omzetten naar wonen kent geen van beide sporen.</template>
      </p>
    </nldd-text>
  </nldd-container>
</template>

<style scoped>
.cijfers { display: grid; grid-template-columns: repeat(auto-fit, minmax(140px, 1fr)); gap: 8px; }
.cijfers > div { border: 1px solid color-mix(in srgb, var(--primitives-color-neutral-400, #999) 40%, transparent); border-radius: 8px; padding: 8px 12px; }
.nr { font: 700 11px/1 var(--primitives-font-family-body, system-ui, sans-serif); padding: 4px 6px; border-radius: 5px; background: #154273; color: #fff; display: inline-block; min-width: 18px; text-align: center; }
.sporen { display: flex; gap: 10px; }
.spoor { display: flex; flex-direction: column; align-items: flex-start; gap: 2px; }
</style>
