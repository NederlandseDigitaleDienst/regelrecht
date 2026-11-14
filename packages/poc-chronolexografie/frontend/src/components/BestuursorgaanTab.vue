<script setup>
// Het bestuursorgaan (wie dat is, zegt de wet: het hoofd, het college)
// beslist op aanvragen en bezwaren. De bevindingen die het per aanvraag
// geeft, komen uit de wet: `decided_per_case_by` op de open begrippen van het artikel
// (RFC-045), behalve wat de cel zelf uit haar kroniek beantwoordt. Naast de
// bevindingen staat wat het besluit ermee zou worden (Voorvertoning.vue),
// zonder dat er iets wordt vastgelegd; op een smal scherm eronder. Een
// begrip dat de cel beantwoordt, vraagt het formulier niet, tenzij de cel
// het voor deze aanvraag niet weet: dat meldt de voorvertoning (`stil`), en
// dan oordeelt het bestuursorgaan zelf (RFC-051). Bij een
// bezwaar komen ze uit het bestreden besluit: de bevindingen die het
// gebruikte, met wat het bestuursorgaan toen vond. Volg je een scenario, dan
// staan de bevindingen ingevuld zoals de stap ze geeft (scenarioInvoer.js).
import { computed, reactive } from 'vue';
import { api } from '../api.js';
import { stilBegrippen } from '../stil.js';
import {
  aanvragen,
  besluitBasis,
  beslissingOp,
  besluitOp,
  alsJson,
  jaNee,
  kort,
  samenvatting,
  state,
  stap,
  zonderLeeg,
} from '../state.js';
import { guideFor } from '../scenarioGuides.js';
import { formulier, scenarioWaarden } from '../scenarioInvoer.js';
import ArtikelLink from './ArtikelLink.vue';
import Uitkomst from './Uitkomst.vue';
import OordeelKeuze from './OordeelKeuze.vue';
import StilOordeel from './StilOordeel.vue';
import Voorvertoning from './Voorvertoning.vue';
import Waarom from './Waarom.vue';
import VeldInvoer from './VeldInvoer.vue';

const oordelen = () => state.vormen?.besluit.bevindingen ?? [];

const open = computed(() => aanvragen().filter((a) => !besluitOp(a.id)));
const openBezwaren = computed(() =>
  state.grammen.filter((g) => g.kind === 'BEZWAAR' && !beslissingOp(g.id)),
);

// Per aanvraag de oordelen van het bestuursorgaan. Een schakelaar die uit
// staat, zegt nee, behalve bij een bevinding die leeg mag: daar zegt een
// onaangeraakte schakelaar niets. Een begrip dat de cel niet weet, blijft leeg
// tot het bestuursorgaan oordeelt. Een scenario met een gids speelt zo,
// waar de casus ook vandaan komt. Alleen in een scenario zonder gids (alleen
// tekst) van een casus uit een bron zegt een bevinding zonder voorbeeldwaarde
// ja, zoals de voorbeeldwaarden van zo'n bron altijd deden.
const besluitVorm = formulier((id) => scenarioWaarden('besluit', { aanvraag: id }));
const oordeel = besluitVorm.eigen;
const uit = () => {
  const standaard = !!(state.casus.bron && state.scenario && !guideFor(state.casus, state.scenario));
  return Object.fromEntries(
    oordelen()
      .filter((o) => o.type === 'boolean' && !o.mag_leeg)
      .map((o) => [o.naam, standaard]),
  );
};
const oordeelVoor = (id) => ({ ...uit(), ...zonderLeeg(besluitVorm.waarden(id)) });
const zetOordeel = (id, naam, waarde) => besluitVorm.zet(id, naam, waarde);

async function besluit(a) {
  const antwoord = await stap(() => api.besluiten(a.id, oordeelVoor(a.id), state.datum));
  if (antwoord) besluitVorm.wis(a.id);
}

