<script setup>
// De tekst van een artikel in een NLDD-sheet die rechts inschuift, met "YAML" het stuk van het
// regelbestand waarmee de toets dit artikel rekent. `artikel` is null (dicht) of
// { titel, overheid, regeling, tags: [], tekst, yaml?: { bestand, yaml } | { reden } }.
import { ref } from 'vue';

const artikel = defineModel('artikel', { type: Object, default: null });
const weergave = ref('tekst');
</script>

<template>
  <nldd-sheet placement="right" width="640px" :open="!!artikel || undefined" accessible-label="Tekst van het artikel" @close="artikel = null">
    <template v-if="artikel">
      <nldd-top-title-bar :text="artikel.titel" :supporting-text="`${artikel.overheid} · ${artikel.regeling}`" :heading-level="2">
        <nldd-icon-button slot="toolbar" size="sm" icon="close" text="Sluiten" appearance="neutral-tinted" @click="artikel = null"></nldd-icon-button>
      </nldd-top-title-bar>
      <div class="inhoud">
        <div v-if="artikel.tags?.length" class="tags">
          <nldd-tag v-for="(t, i) in artikel.tags" :key="t" :color="i === 0 ? 'accent' : 'neutral'" :text="t"></nldd-tag>
        </div>
        <nldd-segmented-control :value="weergave" size="sm" accessible-label="Tekst of YAML" @change="weergave = $event.target.value">
          <nldd-segmented-control-item value="tekst" text="Tekst" :selected="weergave === 'tekst' || undefined"></nldd-segmented-control-item>
          <nldd-segmented-control-item value="yaml" text="YAML" :selected="weergave === 'yaml' || undefined"></nldd-segmented-control-item>
        </nldd-segmented-control>
        <nldd-text v-if="weergave === 'tekst'" size="sm" class="tekst">{{ artikel.tekst || 'Tekst laden…' }}</nldd-text>
        <template v-else>
          <nldd-text v-if="!artikel.yaml" size="sm" color="secondary">YAML laden…</nldd-text>
          <nldd-text v-else-if="artikel.yaml.reden" size="sm" color="secondary">{{ artikel.yaml.reden }}</nldd-text>
          <template v-else>
            <nldd-text size="xs" color="secondary">Uit {{ artikel.yaml.bestand }}: zo rekent de toets dit artikel.</nldd-text>
            <nldd-code-viewer wrap language="yaml">{{ artikel.yaml.yaml }}</nldd-code-viewer>
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
