<script setup>
// De graaf van het besluit op het gekozen punt, naar de GraafView van
// de RegelRecht-demo: alle regels die het besluit bepalen, met hun verwijzingen, en de opties die
// afvallen. Neemt de hele kaartruimte. Rekent niets zelf: de toets komt uit src/toets/toets.js
// (dezelfde berekening als de tab Toets), de graaf uit src/graaf/bouwGraaf.js. Het tekenvlak is
// vue-flow, zoals in de demo: het design system heeft geen graafcomponent.
import { computed, nextTick, ref, shallowRef, watch } from 'vue';
import { VueFlow, useVueFlow } from '@vue-flow/core';
import { Background } from '@vue-flow/background';
import '@vue-flow/core/dist/style.css';
import '@vue-flow/core/dist/theme-default.css';
import GraafKnoop from './GraafKnoop.vue';
import { maakEngine } from '../engine.js';
import { bouwplan } from '../toets/bouwplan.js';
import { aannames, aantalIngevuld, naarParams } from '../toets/aannames.js';
import { laadRegelingen } from '../toets/laden.js';
import { rekenToets } from '../toets/toets.js';
import { bouwGraaf } from '../graaf/bouwGraaf.js';

const props = defineProps({
  stapel: { type: Object, default: null },
  rd: { type: Array, default: null },
});

const { fitView, zoomIn, zoomOut } = useVueFlow({ id: 'spoorzone-graaf' });

const toets = shallowRef(null);
const fout = ref(null);
const bezig = ref(false);
const kolommen = ref('beide');
const afgevallen = ref(true);
const gekozen = ref(null);
const vandaag = new Date().toISOString().slice(0, 10);
const LEGENDA = [
  { toon: 'ok', tekst: 'waar, toegestaan' },
  { toon: 'nee', tekst: 'onwaar, niet toegestaan' },
  { toon: 'onbekend', tekst: 'onbekend, onthouding' },
  { toon: 'af', tekst: 'valt af' },
  { toon: 'neutraal', tekst: 'gegeven of verwijzing, zonder eigen uitkomst' },
];

async function reken() {
  if (!props.stapel) { toets.value = null; return; }
  bezig.value = true;
  try {
    const { index, varianten } = await laadRegelingen();
    toets.value = await rekenToets({ stapel: props.stapel, bouwplan: { ...bouwplan }, aannames: naarParams(), datum: vandaag, index, varianten, maakEngine });
    fout.value = null;
  } catch (e) {
    fout.value = String(e?.message ?? e);
  } finally {
    bezig.value = false;
  }
}
watch([() => props.stapel, () => ({ ...bouwplan }), () => ({ ...aannames })], reken, { deep: true, immediate: true });

const graaf = computed(() => (toets.value && props.stapel
  ? bouwGraaf({ stapel: props.stapel, toets: toets.value, rd: props.rd, afgevallen: afgevallen.value, kolommen: kolommen.value })
  : null));

// De lijnen van het gekozen kader lichten op.
const lijnen = computed(() => (graaf.value?.edges ?? []).map((e) => (gekozen.value && (e.source === gekozen.value || e.target === gekozen.value)
  ? { ...e, class: `${e.class} gl--focus` } : e)));
const kaderGekozen = computed(() => graaf.value?.kaders.find((k) => k.id === gekozen.value) ?? null);

function passend() {
  setTimeout(() => fitView({ padding: 0.06 }), 30);
}
watch(() => graaf.value?.nodes.map((n) => n.id).join(), async () => {
  gekozen.value = null;
  await nextTick();
  passend();
});

const samenvatting = computed(() => {
  const bp = toets.value?.bouwplan;
  if (!bp || !props.rd) return '';
  const n = aantalIngevuld();
  return `RD ${Math.round(props.rd[0])}, ${Math.round(props.rd[1])} · ${bp.lagen} lagen × ${bp.meterPerLaag} m = ${bp.bouwhoogte} m, ${bp.woningtype}, ${bp.aantalWoningen} woningen${n ? ` · ${n} aannames` : ''}`;
});
const telling = computed(() => {
  const g = graaf.value;
  if (!g) return '';
  const regels = g.kaders.filter((k) => k.soort === 'artikel' || k.soort === 'hub').length;
  const af = g.kaders.flatMap((k) => k.secties.flatMap((s) => s.rijen)).filter((r) => r.toon === 'af').length;
  return `${regels} artikelen, ${g.edges.length} verwijzingen${afgevallen.value ? `, ${af} afgevallen opties` : ''}`;
});

const TAG = { ok: 'success', nee: 'critical', onbekend: 'warning', af: 'neutral', neutraal: 'neutral' };
</script>

