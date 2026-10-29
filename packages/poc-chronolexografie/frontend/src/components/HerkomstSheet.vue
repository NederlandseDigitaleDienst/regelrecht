<script setup>
// The sheet a line of the chain opens (herkomst.js): for law or policy the
// text of the article, for a reduction the chronolexograms each cell read for
// its answer. One sheet for the whole app.
import { computed, ref, watch } from 'vue';
import { artikelTekst, gelezenGrammen, getoond, leden, SOORTEN } from '../herkomst.js';
import { artikelNaam, celNaam } from '../logboek.js';
import { leesbaar } from '../state.js';
import GramCode from './GramCode.vue';

const h = getoond;
const sluiten = () => {
  getoond.value = null;
};
// null while loading, then { leden } or { fout }.
const tekst = ref(null);

watch(h, async (nu) => {
  tekst.value = null;
  if (!nu?.artikel) return;
  try {
    const t = await artikelTekst(nu.wet, nu.artikel);
    if (h.value !== nu) return;
    tekst.value = { leden: t ? leden(t) : [] };
  } catch (e) {
    if (h.value !== nu) return;
    tekst.value = { fout: e.message };
  }
});

const kop = computed(() => {
  const nu = h.value;
  if (!nu) return '';
  return nu.artikel ? artikelNaam(nu.wet, nu.artikel) : 'Gelezen uit de kroniek';
});
</script>

<template>
  <Teleport to="body">
    <nldd-sheet width="720px" :open="h ? true : undefined" @close="sluiten">
      <nldd-page>
        <nldd-container slot="header" padding="16">
          <nldd-top-title-bar
            heading-level="2"
            :text="kop"
            :supporting-text="h ? SOORTEN[h.soort].tekst : ''"
            dismiss-text="Sluiten"
          ></nldd-top-title-bar>
        </nldd-container>
        <nldd-simple-section v-if="h?.artikel">
          <nldd-text v-if="!tekst" size="sm" color="secondary">De tekst wordt geladen.</nldd-text>
          <nldd-inline-dialog
            v-else-if="tekst.fout || !tekst.leden.length"
            icon="info"
            text="Geen tekst"
            :supporting-text="tekst.fout ?? 'De tekst van dit artikel zit niet in de bundel van de casus.'"
          ></nldd-inline-dialog>
          <nldd-blockquote v-else lang="nl">
            <p v-for="(lid, i) in tekst.leden" :key="i">{{ lid }}</p>
          </nldd-blockquote>
        </nldd-simple-section>
        <nldd-simple-section v-else-if="h?.antwoorden">
          <nldd-container gap="24">
            <nldd-container v-for="a in h.antwoorden" :key="`${a.answerer}-${a.answer}`" gap="12">
              <nldd-title
                size="5"
                :text="`${leesbaar(a.answer)} van ${celNaam(a.answerer)}`"
                :supporting-text="`reductie ${a.law_id} · ${a.records_read} chronolexogram${a.records_read === 1 ? '' : 'men'} gelezen`"
              ></nldd-title>
              <nldd-text v-if="!a.last_record" size="sm" color="secondary">
                De reductie las niets uit de kroniek.
              </nldd-text>
              <GramCode v-for="g in gelezenGrammen(a)" :key="g.id" :gram="g" />
            </nldd-container>
          </nldd-container>
        </nldd-simple-section>
      </nldd-page>
    </nldd-sheet>
  </Teleport>
</template>