// Bij een bezwaar eerst de ontvankelijkheid, zoals Awb 7:11 die beslist:
// op tijd volgens 6:9 of verschoonbaar te laat volgens 6:11, niet tegen een
// besluit dat in bezwaar is genomen, en door een belanghebbende (1:2, 7:1
// en 8:1). Of de indiener belanghebbende is en of hij bij een te laat
// bezwaarschrift in verzuim was, oordeelt het bestuursorgaan zelf; die
// oordelen komen uit de wet (`decided_per_case_by` bij artikel 7:11). Alleen
// bij een ontvankelijk bezwaar heroverweegt het bestuursorgaan en vraagt de
// pagina bevindingen: een bevinding zonder heroverweging weigert de cel. De
// vragen komen uit de bevindingen van het bestreden besluit; 'Laat staan'
// houdt wat het toen vond.
const toetsen = reactive({});
const opnieuwVorm = formulier((id) => scenarioWaarden('beslissing_op_bezwaar', { bezwaar: id }));
const opnieuw = opnieuwVorm.eigen;
const ontvankelijkheidsOordelen = () => state.vormen?.beslissing_op_bezwaar?.bevindingen ?? [];
// Wat het bestuursorgaan over de ontvankelijkheid oordeelt. Een schakelaar
// die uit staat, zegt nee, zoals bij een besluit (`uit`); ook bij een casus
// uit een bron, zodat een te laat bezwaar niet stil verschoonbaar wordt.
const ontvankelijkheid = (bz) => {
  const gegeven = zonderLeeg(opnieuwVorm.waarden(bz.id));
  return Object.fromEntries(
    ontvankelijkheidsOordelen().map((o) => [o.naam, gegeven[o.naam] ?? false]),
  );
};
// Wat het bestuursorgaan oordeelt over begrippen die de cel niet weet (stil.js).
const { stil, meldStil } = stilBegrippen(oordeel, opnieuw);
const beslissingen = reactive({});

// De oordelen over de ontvankelijkheid blijven bij de toets staan: het
// formulier wordt daarna gewist voor de heroverweging, en de beslissing moet
// op dezelfde oordelen berusten als de toets.
async function toets(bz) {
  const oordeel = ontvankelijkheid(bz);
  const antwoord = await stap(() => api.heroverweging(bz.id, oordeel, state.datum));
  if (antwoord) {
    toetsen[bz.id] = { ...antwoord, ontvankelijkheid: oordeel };
    opnieuwVorm.wis(bz.id);
  }
}

// Wat het bestuursorgaan over de ontvankelijkheid oordeelt, en wat het bij
// de heroverweging opnieuw vindt; 'Laat staan' geeft niets.
const gegevenOpBezwaar = (bz) => ({
  ...(toetsen[bz.id]?.ontvankelijk ? zonderLeeg(opnieuwVorm.waarden(bz.id)) : {}),
  ...(toetsen[bz.id]?.ontvankelijkheid ?? ontvankelijkheid(bz)),
});

// Waarom een bezwaar niet-ontvankelijk is, in de volgorde waarin Awb 7:11
// het toetst (kern::heroverweging, `reden`).
const NIET_ONTVANKELIJK = {
  te_laat:
    'Het bezwaar kwam te laat (Awb 6:9), en het bestuursorgaan oordeelde dat de indiener in verzuim was (Awb 6:11). Er is geen heroverweging, dus het bestuursorgaan geeft geen bevindingen.',
  genomen_in_bezwaar:
    'Tegen dit besluit staat geen bezwaar open (Awb 7:1): het is in bezwaar genomen. Er is geen heroverweging.',
  geen_belanghebbende:
    'Het bestuursorgaan oordeelde dat de indiener geen belanghebbende is (Awb 1:2, 7:1 en 8:1). Er is geen heroverweging.',
};
// De begrippen die de cel bij dit bezwaar niet weet en die niet al onder de
// bevindingen van het bestreden besluit staan.
const stilBijBezwaar = (bz) =>
  (stil[bz.id] ?? []).filter((v) => !toetsen[bz.id].bevindingen.some((o) => o.naam === v.naam));