<template>
  <div class="graaftab">
    <div class="graaf-balk">
      <nldd-container layout="row" gap="16" vertical-alignment="center" padding-inline="16" padding-block="8">
        <nldd-title size="display-5" :heading-level="2" text="Graaf van het besluit" :supporting-text="samenvatting || 'Kies eerst een plek op de kaart'"></nldd-title>
        <nldd-segmented-control size="sm" :value="kolommen" accessible-label="Kolommen" @change="kolommen = $event.target.value">
          <nldd-segmented-control-item value="beide" text="Beide" :selected="kolommen === 'beide' || undefined"></nldd-segmented-control-item>
          <nldd-segmented-control-item value="tijdelijk" text="Tijdelijk" :selected="kolommen === 'tijdelijk' || undefined"></nldd-segmented-control-item>
          <nldd-segmented-control-item value="nieuw" text="Nieuw" :selected="kolommen === 'nieuw' || undefined"></nldd-segmented-control-item>
        </nldd-segmented-control>
        <nldd-checkbox-field label="Afvallers tonen" :checked="afgevallen || undefined" @change="afgevallen = $event.target.checked"></nldd-checkbox-field>
        <nldd-button size="sm" appearance="neutral-tinted" start-icon="fit-to-view" text="Passend" @click="passend()"></nldd-button>
        <nldd-text size="xs" color="secondary"><p>{{ bezig ? 'Rekenen…' : telling }}</p></nldd-text>
      </nldd-container>
    </div>

    <div class="graaf-vlak">
      <nldd-container v-if="!stapel" padding="24" layout="stack" gap="8">
        <nldd-title size="display-4" :heading-level="2" text="Nog geen plek gekozen"></nldd-title>
        <nldd-text><p>Ga terug naar de kaart en klik op een plek, of kies een testpunt. De graaf toont dan alle regels die het besluit daar bepalen.</p></nldd-text>
      </nldd-container>
      <nldd-banner v-else-if="fout" variant="critical" text="De graaf kon niet worden opgebouwd" :supporting-text="fout"></nldd-banner>
      <VueFlow
        v-else-if="graaf"
        id="spoorzone-graaf"
        :nodes="graaf.nodes"
        :edges="lijnen"
        :min-zoom="0.08"
        :max-zoom="2"
        :nodes-connectable="false"
        :elevate-edges-on-select="false"
        fit-view-on-init
        @node-click="({ node }) => (gekozen = gekozen === node.id ? null : node.id)"
        @pane-click="gekozen = null"
      >
        <template #node-kader="{ data }"><GraafKnoop :data="data" /></template>
        <Background />
      </VueFlow>

      <nldd-button-bar v-if="graaf" class="graaf-zoom" appearance="neutral-tinted">
        <nldd-icon-button icon="add" text="Inzoomen" @click="zoomIn()"></nldd-icon-button>
        <nldd-icon-button icon="remove" text="Uitzoomen" @click="zoomOut()"></nldd-icon-button>
        <nldd-button-bar-divider></nldd-button-bar-divider>
        <nldd-icon-button icon="fit-to-view" text="Alles in beeld" @click="passend()"></nldd-icon-button>
      </nldd-button-bar>

      <div v-if="graaf" class="graaf-legenda" aria-label="Legenda van de graaf">
        <nldd-text size="xs"><p><strong>Lijnen en rijen</strong></p></nldd-text>
        <!-- Zelfde kleur en streep als de lijnen in de graaf (stijlen onderaan, gl--*). -->
        <span v-for="l in LEGENDA" :key="l.toon" class="lg">
          <svg class="lg-lijn" :class="`lg--${l.toon}`" width="26" height="8" aria-hidden="true"><line x1="1" y1="4" x2="25" y2="4" /></svg>
          <nldd-text size="xs"><span>{{ l.tekst }}</span></nldd-text>
        </span>
        <nldd-text size="xs" color="secondary"><p>Klik op een kader voor de volledige tekst.</p></nldd-text>
      </div>

      <aside v-if="kaderGekozen" class="graaf-detail" aria-label="Details van het gekozen kader">
        <nldd-container padding="16" layout="stack" gap="12">
          <nldd-title size="display-5" :heading-level="2" :text="kaderGekozen.kop" :supporting-text="kaderGekozen.sub" :overline="kaderGekozen.status ?? ''"></nldd-title>
          <nldd-link v-if="kaderGekozen.link" :href="kaderGekozen.link" target="_blank" rel="noopener" text="Bron openen" end-icon="external-link"></nldd-link>
          <nldd-text v-if="kaderGekozen.bron" size="xs" color="secondary"><p>{{ kaderGekozen.bron }}</p></nldd-text>
          <nldd-banner v-if="kaderGekozen.fout" variant="critical" text="Fout in de berekening" :supporting-text="kaderGekozen.fout"></nldd-banner>
          <template v-for="s in kaderGekozen.secties" :key="s.titel">
            <nldd-text size="sm" weight="bold"><p>{{ s.titel }}</p></nldd-text>
            <nldd-list appearance="simple" :accessible-label="s.titel">
              <template v-for="r in s.rijen" :key="r.id">
                <nldd-list-item size="sm">
                  <nldd-text-cell :text="r.naam" :supporting-text="r.uitleg || ''"></nldd-text-cell>
                  <nldd-cell v-if="r.waarde" width="fit-content">
                    <nldd-tag :color="TAG[r.toon] ?? 'neutral'" :text="r.waarde"></nldd-tag>
                  </nldd-cell>
                </nldd-list-item>
                <nldd-list-item v-for="(t, i) in r.takken ?? []" :key="`${r.id}-${i}`" size="sm">
                  <nldd-text-cell :text="`↳ ${t.naam}`"></nldd-text-cell>
                  <nldd-cell width="fit-content"><nldd-tag :color="TAG[t.toon] ?? 'neutral'" :text="t.waarde"></nldd-tag></nldd-cell>
                </nldd-list-item>
              </template>
            </nldd-list>
          </template>
          <nldd-button size="sm" appearance="secondary" start-icon="close" text="Sluiten" @click="gekozen = null"></nldd-button>
        </nldd-container>
      </aside>
    </div>
  </div>
