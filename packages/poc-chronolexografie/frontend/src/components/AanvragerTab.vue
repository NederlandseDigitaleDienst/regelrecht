<script setup>
// De aanvrager: een aanvraag doen, zien hoe het ervoor staat, bezwaar
// maken tegen een besluit en in beroep gaan tegen een beslissing op bezwaar.
// Welke velden er zijn, zegt de wet (het artikel van de casus, de Awb-haken
// 4:2 en, voor bezwaar en beroep, 6:5 via 7:11 en 8:70). Een bezwaar is een
// indiening onder Awb 7:11 die naar het bestreden besluit verwijst; een
// beroep een indiening bij de rechtbank met een afschrift ervan (RFC-053).
// Of ze ontvankelijk zijn, beslist de wet, niet deze pagina. Een formulier
// staat ingevuld zolang je een scenario volgt (scenarioInvoer.js).
import { computed, ref } from 'vue';
import { api } from '../api.js';
import { beroepApi } from '../beroep.js';
import { artikelNaam } from '../logboek.js';
import {
  aanvragen,
  bezwaarBasis,
  dossier,
  geldendBesluit,
  isBesluit,
  kort,
  leesbaar,
  samenvatting,
  state,
  stap,
  toegewezen,
  zonderLeeg,
} from '../state.js';
import { formulier, scenarioWaarden } from '../scenarioInvoer.js';
import VeldInvoer from './VeldInvoer.vue';

const aanvraagVorm = formulier(() => scenarioWaarden('aanvraag'));
const aanvraag = computed(() => aanvraagVorm.waarden('aanvraag'));

const artikel = computed(() => {
  const [wet, nummer] = state.casus.besluit.split('#');
  return artikelNaam(wet, nummer);
});

async function indienen() {
  if (await stap(() => api.aanvragen(zonderLeeg(aanvraag.value), state.datum))) {
    aanvraagVorm.wis('aanvraag');
  }
}

// Het bezwaar of beroep tegen één besluit tegelijk, in een sheet: `soort`
// is de sleutel van zijn vorm en zijn voorbeeldwaarden ('bezwaar' of
// 'beroep'). Volg je een scenario, dan is de dagtekening de speeldatum.
const tegen = ref(null);
const scenarioIndiening = () => {
  const t = tegen.value;
  const w = t && scenarioWaarden(t.soort, { besluit: t.besluit.id });
  return w && { dagtekening_bezwaar_of_beroepschrift: state.datum, ...w };
};
const indieningVorm = formulier(scenarioIndiening);
const indiening = computed(() => indieningVorm.waarden(tegen.value?.besluit.id));

const indieningVelden = computed(() => {
  const t = tegen.value;
  if (!t || !state.vormen) return [];
  // Het bestreden besluit en wat het al bevat (de termijn, Awb 6:8) vult de
  // cel zelf.
  return state.vormen[t.soort].velden.filter(
    (v) => !(v.naam in (t.besluit.outputs ?? {})) && !(v.naam in (t.besluit.fields ?? {})),
  );
});

function start(soort, besluit) {
  indieningVorm.wis(besluit.id);
  tegen.value = { soort, besluit };
}

// De kop van de sheet met het formulier.
const kop = computed(() => {
  const t = tegen.value;
  if (!t) return '';
  return t.soort === 'beroep'
    ? `Beroep tegen beslissing op bezwaar ${kort(t.besluit.id)}`
    : `Bezwaar tegen besluit ${kort(t.besluit.id)}`;
});

const beroepArtikel = computed(() =>
  state.casus.beroep ? artikelNaam(...state.casus.beroep.split('#')) : '',
);

// Beroep staat open tegen een beslissing op bezwaar, bij een casus met een
// rechter.
const beroepMogelijk = (g) =>
  !!state.casus.beroep && isBesluit(g) && g.basis === bezwaarBasis();

/** De stand van een aanvraag, als tag: tekst en kleur. */
function stand(a) {
  const b = geldendBesluit(a);
  if (!b) return { tekst: 'In behandeling', kleur: 'neutral' };
  return toegewezen(b) ? { tekst: 'Toegewezen', kleur: 'success' } : { tekst: 'Afgewezen', kleur: 'critical' };
}

async function indienenTegen() {
  const { soort, besluit } = tegen.value;
  const velden = zonderLeeg(indiening.value);
  const ok = await stap(() =>
    soort === 'beroep'
      ? beroepApi.instellen(besluit.id, velden, state.datum)
      : api.bezwaren(besluit.id, velden, state.datum),
  );
  if (ok) {
    indieningVorm.wis(besluit.id);
    tegen.value = null;
  }
}

</script>