async function beslisOpBezwaar(bz) {
  const gegeven = gegevenOpBezwaar(bz);
  const antwoord = await stap(() => api.beslissenOpBezwaar(bz.id, gegeven, state.datum));
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
    <nldd-title heading-level="2" slot="header" size="display-3" text="Open aanvragen">
      <nldd-text slot="supporting-text">
        Het bestuursorgaan is hier {{ state.casus.rol }} (<ArtikelLink :basis="besluitBasis()" />). Het beslist met de
        aanvraag, het beleid en de bevindingen hieronder. Wat de cel zelf uit haar
        kroniek beantwoordt, vraagt deze pagina niet.
      </nldd-text>
    </nldd-title>
    <nldd-inline-dialog
      v-if="open.length === 0"
      icon="info"
      text="Geen open aanvragen"
      supporting-text="Een aanvraag dien je in onder het tabblad Aanvrager."
    ></nldd-inline-dialog>
    <nldd-container gap="16">
      <nldd-card v-for="a in open" :key="a.id">
        <nldd-container padding="16" gap="12">
        <nldd-title heading-level="3"
          size="display-5"
          :text="`Aanvraag ${kort(a.id)}`"
          :supporting-text="samenvatting(a)"
        ></nldd-title>
        <nldd-container layout="grid" gap="16">
          <nldd-container gap="12">
            <VeldInvoer
              v-for="o in oordelen()"
              :key="o.naam"
              :model-value="besluitVorm.waarden(a.id)[o.naam]"
              :veld="o"
              @update:model-value="zetOordeel(a.id, o.naam, $event)"
            />
            <StilOordeel
              v-for="v in stil[a.id] ?? []"
              :key="v.naam"
              :veld="v"
              :model-value="besluitVorm.waarden(a.id)[v.naam]"
              @update:model-value="zetOordeel(a.id, v.naam, $event)"
            />
            <nldd-button appearance="primary" text="Besluit nemen" @click="besluit(a)"></nldd-button>
          </nldd-container>
          <Voorvertoning
            :op="a.id"
            :bevindingen="oordeelVoor(a.id)"
            @stil="meldStil(a.id, $event)"
          />
        </nldd-container>
        </nldd-container>
      </nldd-card>
    </nldd-container>
  </nldd-simple-section>

  <nldd-simple-section>
    <nldd-title heading-level="2" slot="header" size="display-3" text="Open bezwaren">
      <nldd-text slot="supporting-text">
        Beslissen op bezwaar (Awb 7:11). Eerst de ontvankelijkheid: een bezwaar dat zonder
        verschoning te laat kwam, tegen een besluit dat in bezwaar is genomen, of van iemand die
        geen belanghebbende is, wordt niet heroverwogen. Anders heroverweegt
        het bestuursorgaan: <ArtikelLink :basis="besluitBasis()" /> wordt opnieuw toegepast op dezelfde aanvraag. Het kan
        een bevinding opnieuw geven, bijvoorbeeld na nieuwe stukken bij het bezwaar. Verschilt
        de uitkomst, dan herroept de beslissing het besluit en komt er een nieuw besluit. Een
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
        <nldd-title heading-level="3"
          size="display-5"
          :text="`Bezwaar ${kort(bz.id)} tegen besluit ${kort(bz.refers_to)}`"
          :supporting-text="`ontvangen ${bz.recorded_on} · ${bz.fields.gronden_bezwaar_of_beroep ?? ''}`"
        ></nldd-title>
        <template v-if="!toetsen[bz.id]">
          <nldd-text size="sm" color="secondary">Stap 1: is het bezwaar ontvankelijk? Het bestuursorgaan oordeelt of de indiener belanghebbende is, en of hij bij een te laat bezwaarschrift in verzuim was. Daarmee beslist Awb 7:11 op de speeldatum.</nldd-text>
          <VeldInvoer
            v-for="o in ontvankelijkheidsOordelen()"
            :key="o.naam"
            :model-value="opnieuwVorm.waarden(bz.id)[o.naam]"
            :veld="o"
            @update:model-value="opnieuwVorm.zet(bz.id, o.naam, $event)"
          />
          <nldd-button appearance="primary" text="Ontvankelijkheid toetsen" @click="toets(bz)"></nldd-button>
        </template>
        <template v-else-if="!toetsen[bz.id].ontvankelijk">
          <nldd-inline-dialog
            icon="warning"
            icon-color="warning"
            text="Niet-ontvankelijk"
            :supporting-text="NIET_ONTVANKELIJK[toetsen[bz.id].reden]"
          ></nldd-inline-dialog>
          <nldd-container layout="grid" gap="16">
            <nldd-container>
              <nldd-button appearance="primary" text="Beslissen op bezwaar" @click="beslisOpBezwaar(bz)"></nldd-button>
            </nldd-container>
            <Voorvertoning soort="bezwaar" :op="bz.id" :bevindingen="gegevenOpBezwaar(bz)" />
          </nldd-container>
        </template>
        <template v-else>
          <nldd-inline-dialog
            icon="check-mark-circle"
            icon-color="success"
            text="Ontvankelijk"
            supporting-text="Stap 2: het bestuursorgaan heroverweegt. Geef een bevinding opnieuw, of laat staan wat het in het bestreden besluit vond."
          ></nldd-inline-dialog>
          <nldd-container layout="grid" gap="16">
            <nldd-container gap="12">
              <OordeelKeuze
                v-for="o in toetsen[bz.id].bevindingen"
                :key="o.naam"
                :veld="o"
                :model-value="opnieuwVorm.waarden(bz.id)[o.naam]"
                :leeg="`Laat staan (${jaNee(o.eerder)})`"
                :toelichting="`${o.basis} · eerder: ${jaNee(o.eerder)}`"
                @update:model-value="opnieuwVorm.zet(bz.id, o.naam, $event)"
              />
              <StilOordeel
                v-for="v in stilBijBezwaar(bz)"
                :key="v.naam"
                :veld="v"
                :model-value="opnieuwVorm.waarden(bz.id)[v.naam]"
                @update:model-value="opnieuwVorm.zet(bz.id, v.naam, $event)"
              />
              <nldd-button appearance="primary" text="Beslissen op bezwaar" @click="beslisOpBezwaar(bz)"></nldd-button>
            </nldd-container>
            <Voorvertoning
              soort="bezwaar"
              :op="bz.id"
              :bevindingen="gegevenOpBezwaar(bz)"
              @stil="meldStil(bz.id, $event)"
            />
          </nldd-container>
        </template>
        </nldd-container>
      </nldd-card>
    </nldd-container>
  </nldd-simple-section>

  <nldd-simple-section v-if="Object.keys(beslissingen).length">
    <nldd-title heading-level="2" slot="header" size="display-3" text="Beslissingen op bezwaar"></nldd-title>
    <nldd-container gap="24">
      <nldd-container v-for="(b, id) in beslissingen" :key="id" gap="12">
        <nldd-title heading-level="3"
          size="display-5"
          :text="`Bezwaar ${kort(id)}`"
          :supporting-text="uitkomstOpBezwaar(b.beslissing.outputs)"
        ></nldd-title>
        <nldd-code-viewer language="json" appearance="box-tinted">{{ alsJson(b.beslissing.outputs) }}</nldd-code-viewer>
        <Uitkomst
          v-if="b.nieuw_besluit"
          titel="Nieuw besluit"
          :besluit="b.nieuw_besluit"
          :waarom="b.waarom"
        />
        <Waarom v-else :waarom="b.waarom" />
      </nldd-container>
    </nldd-container>
  </nldd-simple-section>
</template>
