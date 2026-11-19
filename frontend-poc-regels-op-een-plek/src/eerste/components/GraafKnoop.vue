<script setup>
// Eén kader in de graaf: een bron, een keuze, een artikel of het besluit. Naar GraphLawNode en
// GraphItemNode van de RegelRecht-demo: een kop, vakken met rijen, en per rij een handle waar
// een lijn aankomt (links) of vertrekt (rechts). De hoogtes staan vast en moeten passen bij MAAT
// in src/graaf/bouwGraaf.js, want de indeling rekent ermee. Het design system heeft geen
// graafcomponent (zelfde uitzondering als in de demo); de kleuren komen uit de tokens.
import { Handle, Position } from '@vue-flow/core';

defineProps({ data: { type: Object, required: true } });

const SOORT = {
  bron: 'Bron', selectie: 'Keuze', artikel: 'Regel', hub: 'Oordeel', besluit: 'Besluit', open: 'Open', afgevallen: 'Valt af', samen: 'Samen',
};
const TOON_TEKST = { ok: 'ja', nee: 'nee', onbekend: 'open', af: 'valt af', neutraal: '' };
</script>

<template>
  <div :class="['gk', `gk--${data.soort}`, data.toon ? `gk--toon-${data.toon}` : '']">
    <Handle id="kop-in" type="target" :position="Position.Left" class="gk-kophandle" />
    <div class="gk-kop" :title="data.bron || data.kop">
      <div class="gk-kopregel">
        <span class="gk-soort">{{ SOORT[data.soort] ?? data.soort }}</span>
        <span v-if="data.status" :class="['gk-status', `gk-toon-${data.toon}`]">{{ data.status }}</span>
        <a v-if="data.link" class="gk-link" :href="data.link" target="_blank" rel="noopener" title="Bron openen" @click.stop>bron ↗</a>
      </div>
      <div class="gk-titel">{{ data.kop }}</div>
      <div class="gk-sub">{{ data.sub }}</div>
    </div>
    <Handle id="kop-uit" type="source" :position="Position.Right" class="gk-kophandle" />

    <div v-for="s in data.secties" :key="s.titel" class="gk-sectie">
      <div class="gk-sectietitel">{{ s.titel }}</div>
      <template v-for="r in s.rijen" :key="r.id">
        <div :class="['gk-rij', `gk-rij--${r.toon}`]" :title="[r.naam, r.uitleg].filter(Boolean).join('\n\n')">
          <Handle v-if="r.in" :id="`in:${r.id}`" type="target" :position="Position.Left" />
          <span class="gk-naam">{{ r.naam }}</span>
          <span v-if="r.waarde !== ''" :class="['gk-waarde', `gk-toon-${r.toon}`]">{{ r.waarde || TOON_TEKST[r.toon] }}</span>
          <Handle v-if="r.uit" :id="`uit:${r.id}`" type="source" :position="Position.Right" />
        </div>
        <div v-for="(t, i) in r.takken ?? []" :key="`${r.id}-t${i}`" :class="['gk-tak', `gk-rij--${t.toon}`]" :title="t.naam">
          <span class="gk-naam">↳ {{ t.naam }}</span>
          <span :class="['gk-waarde', `gk-toon-${t.toon}`]">{{ t.waarde }}</span>
        </div>
      </template>
    </div>
  </div>
</template>

