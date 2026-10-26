<script setup>
// The schemas on the slides, as inline SVG. They draw in the ink of the deck
// (currentColor and the --ink-* variables of PresentationDeck.vue), so they
// follow the panel and not the page; the accent is the yellow of the
// progress bar. The text in them is the text of the slide: the names come
// from the corpus, like those in slides.js.
import { computed } from 'vue';

const props = defineProps({
  name: { type: String, required: true },
  label: { type: String, required: true },
});
// One diagram shows at a time, but an id stays unique per diagram anyway.
const arrowId = computed(() => `rr-arrow-${props.name}`);
const arrow = computed(() => `url(#${arrowId.value})`);

const kroniek = [
  { dag: '13 okt 2026', soort: 'AANVRAAG', tekst: 'Aanvraag, BSN 999990020' },
  { dag: '20 okt 2026', soort: 'BESCHIKKING', tekst: 'Kaart verstrekt, geldig t/m 19 okt 2031', accent: true },
  { dag: '1 mrt 2027', soort: 'AANVRAAG', tekst: 'Tweede aanvraag, zelfde BSN' },
  { dag: '1 mrt 2027', soort: 'BESCHIKKING', tekst: 'Geweigerd: al een geldige kaart' },
];

const keten = [
  { titel: 'BABW, artikel 49', code: 'bevoegdheid_tot_verstrekken_gebruikt' },
  { titel: 'Beleid Wilgendaal, artikel 1', code: 'aanvrager_heeft_geldige_gehandicaptenparkeerkaart' },
  { titel: 'Beleid Wilgendaal, artikel 2', code: 'source.reduction' },
  { titel: 'Cel gemeente_wilgendaal', code: 'cel.yaml: answers' },
  { titel: 'Reductie van de gemeente, artikel 1', code: 'reductie_geldige_gehandicaptenparkeerkaart' },
  { titel: 'Reductie van de Awb, artikel 2', code: 'geldende_besluiten (basis BABW#49)' },
  { titel: 'Kroniek van Wilgendaal', code: 'de besluiten onder artikel 49', accent: true },
];

const synthese = {
  synthese: {
    cellen: [
      { naam: 'Cel A', antwoord: 'antwoord uit eigen kroniek' },
      { naam: 'Cel B', antwoord: 'antwoord uit eigen kroniek' },
    ],
    artikel: 'wetsartikel',
    uitkomst: 'één uitkomst',
  },
  'synthese-kaart': {
    cellen: [
      { naam: 'GGD Wilgendaal', antwoord: 'onderzoek en advies' },
      { naam: 'Gemeente Wilgendaal', antwoord: 'al een geldige kaart?', accent: true },
    ],
    artikel: 'BABW artikel 49',
    uitkomst: 'kaart verstrekt of geweigerd',
  },
};
</script>

