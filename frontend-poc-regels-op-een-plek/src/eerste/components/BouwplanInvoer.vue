<script setup>
// Compacte invoer van het bouwplan, gedeeld door Toets en Simuleren.
import { computed } from 'vue';
import { bouwplan, SOORTEN_GEBOUW, WONINGTYPES } from '../toets/bouwplan.js';

// `alles`: ook de velden die maar in één plan meetellen (Simuleren).
const props = defineProps({ metLagen: { type: Boolean, default: true }, alles: { type: Boolean, default: false } });
const hoogte = computed(() => String(Math.round(bouwplan.lagen * bouwplan.meterPerLaag * 100) / 100).replace('.', ','));

function getal(e, sleutel) {
  if (e.target.value === null || e.target.value === '') return;
  const v = Number(e.target.value);
  if (Number.isFinite(v) && v > 0) bouwplan[sleutel] = v;
}
</script>

<template>
  <nldd-container layout="stack" gap="8" class="bouwplan">
    <nldd-text size="sm" color="secondary">
      Bouwplan, aannames:
      <template v-if="props.metLagen">{{ bouwplan.lagen }} lagen van {{ String(bouwplan.meterPerLaag).replace('.', ',') }} m is {{ hoogte }} m, </template>
      plat dak, {{ bouwplan.aantalWoningen }} woningen.
    </nldd-text>
    <div class="velden">
      <nldd-form-field v-if="props.metLagen" label="Lagen">
        <nldd-number-field :value="bouwplan.lagen" min="1" max="15" step="1" size="sm" @change="getal($event, 'lagen')"></nldd-number-field>
      </nldd-form-field>
      <nldd-form-field label="Meter per laag">
        <nldd-number-field :value="bouwplan.meterPerLaag" min="2.5" max="5" step="0.1" size="sm" @change="getal($event, 'meterPerLaag')"></nldd-number-field>
      </nldd-form-field>
      <nldd-form-field label="Woningen">
        <nldd-number-field :value="bouwplan.aantalWoningen" min="1" max="500" step="1" size="sm" @change="getal($event, 'aantalWoningen')"></nldd-number-field>
      </nldd-form-field>
    </div>
    <nldd-segmented-control :value="bouwplan.woningtype" size="sm" width="full" accessible-label="Woningtype" @change="bouwplan.woningtype = $event.target.value">
      <nldd-segmented-control-item v-for="t in WONINGTYPES" :key="t.waarde" :value="t.waarde" :text="t.tekst" :selected="bouwplan.woningtype === t.waarde || undefined"></nldd-segmented-control-item>
    </nldd-segmented-control>
    <template v-if="props.alles">
      <nldd-form-field label="Soort gebouw (telt in Het Ambacht, artikel 54)">
        <nldd-segmented-control :value="bouwplan.soortGebouw" size="sm" width="full" accessible-label="Soort gebouw" @change="bouwplan.soortGebouw = $event.target.value">
          <nldd-segmented-control-item v-for="t in SOORTEN_GEBOUW" :key="t.waarde" :value="t.waarde" :text="t.tekst" :selected="bouwplan.soortGebouw === t.waarde || undefined"></nldd-segmented-control-item>
        </nldd-segmented-control>
      </nldd-form-field>
      <nldd-switch-field label="Het bouwplan voegt woningen toe" :checked="bouwplan.voegtWoningenToe || undefined" @change="bouwplan.voegtWoningenToe = $event.target.checked"></nldd-switch-field>
    </template>
  </nldd-container>
</template>

<style scoped>
.velden { display: grid; grid-template-columns: repeat(auto-fit, minmax(130px, 1fr)); gap: 8px; }
</style>
