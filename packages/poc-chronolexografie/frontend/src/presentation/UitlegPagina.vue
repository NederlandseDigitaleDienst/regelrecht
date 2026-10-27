<script setup>
// The explanation page: one long page in the form of the deck (deck.css),
// over the whole screen, that shows with small pieces of YAML how law,
// policy, cell, reduction and the reduction of the Awb hang together, on one
// case (uitleg.js). It scrolls; nothing on it opens or closes. The choice
// screen opens it, and so does an address with `?uitleg` (useUitleg.js).
import { ref } from 'vue';
import { emphasize } from './emphasize.js';
import { intro, steps } from './uitleg.js';
import { useOverlay } from './useOverlay.js';
import { useUitleg } from './useUitleg.js';

const u = useUitleg();
const page = ref(null);
const { top } = useOverlay(() => u.active.value, page);
</script>

<template>
  <Teleport to="body">
    <div
      v-if="u.active.value"
      ref="page"
      class="deck uitleg"
      role="region"
      :aria-label="intro.title"
      tabindex="-1"
      :style="{ top: `${top}px` }"
    >
      <div class="bar">
        <nldd-button appearance="inherit-tinted" text="Sluiten" @click="u.close()"></nldd-button>
      </div>
      <article class="stage">
        <header class="intro">
          <span class="overline">{{ intro.overline }}</span>
          <h1 class="title title-hero">{{ intro.title }}</h1>
          <p class="lead" v-html="emphasize(intro.lead)"></p>
        </header>
        <ol class="steps">
          <li v-for="(s, i) in steps" :key="s.title" class="step">
            <span class="overline">Stap {{ i + 1 }} · {{ s.overline }}</span>
            <h2 class="title">{{ s.title }}</h2>
            <p v-for="(t, j) in s.text" :key="j" class="lead" v-html="emphasize(t)"></p>
            <div class="panels">
              <figure v-for="pnl in s.panels" :key="pnl.label" class="code">
                <pre tabindex="0" :aria-label="pnl.label"><code>{{ pnl.code }}</code></pre>
                <figcaption>{{ pnl.label }} · <code>{{ pnl.source.split('/').slice(-3).join('/') }}</code></figcaption>
              </figure>
            </div>
          </li>
        </ol>
      </article>
    </div>
  </Teleport>
</template>

<style scoped src="./deck.css"></style>

<style scoped>
/* De opbouw van de uitlegpagina: één kolom die scrolt, in de vorm van het dek
 * (deck.css). Eigen CSS om dezelfde reden als het dek: het design system heeft
 * geen presentatiecomponent. */
.uitleg {
  position: fixed;
  inset: 0;
  z-index: 80;
  overflow-y: auto;
  box-sizing: border-box;
}
.bar {
  position: sticky;
  top: 0;
  display: flex;
  justify-content: flex-end;
  padding: 0.75rem clamp(1rem, 4vw, 4rem);
  background: color-mix(in srgb, var(--primitives-color-lintblauw-750) 92%, transparent);
  z-index: 1;
}
/* Een kolom van hooguit 1100px. De typografie volgt de kolombreedte, kleiner
 * dan op een dia: dit is een pagina om te lezen. */
.stage {
  --slide-unit: calc(0.62cqw);
  container-type: inline-size;
  width: min(100%, 1100px);
  margin: 0 auto;
  padding: 1rem 1rem 4rem;
  box-sizing: border-box;
}
.intro,
.step {
  display: flex;
  flex-direction: column;
  gap: max(0.5rem, calc(1.4 * var(--slide-unit)));
}
.intro {
  margin-bottom: max(2rem, calc(5 * var(--slide-unit)));
}
.steps {
  list-style: none;
  margin: 0;
  padding: 0;
  display: flex;
  flex-direction: column;
  gap: max(2.5rem, calc(7 * var(--slide-unit)));
}
.step + .step {
  border-top: 1px solid var(--ink-18);
  padding-top: max(2rem, calc(5 * var(--slide-unit)));
}
.panels {
  display: grid;
  grid-template-columns: repeat(auto-fit, minmax(min(100%, 420px), 1fr));
  gap: 1rem;
  margin-top: 0.5rem;
}
.panels pre {
  font-size: max(0.75rem, calc(1.5 * var(--slide-unit)));
}
</style>
