<script setup>
// De aanvrager: een aanvraag doen, zien hoe het ervoor staat, bezwaar
// maken tegen een besluit en in beroep gaan tegen een beslissing op bezwaar.
// Welke velden er zijn, zegt de wet (het artikel van de casus, de Awb-haken
// 4:2 en, voor bezwaar en beroep, 6:5 via 7:11 en 8:70). Een bezwaar is een
// indiening onder Awb 7:11 die naar het bestreden besluit verwijst; een
// beroep een indiening bij de rechtbank met een afschrift ervan (RFC-053).
// Of ze ontvankelijk zijn, beslist de wet, niet deze pagina. Een formulier
// staat ingevuld zolang je een scenario volgt (scenarioInvoer.js).
import { computed, nextTick, ref, watch } from 'vue';
import { api } from '../api.js';
import { awb } from '../awb.js';
import { beroepApi } from '../beroep.js';
import { melding } from '../melding.js';
import {
  aanvragen,
  bezwaarBasis,
  dossier,
  geldendBesluit,
  inBehandeling,
  isBesluit,
  kort,
  leesbaar,
  lopendeAanvragen,
  samenvatting,
  state,
  stap,
  toegewezen,
  zonderLeeg,
} from '../state.js';
import { formulier, huidigeStap, scenarioWaarden } from '../scenarioInvoer.js';
import { celNaam } from '../logboek.js';
import {
  beginWaarden,
  bronInvoer,
  herkomstTekst,
  metSysteem,
  opHetFormulier,
  systeemBronnen,
  systeemTekst,
  systeemVult,
  tegenBesluit,
} from '@regelrecht/frontend-shared/chronolex/voorinvulling';
import { NOT_LOGGED_IN, gekozen, inlogTekst, keuzeLabel, metKeuze } from '../inloggen.js';
import {
  claimRegel,
  eerstInloggenTekst,
  ingelogdAls,
  ingevuldDoorInloggen,
  metSessie,
  nietIngelogdTekst,
} from '../inlogstap.js';
import { inlogScherm, keuzeVan, sessieVan, wisSessie, zetKeuze } from '../sessie.js';
import ArtikelLink from './ArtikelLink.vue';
import VeldInvoer from '@regelrecht/frontend-shared/components/chronolex/VeldInvoer.vue';

// Who the applicant is logged in as, in a block of its own at the top of the
// page: it holds for the new application and for the ones made before. With
// a login step (inlogstap.js): the session, with the fields the login fills
// left out of the form. Otherwise (inloggen.js): the record whose key the
// login field holds, chosen in a dropdown.
const inloggen = computed(() => state.vormen?.aanvraag?.inloggen ?? null);
const inlogstap = computed(() => inloggen.value?.inlogstap ?? null);
const sessie = computed(() => (inlogstap.value ? sessieVan(state.casus.id) : null));
const veldnamen = computed(() => (state.vormen?.aanvraag?.velden ?? []).map((v) => v.naam));

// What the sources give for the fields the UI spec prefills
// (voorinvulling.js), per field `{ bron, waarde, herkomst, fout }`. The form
// starts from it over the case's example; the values of a scenario step and
// the applicant's own go over it.
const voorgevuld = ref({});
const aanvraagVorm = formulier(() => {
  const scenario = scenarioWaarden('aanvraag');
  return beginWaarden(scenario, scenario && huidigeStap()?.step.stap.invoer, voorgevuld.value);
});
// With a login step, who is logged in comes from the session, never from the
// form or the scenario (inlogstap.js). With a dropdown, from the choice made
// before when this form says nothing else (inloggen.js, metKeuze). What the
// system fills goes over both; without a session the application does not
// come through the form, as one on paper, and the system fills nothing
// (voorinvulling.js).
const systeem = computed(() => systeemVult(inloggen.value, sessie.value, state.vormen?.aanvraag?.systeem));
const doorInloggen = computed(() => ingevuldDoorInloggen(inloggen.value, sessie.value, veldnamen.value));
const aanvraag = computed(() => {
  const opStap = !!scenarioWaarden('aanvraag');
  const uitStap = opStap ? huidigeStap()?.step.stap.invoer?.[inloggen.value?.veld] : undefined;
  const zelf = opStap || inloggen.value?.veld in (aanvraagVorm.eigen.aanvraag ?? {});
  const metInlog = metKeuze(inloggen.value, keuzeVan(state.casus.id), aanvraagVorm.waarden('aanvraag'), zelf);
  const waarden = metSessie(inloggen.value, sessie.value, metInlog, veldnamen.value, uitStap);
  return metSysteem(waarden, systeem.value, voorgevuld.value, doorInloggen.value);
});
// The sources asked: the prefill, and what the system fills from a source.
const prefill = computed(() => ({ ...(state.vormen?.aanvraag?.prefill ?? {}), ...systeemBronnen(systeem.value) }));

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
const herkomst = (veld) =>
  veld.naam in systeem.value
    ? systeemTekst(systeem.value[veld.naam], voorgevuld.value[veld.naam], celNaam)
    : herkomstTekst(voorgevuld.value[veld.naam], aanvraag.value[veld.naam], celNaam, veld.kolommen);