</template>

<style scoped>
.graaftab { position: absolute; inset: 0; display: flex; flex-direction: column; background: var(--semantics-surfaces-tinted-background-color, #f3f3f3); }
.graaf-balk {
  flex: none; padding-left: 270px; /* ruimte voor de schakelaar Kaart | Graaf */
  border-bottom: 1px solid var(--semantics-dividers-color, #ddd);
  background: var(--semantics-surfaces-base-background-color, Canvas);
}
.graaf-vlak { position: relative; flex: 1; min-height: 0; }
.graaf-zoom { position: absolute; left: 12px; bottom: 12px; z-index: 5; }
.graaf-legenda {
  position: absolute; right: 12px; bottom: 12px; z-index: 5;
  display: flex; flex-direction: column; gap: 2px; padding: 8px 12px; border-radius: 8px;
  background: color-mix(in srgb, var(--semantics-surfaces-base-background-color, Canvas) 90%, transparent);
  border: 1px solid color-mix(in srgb, var(--primitives-color-neutral-400, #999) 35%, transparent);
}
.lg { display: inline-flex; align-items: center; gap: 8px; }
/* De streepjes in de legenda zijn SVG-lijnen met dezelfde kleur en hetzelfde streeppatroon als
   de lijnen in de graaf. (Eerder CSS-randen: de kleur ging verloren aan een specifiekere regel.) */
.lg-lijn { flex: none; }
.lg-lijn line { stroke-width: 2.5; stroke-linecap: butt; }
.lg--ok line { stroke: var(--primitives-color-groen-600, #39870c); }
.lg--nee line { stroke: var(--primitives-color-rood-600, #d52b1e); }
.lg--onbekend line { stroke: var(--primitives-color-donkergeel-600, #b58500); stroke-dasharray: 6 4; }
.lg--af line { stroke: var(--primitives-color-neutral-500, #8a8a8a); stroke-dasharray: 3 4; }
.lg--neutraal line { stroke: var(--primitives-color-donkerblauw-400, #7a95b5); stroke-width: 1.6; }
/* Werkbalk: de schakelaars en het vinkje niet laten afkappen of afbreken. */
.graaf-balk nldd-segmented-control, .graaf-balk nldd-checkbox-field, .graaf-balk nldd-button { flex: none; }
.graaf-balk nldd-checkbox-field { white-space: nowrap; }
.graaf-detail {
  position: absolute; top: 12px; right: 12px; bottom: 12px; z-index: 6;
  width: clamp(320px, 30vw, 480px); overflow-y: auto; overscroll-behavior: contain;
  border-radius: 10px; border: 1px solid color-mix(in srgb, var(--primitives-color-neutral-400, #999) 40%, transparent);
  background: color-mix(in srgb, var(--semantics-surfaces-base-background-color, Canvas) 94%, transparent);
  backdrop-filter: blur(6px); box-shadow: 0 6px 24px rgb(0 0 0 / 0.14);
}

/* Lijnen: kleur naar de uitkomst die erdoor loopt, gestippeld als het afvalt. */
.graaf-vlak :deep(.vue-flow__edge-path) { stroke-width: 1.6; stroke: var(--primitives-color-donkerblauw-400, #7a95b5); }
.graaf-vlak :deep(.gl--ok .vue-flow__edge-path) { stroke: var(--primitives-color-groen-600, #39870c); }
.graaf-vlak :deep(.gl--nee .vue-flow__edge-path) { stroke: var(--primitives-color-rood-600, #d52b1e); }
.graaf-vlak :deep(.gl--onbekend .vue-flow__edge-path) { stroke: var(--primitives-color-donkergeel-600, #b58500); stroke-dasharray: 6 4; }
.graaf-vlak :deep(.gl--af .vue-flow__edge-path) { stroke: var(--primitives-color-neutral-500, #8a8a8a); stroke-dasharray: 3 4; }
.graaf-vlak :deep(.gl--focus .vue-flow__edge-path) { stroke-width: 3.5; }
.graaf-vlak :deep(.vue-flow__node) { cursor: pointer; }
</style>
