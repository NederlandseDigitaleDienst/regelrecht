<script setup>
// The introductory deck: a Rijkshuisstijl-blue panel over the whole screen,
// one slide at a time (slides.js), with the counter, the keys and the
// buttons underneath. The form follows the deck of the demo
// (frontend-demo/src/presentation/PresentationDeck.vue); this one has no rail
// and no slide that opens a tab. The closing slide offers the demo scenarios
// (demo.js); choosing one closes the deck and begins it (`scenario`). The
// button "Presentatie" on the page brings the deck back on that slide.
import { computed, nextTick, onBeforeUnmount, ref, watch } from 'vue';
import { usePresentation } from './usePresentation.js';
import { demoGuides } from './demo.js';
import Diagram from './Diagram.vue';

const emit = defineEmits(['scenario']);
const p = usePresentation();
const deck = ref(null);

const slide = computed(() => p.current.value);
const counter = computed(() => `${p.index.value + 1} / ${p.total.value}`);
const progress = computed(() => `${((p.index.value + 1) / p.total.value) * 100}%`);
const today = new Date().toLocaleDateString('nl-NL', { day: 'numeric', month: 'long', year: 'numeric' });
const scenarios = demoGuides();

// Behind the portal a strip with the voorbehoud sits sticky at the top of
// every page (packages/poc-portal, `data-poc-portaal`), above everything. The
// deck starts under it, so the strip stays readable and covers no slide.
const top = ref(0);
let strip = null;
const resize = new ResizeObserver(() => {
  top.value = strip ? Math.max(0, strip.getBoundingClientRect().bottom) : 0;
});
function followStrip() {
  strip = document.querySelector('[data-poc-portaal]');
  resize.disconnect();
  if (strip) resize.observe(strip);
  top.value = strip ? Math.max(0, strip.getBoundingClientRect().bottom) : 0;
}
onBeforeUnmount(() => resize.disconnect());

// The deck takes the focus when it opens, so the keys and a screen reader
// start on the slide, and gives it back to where it was when it closes. The
// app under it is inert meanwhile: Tab and a screen reader stay on the deck
// and the portal's strip above it.
const app = () => document.getElementById('app');
let before = null;
watch(
  () => p.active.value,
  async (open) => {
    if (open) {
      followStrip();
      before = document.activeElement;
      if (app()) app().inert = true;
      await nextTick();
      deck.value?.focus();
    } else {
      if (app()) app().inert = false;
      // After a scenario was chosen the choice screen is gone, and the
      // button that opened the deck with it; `choose` clears `before`.
      if (before?.isConnected) before.focus?.();
      before = null;
    }
  },
);

