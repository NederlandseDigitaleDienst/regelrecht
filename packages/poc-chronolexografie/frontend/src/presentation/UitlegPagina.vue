<script setup>
// The explanation page: a page of the app, built like the choice screen
// (CasusKeuze.vue), that shows with small pieces of YAML how law, policy,
// cell, reduction and the reduction of the Awb hang together, on one case
// (uitleg.js). Each step is a section with a heading, the text of the
// article it rests on where there is one, two or three
// sentences, and per panel a whole file of the corpus as a tree that folds,
// as the demo shows a law (YamlNode from frontend-shared), with the parts the
// step is about open. The files come from the bundle the cell
// loads too (api.js); a test passes them as `bestanden`.
// The choice screen opens it, and so does an address with `?uitleg`
// (useUitleg.js); the button at the top goes back to the choice screen, the
// one at the bottom opens the case the page follows.
import { nextTick, onMounted, ref } from 'vue';
import { load } from 'js-yaml';
import YamlNode from '@regelrecht/frontend-shared/components/YamlNode.vue';
import { bestanden as haalBestanden } from '../api.js';
import { bronnen, casus, intro, openPaden, steps } from './uitleg.js';
import { useUitleg } from './useUitleg.js';

const props = defineProps({
  /** The files of the panels, per path; without them the page loads the bundle. */
  bestanden: { type: Object, default: null },
});
const emit = defineEmits(['terug', 'casus']);

/** The parsed files of the panels, per path, once they are there. */
const docs = ref(props.bestanden ? lees(props.bestanden) : null);
const fout = ref(false);
function lees(alle) {
  return Object.fromEntries(bronnen.map((pad) => [pad, load(alle[pad])]));
}
onMounted(async () => {
  if (docs.value) return;
  try {
    docs.value = lees(await haalBestanden());
  } catch {
    fout.value = true;
  }
});

/** What starts open, in the form YamlNode takes. */
const stand = (p) => ({ paths: openPaden(p), folded: [], version: 0, all: null });
// Opened by a click, the button that opened the page is gone; the focus
// starts on the way back.
const terugKnop = ref(null);
const uitleg = useUitleg();
onMounted(async () => {
  if (!uitleg.neemFocus()) return;
  await nextTick();
  terugKnop.value?.focus();
});

/** A sentence in pieces: plain text, and `name` as code. */
const stukken = (zin) =>
  String(zin)
    .split(/(`[^`]+`)/)
    .filter(Boolean)
    .map((s) => (s.startsWith('`') ? { code: s.slice(1, -1) } : { tekst: s }));

/** The file of a panel, short: its last three parts. */
const kortPad = (source) => source.split('/').slice(-3).join('/');
</script>

<template>
  <nldd-simple-section>
    <nldd-container gap="16">
      <nldd-button-bar>
        <nldd-button ref="terugKnop" appearance="neutral-tinted" start-icon="back" text="Naar de casuskeuze" @click="emit('terug')"></nldd-button>
      </nldd-button-bar>
      <nldd-title size="2" heading-level="1" :overline="intro.overline" :text="intro.title">
        <nldd-text slot="supporting-text">
          <template v-for="(s, i) in stukken(intro.lead)" :key="i"><code v-if="s.code">{{ s.code }}</code><template v-else>{{ s.tekst }}</template></template>
        </nldd-text>
      </nldd-title>
    </nldd-container>
  </nldd-simple-section>
  <nldd-simple-section v-for="(s, i) in steps" :key="s.title">
    <nldd-title slot="header" size="3" heading-level="2" :overline="`Stap ${i + 1} · ${s.overline}`" :text="s.title"></nldd-title>
    <nldd-container gap="12">
      <!-- The text of the article the step rests on: text, not YAML, and
           Dutch. Its source is a plain caption under it; the quote's own
           attribution slot would put a dash in front. -->
      <nldd-container v-if="s.quote" gap="4">
        <nldd-blockquote lang="nl">
          <p v-for="(lid, j) in [].concat(s.quote.tekst)" :key="j">{{ lid }}</p>
        </nldd-blockquote>
        <nldd-text size="sm" color="secondary">{{ s.quote.label }}</nldd-text>
      </nldd-container>
      <nldd-text v-for="(t, j) in s.text" :key="j">
        <template v-for="(st, k) in stukken(t)" :key="k"><code v-if="st.code">{{ st.code }}</code><template v-else>{{ st.tekst }}</template></template>
      </nldd-text>
      <nldd-container v-for="p in s.panels" :key="p.label" gap="4">
        <!-- The caption comes first, so it is read before the code. A file
             name has no place to break, hence overflow-wrap. -->
        <nldd-text size="sm" color="secondary" style="overflow-wrap: anywhere">{{ p.label }} · {{ kortPad(p.source) }}</nldd-text>
        <nldd-box>
          <nldd-container padding="12">
            <div class="yaml-tree" lang="nl" style="font-size: 0.875rem">
              <YamlNode v-if="docs" :value="docs[p.source]" :expanded="stand(p)" />
              <nldd-text v-else size="sm" color="secondary">{{ fout ? 'De YAML kwam niet binnen. Herlaad de pagina om het opnieuw te proberen.' : 'De YAML wordt geladen.' }}</nldd-text>
            </div>
          </nldd-container>
        </nldd-box>
      </nldd-container>
    </nldd-container>
  </nldd-simple-section>
  <nldd-simple-section>
    <nldd-button-bar>
      <nldd-button appearance="primary" text="Naar de casus" @click="emit('casus', casus)"></nldd-button>
    </nldd-button-bar>
  </nldd-simple-section>
</template>

<style scoped>
/* A name in a sentence has no place to break; on a phone it would run out of
 * the page. */
code {
  overflow-wrap: anywhere;
}
</style>
