<script setup>
// De tekst van een artikel uit het DSO, in een NLDD-sheet die rechts inschuift. Met "YAML" het
// stuk van het regelbestand waarmee het DSO-spoor dit artikel rekent.
import { ref, watch } from 'vue';
import { BUNDEL, OVERHEID, laadTeksten } from '../data.js';
import { yamlVanArtikel } from '../sim.js';

const sleutel = defineModel('sleutel', { type: String, default: null });
const props = defineProps({ data: Object });
const tekst = ref('');
const weergave = ref('tekst');
const yaml = ref(null); // { bestand, yaml } of { reden }; null tijdens het laden

watch(sleutel, async (k) => {
  tekst.value = '';
  yaml.value = null;
  if (!k) return;
  yamlVanArtikel(k).then((y) => { if (sleutel.value === k) yaml.value = y; })
    .catch((e) => { if (sleutel.value === k) yaml.value = { reden: `YAML niet geladen: ${e?.message ?? e}` }; });
  const t = await laadTeksten();
  if (sleutel.value === k) tekst.value = t[k] || '(geen tekst in het DSO)';
});
const m = (k) => props.data?.artikelen[k];
</script>

<template>
  <nldd-sheet placement="right" width="640px" :open="!!sleutel || undefined" accessible-label="Tekst van het artikel" @close="sleutel = null">
    <template v-if="sleutel && m(sleutel)">
      <nldd-top-title-bar :text="m(sleutel).t || `Artikel ${m(sleutel).a}`" :supporting-text="`${OVERHEID[m(sleutel).o] ?? m(sleutel).o} · ${m(sleutel).rt}`" :heading-level="2">
        <nldd-icon-button slot="toolbar" size="sm" icon="close" text="Sluiten" appearance="neutral-tinted" @click="sleutel = null"></nldd-icon-button>
      </nldd-top-title-bar>
      <div class="inhoud">
        <div class="tags">
          <nldd-tag v-if="m(sleutel).rol" color="accent" :text="m(sleutel).rol"></nldd-tag>
          <nldd-tag v-for="b in m(sleutel).b" :key="b" color="neutral" :text="BUNDEL[b] ?? b"></nldd-tag>
          <nldd-tag v-if="m(sleutel).v" color="neutral" :text="m(sleutel).v === 'GEGEVEN' ? 'als gegeven' : m(sleutel).v.toLowerCase()"></nldd-tag>
        </div>
        <nldd-text v-if="m(sleutel).act.length" size="sm"><strong>Activiteit in het DSO:</strong> {{ m(sleutel).act.join('; ') }}</nldd-text>
        <nldd-text v-if="m(sleutel).kw.length" size="sm"><strong>Kwalificatie:</strong> {{ m(sleutel).kw.join(', ') }}</nldd-text>
        <nldd-text v-if="m(sleutel).g" size="sm" color="secondary">{{ m(sleutel).g }}</nldd-text>
        <nldd-segmented-control :value="weergave" size="sm" accessible-label="Tekst of YAML" @change="weergave = $event.target.value">
          <nldd-segmented-control-item value="tekst" text="Tekst" :selected="weergave === 'tekst' || undefined"></nldd-segmented-control-item>
          <nldd-segmented-control-item value="yaml" text="YAML" :selected="weergave === 'yaml' || undefined"></nldd-segmented-control-item>
        </nldd-segmented-control>
        <nldd-text v-if="weergave === 'tekst'" size="sm" class="tekst">{{ tekst || 'Tekst laden…' }}</nldd-text>
        <template v-else>
          <nldd-text v-if="!yaml" size="sm" color="secondary">YAML laden…</nldd-text>
          <nldd-text v-else-if="yaml.reden" size="sm" color="secondary">{{ yaml.reden }}</nldd-text>
          <template v-else>
            <nldd-text size="xs" color="secondary">Uit {{ yaml.bestand }}: zo rekent het DSO-spoor dit artikel.</nldd-text>
            <nldd-code-viewer wrap language="yaml">{{ yaml.yaml }}</nldd-code-viewer>
          </template>
        </template>
      </div>
    </template>
  </nldd-sheet>
</template>

<style scoped>
.inhoud {
  flex: 1; min-height: 0; overflow-y: auto; overscroll-behavior: contain;
  padding: 8px 20px 24px; display: flex; flex-direction: column; gap: 10px;
}
.tags { display: flex; flex-wrap: wrap; gap: 6px; }
.tekst { white-space: pre-wrap; }
</style>
