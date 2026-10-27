<script setup>
// The explanation page: a page of the app, built like the choice screen
// (CasusKeuze.vue), that shows with small pieces of YAML how law, policy,
// cell, reduction and the reduction of the Awb hang together, on one case
// (uitleg.js). Each step is a section with a heading, two or three sentences
// and a code viewer per panel. It scrolls; nothing on it opens or closes.
// The choice screen opens it, and so does an address with `?uitleg`
// (useUitleg.js); the button at the top goes back to the choice screen.
import { nextTick, onMounted, ref } from 'vue';
import { intro, steps } from './uitleg.js';
import { useUitleg } from './useUitleg.js';

const emit = defineEmits(['terug']);
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
      <nldd-text v-for="(t, j) in s.text" :key="j">
        <template v-for="(st, k) in stukken(t)" :key="k"><code v-if="st.code">{{ st.code }}</code><template v-else>{{ st.tekst }}</template></template>
      </nldd-text>
      <nldd-container v-for="p in s.panels" :key="p.label" gap="4">
        <!-- The caption comes first, so it is read before the code. A file
             name has no place to break, hence overflow-wrap. -->
        <nldd-text size="sm" color="secondary" style="overflow-wrap: anywhere">{{ p.label }} · {{ kortPad(p.source) }}</nldd-text>
        <nldd-code-viewer language="yaml" appearance="box-tinted">{{ p.code }}</nldd-code-viewer>
      </nldd-container>
    </nldd-container>
  </nldd-simple-section>
</template>
