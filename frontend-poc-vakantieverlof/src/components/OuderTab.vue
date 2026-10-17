<script setup>
// De ouder: een aanvraag om vakantieverlof doen, zien hoe het ervoor staat,
// en bezwaar maken tegen een besluit. Welke velden er zijn, zegt de wet
// (artikel 13a, de Awb-haken 4:2 en, voor het bezwaar, 6:5 via 7:11). Een
// bezwaar is een indiening onder Awb 7:11 die naar het bestreden besluit
// verwijst; of het ontvankelijk is, beslist 7:11, niet deze pagina.
import { computed, reactive, ref } from 'vue';
import { api } from '../api.js';
import { aanvragen, dossier, geldendBesluit, isBesluit, kort, leesbaar, spel, stap } from '../spel.js';
import VeldInvoer from './VeldInvoer.vue';

// Fictieve voorbeeldgegevens, zodat je snel door het proces klikt. Een
// BSN uit de testreeks; vijf schooldagen, dus het oordeel van het hoofd
// over het beroep van de ouder beslist.
const aanvraag = reactive({
  bsn_jongere: '999990019',
  eerste_verlofdag: '2027-02-01',
  laatste_verlofdag: '2027-02-05',
  aanvraag_ondertekend: true,
  naam_aanvrager: 'J. Jansen',
  adres_aanvrager: 'Voorbeeldstraat 1',
  dagtekening: '2026-10-12',
  gevraagde_beschikking: 'vakantieverlof',
});

const zonderLeeg = (velden) =>
  Object.fromEntries(Object.entries(velden).filter(([, v]) => v !== undefined));

async function indienen() {
  await stap(() => api.aanvragen(zonderLeeg(aanvraag), spel.datum));
}

// Het bezwaar tegen één besluit tegelijk.
const bezwaarTegen = ref(null);
const bezwaar = reactive({});

const bezwaarVelden = computed(() => {
  const besluit = bezwaarTegen.value;
  if (!besluit || !spel.vormen) return [];
  // Het bestreden besluit en wat het al bevat (de bezwaartermijn, Awb 6:8)
  // vult de cel zelf.
  return spel.vormen.bezwaar.velden.filter(
    (v) => !(v.naam in (besluit.outputs ?? {})) && !(v.naam in (besluit.fields ?? {})),
  );
});

function startBezwaar(besluit) {
  bezwaarTegen.value = besluit;
  Object.assign(bezwaar, {
    bezwaarschrift_ondertekend: true,
    naam_indiener: 'J. Jansen',
    adres_indiener: 'Voorbeeldstraat 1',
    dagtekening_bezwaarschrift: spel.datum,
    omschrijving_bestreden_besluit: 'weigering vakantieverlof',
    gronden_bezwaar: 'verklaring van de werkgever: geen verlof in de schoolvakanties',
  });
}

async function bezwaarIndienen() {
  const besluit = bezwaarTegen.value;
  const ok = await stap(() => api.bezwaren(besluit.id, zonderLeeg(bezwaar), spel.datum));
  if (ok) bezwaarTegen.value = null;
}

</script>

<template>
  <nldd-simple-section>
    <nldd-title slot="header" size="3" text="Aanvraag om vakantieverlof">
      <nldd-text slot="supporting-text">
        Leerplichtwet 1969 artikel 13a. De velden komen uit de wet: wat het artikel vraagt en
        wat de Algemene wet bestuursrecht erbij eist (artikel 4:2). Een leeg veld laat je weg;
        de cel legt de aanvraag dan vast met dat gegeven als ontbrekend. De dag van ontvangst
        is de speeldatum (Awb 4:13).
      </nldd-text>
    </nldd-title>
    <nldd-container v-if="spel.vormen" gap="16" max-width="560px">
      <VeldInvoer
        v-for="veld in spel.vormen.aanvraag.velden"
        :key="veld.naam"
        v-model="aanvraag[veld.naam]"
        :veld="veld"
      />
      <nldd-button appearance="primary" text="Aanvraag indienen" @click="indienen"></nldd-button>
    </nldd-container>
  </nldd-simple-section>

  <nldd-simple-section>
    <nldd-title slot="header" size="3" text="Mijn aanvragen"></nldd-title>
    <nldd-inline-dialog
      v-if="aanvragen().length === 0"
      icon="info"
      text="Nog geen aanvragen"
      supporting-text="Dien hierboven een aanvraag in."
    ></nldd-inline-dialog>
    <nldd-container gap="16">
      <nldd-card v-for="a in aanvragen()" :key="a.id">
        <nldd-container padding="16" gap="12">
        <nldd-title
          size="5"
          :text="`Aanvraag ${kort(a.id)}`"
          :supporting-text="`${a.fields.eerste_verlofdag} t/m ${a.fields.laatste_verlofdag} · ontvangen ${a.recorded_on}`"
        >
          <nldd-tag
            slot="end"
            :color="geldendBesluit(a) ? (geldendBesluit(a).outputs.verlof_verleend ? 'success' : 'critical') : 'neutral'"
            :text="geldendBesluit(a) ? (geldendBesluit(a).outputs.verlof_verleend ? 'Verleend' : 'Geweigerd') : 'In behandeling'"
          ></nldd-tag>
        </nldd-title>
        <nldd-list>
          <nldd-list-item v-for="s in dossier(a)" :key="s.gram.id" size="sm">
            <nldd-text-cell
              :text="`${s.rol} ${kort(s.gram.id)}`"
              :supporting-text="`${s.gram.recorded_on} · ${s.gram.grondslag}`"
            ></nldd-text-cell>
            <nldd-cell v-if="isBesluit(s.gram)" width="fit-content">
              <nldd-button
                appearance="secondary"
                size="sm"
                text="Bezwaar maken"
                @click="startBezwaar(s.gram)"
              ></nldd-button>
            </nldd-cell>
          </nldd-list-item>
        </nldd-list>
        <nldd-text v-if="a.missing?.length" color="warning" size="sm">
          Ontbreekt in de aanvraag: {{ a.missing.map(leesbaar).join(', ') }}
        </nldd-text>
        </nldd-container>
      </nldd-card>
    </nldd-container>
  </nldd-simple-section>

  <nldd-simple-section v-if="bezwaarTegen">
    <nldd-title slot="header" size="3" :text="`Bezwaar tegen besluit ${kort(bezwaarTegen.id)}`">
      <nldd-text slot="supporting-text">
        De velden komen uit de Awb (artikel 6:5, via 7:11). Het bestreden besluit en de
        bezwaartermijn neemt de cel uit het besluit. Het bezwaar wijzigt de aanvraag niet: het
        hoofd heroverweegt het besluit op grond van wat je aanvoert. Wil je minder dagen
        vragen, dan doe je een nieuwe aanvraag.
      </nldd-text>
    </nldd-title>
    <nldd-container gap="16" max-width="560px">
      <VeldInvoer v-for="veld in bezwaarVelden" :key="veld.naam" v-model="bezwaar[veld.naam]" :veld="veld" />
      <nldd-button-group>
        <nldd-button appearance="primary" text="Bezwaar indienen" @click="bezwaarIndienen"></nldd-button>
        <nldd-button appearance="neutral-transparent" text="Annuleren" @click="bezwaarTegen = null"></nldd-button>
      </nldd-button-group>
    </nldd-container>
  </nldd-simple-section>
</template>