/** `**bold**` → <strong>, `` `name` `` → <code>; everything else escaped. */
function emphasize(line) {
  const escaped = String(line).replaceAll('&', '&amp;').replaceAll('<', '&lt;').replaceAll('>', '&gt;');
  return escaped.replace(/\*\*(.+?)\*\*/g, '<strong>$1</strong>').replace(/`([^`]+)`/g, '<code>$1</code>');
}

function choose(naam) {
  before = null;
  p.leave();
  emit('scenario', naam);
}
</script>

<template>
  <Teleport to="body">
    <div
      v-if="p.active.value && slide"
      ref="deck"
      class="deck"
      role="region"
      aria-roledescription="presentatie"
      aria-label="Inleiding op chronolexografie"
      tabindex="-1"
      :style="{ top: `${top}px` }"
    >
      <div class="stage">
        <div class="content" :class="{ 'with-figure': slide.figure }" aria-live="polite">
          <div class="content-main">
            <template v-if="slide.kind === 'title'">
              <span class="overline">{{ today }}</span>
              <h1 class="title title-hero">{{ slide.title }}</h1>
              <p v-if="slide.subtitle" class="lead lead-hero">{{ slide.subtitle }}</p>
              <span v-if="slide.footer" class="affiliation">{{ slide.footer }}</span>
            </template>

            <template v-else-if="slide.kind === 'statement'">
              <span v-if="slide.overline" class="overline">{{ slide.overline }}</span>
              <h1 class="title statement">
                <span v-for="(line, j) in slide.lines" :key="j" class="statement-line" v-html="emphasize(line)"></span>
              </h1>
            </template>

            <template v-else-if="slide.kind === 'section' || slide.kind === 'closing'">
              <span v-if="slide.overline" class="overline">{{ slide.overline }}</span>
              <h1 class="title title-hero">{{ slide.title }}</h1>
              <ul v-if="slide.lines?.length" class="bullets bullets-plain">
                <li v-for="(line, j) in slide.lines" :key="j" v-html="emphasize(line)"></li>
              </ul>
              <!-- The demo scenarios, as buttons of the design system. The
                   inherit appearances take their colour from the blue panel. -->
              <ol v-if="slide.kind === 'closing'" class="scenarios">
                <li v-for="(g, j) in scenarios" :key="g.name">
                  <nldd-button
                    :appearance="j === 0 ? 'inherit-filled' : 'inherit-tinted'"
                    size="lg"
                    :text="g.name"
                    @click="choose(g.name)"
                  ></nldd-button>
                  <span class="scenario-summary">{{ g.summary }}</span>
                </li>
              </ol>
            </template>

            <template v-else>
              <span v-if="slide.overline" class="overline">{{ slide.overline }}</span>
              <h1 class="title">{{ slide.title }}</h1>
              <p v-if="slide.lead" class="lead" v-html="emphasize(slide.lead)"></p>
              <ul v-if="slide.bullets?.length" class="bullets">
                <li v-for="(b, j) in slide.bullets" :key="j" v-html="emphasize(b)"></li>
              </ul>
            </template>
          </div>
          <div v-if="slide.figure" class="figure">
            <Diagram v-if="slide.figure.diagram" :name="slide.figure.diagram" :label="slide.figure.label" />
            <figure v-else class="code">
              <pre tabindex="0" :aria-label="slide.figure.label"><code>{{ slide.figure.code }}</code></pre>
              <figcaption>{{ slide.figure.label }}</figcaption>
            </figure>
          </div>
        </div>
      </div>

      <div class="footer">
        <div class="footer-text">
          <span class="counter" :aria-label="`Dia ${p.index.value + 1} van ${p.total.value}`">{{ counter }}</span>
          <div class="hints">
            <span class="hint">
              <nldd-keyboard-shortcut size="sm" color="inherit" keys="←" always-visible></nldd-keyboard-shortcut>
              <nldd-keyboard-shortcut size="sm" color="inherit" keys="→" always-visible></nldd-keyboard-shortcut>
              of
              <nldd-keyboard-shortcut size="sm" color="inherit" keys="Space" always-visible></nldd-keyboard-shortcut>
              bladeren
            </span>
            <span class="hint">
              <nldd-keyboard-shortcut size="sm" color="inherit" keys="Esc" always-visible></nldd-keyboard-shortcut>
              sluiten
            </span>
            <span class="hint">
              <nldd-keyboard-shortcut size="sm" color="inherit" keys="F" always-visible></nldd-keyboard-shortcut>
              volledig scherm
            </span>
          </div>
        </div>
        <nldd-button-bar>
          <nldd-icon-button
            appearance="inherit-tinted"
            icon="back"
            text="Vorige dia"
            tooltip-timing="never"
            :disabled="p.index.value === 0 || undefined"
            @click="p.prev()"
          ></nldd-icon-button>
          <nldd-button v-if="p.isLast.value" appearance="inherit-tinted" text="Sluiten" @click="p.stop()"></nldd-button>
          <nldd-icon-button
            v-else
            appearance="inherit-tinted"
            icon="forward"
            text="Volgende dia"
            tooltip-timing="never"
            @click="p.next()"
          ></nldd-icon-button>
        </nldd-button-bar>
      </div>
      <div class="progress" aria-hidden="true"><div class="progress-fill" :style="{ width: progress }"></div></div>
    </div>
  </Teleport>
</template>

<style scoped>
/* Eigen CSS, zoals in het dek van de demo: het design system heeft geen
 * presentatiecomponent. De kleuren zijn tokens. Het dek is altijd donkerblauw
 * met witte tekst, ook in donkere modus: `color-scheme: light` houdt de
 * kleurschalen aan hun lichte kant, waar lintblauw-750 donkerblauw is. */
.deck {
  position: fixed;
  inset: 0;
  z-index: 80;
  display: flex;
  flex-direction: column;
  padding: 2vh 0 1.5rem;
  box-sizing: border-box;
  color-scheme: light;
  outline: none;
  --ink: var(--primitives-color-coolgray-0);
  --ink-96: color-mix(in srgb, var(--ink) 96%, transparent);
  --ink-85: color-mix(in srgb, var(--ink) 85%, transparent);
  --ink-72: color-mix(in srgb, var(--ink) 72%, transparent);
  --ink-55: color-mix(in srgb, var(--ink) 55%, transparent);
  --ink-35: color-mix(in srgb, var(--ink) 35%, transparent);
  --ink-18: color-mix(in srgb, var(--ink) 18%, transparent);
  --ink-08: color-mix(in srgb, var(--ink) 8%, transparent);
  --accent: var(--primitives-color-donkergeel-200);
  color: var(--ink);
  background: linear-gradient(160deg, var(--primitives-color-lintblauw-750) 0%, var(--primitives-color-lintblauw-700) 100%);
  font-family: 'RijksSans', system-ui, sans-serif;
}
/* Het podium: een gecentreerde kolom van hooguit 1600px. Zijn breedte is de
 * maat van de typografie (`--slide-unit`). */
.stage {
  --slide-unit: calc(1cqw);
  container-type: inline-size;
  flex: 1 1 auto;
  min-height: 0;
  width: min(100%, 1600px);
  margin: 0 auto;
  padding: 3vh clamp(1rem, 5vw, 6rem);
  box-sizing: border-box;
  display: flex;
  flex-direction: column;
}
/* Op een groot scherm scrolt een dia niet; de tekst schaalt mee. Op een
 * telefoon past een dia met een fragment niet altijd, en dan scrolt hij liever
 * dan dat hij iets afsnijdt. */
.content {
  flex: 1 1 auto;
  min-height: 0;
  overflow-y: auto;
  display: grid;
  align-content: center;
  gap: max(1rem, calc(3 * var(--slide-unit)));
}
@container (min-width: 900px) {
  .content.with-figure {
    grid-template-columns: minmax(0, 1fr) minmax(0, 1.1fr);
    align-items: center;
  }
}
.content-main {
  display: flex;
  flex-direction: column;
  gap: max(0.5rem, calc(1.6 * var(--slide-unit)));
  min-width: 0;
}
.overline {
  font-size: max(0.8rem, calc(1.6 * var(--slide-unit)));
  font-weight: 600;
  letter-spacing: 0.02em;
  color: var(--ink-72);
}
.title {
  font-weight: 700;
  font-size: max(1.5rem, calc(4.4 * var(--slide-unit)));
  line-height: 1.08;
  letter-spacing: -0.015em;
  text-wrap: balance;
  margin: 0;
  color: var(--ink);
}
.with-figure .title {
  font-size: max(1.4rem, calc(3.6 * var(--slide-unit)));
}
.title-hero {
  font-size: max(2rem, calc(8 * var(--slide-unit)));
}
.statement {
  font-weight: 400;
  line-height: 1.2;
}
.statement-line {
  display: block;
  text-wrap: balance;
}
.lead {
  font-size: max(1rem, calc(2.3 * var(--slide-unit)));
  line-height: 1.4;
  margin: 0;
  font-weight: 500;
  text-wrap: pretty;
}
.with-figure .lead {
  font-size: max(1rem, calc(1.9 * var(--slide-unit)));
}
.lead-hero {
  font-style: italic;
  font-weight: 400;
  font-size: max(1.15rem, calc(3.4 * var(--slide-unit)));
  color: var(--ink-96);
}
.affiliation {
  margin-top: calc(1.5 * var(--slide-unit));
  font-size: max(0.95rem, calc(1.9 * var(--slide-unit)));
  color: var(--ink-85);
}
.bullets {
  font-size: max(0.95rem, calc(2 * var(--slide-unit)));
  line-height: 1.45;
  margin: 0;
  padding-left: 1.3em;
  display: flex;
  flex-direction: column;
  gap: calc(0.9 * var(--slide-unit));
  color: var(--ink-96);
  text-wrap: pretty;
}
.with-figure .bullets {
  font-size: max(0.95rem, calc(1.6 * var(--slide-unit)));
}
.bullets li::marker {
  color: var(--ink-72);
}
.bullets-plain {
  list-style: none;
  padding-left: 0;
  font-size: max(1rem, calc(2.6 * var(--slide-unit)));
}
.content :deep(strong) {
  font-weight: 700;
}
.content :deep(code),
pre {
  font-family: 'JetBrains Mono', ui-monospace, monospace;
}
.content :deep(code) {
  font-size: 0.86em;
  overflow-wrap: anywhere;
  background: var(--ink-08);
  padding: 0.05em 0.3em;
  border-radius: 4px;
}
.figure {
  min-width: 0;
}
.code {
  margin: 0;
  display: flex;
  flex-direction: column;
  gap: 0.5rem;
}
pre {
  margin: 0;
  padding: max(0.75rem, calc(1.4 * var(--slide-unit)));
  font-size: max(0.7rem, calc(1.1 * var(--slide-unit)));
  line-height: 1.45;
  background: color-mix(in srgb, var(--primitives-color-lintblauw-750) 70%, black);
  border: 1px solid var(--ink-18);
  border-radius: 8px;
  overflow: auto;
  max-height: 64vh;
  color: var(--ink-96);
}
.content pre code {
  background: none;
  border-radius: 0;
  padding: 0;
  font-size: inherit;
}
.code figcaption {
  font-size: max(0.75rem, calc(1.3 * var(--slide-unit)));
  color: var(--ink-72);
}
.scenarios {
  list-style: none;
  margin: calc(1.5 * var(--slide-unit)) 0 0;
  padding: 0;
  display: flex;
  flex-direction: column;
  gap: max(0.9rem, calc(1.6 * var(--slide-unit)));
}
.scenarios li {
  display: flex;
  flex-direction: column;
  align-items: flex-start;
  gap: 0.35rem;
}
.scenario-summary {
  font-size: max(0.9rem, calc(1.6 * var(--slide-unit)));
  color: var(--ink-85);
}
.footer {
  flex: 0 0 auto;
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 1rem;
  padding: 1rem clamp(1rem, 4vw, 4rem) 0;
}
.footer-text {
  display: flex;
  flex-direction: column;
  gap: 0.6rem;
  min-width: 0;
}
.counter {
  font-size: 0.95rem;
  font-weight: 600;
  color: var(--ink-72);
  font-variant-numeric: tabular-nums;
}
.hints {
  display: flex;
  flex-wrap: wrap;
  gap: 0.4rem 1rem;
  font-size: 0.8rem;
  color: var(--ink-72);
}
.hint {
  display: inline-flex;
  align-items: center;
  gap: 0.35rem;
}
.progress {
  position: absolute;
  left: 0;
  right: 0;
  bottom: 0;
  height: 3px;
  background: var(--ink-18);
}
.progress-fill {
  height: 100%;
  background: var(--accent);
  transition: width 0.3s ease;
}
@media (prefers-reduced-motion: reduce) {
  .progress-fill {
    transition: none;
  }
}
/* Op een telefoon zijn de toetsen er niet; de knoppen wel. */
@media (max-width: 640px) {
  .hints {
    display: none;
  }
}
</style>
