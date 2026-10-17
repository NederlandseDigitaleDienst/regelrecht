<script setup>
// De kroniek van de cel: elk chronolexogram, in de volgorde waarin het werd
// vastgelegd, en wat erin staat (RFC-046). Bij een gekozen chronolexogram:
// waarnaar het verwijst (`refers_to`), welke bevindingen van het hoofd een
// besluit gebruikte en de rest als JSON, met decimalen als getal. Welke
// bevindingen een besluit gebruikte, staat niet in de kroniek: de cel leidt
// het af uit het besluit opnieuw genomen (Cell::findings, RFC-050).
import { computed, ref, watch } from 'vue';
import { api } from '../api.js';
import { alsJson, isBesluit, kort, leesbaar, spel, waarde } from '../spel.js';

const gekozen = ref(null);
const bevindingen = ref([]);
watch(gekozen, async (id) => {
  bevindingen.value = [];
  const g = spel.grammen.find((x) => x.id === id);
  if (!g || !isBesluit(g)) return;
  try {
    const antwoord = await api.waarom(id);
    if (gekozen.value === id) bevindingen.value = antwoord.bevindingen;
  } catch (e) {
    spel.fout = e.message;
  }
});
const gram = computed(() => spel.grammen.find((g) => g.id === gekozen.value) ?? null);
const verwijzing = (g) => (g.refers_to ? ` · verwijst naar ${kort(g.refers_to)}` : '');
const naar = computed(() => spel.grammen.find((g) => g.id === gram.value?.refers_to) ?? null);
</script>

<template>
  <nldd-simple-section>
    <nldd-title slot="header" size="3" text="Kroniek van OBS De Klimop">
      <nldd-text slot="supporting-text">
        Eén kroniek, van de cel van de school. Elke stap in het proces legt er één
        chronolexogram in vast (een beslissing op bezwaar die herroept, legt er twee vast); er wordt
        nooit iets in gewijzigd of uit verwijderd.
      </nldd-text>
    </nldd-title>
    <nldd-inline-dialog
      v-if="spel.grammen.length === 0"
      icon="info"
      text="De kroniek is leeg"
      supporting-text="Dien als ouder een aanvraag in om het eerste chronolexogram te maken."
    ></nldd-inline-dialog>
    <nldd-list v-else appearance="box-base" accessible-label="Chronolexogrammen">
      <nldd-list-item
        v-for="(g, i) in spel.grammen"
        :key="g.id"
        button
        :selected="g.id === gekozen"
        @click="gekozen = g.id"
      >
        <nldd-text-cell
          :overline="`${i + 1} · ${g.recorded_on}`"
          :text="`${g.kind} · ${g.grondslag}`"
          :supporting-text="`${kort(g.id)}${verwijzing(g)}`"
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
            :supporting-text="naar ? `${naar.kind} ${kort(naar.id)} · ${naar.grondslag}` : 'niets'"
          ></nldd-text-cell>
        </nldd-list-item>
        <nldd-list-item size="sm">
          <nldd-text-cell
            text="Bevindingen van het hoofd"
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
