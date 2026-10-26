<script setup>
// De kroniek van de cel: elk chronolexogram, in de volgorde waarin het werd
// vastgelegd, en wat erin staat (RFC-046). Daaronder de kronieken van de
// andere cellen van de sfeer (RFC-052): elke cel houdt de hare bij; het
// bestuursorgaan vraagt ze, maar schrijft er niet in. De rechtbank is er
// een van (RFC-053). Bij een gekozen chronolexogram: waarnaar het verwijst (`refers_to`), wat het
// van een andere cel ontving (`received`) en na welk chronolexogram het
// opnieuw is genomen (`after`), welke bevindingen een besluit gebruikte en
// de rest als JSON, met decimalen als getal. Welke bevindingen een besluit
// gebruikte, staat niet in de kroniek: de cel leidt het af uit het besluit
// opnieuw genomen (Cell::findings, RFC-050).
import { computed, ref, watch } from 'vue';
import { api } from '../api.js';
import { beroepApi, herkomst } from '../beroep.js';
import { alsJson, isBesluit, kort, leesbaar, state, waarde } from '../state.js';

// De kroniek van de cel van de casus, dan die van de andere cellen. Waarom
// een besluit zo uitviel, vraagt de app aan de cel van de casus en aan de
// rechtbank; de redenering van een andere cel is de hare (RFC-048).
const kronieken = computed(() => [
  { naam: state.casus.cel_naam, grammen: state.grammen, eigen: true, rechter: false },
  ...state.andere.map((a) => ({
    naam: a.cel_naam,
    grammen: a.grammen,
    eigen: false,
    rechter: a.cel === state.casus.rechter?.cel,
  })),
]);
const vind = (id) => {
  for (const k of kronieken.value) {
    const g = k.grammen.find((x) => x.id === id);
    if (g) return { gram: g, eigen: k.eigen, rechter: k.rechter };
  }
  return null;
};

const gekozen = ref(state.inKroniek);
state.inKroniek = null;
const bevindingen = ref([]);
watch(gekozen, async (id) => {
  bevindingen.value = [];
  const gevonden = vind(id);
  if (!gevonden || !(gevonden.eigen || gevonden.rechter) || !isBesluit(gevonden.gram)) return;
  try {
    const antwoord = await (gevonden.rechter ? beroepApi.waarom(id) : api.waarom(id));
    if (gekozen.value === id) bevindingen.value = antwoord.bevindingen;
  } catch (e) {
    state.fout = e.message;
  }
}, { immediate: true });
const keuze = computed(() => vind(gekozen.value));
const gram = computed(() => keuze.value?.gram ?? null);
const naar = computed(() => vind(gram.value?.refers_to)?.gram ?? null);
const ontvangen = computed(() => vind(gram.value?.received?.id)?.gram ?? null);
const na = computed(() => vind(gram.value?.after)?.gram ?? null);
const wie = computed(() => (keuze.value?.rechter ? 'de rechter' : 'het bestuursorgaan'));
</script>

<template>
  <nldd-simple-section v-for="k in kronieken" :key="k.naam">
    <nldd-title slot="header" size="3" :text="`Kroniek van ${k.naam}`">
      <nldd-text v-if="k.eigen" slot="supporting-text">
        Eén kroniek, van de cel van de uitvoerder. Elke stap in het proces legt er één
        chronolexogram in vast (een beslissing op bezwaar die herroept, legt er twee vast); er wordt
        nooit iets in gewijzigd of uit verwijderd.<template v-if="state.casus?.begin"> Bovenaan
        staat wat de cel vóór het eerste scenario vastlegde, zoals de vestiging van een school
        (RFC-051).</template>
      </nldd-text>
      <nldd-text v-else-if="k.rechter" slot="supporting-text">
        De kroniek van de rechtbank, een andere cel. Een beroep verwijst niet naar het bestreden
        besluit, want dat staat in een andere kroniek: het draagt wat de rechtbank ontving.
      </nldd-text>
      <nldd-text v-else slot="supporting-text">Een andere cel van de sfeer, met haar eigen kroniek.</nldd-text>
    </nldd-title>
    <nldd-inline-dialog
      v-if="k.grammen.length === 0"
      icon="info"
      text="De kroniek is leeg"
      :supporting-text="k.eigen ? 'Dien als aanvrager een aanvraag in om het eerste chronolexogram te maken.' : k.rechter ? 'Stel als aanvrager beroep in tegen een beslissing op bezwaar.' : undefined"
    ></nldd-inline-dialog>
    <nldd-list v-else appearance="box-base" :accessible-label="`Chronolexogrammen van ${k.naam}`">
      <nldd-list-item
        v-for="(g, i) in k.grammen"
        :key="g.id"
        button
        :selected="g.id === gekozen"
        @click="gekozen = g.id"
      >
        <nldd-text-cell
          :overline="`${i + 1} · ${g.recorded_on}`"
          :text="`${g.kind} · ${g.basis}`"
          :supporting-text="[kort(g.id), herkomst(g)].filter(Boolean).join(' · ')"
        ></nldd-text-cell>
      </nldd-list-item>
    </nldd-list>
  </nldd-simple-section>
  <nldd-simple-section v-if="gram">
    <nldd-title slot="header" size="4" :text="`Chronolexogram ${kort(gram.id)}`"></nldd-title>
    <nldd-container gap="16">
      <nldd-list appearance="box-base" accessible-label="Samenhang">
        <nldd-list-item size="sm" :button="!!naar" @click="naar && (gekozen = naar.id)">
          <nldd-text-cell
            text="Verwijst naar"
            :supporting-text="naar ? `${naar.kind} ${kort(naar.id)} · ${naar.basis}` : 'niets'"
          ></nldd-text-cell>
        </nldd-list-item>
        <nldd-list-item v-if="gram.received" size="sm" :button="!!ontvangen" @click="ontvangen && (gekozen = ontvangen.id)">
          <nldd-text-cell
            text="Ontvangen van"
            :supporting-text="`${gram.received.from} · ${ontvangen ? `${ontvangen.kind} ${kort(ontvangen.id)} · ${ontvangen.basis}` : kort(gram.received.id)}`"
          ></nldd-text-cell>
        </nldd-list-item>
        <nldd-list-item v-if="gram.after" size="sm" :button="!!na" @click="na && (gekozen = na.id)">
          <nldd-text-cell
            text="Opnieuw genomen na"
            :supporting-text="na ? `${na.kind} ${kort(na.id)} · ${na.basis}` : kort(gram.after)"
          ></nldd-text-cell>
        </nldd-list-item>
        <nldd-list-item size="sm">
          <nldd-text-cell
            :text="`Bevindingen van ${wie}`"
            :supporting-text="bevindingen.length ? 'die dit besluit gebruikte (RFC-050)' : 'geen'"
          ></nldd-text-cell>
        </nldd-list-item>
        <nldd-list-item v-for="f in bevindingen" :key="f" size="sm">
          <nldd-text-cell :text="leesbaar(f)"></nldd-text-cell>
          <nldd-text-cell width="fit-content" :text="waarde(gram.fields[f])"></nldd-text-cell>
        </nldd-list-item>
      </nldd-list>
      <nldd-code-viewer language="json" appearance="box-tinted">{{ alsJson(gram) }}</nldd-code-viewer>
    </nldd-container>
  </nldd-simple-section>
</template>
