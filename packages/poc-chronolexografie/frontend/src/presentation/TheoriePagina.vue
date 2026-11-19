<script setup>
// The theory page: what a visitor who opens the app for the first time needs
// to know to read its screens (theorie.js), on one page with little text and
// two pictures. It has a path of its own, `<base>/theorie` (route.js), and
// takes the place of the choice screen; the link in the choice screen opens
// it, and the buttons at the top and the bottom go back.
//
// The pictures are lists and boxes of the page itself, not images: their
// text is the text a screen reader reads, in order, and it grows with the
// font size. The caption of each says what the picture shows.
import { ref } from 'vue';
import { begrippen, besluit, cellen, dagTekst, intro, kroniek, tabbladen } from './theorie.js';

const emit = defineEmits(['terug']);

const base = import.meta.env.BASE_URL;

// Opened by a click, the link that opened the page is gone; App.vue puts the
// focus on the way back.
const terugKnop = ref(null);
defineExpose({ focus: () => terugKnop.value?.focus() });

/**
 * A click on a link the app follows itself; with a modifier key held, the
 * browser follows it, to a tab or window of its own.
 */
function volg(e) {
  if (e.ctrlKey || e.metaKey || e.shiftKey || e.altKey) return;
  e.preventDefault();
  emit('terug');
}
</script>

<template>
  <nldd-simple-section>
    <nldd-container gap="16">
      <nldd-button-bar>
        <nldd-button ref="terugKnop" appearance="neutral-tinted" start-icon="back" :href="base" text="Naar de casuskeuze" @click="volg"></nldd-button>
      </nldd-button-bar>
      <nldd-title size="display-2" heading-level="1" :overline="intro.overline" :text="intro.title">
        <nldd-text slot="supporting-text">{{ intro.lead }}</nldd-text>
      </nldd-title>
    </nldd-container>
  </nldd-simple-section>

  <nldd-simple-section>
    <nldd-title slot="header" size="display-3" heading-level="2" :text="kroniek.title"></nldd-title>
    <nldd-container gap="16">
      <nldd-container gap="8">
        <nldd-text v-for="t in kroniek.text" :key="t">{{ t }}</nldd-text>
      </nldd-container>
      <figure class="plaatje">
        <ol class="tijdlijn">
          <li v-for="(v, i) in kroniek.vastleggingen" :key="i">
            <span :class="v.besluit ? 'nummer besluit' : 'nummer'" aria-hidden="true">{{ i + 1 }}</span>
            <nldd-text size="sm" color="secondary"><time :datetime="v.dag">{{ dagTekst(v.dag) }}</time> · <strong>{{ v.soort }}</strong></nldd-text>
            <nldd-text>{{ v.tekst }}</nldd-text>
          </li>
        </ol>
        <figcaption><nldd-text size="sm" color="secondary">{{ kroniek.bijschrift }}</nldd-text></figcaption>
      </figure>
    </nldd-container>
  </nldd-simple-section>

  <nldd-simple-section>
    <nldd-title slot="header" size="display-3" heading-level="2" :text="besluit.title"></nldd-title>
    <nldd-container gap="8">
      <nldd-text v-for="t in besluit.text" :key="t">{{ t }}</nldd-text>
    </nldd-container>
  </nldd-simple-section>

  <nldd-simple-section>
    <nldd-title slot="header" size="display-3" heading-level="2" :text="cellen.title"></nldd-title>
    <nldd-container gap="16">
      <nldd-container gap="8">
        <nldd-text v-for="t in cellen.text" :key="t">{{ t }}</nldd-text>
      </nldd-container>
      <figure class="plaatje">
        <div class="cellen">
          <div class="cel">
            <nldd-text size="sm" color="secondary">Cel</nldd-text>
            <nldd-text><strong>{{ cellen.van.naam }}</strong></nldd-text>
            <div class="vastlegging">
              <nldd-text size="sm" color="secondary">In haar kroniek · <time :datetime="cellen.dag">{{ dagTekst(cellen.dag) }}</time></nldd-text>
              <nldd-text>{{ cellen.van.vastlegging }}</nldd-text>
            </div>
          </div>
          <div class="pijl">
            <nldd-text size="sm">{{ cellen.pijl }}</nldd-text>
            <svg viewBox="0 0 48 16" width="48" height="16" aria-hidden="true">
              <path d="M0 8 H44 M37 2 L45 8 L37 14" />
            </svg>
          </div>
          <div class="cel">
            <nldd-text size="sm" color="secondary">Cel</nldd-text>
            <nldd-text><strong>{{ cellen.naar.naam }}</strong></nldd-text>
            <div class="vastlegging">
              <nldd-text size="sm" color="secondary">In haar kroniek · <time :datetime="cellen.dag">{{ dagTekst(cellen.dag) }}</time></nldd-text>
              <nldd-text>{{ cellen.naar.vastlegging }}</nldd-text>
            </div>
          </div>
        </div>
        <figcaption><nldd-text size="sm" color="secondary">{{ cellen.bijschrift }}</nldd-text></figcaption>
      </figure>
    </nldd-container>
  </nldd-simple-section>

  <nldd-simple-section v-for="b in begrippen" :key="b.title">
    <nldd-title slot="header" size="display-3" heading-level="2" :text="b.title"></nldd-title>
    <nldd-container gap="8">
      <nldd-text v-for="t in b.text" :key="t">{{ t }}</nldd-text>
    </nldd-container>
  </nldd-simple-section>

  <nldd-simple-section>
    <nldd-title slot="header" size="display-3" heading-level="2" :text="tabbladen.title"></nldd-title>
    <nldd-container gap="16">
      <nldd-list appearance="box-base" :accessible-label="tabbladen.title">
        <nldd-list-item v-for="r in tabbladen.rijen" :key="r.naam" size="sm">
          <nldd-text-cell :text="r.naam" :supporting-text="r.tekst"></nldd-text-cell>
        </nldd-list-item>
      </nldd-list>
      <nldd-button-bar>
        <nldd-button appearance="primary" :href="base" text="Kies een casus" @click="volg"></nldd-button>
      </nldd-button-bar>
    </nldd-container>
  </nldd-simple-section>