<template>
  <!-- No role="img": the text in the SVG is the content, and a screen
       reader reads it in order. The caption names the figure. -->
  <figure class="diagram">
    <!-- De kroniek: een tijdlijn van boven naar beneden. -->
    <svg v-if="name === 'kroniek'" viewBox="0 0 420 300">
      <line x1="20" y1="24" x2="20" y2="276" class="line" />
      <g v-for="(g, i) in kroniek" :key="i" :transform="`translate(0 ${30 + i * 72})`">
        <circle cx="20" cy="0" r="7" :class="g.accent ? 'dot accent' : 'dot'" />
        <text x="40" y="-8" class="small">{{ g.dag }} · {{ g.soort }}</text>
        <text x="40" y="14" :class="g.accent ? 'strong accent-text' : 'strong'">{{ g.tekst }}</text>
      </g>
    </svg>

    <!-- De keten: van het artikel tot de kroniek, elk blok een stap. -->
    <svg v-else-if="name === 'keten'" viewBox="0 0 560 532">
      <defs aria-hidden="true"><marker :id="arrowId" viewBox="0 0 10 10" refX="8" refY="5" markerWidth="6" markerHeight="6" orient="auto-start-reverse"><path d="M 0 0 L 10 5 L 0 10 z" class="head" /></marker></defs>
      <g v-for="(k, i) in keten" :key="i" :transform="`translate(0 ${i * 76})`">
        <rect x="2" y="2" width="556" height="56" rx="8" :class="k.accent ? 'box accent' : 'box'" />
        <text x="18" y="25" class="strong">{{ k.titel }}</text>
        <text x="18" y="46" class="mono">{{ k.code }}</text>
        <path v-if="i < keten.length - 1" d="M 280 60 L 280 76" class="arrow" :marker-end="arrow" />
      </g>
    </svg>

    <!-- Synthese: two cells each answer from their own chronicle, and the
         article that decides takes the answers together. -->
    <svg v-else-if="synthese[name]" viewBox="0 0 480 300">
      <defs aria-hidden="true"><marker :id="arrowId" viewBox="0 0 10 10" refX="8" refY="5" markerWidth="6" markerHeight="6" orient="auto-start-reverse"><path d="M 0 0 L 10 5 L 0 10 z" class="head" /></marker></defs>
      <g v-for="(c, i) in synthese[name].cellen" :key="i" :transform="`translate(${i * 240} 0)`">
        <rect x="10" y="4" width="220" height="78" rx="8" :class="c.accent ? 'box accent' : 'box'" />
        <text x="120" y="34" class="strong" text-anchor="middle">{{ c.naam }}</text>
        <text x="120" y="62" class="small" text-anchor="middle">{{ c.antwoord }}</text>
        <path :d="`M 120 84 L ${240 - i * 240 + (i ? 20 : -20)} 160`" class="arrow" :marker-end="arrow" />
      </g>
      <rect x="120" y="160" width="240" height="44" rx="22" class="pill" />
      <text x="240" y="188" class="mono" text-anchor="middle">{{ synthese[name].artikel }}</text>
      <path d="M 240 206 L 240 236" class="arrow" :marker-end="arrow" />
      <rect x="70" y="238" width="340" height="56" rx="8" class="box" />
      <text x="240" y="272" class="strong" text-anchor="middle">{{ synthese[name].uitkomst }}</text>
    </svg>

    <figcaption>{{ label }}</figcaption>
  </figure>
</template>

<style scoped>
/* Eigen CSS: het design system heeft geen schema-component. Alleen kleuren
 * uit de inkt van het dek en het geel van de voortgangsbalk. */
.diagram {
  margin: 0;
  display: flex;
  flex-direction: column;
  gap: 0.5rem;
}
svg {
  width: 100%;
  height: auto;
  max-height: 62vh;
  overflow: visible;
  font-family: 'RijksSans', system-ui, sans-serif;
}
text {
  fill: currentColor;
}
.strong {
  font-size: 17px;
  font-weight: 700;
}
.small {
  font-size: 14px;
  fill: var(--ink-85);
}
.mono {
  font-family: 'JetBrains Mono', ui-monospace, monospace;
  font-size: 13px;
  fill: var(--ink-85);
}
.box {
  fill: var(--ink-08);
  stroke: var(--ink-35);
  stroke-width: 1.5;
}
.pill {
  fill: var(--ink-18);
  stroke: none;
}
.box.accent {
  stroke: var(--accent);
  stroke-width: 2.5;
}
.line {
  stroke: var(--ink-35);
  stroke-width: 2;
}
.dot {
  fill: var(--ink-72);
}
.dot.accent {
  fill: var(--accent);
}
.accent-text {
  fill: var(--accent);
}
.arrow {
  fill: none;
  stroke: var(--ink-55);
  stroke-width: 2;
}
.head {
  fill: var(--ink-55);
}
figcaption {
  font-size: max(0.75rem, calc(1.3 * var(--slide-unit)));
  color: var(--ink-72);
}
</style>
