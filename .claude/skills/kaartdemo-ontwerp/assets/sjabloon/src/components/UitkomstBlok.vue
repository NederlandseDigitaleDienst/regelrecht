<script setup>
// Het vaste patroon voor een oordeel in het paneel: kop met recht en bouwplan, banner met het
// oordeel en de reden, lijst met de gerekende artikelen en een tag per artikel, een uitklap voor
// de trace. Een klik op een artikel vraagt om de tekst (emit 'artikel').
//
// uitkomst: { klasse, reden, stappen: [{ sleutel, regeling, artikel, uitkomst }], trace? }
import { ref } from 'vue';
import Uitklap from './Uitklap.vue';
import { TAG, TEKST, banner } from '../kleuren.js';
import { rdTekst } from '../geo.js';

const props = defineProps({
  rd: { type: Array, required: true },
  titel: { type: String, default: '' },
  beoogd: { type: Boolean, default: false },
  bouwplan: { type: String, default: '' },
  uitkomst: { type: Object, default: null },
});
const emit = defineEmits(['artikel']);
const traceOpen = ref(false);
</script>

<template>
  <nldd-container layout="stack" gap="12">
    <nldd-title
      size="display-5" :heading-level="2"
      :overline="props.beoogd ? 'Beoogd recht: ontwerp, geen geldend recht' : 'Geldend recht'"
      :text="props.titel || `Vak bij RD ${rdTekst(props.rd)}`"
      :supporting-text="props.bouwplan"
    ></nldd-title>

    <nldd-text v-if="!props.uitkomst" size="sm" color="secondary"><p>Rekenen…</p></nldd-text>
    <template v-else>
      <nldd-banner
        :variant="banner(props.uitkomst.klasse).variant" :text="banner(props.uitkomst.klasse).tekst"
        size="sm" :heading-level="3" :supporting-text="props.uitkomst.reden ?? ''"
      ></nldd-banner>

      <nldd-text size="sm"><p><strong>Gerekende artikelen ({{ props.uitkomst.stappen.length }})</strong></p></nldd-text>
      <nldd-list v-if="props.uitkomst.stappen.length" appearance="simple" accessible-label="Gerekende artikelen">
        <nldd-list-item v-for="s in props.uitkomst.stappen" :key="s.sleutel" size="sm" button @click="emit('artikel', s.sleutel)">
          <nldd-text-cell :text="`${s.regeling}, artikel ${s.artikel}`"></nldd-text-cell>
          <nldd-cell width="fit-content"><nldd-tag :color="TAG[s.uitkomst] ?? 'neutral'" :text="TEKST[s.uitkomst] ?? s.uitkomst"></nldd-tag></nldd-cell>
        </nldd-list-item>
      </nldd-list>
      <nldd-text v-else size="sm" color="secondary"><p>Niets gerekend.</p></nldd-text>

      <Uitklap v-if="props.uitkomst.trace" v-model:open="traceOpen" titel="Trace" sub="Wat de engine per artikel deed: invoer, stappen en uitkomst.">
        <nldd-code-viewer wrap max-height="420px" language="text">{{ props.uitkomst.trace }}</nldd-code-viewer>
      </Uitklap>
    </template>
  </nldd-container>
</template>
