<script setup>
// De aanvrager: een aanvraag doen, zien hoe het ervoor staat, bezwaar
// maken tegen een besluit en in beroep gaan tegen een beslissing op bezwaar.
// Welke velden er zijn, zegt de wet (het artikel van de casus, de Awb-haken
// 4:2 en, voor bezwaar en beroep, 6:5 via 7:11 en 8:70). Een bezwaar is een
// indiening onder Awb 7:11 die naar het bestreden besluit verwijst; een
// beroep een indiening bij de rechtbank met een afschrift ervan (RFC-053).
// Of ze ontvankelijk zijn, beslist de wet, niet deze pagina. Een formulier
// staat ingevuld zolang je een scenario volgt (scenarioInvoer.js).
import { computed, ref, watch } from 'vue';
import { api } from '../api.js';
import { beroepApi } from '../beroep.js';
import { melding } from '../melding.js';
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
import { formulier, huidigeStap, scenarioWaarden } from '../scenarioInvoer.js';
import { celNaam } from '../logboek.js';
import { beginWaarden, bronInvoer, herkomstTekst } from '../voorinvulling.js';
import { gekozen, inlogTekst, keuzeLabel } from '../inloggen.js';
import ArtikelLink from './ArtikelLink.vue';
import VeldInvoer from './VeldInvoer.vue';

// What the sources give for the fields the UI spec prefills
// (voorinvulling.js), per field `{ bron, waarde, herkomst, fout }`. The form
// starts from it over the case's example; the values of a scenario step and
// the applicant's own go over it.
const voorgevuld = ref({});
const aanvraagVorm = formulier(() => {
  const scenario = scenarioWaarden('aanvraag');
  return beginWaarden(scenario, scenario && huidigeStap()?.step.stap.invoer, voorgevuld.value);
});
const aanvraag = computed(() => aanvraagVorm.waarden('aanvraag'));
const prefill = computed(() => state.vormen?.aanvraag?.prefill ?? {});

// Ask the sources again when what they read changes: the fields of the form,
// the day, or a chronicle (a register that recorded something new). Only the
// answer to the last question counts.
let vraag = 0;
// A source may read another prefilled field, so an answer can change what is
// asked next. Answers that keep changing each other without anything from
// outside (the applicant, the day, a chronicle) mean two sources read each
// other: the form stops asking and says so.
const MAX_OP_ELKAAR = 5;
let buiten = '';
let opElkaar = 0;
const kroniekStand = () =>
  [state.grammen.length, ...(state.andere ?? []).map((a) => a.grammen.length)].join(',');
watch(
  () =>
    JSON.stringify([prefill.value, bronInvoer(aanvraag.value), state.datum, kroniekStand()]),
  async () => {
    const nr = ++vraag;
    if (!Object.keys(prefill.value).length) {
      voorgevuld.value = {};
      return;
    }
    const nu = JSON.stringify([
      aanvraagVorm.eigen.aanvraag,
      state.datum,
      kroniekStand(),
      prefill.value,
      state.scenario,
      state.begonnen,
      huidigeStap()?.index,
    ]);
    opElkaar = nu === buiten ? opElkaar + 1 : 0;
    buiten = nu;
    if (opElkaar > MAX_OP_ELKAAR) {
      state.fout = 'de bronnen van het voorinvullen lezen elkaar en blijven elkaar wijzigen';
      return;
    }
    try {
      const antwoord = await api.voorinvullen(
        prefill.value,
        bronInvoer(aanvraag.value),
        state.datum,
      );
      if (nr === vraag) voorgevuld.value = antwoord;
    } catch (e) {
      // A source that is no text, or a corpus that does not load: the form
      // keeps what the applicant fills, and the message says why.
      if (nr === vraag) {
        voorgevuld.value = {};
        state.fout = e.message;
      }
    }
  },
  { immediate: true },
);
const herkomst = (naam) => herkomstTekst(voorgevuld.value[naam], aanvraag.value[naam], celNaam);

// Who the applicant is logged in as (inloggen.js): the record whose key the
// login field holds.
const inloggen = computed(() => state.vormen?.aanvraag?.inloggen ?? null);
const ingelogd = computed(() => gekozen(inloggen.value, aanvraag.value[inloggen.value?.veld]));
function inlog(waarde) {
  aanvraagVorm.zet('aanvraag', inloggen.value.veld, waarde === '' ? undefined : waarde);
}

async function indienen() {
  if (await stap(() => api.aanvragen(zonderLeeg(aanvraag.value), state.datum))) {
    aanvraagVorm.wis('aanvraag');
  }
}

