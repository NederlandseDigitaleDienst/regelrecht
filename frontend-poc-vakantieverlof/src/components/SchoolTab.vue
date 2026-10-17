<script setup>
// De school: het hoofd beslist op aanvragen en bezwaren, en ziet bij elk
// besluit waarom het zo uitviel (Waarom.vue). De bevindingen die het hoofd
// per aanvraag geeft (artikel 11, onder f) komen uit de wet:
// `decided_per_case_by` op de open begrippen van 13a (RFC-050), behalve wat
// de school zelf uit haar kroniek beantwoordt (of de jongere dit schooljaar
// al verlof kreeg). Bij een bezwaar komen ze uit het bestreden besluit: de
// bevindingen die het gebruikte, met wat het hoofd toen vond.
import { computed, reactive } from 'vue';
import { api } from '../api.js';
import {
  BESLUIT_13A,
  aanvragen,
  beslissingOp,
  besluitOp,
  alsJson,
  kort,
  leesbaar,
  spel,
  stap,
} from '../spel.js';
import Uitkomst from './Uitkomst.vue';
import Waarom from './Waarom.vue';
import VeldInvoer from './VeldInvoer.vue';

const oordelen = () => spel.vormen?.besluit.bevindingen ?? [];
const standaard = () => Object.fromEntries(oordelen().map((o) => [o.naam, true]));

const open = computed(() => aanvragen().filter((a) => !besluitOp(a.id)));
const besluiten = computed(() =>
  spel.grammen.filter((g) => g.grondslag === BESLUIT_13A && g.kind !== 'AANVRAAG'),
);
const openBezwaren = computed(() =>
  spel.grammen.filter((g) => g.kind === 'BEZWAAR' && !beslissingOp(g.id)),
);

// Per aanvraag de oordelen van het hoofd.
const oordeel = reactive({});
const oordeelVoor = (id) => (oordeel[id] ??= standaard());

const uitkomsten = reactive({});

async function besluit(a) {
  const antwoord = await stap(() => api.besluiten(a.id, oordeelVoor(a.id), spel.datum));
  if (antwoord) uitkomsten[antwoord.besluit.id] = antwoord;
}

async function waarom(b) {
  const antwoord = await stap(() => api.waarom(b.id));
  if (antwoord) uitkomsten[b.id] = { besluit: b, waarom: antwoord };
}

// Bij een bezwaar eerst de ontvankelijkheid, zoals Awb 7:11 die beslist
// (op tijd volgens 6:9, en niet tegen een besluit op bezwaar). Alleen dan
// heroverweegt het hoofd en vraagt de pagina bevindingen: een bevinding
// zonder heroverweging weigert de cel. De vragen komen uit de bevindingen
// van het bestreden besluit; 'Laat staan' houdt wat het hoofd toen vond.
const toetsen = reactive({});
const opnieuw = reactive({});
const beslissingen = reactive({});

async function toets(bz) {
  const antwoord = await stap(() => api.heroverweging(bz.id, spel.datum));
  if (antwoord) {
    toetsen[bz.id] = antwoord;
    opnieuw[bz.id] = {};
  }
}

const jaNee = (v) => (v === true ? 'ja' : v === false ? 'nee' : String(v));

async function beslisOpBezwaar(bz) {
  const t = toetsen[bz.id];
  const gegeven = Object.fromEntries(
    Object.entries(t?.ontvankelijk ? (opnieuw[bz.id] ?? {}) : {})
      .filter(([, v]) => v !== '')
      .map(([k, v]) => [k, v === 'ja']),
  );
  const antwoord = await stap(() => api.beslissenOpBezwaar(bz.id, gegeven, spel.datum));
  if (antwoord) {
    beslissingen[bz.id] = antwoord;
    delete toetsen[bz.id];
  }
}

