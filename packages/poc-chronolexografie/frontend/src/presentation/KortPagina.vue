<script setup>
// The short page: what RegelRecht is and what chronolexografie adds, on one
// screen (kort.js). First two short lists, then a picture of the two layers
// (KortSchema.vue), then a case whose steps carry the numbers in the picture.
// The choice screen opens it, and so does an address with `?kort`
// (useKort.js); the button at the top goes back to the choice screen. The
// button under the case opens that case, the one at the bottom the
// explanation page (UitlegPagina.vue). Both are links too, so they open in a
// tab of their own.
import { nextTick, onMounted, ref } from 'vue';
import { routePath } from '../route.js';
import KortSchema from './KortSchema.vue';
import { casus, casusBlok, chronolexografie, intro, regelrecht, schema, verder } from './kort.js';
import { useKort } from './useKort.js';
import { PARAM as UITLEG } from './useUitleg.js';

const emit = defineEmits(['terug', 'casus', 'uitleg']);

const base = import.meta.env.BASE_URL;
const casusHref = routePath(base, casus);
const uitlegHref = `${base}?${UITLEG}`;

// Opened by a click, the button that opened the page is gone; the focus
// starts on the way back.
const terugKnop = ref(null);
const kort = useKort();
onMounted(async () => {
  if (!kort.neemFocus()) return;
  await nextTick();
  terugKnop.value?.focus();
});

/** A sentence in pieces: plain text, and `name` as code. */
const stukken = (zin) =>
  String(zin)
    .split(/(`[^`]+`)/)
    .filter(Boolean)
    .map((s) => (s.startsWith('`') ? { code: s.slice(1, -1) } : { tekst: s }));
</script>

<template>
  <nldd-simple-section>
    <nldd-container gap="16">
      <nldd-button-bar>
        <nldd-button ref="terugKnop" appearance="neutral-tinted" start-icon="back" text="Naar de casuskeuze" @click="emit('terug')"></nldd-button>
      </nldd-button-bar>
      <nldd-title size="2" heading-level="1" :overline="intro.overline" :text="intro.title">
        <nldd-text slot="supporting-text">{{ intro.lead }}</nldd-text>
      </nldd-title>
    </nldd-container>
  </nldd-simple-section>
  <nldd-simple-section v-for="lijst in [regelrecht, chronolexografie]" :key="lijst.title">
    <nldd-title slot="header" size="3" heading-level="2" :text="lijst.title"></nldd-title>
    <nldd-rich-text>
      <ul>
        <li v-for="b in lijst.bullets" :key="b">{{ b }}</li>
      </ul>
    </nldd-rich-text>
  </nldd-simple-section>
  <nldd-simple-section>
    <nldd-container gap="8">
      <KortSchema beschrijving="kort-bijschrift" />
      <nldd-text id="kort-bijschrift" size="sm" color="secondary">{{ schema.bijschrift }}</nldd-text>
    </nldd-container>
  </nldd-simple-section>
  <nldd-simple-section>
    <nldd-title slot="header" size="3" heading-level="2" :overline="casusBlok.overline" :text="casusBlok.title">
      <nldd-text slot="supporting-text">{{ casusBlok.lead }}</nldd-text>
    </nldd-title>
    <nldd-container gap="16">
      <nldd-rich-text>
        <ol>
          <li v-for="(s, i) in casusBlok.stappen" :key="i">
            <template v-for="(st, k) in stukken(s)" :key="k"><code v-if="st.code">{{ st.code }}</code><template v-else>{{ st.tekst }}</template></template>
          </li>
        </ol>
      </nldd-rich-text>
      <nldd-button-bar>
        <nldd-button appearance="secondary" :href="casusHref" :text="casusBlok.knop" @click.prevent="emit('casus', casus)"></nldd-button>
      </nldd-button-bar>
    </nldd-container>
  </nldd-simple-section>
  <nldd-simple-section>
    <nldd-button-bar>
      <nldd-button appearance="primary" :href="uitlegHref" :text="verder" @click.prevent="emit('uitleg')"></nldd-button>
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