// Het bezwaar of beroep tegen één besluit tegelijk: `soort` is de sleutel
// van zijn vorm en zijn voorbeeldwaarden ('bezwaar' of 'beroep'). Volg je
// een scenario, dan is de dagtekening de speeldatum.
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
  // The sheet shows a refusal of its own submission, not one from before.
  state.fout = '';
  indieningVorm.wis(besluit.id);
  tegen.value = { soort, besluit };
}

// The heading of the sheet with the form.
const kop = computed(() => {
  const t = tegen.value;
  if (!t) return '';
  return t.soort === 'beroep'
    ? `Beroep tegen beslissing op bezwaar ${kort(t.besluit.id)}`
    : `Bezwaar tegen besluit ${kort(t.besluit.id)}`;
});

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
  <!-- The form on the left, what was submitted on the right; on a narrow
       screen Mijn aanvragen falls below the form. -->
  <nldd-one-half-one-half-section>
    <nldd-container slot="left" gap="16">
      <nldd-title heading-level="2" size="display-3" :text="`Aanvraag: ${state.casus.titel.toLowerCase()}`">
        <nldd-text slot="supporting-text">
          <ArtikelLink :basis="state.casus.besluit" />. De velden komen uit de wet: wat het
          artikel vraagt en wat de Algemene wet
          bestuursrecht erbij eist (artikel 4:2). Een leeg veld laat je weg; de cel legt de
          aanvraag dan vast met dat gegeven als ontbrekend. De dag van ontvangst is de speeldatum
          (Awb 4:13).
        </nldd-text>
      </nldd-title>
      <nldd-container gap="16" max-width="560px">
        <nldd-form-field
          v-if="inloggen"
          label="Ingelogd als"
          :supporting-label="inloggen.feit"
        >
          <nldd-dropdown>
            <select :value="ingelogd?.waarde ?? ''" @change="inlog($event.target.value)">
              <option value="">Niet ingelogd</option>
              <option v-for="k in inloggen.keuzes" :key="k.waarde" :value="k.waarde">
                {{ keuzeLabel(k) }}
              </option>
            </select>
          </nldd-dropdown>
          <nldd-form-field-help-text v-if="ingelogd">{{ inlogTekst(ingelogd, celNaam) }}</nldd-form-field-help-text>
        </nldd-form-field>
        <VeldInvoer
          v-for="veld in state.vormen.aanvraag.velden"
          :key="veld.naam"
          :model-value="aanvraag[veld.naam]"
          :veld="veld"
          :herkomst="herkomst(veld.naam)"
          @update:model-value="aanvraagVorm.zet('aanvraag', veld.naam, $event)"
        />
        <nldd-button appearance="primary" text="Aanvraag indienen" @click="indienen"></nldd-button>
      </nldd-container>
    </nldd-container>
    <nldd-container slot="right" gap="16">
      <nldd-title heading-level="2" size="display-3" text="Mijn aanvragen"></nldd-title>
      <nldd-inline-dialog
        v-if="aanvragen().length === 0"
        icon="info"
        text="Nog geen aanvragen"
      ></nldd-inline-dialog>
      <nldd-container gap="16">
        <nldd-card v-for="a in aanvragen()" :key="a.id">
          <nldd-container padding="16" gap="12">
          <nldd-title heading-level="3"
            size="display-5"
            :text="`Aanvraag ${kort(a.id)}`"
            :supporting-text="samenvatting(a)"
          >
            <nldd-tag slot="end" :color="stand(a).kleur" :text="stand(a).tekst"></nldd-tag>
          </nldd-title>
          <!-- The rows are no actions; the link and the buttons in them are
               (a form, so each keeps its tab stop). -->
          <nldd-list type="form">
            <nldd-list-item v-for="s in dossier(a)" :key="s.gram.id" size="sm">
              <!-- The article each step rests on opens in the sheet of the
                   chain: the application its establishing article, a
                   decision the article that took it. -->
              <nldd-text-cell :text="`${s.rol} ${kort(s.gram.id)}`">
                <nldd-text slot="supporting-text" size="sm" color="secondary">
                  {{ s.gram.recorded_on }} ·
                  <ArtikelLink :basis="s.gram.basis" :datum="s.gram.recorded_on" /><template v-if="s.rechter">
                    · {{ state.casus.rechter.cel_naam }}</template>
                </nldd-text>
              </nldd-text-cell>
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

  <!-- Objection and appeal share one sheet: the same fields from the law,
       only the heading, the explanation and the button differ. -->
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
              kroniek. De velden komen uit de wet: <ArtikelLink :basis="state.casus.beroep" />, met artikel 6:5 dat erop haakt.
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
            <!-- The banner of the page sits behind the sheet. -->
            <nldd-inline-dialog
              v-if="state.fout"
              icon="warning"
              icon-color="critical"
              text="Deze stap lukt niet"
              :supporting-text="melding(state.fout)"
            ></nldd-inline-dialog>
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
