<script setup>
// The sheet a line of the chain opens (herkomst.js). For law or policy it
// shows the text of the article, with its `machine_readable` as written in
// the YAML behind a button (UitklapCode.vue). For a reduction, each answering
// cell gets two parts: the reduction that answered (a link to its article)
// and the chronicle it read.
// The heading is the term and its value, as the line of the chain says it.
// A link to the article of a chronolexogram (ArtikelLink.vue) opens it too.
// One sheet for the whole app.
import { computed, ref, watch } from 'vue';
import { artikelInhoud, gelezenGrammen, getoond, leden, SOORTEN } from '../herkomst.js';
import { artikelNaam, celNaam, gelezen, grammenVan, toon } from '../logboek.js';
import { hoofdletter, leesbaar, state } from '../state.js';
import ArtikelLink from './ArtikelLink.vue';
import GramCode from './GramCode.vue';
import UitklapCode from './UitklapCode.vue';

const h = getoond;
const sluiten = () => {
  getoond.value = null;
};
// null while loading, then { leden, machineReadable } or { fout }.
const tekst = ref(null);

watch(h, async (nu) => {
  tekst.value = null;
  if (!nu?.artikel) return;
  try {
    const a = await artikelInhoud(nu.wet, nu.artikel, nu.datum ?? undefined);
    if (h.value !== nu) return;
    tekst.value = { leden: a?.tekst ? leden(a.tekst) : [], machineReadable: a?.machineReadable ?? null };
  } catch (e) {
    if (h.value !== nu) return;
    tekst.value = { fout: e.message };
  }
});

const kop = computed(() => {
  const nu = h.value;
  if (!nu) return '';
  if (nu.artikel) return artikelNaam(nu.wet, nu.artikel);
  return nu.begrip ? `${hoofdletter(leesbaar(nu.begrip))}: ${toon(nu.waarde)}` : 'Gelezen uit de kroniek';
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
            :supporting-text="tekst.fout ?? 'De tekst van dit artikel zit niet in de bestanden van de casus.'"
          ></nldd-inline-dialog>
          <nldd-blockquote v-else lang="nl">
            <p v-for="(lid, i) in tekst.leden" :key="i">{{ lid }}</p>
          </nldd-blockquote>
          <template v-if="tekst?.machineReadable">
            <nldd-spacer size="16"></nldd-spacer>
            <UitklapCode
              :key="`${h.wet}#${h.artikel}`"
              text="machine_readable"
              :naam="`machine_readable van ${kop}`"
              language="yaml"
              :code="tekst.machineReadable"
            />
          </template>
        </nldd-simple-section>
        <nldd-simple-section v-else-if="h?.antwoorden">
          <nldd-container gap="24">
            <nldd-container v-for="(a, i) in h.antwoorden" :key="`${i}-${a.answerer}-${a.answer}`" gap="12">
              <nldd-title heading-level="3" size="display-5" text="Reductie">
                <nldd-text slot="supporting-text" size="sm" color="secondary">
                  {{ celNaam(a.answerer) }} ·
                  <ArtikelLink :basis="`${a.law_id}#${a.article}`" :datum="a.valid_from" />
                </nldd-text>
              </nldd-title>
              <nldd-title heading-level="3"
                size="display-5"
                text="Kroniek"
                :supporting-text="gelezen(a, grammenVan(a.answerer, state.grammen))"
              ></nldd-title>
              <nldd-text v-if="a.last_record && !gelezenGrammen(a).length" size="sm" color="secondary">
                Dat chronolexogram staat niet in een kroniek die de app heeft.
              </nldd-text>
              <GramCode v-for="g in gelezenGrammen(a)" :key="g.id" :gram="g" :heading-level="4" />
            </nldd-container>
          </nldd-container>
        </nldd-simple-section>
      </nldd-page>
    </nldd-sheet>
  </Teleport>
</template>
