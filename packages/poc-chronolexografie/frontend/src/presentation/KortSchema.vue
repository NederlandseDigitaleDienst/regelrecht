<script setup>
// The picture of the short page: RegelRecht in the band above, what
// chronolexografie adds in the band below, as inline SVG from kort.js. Its
// colours are tokens of the design system, so it follows the light and the
// dark scheme as the page does; the primitives switch with the scheme
// themselves (light-dark in @nldd/design-system). The badges on the arrows
// are the numbers of the steps of the case under it.
import { schema } from './kort.js';

defineProps({
  /** The id of the text that describes the picture, under it. */
  beschrijving: { type: String, required: true },
});

const BREEDTE = 150;
const HOOGTE = 64;
</script>

<template>
  <!-- The caption describes the picture in words; the SVG itself is one image. -->
  <div class="schema" role="img" :aria-describedby="beschrijving">
    <svg :viewBox="`0 0 ${schema.breedte} ${schema.hoogte}`" xmlns="http://www.w3.org/2000/svg" aria-hidden="true">
      <defs>
        <marker
          v-for="l in schema.lagen"
          :id="`kort-pijl-${l.id}`"
          :key="l.id"
          viewBox="0 0 10 10"
          refX="9"
          refY="5"
          markerWidth="7"
          markerHeight="7"
          orient="auto-start-reverse"
        >
          <path :class="`kop ${l.id}`" d="M0,0 L10,5 L0,10 z" />
        </marker>
      </defs>
      <g v-for="l in schema.lagen" :key="l.id">
        <rect :class="`band ${l.id}`" x="0" :y="l.y" :width="schema.breedte" :height="l.hoogte" />
        <text :class="`laag ${l.id}`" x="20" :y="l.y + 28">{{ l.label }}</text>
      </g>
      <g v-for="v in schema.vakken" :key="v.titel">
        <rect :class="`vak ${v.laag}`" :x="v.x" :y="v.y" :width="BREEDTE" :height="v.h ?? HOOGTE" rx="4" />
        <text class="titel" :x="v.x + 12" :y="v.y + (v.h ? 23 : 26)">{{ v.titel }}</text>
        <text class="sub" :x="v.x + 12" :y="v.y + (v.h ? 41 : 46)">{{ v.sub }}</text>
      </g>
      <g v-for="(p, i) in schema.pijlen" :key="i">
        <path :class="`pijl ${p.laag}`" :d="p.d" :marker-end="`url(#kort-pijl-${p.laag})`" />
        <text v-if="p.label" class="noot" :x="p.label.x" :y="p.label.y">{{ p.label.tekst }}</text>
      </g>
      <g v-for="(p, i) in schema.pijlen.filter((q) => q.nummer)" :key="`n${i}`">
        <circle class="badge" :cx="p.badge[0]" :cy="p.badge[1]" r="10" />
        <text class="nummer" :x="p.badge[0]" :y="p.badge[1] + 4" text-anchor="middle">{{ p.nummer }}</text>
      </g>
    </svg>
  </div>
</template>

<style scoped>
/* Own CSS: the design system has no diagram (as in SchemaOverzicht.vue).
 * Only its tokens: the surfaces and content colours, lintblauw for
 * RegelRecht and oranje for chronolexografie. Wider than a phone, the picture
 * scrolls sideways in its own box. */
.schema {
  overflow-x: auto;
  border: var(--semantics-surfaces-border-width, 1px) solid var(--semantics-surfaces-base-border-color);
  border-radius: var(--semantics-surfaces-corner-radius, 6px);
  background: var(--semantics-surfaces-base-background-color);
}
svg {
  display: block;
  min-width: 760px;
  width: 100%;
  height: auto;
}
text {
  fill: var(--semantics-content-color);
}
.band.rr {
  fill: var(--primitives-color-lintblauw-50);
}
.band.cl {
  fill: var(--primitives-color-oranje-50);
}
.laag {
  font-size: 12px;
  font-weight: 700;
  letter-spacing: 0.08em;
  text-transform: uppercase;
}
.laag.rr {
  fill: var(--primitives-color-lintblauw-700);
}
.laag.cl {
  fill: var(--primitives-color-oranje-700);
}
.vak {
  fill: var(--semantics-surfaces-base-background-color);
  stroke-width: 1.5;
}
.vak.rr {
  stroke: var(--primitives-color-lintblauw-700);
}
.vak.cl {
  stroke: var(--primitives-color-oranje-700);
}
.titel {
  font-size: 14px;
  font-weight: 700;
}
.sub {
  font-size: 12px;
  fill: var(--semantics-content-secondary-color);
}
.noot {
  font-size: 12px;
  fill: var(--semantics-content-secondary-color);
}
.pijl {
  fill: none;
  stroke-width: 1.5;
}
.pijl.rr {
  stroke: var(--semantics-content-color);
}
.pijl.cl {
  stroke: var(--primitives-color-oranje-700);
}
.kop.rr {
  fill: var(--semantics-content-color);
}
.kop.cl,
.badge {
  fill: var(--primitives-color-oranje-700);
}
.nummer {
  font-size: 11px;
  font-weight: 700;
  fill: var(--semantics-surfaces-base-background-color);
}
</style>