const velden = computed(() =>
  opHetFormulier(state.vormen?.aanvraag?.velden, state.vormen?.aanvraag?.verborgen, doorInloggen.value),
);
function uitloggen() {
  wisSessie(state.casus.id);
}
function naarInloggen() {
  inlogScherm.value = true;
}
const ingelogd = computed(() =>
  inlogstap.value ? undefined : gekozen(inloggen.value, aanvraag.value[inloggen.value?.veld]),
);
function inlog(waarde) {
  const keuze = waarde === '' ? undefined : waarde;
  aanvraagVorm.zet('aanvraag', inloggen.value.veld, keuze);
  zetKeuze(state.casus.id, keuze);
}

// While the applicant has an application in behandeling, the form for a new
// one stays closed behind a button. A second application is still possible:
// whether the cell takes it is for the law to say, not for this page. A
// scenario at an application step opens the form itself.
const wie = computed(() => (inloggen.value ? aanvraag.value[inloggen.value.veld] : undefined));
const lopend = computed(() => lopendeAanvragen(inloggen.value?.veld, wie.value));
const nieuweAanvraag = ref(false);
// Someone else logged in: their own applications decide again. Only a new
// login counts, not typing in a field of the form, which would close the
// form under the applicant's hands.
watch([() => keuzeVan(state.casus.id), sessie], () => {
  nieuweAanvraag.value = false;
});
// With a login step, the form waits for a login: the block at the top of the
// page is the way to it. A scenario at an application step opens it all the
// same, for a submission that does not come through the login, such as one
// on paper (inlogstap.js, metSessie).
const eerstInloggen = computed(() => !!inlogstap.value && !sessie.value && !scenarioWaarden('aanvraag'));
const formulierOpen = computed(
  () => lopend.value.length === 0 || nieuweAanvraag.value || !!scenarioWaarden('aanvraag'),
);
const lopendKop = computed(() =>
  lopend.value.length === 1 ? 'Je aanvraag is in behandeling' : 'Je aanvragen zijn in behandeling',
);
const lopendTekst = computed(() => {
  const nummers = lopend.value.map((a) => kort(a.id)).join(', ');
  const welke = lopend.value.length === 1 ? `Aanvraag ${nummers} wacht` : `Aanvragen ${nummers} wachten`;
  return `${welke} op een besluit van ${state.casus.cel_naam}. Het besluit komt onder Mijn aanvragen te staan. Je kunt nog wel een nieuwe aanvraag doen. Wat daarmee gebeurt, bepaalt de wet.`;
});
// Opening the form takes the button away, so the focus goes to the first
// field of the form rather than falling back to the page.
const formulierVelden = ref(null);
const VELD = 'nldd-text-field, nldd-date-field, nldd-switch-field, nldd-dropdown, nldd-button';
async function openNieuweAanvraag() {
  nieuweAanvraag.value = true;
  await nextTick();
  formulierVelden.value?.querySelector(VELD)?.focus();
}