const uitkomstOpBezwaar = (uit) =>
  uit.bezwaar_ontvankelijk === false
    ? 'Niet-ontvankelijk: er is niet heroverwogen, het besluit blijft'
    : uit.bestreden_besluit_herroepen
      ? 'Gegrond: het besluit is herroepen, er is een nieuw besluit'
      : 'Ongegrond: het besluit blijft';
</script>

<template>
  <nldd-simple-section>
    <nldd-title slot="header" size="3" text="Open aanvragen">
      <nldd-text slot="supporting-text">
        Het hoofd beslist. De aanvraag en het beleid van de school (de eerste lesdag) leveren
        de rest. Of de jongere dit schooljaar al verlof kreeg, beantwoordt de school zelf uit
        haar kroniek. De bevindingen hieronder geeft alleen het hoofd.
      </nldd-text>
    </nldd-title>
    <nldd-inline-dialog
      v-if="open.length === 0"
      icon="info"
      text="Geen open aanvragen"
      supporting-text="Een ouder dient een aanvraag in onder het tabblad Ouder."
    ></nldd-inline-dialog>
    <nldd-container gap="16">
      <nldd-card v-for="a in open" :key="a.id">
        <nldd-container padding="16" gap="12">
        <nldd-title
          size="5"
          :text="`Aanvraag ${kort(a.id)}`"
          :supporting-text="`${a.fields.eerste_verlofdag} t/m ${a.fields.laatste_verlofdag} · BSN ${a.fields.bsn_jongere} · ontvangen ${a.recorded_on}`"
        ></nldd-title>
        <nldd-container gap="12">
          <VeldInvoer
            v-for="o in oordelen()"
            :key="o.naam"
            :model-value="oordeel[a.id]?.[o.naam] ?? true"
            :veld="o"
            @update:model-value="oordeelVoor(a.id)[o.naam] = $event"
          />
        </nldd-container>
        <nldd-button appearance="primary" text="Besluit nemen" @click="besluit(a)"></nldd-button>
        </nldd-container>
      </nldd-card>
    </nldd-container>
  </nldd-simple-section>

  <nldd-simple-section>
    <nldd-title slot="header" size="3" text="Open bezwaren">
      <nldd-text slot="supporting-text">
        Beslissen op bezwaar (Awb 7:11). Eerst de ontvankelijkheid: een bezwaar dat te laat
        kwam, of tegen een besluit op bezwaar, wordt niet heroverwogen. Anders heroverweegt
        het hoofd: artikel 13a wordt opnieuw toegepast op dezelfde aanvraag. Het hoofd kan een
        bevinding opnieuw geven, bijvoorbeeld na een verklaring van de werkgever. Verschilt de
        uitkomst, dan herroept de beslissing het besluit en komt er een nieuw besluit. Een
        besluit wordt maar één keer vervangen.
      </nldd-text>
    </nldd-title>
    <nldd-inline-dialog
      v-if="openBezwaren.length === 0"
      icon="info"
      text="Geen open bezwaren"
    ></nldd-inline-dialog>
    <nldd-container gap="16">
      <nldd-card v-for="bz in openBezwaren" :key="bz.id">
        <nldd-container padding="16" gap="12">
        <nldd-title
          size="5"
          :text="`Bezwaar ${kort(bz.id)} tegen besluit ${kort(bz.refers_to)}`"
          :supporting-text="`ontvangen ${bz.recorded_on} · ${bz.fields.gronden_bezwaar ?? ''}`"
        ></nldd-title>
        <template v-if="!toetsen[bz.id]">
          <nldd-text size="sm" color="secondary">Stap 1: is het bezwaar ontvankelijk? Dat beslist Awb 7:11 op de speeldatum.</nldd-text>
          <nldd-button appearance="primary" text="Ontvankelijkheid toetsen" @click="toets(bz)"></nldd-button>
        </template>
        <template v-else-if="!toetsen[bz.id].ontvankelijk">
          <nldd-inline-dialog
            icon="warning"
            icon-color="warning"
            text="Niet-ontvankelijk"
            :supporting-text="toetsen[bz.id].tijdig === false ? 'Het bezwaar kwam te laat (Awb 6:9). Er is geen heroverweging, dus het hoofd geeft geen bevindingen.' : 'Tegen dit besluit staat geen bezwaar open (Awb 7:1): het is genomen op bezwaar. Er is geen heroverweging.'"
          ></nldd-inline-dialog>
          <nldd-button appearance="primary" text="Beslissen op bezwaar" @click="beslisOpBezwaar(bz)"></nldd-button>
        </template>
        <template v-else>
          <nldd-inline-dialog
            icon="check-mark-circle"
            icon-color="success"
            text="Ontvankelijk"
            supporting-text="Stap 2: het hoofd heroverweegt. Geef een bevinding opnieuw, of laat staan wat het hoofd in het bestreden besluit vond."
          ></nldd-inline-dialog>
          <nldd-container gap="12">
            <nldd-form-field
              v-for="o in toetsen[bz.id].bevindingen"
              :key="o.naam"
              :label="leesbaar(o.naam)"
              :supporting-label="`${o.grondslag} · eerder: ${jaNee(o.eerder)}`"
            >
              <nldd-dropdown>
                <select @change="opnieuw[bz.id][o.naam] = $event.target.value">
                  <option value="">Laat staan ({{ jaNee(o.eerder) }})</option>
                  <option value="ja">Ja</option>
                  <option value="nee">Nee</option>
                </select>
              </nldd-dropdown>
            </nldd-form-field>
          </nldd-container>
          <nldd-button appearance="primary" text="Beslissen op bezwaar" @click="beslisOpBezwaar(bz)"></nldd-button>
        </template>
        </nldd-container>
      </nldd-card>
    </nldd-container>
  </nldd-simple-section>

  <nldd-simple-section v-if="Object.keys(beslissingen).length">
    <nldd-title slot="header" size="3" text="Beslissingen op bezwaar"></nldd-title>
    <nldd-container gap="24">
      <nldd-container v-for="(b, id) in beslissingen" :key="id" gap="12">
        <nldd-title
          size="5"
          :text="`Bezwaar ${kort(id)}`"
          :supporting-text="uitkomstOpBezwaar(b.beslissing.outputs)"
        ></nldd-title>
        <nldd-code-viewer language="json" appearance="box-tinted">{{ alsJson(b.beslissing.outputs) }}</nldd-code-viewer>
        <Uitkomst
          v-if="b.nieuw_besluit"
          titel="Nieuw besluit"
          :besluit="b.nieuw_besluit"
          :waarom="b.waarom"
          :waarom-van="b.beslissing.id"
        />
        <Waarom v-else :besluit="b.beslissing.id" :waarom="b.waarom" />
      </nldd-container>
    </nldd-container>
  </nldd-simple-section>

  <nldd-simple-section>
    <nldd-title slot="header" size="3" text="Besluiten">
      <nldd-text slot="supporting-text">Elk besluit onder artikel 13a, met waarom het zo uitviel.</nldd-text>
    </nldd-title>
    <nldd-container gap="24">
      <nldd-container v-for="b in besluiten" :key="b.id" gap="8">
        <Uitkomst
          v-if="uitkomsten[b.id]"
          :titel="`Besluit ${kort(b.id)} (${b.recorded_on})`"
          :besluit="uitkomsten[b.id].besluit"
          :waarom="uitkomsten[b.id].waarom"
        />
        <template v-else>
          <Uitkomst :titel="`Besluit ${kort(b.id)} (${b.recorded_on})`" :besluit="b" />
          <nldd-button appearance="secondary" size="sm" text="Waarom?" @click="waarom(b)"></nldd-button>
        </template>
      </nldd-container>
    </nldd-container>
  </nldd-simple-section>
</template>