<template>
  <!-- Het formulier links, wat je al indiende rechts; op een smal scherm
       valt Mijn aanvragen onder het formulier. -->
  <nldd-one-half-one-half-section>
    <nldd-container slot="left" gap="16">
      <nldd-title size="3" :text="`Aanvraag: ${state.casus.titel.toLowerCase()}`">
        <nldd-text slot="supporting-text">
          {{ artikel }}. De velden komen uit de wet: wat het artikel vraagt en wat de Algemene wet
          bestuursrecht erbij eist (artikel 4:2). Een leeg veld laat je weg; de cel legt de
          aanvraag dan vast met dat gegeven als ontbrekend. De dag van ontvangst is de speeldatum
          (Awb 4:13).
        </nldd-text>
      </nldd-title>
      <nldd-container gap="16" max-width="560px">
        <VeldInvoer
          v-for="veld in state.vormen.aanvraag.velden"
          :key="veld.naam"
          :model-value="aanvraag[veld.naam]"
          :veld="veld"
          @update:model-value="aanvraagVorm.zet('aanvraag', veld.naam, $event)"
        />
        <nldd-button appearance="primary" text="Aanvraag indienen" @click="indienen"></nldd-button>
      </nldd-container>
    </nldd-container>
    <nldd-container slot="right" gap="16">
      <nldd-title size="3" text="Mijn aanvragen"></nldd-title>
      <nldd-inline-dialog
        v-if="aanvragen().length === 0"
        icon="info"
        text="Nog geen aanvragen"
      ></nldd-inline-dialog>
      <nldd-container gap="16">
        <nldd-card v-for="a in aanvragen()" :key="a.id">
          <nldd-container padding="16" gap="12">
          <nldd-title
            size="5"
            :text="`Aanvraag ${kort(a.id)}`"
            :supporting-text="samenvatting(a)"
          >
            <nldd-tag slot="end" :color="stand(a).kleur" :text="stand(a).tekst"></nldd-tag>
          </nldd-title>
          <nldd-list>
            <nldd-list-item v-for="s in dossier(a)" :key="s.gram.id" size="sm">
              <nldd-text-cell
                :text="`${s.rol} ${kort(s.gram.id)}`"
                :supporting-text="`${s.gram.recorded_on} · ${s.gram.basis}${s.rechter ? ` · ${state.casus.rechter.cel_naam}` : ''}`"
              ></nldd-text-cell>
              <nldd-cell v-if="!s.rechter && isBesluit(s.gram)" width="fit-content">
                <nldd-button
                  appearance="secondary"
                  size="sm"
                  text="Bezwaar maken"
                  @click="start('bezwaar', s.gram)"
                ></nldd-button>
              </nldd-cell>
              <nldd-cell v-if="!s.rechter && beroepMogelijk(s.gram)" width="fit-content">
                <nldd-button
                  appearance="secondary"
                  size="sm"
                  text="Beroep instellen"
                  @click="start('beroep', s.gram)"
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
    </nldd-container>
  </nldd-one-half-one-half-section>

  <!-- Bezwaar en beroep in één sheet: zelfde velden uit de wet, alleen de
       kop, de uitleg en de knop verschillen. -->
  <Teleport to="body">
    <nldd-sheet width="720px" :open="tegen ? true : undefined" @close="tegen = null">
      <nldd-page>
        <nldd-container slot="header" padding="16">
          <nldd-top-title-bar heading-level="2" :text="kop" dismiss-text="Sluiten"></nldd-top-title-bar>
        </nldd-container>
        <nldd-simple-section v-if="tegen">
          <nldd-container gap="16">
            <nldd-text v-if="tegen.soort === 'bezwaar'" color="secondary">
              De velden komen uit de Awb (artikel 6:5, via 7:11). Het bestreden besluit en de
              bezwaartermijn neemt de cel uit het besluit. Het bezwaar wijzigt de aanvraag niet:
              {{ state.casus.rol }} heroverweegt het besluit op grond van wat je aanvoert. Wil je iets
              anders vragen, dan doe je een nieuwe aanvraag.
            </nldd-text>
            <nldd-text v-else color="secondary">
              Het beroepschrift gaat naar {{ state.casus.rechter.cel_naam }}, een eigen cel met een eigen
              kroniek. De velden komen uit de wet: {{ beroepArtikel }}, met artikel 6:5 dat erop haakt.
              {{ state.casus.cel_naam }} maakt een afschrift van haar beslissing, en de rechtbank legt het
              beroep vast met wat zij ontving. De beroepstermijn neemt zij uit dat afschrift.
            </nldd-text>
            <VeldInvoer
              v-for="veld in indieningVelden"
              :key="veld.naam"
              :model-value="indiening[veld.naam]"
              :veld="veld"
              @update:model-value="indieningVorm.zet(tegen.besluit.id, veld.naam, $event)"
            />
            <nldd-button-group>
              <nldd-button
                appearance="primary"
                :text="tegen.soort === 'beroep' ? 'Beroepschrift indienen' : 'Bezwaar indienen'"
                @click="indienenTegen"
              ></nldd-button>
              <nldd-button appearance="neutral-transparent" text="Annuleren" @click="tegen = null"></nldd-button>
            </nldd-button-group>
          </nldd-container>
        </nldd-simple-section>
      </nldd-page>
    </nldd-sheet>
  </Teleport>
</template>