</template>

<style scoped>
/* Own CSS: the design system has no timeline and no diagram (as in
 * KortSchema.vue). Only its tokens, so the pictures follow the light and the
 * dark scheme as the page does. */
.plaatje {
  margin: 0;
  display: grid;
  gap: 8px;
  max-width: 640px;
}
.tijdlijn {
  list-style: none;
  margin: 0;
  padding: 0;
  display: grid;
}
.tijdlijn li {
  position: relative;
  padding: 0 0 20px 44px;
  display: grid;
  gap: 2px;
}
.tijdlijn li:last-child {
  padding-bottom: 0;
}
/* The line of the chronicle, from each marker to the next. */
.tijdlijn li:not(:last-child)::before {
  content: '';
  position: absolute;
  left: 13px;
  top: 28px;
  bottom: 0;
  border-left: 2px solid var(--semantics-content-secondary-color);
}
.nummer {
  position: absolute;
  left: 0;
  top: 0;
  box-sizing: border-box;
  width: 28px;
  height: 28px;
  display: grid;
  place-items: center;
  border: 2px solid var(--primitives-color-oranje-700);
  border-radius: 50%;
  font: 700 0.8125rem/1 var(--primitives-font-family-body, sans-serif);
  color: var(--semantics-content-color);
  background: var(--semantics-surfaces-base-background-color);
}
.nummer.besluit {
  color: var(--semantics-surfaces-base-background-color);
  background: var(--primitives-color-oranje-700);
}
.cellen {
  display: flex;
  align-items: stretch;
  gap: 8px;
}
.cel {
  flex: 1;
  min-width: 0;
  display: grid;
  gap: 4px;
  padding: 12px;
  border: 2px solid var(--primitives-color-oranje-700);
  border-radius: var(--semantics-surfaces-corner-radius, 6px);
  background: var(--semantics-surfaces-base-background-color);
}
.vastlegging {
  display: grid;
  gap: 2px;
  margin-top: 4px;
  padding: 8px;
  border-radius: var(--semantics-surfaces-corner-radius, 6px);
  background: var(--semantics-surfaces-tinted-background-color);
}
.pijl {
  display: grid;
  align-content: center;
  justify-items: center;
  gap: 2px;
}
.pijl path {
  fill: none;
  stroke: var(--semantics-content-color);
  stroke-width: 2;
}
/* On a phone the cells stand under each other and the arrow points down. */
@media (max-width: 560px) {
  .cellen {
    flex-direction: column;
    align-items: stretch;
  }
  .pijl {
    grid-auto-flow: column;
    justify-content: center;
    align-items: center;
    gap: 8px;
    padding: 12px 0;
  }
  .pijl svg {
    transform: rotate(90deg);
  }
}
</style>