<style scoped>
/* Hoogtes: kop 60, sectietitel 22, rij 26, tak 22, ruimte na een sectie 4, onderkant 10. */
.gk {
  --gk-ok: var(--primitives-color-groen-600, #39870c);
  --gk-nee: var(--primitives-color-rood-600, #d52b1e);
  --gk-onbekend: var(--primitives-color-donkergeel-600, #b58500);
  --gk-af: var(--primitives-color-neutral-500, #8a8a8a);
  --gk-rand: var(--primitives-color-donkerblauw-600, #154273);
  width: 100%;
  height: 100%;
  box-sizing: border-box;
  border: 1.5px solid var(--gk-rand);
  border-radius: 12px;
  background: var(--semantics-surfaces-base-background-color, Canvas);
  color: var(--semantics-content-color, CanvasText);
  font-family: var(--primitives-font-family-body, system-ui, sans-serif);
  overflow: hidden;
  padding-bottom: 10px;
}
.gk--besluit { border-width: 2.5px; }
.gk--bron, .gk--selectie { --gk-rand: var(--primitives-color-lintblauw-600, #01689b); }
.gk--hub { --gk-rand: var(--primitives-color-donkerblauw-750, #0d2a4a); }
.gk--open { --gk-rand: var(--gk-onbekend); border-style: dashed; }
.gk--samen { --gk-rand: var(--primitives-color-donkerblauw-400, #7a95b5); border-style: dotted; }
.gk--afgevallen { --gk-rand: var(--gk-af); border-style: dashed; opacity: 0.75; }

.gk-kop { height: 60px; box-sizing: border-box; padding: 7px 12px 0; }
.gk-kopregel { display: flex; align-items: center; gap: 8px; height: 16px; font-size: 0.6875rem; }
.gk-soort { text-transform: uppercase; letter-spacing: 0.05em; font-weight: 700; color: var(--gk-rand); }
.gk-status { font-weight: 700; }
.gk-link { margin-left: auto; color: var(--primitives-color-lintblauw-600, #01689b); text-decoration: none; }
.gk-link:hover { text-decoration: underline; }
.gk-titel { font-weight: 600; font-size: 0.9375rem; line-height: 20px; white-space: nowrap; overflow: hidden; text-overflow: ellipsis; }
.gk-sub { font-size: 0.75rem; line-height: 16px; color: var(--semantics-content-secondary-color, #555); white-space: nowrap; overflow: hidden; text-overflow: ellipsis; }

.gk-sectie { margin: 0 8px 4px; }
.gk-sectietitel {
  height: 22px; line-height: 22px; padding: 0 4px;
  font-size: 0.6875rem; font-weight: 700; text-transform: uppercase; letter-spacing: 0.04em;
  color: var(--semantics-content-secondary-color, #555);
}
.gk-rij, .gk-tak {
  position: relative;
  display: flex; align-items: center; justify-content: space-between; gap: 8px;
  box-sizing: border-box; padding: 0 8px;
  font-size: 0.8125rem;
}
.gk-rij {
  height: 26px;
  border: 1px solid var(--semantics-dividers-color, #ddd);
  border-left: 3px solid var(--gk-rand-rij, var(--semantics-dividers-color, #ddd));
  border-radius: 6px;
  background: color-mix(in srgb, var(--semantics-content-color, #000) 4%, var(--semantics-surfaces-base-background-color, #fff));
}
.gk-tak { height: 22px; padding-left: 22px; font-size: 0.75rem; color: var(--semantics-content-secondary-color, #555); }
.gk-rij--ok { --gk-rand-rij: var(--gk-ok); }
.gk-rij--nee { --gk-rand-rij: var(--gk-nee); }
.gk-rij--onbekend { --gk-rand-rij: var(--gk-onbekend); }
.gk-rij--af { --gk-rand-rij: var(--gk-af); color: var(--gk-af); }
.gk-rij--af .gk-naam { text-decoration: line-through; }
.gk-naam { white-space: nowrap; overflow: hidden; text-overflow: ellipsis; min-width: 0; }
.gk-waarde { flex: none; max-width: 48%; white-space: nowrap; overflow: hidden; text-overflow: ellipsis; font-weight: 600; font-variant-numeric: tabular-nums; }
.gk-toon-ok { color: var(--gk-ok); }
.gk-toon-nee { color: var(--gk-nee); }
.gk-toon-onbekend { color: var(--gk-onbekend); }
.gk-toon-af { color: var(--gk-af); }
.gk-toon-neutraal { color: var(--primitives-color-donkerblauw-700, #154273); }

.gk :deep(.vue-flow__handle) {
  width: 7px; height: 7px; min-width: 0; min-height: 0; border: 0;
  background: var(--primitives-color-donkerblauw-600, #154273);
}
.gk-kophandle { top: 30px !important; opacity: 0.6; }
</style>