async function indienen() {
  const door = wie.value;
  if (await stap(() => awb.aanvragen(zonderLeeg(aanvraag.value), state.datum))) {
    // Who applied stays logged in once the form starts empty again; a number
    // typed in the form that is no choice of the login leaves the choice be.
    if (inloggen.value && !inlogstap.value && (door === undefined || gekozen(inloggen.value, door))) {
      zetKeuze(state.casus.id, door);
    }
    aanvraagVorm.wis('aanvraag');
    nieuweAanvraag.value = false;
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
  return tegenBesluit(state.vormen[t.soort].velden, t.besluit);
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
  if (inBehandeling(a)) return { tekst: 'In behandeling', kleur: 'neutral' };
  return toegewezen(geldendBesluit(a)) ? { tekst: 'Toegewezen', kleur: 'success' } : { tekst: 'Afgewezen', kleur: 'critical' };
}

async function indienenTegen() {
  const { soort, besluit } = tegen.value;
  const velden = zonderLeeg(indiening.value);
  const ok = await stap(() =>
    soort === 'beroep'
      ? beroepApi.instellen(besluit.id, velden, state.datum)
      : awb.bezwaren(besluit.id, velden, state.datum),
  );
  if (ok) {
    indieningVorm.wis(besluit.id);
    tegen.value = null;
  }
}

</script>

<template>
  <!-- Logging in comes first, on its own: it holds for the new application
       and for the ones made before alike. -->
  <nldd-simple-section v-if="inloggen">
    <nldd-container max-width="560px">
      <nldd-card>
        <nldd-container padding="16">
          <template v-if="inlogstap">
            <nldd-title
              v-if="sessie"
              heading-level="2"
              size="display-5"
              :text="ingelogdAls(sessie)"
              :supporting-text="claimRegel(inloggen, sessie)"
            >
              <nldd-button slot="end" appearance="secondary" size="sm" text="Uitloggen" @click="uitloggen"></nldd-button>
            </nldd-title>
            <nldd-title
              v-else
              heading-level="2"
              size="display-5"
              text="Niet ingelogd"
              :supporting-text="nietIngelogdTekst(inlogstap)"
            >
              <nldd-button slot="end" appearance="primary" size="sm" text="Inloggen" @click="naarInloggen"></nldd-button>
            </nldd-title>
          </template>
          <nldd-form-field v-else label="Ingelogd als" :supporting-label="inloggen.feit">
            <nldd-dropdown>
              <select :value="ingelogd?.waarde ?? ''" @change="inlog($event.target.value)">
                <option value="">{{ NOT_LOGGED_IN }}</option>
                <option v-for="k in inloggen.keuzes" :key="k.waarde" :value="k.waarde">
                  {{ keuzeLabel(k) }}
                </option>
              </select>
            </nldd-dropdown>
            <nldd-form-field-help-text v-if="ingelogd">{{ inlogTekst(ingelogd, inloggen, celNaam) }}</nldd-form-field-help-text>
          </nldd-form-field>
        </nldd-container>
      </nldd-card>
    </nldd-container>
  </nldd-simple-section>

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
      <nldd-container v-if="eerstInloggen" max-width="560px">
        <nldd-inline-dialog
          icon="info"
          heading-level="3"
          horizontal-alignment="left"
          text="Log eerst in om een aanvraag te doen"
          :supporting-text="eerstInloggenTekst(inlogstap)"
        ></nldd-inline-dialog>
      </nldd-container>
      <nldd-container v-else-if="formulierOpen" ref="formulierVelden" gap="16" max-width="560px">
        <VeldInvoer
          v-for="veld in velden"
          :key="veld.naam"
          :model-value="aanvraag[veld.naam]"
          :veld="veld"
          :herkomst="herkomst(veld)"
          :alleen-lezen="veld.naam in systeem"
          @update:model-value="aanvraagVorm.zet('aanvraag', veld.naam, $event)"
        />
        <nldd-button appearance="primary" text="Aanvraag indienen" @click="indienen"></nldd-button>
      </nldd-container>
      <nldd-container v-else max-width="560px">
        <nldd-inline-dialog
          icon="info"
          heading-level="3"
          horizontal-alignment="left"
          :text="lopendKop"
          :supporting-text="lopendTekst"
        >
          <nldd-button slot="actions" appearance="secondary" text="Nieuwe aanvraag" @click="openNieuweAanvraag"></nldd-button>
        </nldd-inline-dialog>
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
