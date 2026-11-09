<template>
  <div class="pop-controls">
    <div v-if="hasOverrides" class="pc-head">
      <nldd-button
        text="Herstel de aannames"
        start-icon="undo"
        appearance="neutral-transparent"
        size="sm"
        @click="resetOverrides"
      ></nldd-button>
    </div>

    <p class="pc-uitleg">
      De steekproef wordt één keer getrokken en daarna hergebruikt: dezelfde {{ number(n) }} records staan in elke
      kolom en bij elke herberekening, dus verschillen tussen kolommen komen alleen uit de regelgeving. Elk record
      staat voor {{ decimal(gewicht, 1) }} echte leerlingen.
    </p>

    <!-- WAT ER NU LIGT ------------------------------------------------------->
    <section v-if="profiel" class="pc-profiel">
      <h4 class="pc-kop">Wat er nu ligt</h4>
      <dl class="pc-kern">
        <div>
          <dt>Records</dt>
          <dd>{{ number(profiel.records) }}</dd>
        </div>
        <div>
          <dt>Echte leerlingen</dt>
          <dd>{{ number(profiel.echt) }}</dd>
        </div>
        <div>
          <dt>Scholen</dt>
          <dd>{{ number(profiel.scholen) }}</dd>
        </div>
        <div>
          <dt>Instroomjaren</dt>
          <dd>{{ profiel.jaarBereik }}</dd>
        </div>
      </dl>

      <table class="pc-tabel">
        <caption class="pc-caption">Verdeling van de getrokken records</caption>
        <thead>
          <tr>
            <th scope="col">Kenmerk</th>
            <th scope="col" class="num">po</th>
            <th scope="col" class="num">vo</th>
            <th scope="col" class="num">samen</th>
          </tr>
        </thead>
        <tbody>
          <tr>
            <th scope="row">Records</th>
            <td class="num">{{ number(profiel.sector.po) }}</td>
            <td class="num">{{ number(profiel.sector.vo) }}</td>
            <td class="num">{{ number(profiel.records) }}</td>
          </tr>
          <tr v-for="cat in profiel.categorieen" :key="cat.naam">
            <th scope="row">{{ categorieNaam(cat.naam) }}</th>
            <td class="num">{{ percent(cat.po) }}</td>
            <td class="num">{{ percent(cat.vo) }}</td>
            <td class="num">{{ percent(cat.totaal) }}</td>
          </tr>
          <tr>
            <th scope="row">Jonger dan 4 bij vestiging</th>
            <td class="num">{{ percent(profiel.onderVier.po) }}</td>
            <td class="num">—</td>
            <td class="num">{{ percent(profiel.onderVier.totaal) }}</td>
          </tr>
          <tr>
            <th scope="row">Mediane leeftijd bij vestiging</th>
            <td class="num">{{ decimal(profiel.leeftijd.po, 1) }}</td>
            <td class="num">{{ decimal(profiel.leeftijd.vo, 1) }}</td>
            <td class="num">{{ decimal(profiel.leeftijd.totaal, 1) }}</td>
          </tr>
          <tr>
            <th scope="row">Mediane wachttijd tot inschrijving (dagen)</th>
            <td class="num">{{ number(profiel.wachttijd.po) }}</td>
            <td class="num">{{ number(profiel.wachttijd.vo) }}</td>
            <td class="num">{{ number(profiel.wachttijd.totaal) }}</td>
          </tr>
          <tr>
            <th scope="row">Scholen onder de drempel van vier</th>
            <td class="num">{{ percent(profiel.onderDrempel.po) }}</td>
            <td class="num">{{ percent(profiel.onderDrempel.vo) }}</td>
            <td class="num">{{ percent(profiel.onderDrempel.totaal) }}</td>
          </tr>
        </tbody>
      </table>

      <p class="pc-bron">
        Herkomst: CBS StatLine (aankomsten naar leeftijd, migratiemotief, asielverzoeken), DUO-open data voor het
        aantal scholen en de OCW-factsheet met bekostigde nieuwkomers per 1 oktober. Waar microdata ontbreekt staat
        een aanname; die staan hieronder als knop.
      </p>
    </section>
    <p v-else class="pc-stand">Nog geen steekproef getrokken; reken huidig recht door.</p>

    <!-- AANNAMES BIJSTELLEN --------------------------------------------------->
    <details class="pc-group">
      <summary>
        Instroom per jaar
        <nldd-badge v-if="gewijzigd('instroom')" size="sm" color="warning" text="aangepast"></nldd-badge>
      </summary>
      <div class="pc-fields">
        <p class="pc-hint">{{ HERKOMST.instroom }} Nieuwe nieuwkomers per kalenderjaar; samen bepalen ze het gewicht per record.</p>
        <div v-for="rij in instroomRijen" :key="rij.jaar" class="pc-row">
          <nldd-form-field :label="`${rij.jaar} po`">
            <nldd-number-field
              :value="rij.po"
              :min="0"
              :step="500"
              size="sm"
              @input="debounced(`i:${rij.jaar}:po`, () => setInstroom(rij.jaar, 'po', num($event)))"
            ></nldd-number-field>
          </nldd-form-field>
          <nldd-form-field :label="`${rij.jaar} vo`">
            <nldd-number-field
              :value="rij.vo"
              :min="0"
              :step="500"
              size="sm"
              @input="debounced(`i:${rij.jaar}:vo`, () => setInstroom(rij.jaar, 'vo', num($event)))"
            ></nldd-number-field>
          </nldd-form-field>
        </div>
        <p class="pc-hint">Samen {{ number(instroomTotaal) }} nieuwkomers over alle instroomjaren.</p>
      </div>
    </details>

    <details class="pc-group">
      <summary>
        Categorieverdeling
        <nldd-badge v-if="gewijzigd('categorie')" size="sm" color="warning" text="aangepast"></nldd-badge>
      </summary>
      <div class="pc-fields">
        <p class="pc-hint">{{ HERKOMST.categorie }} In het po bepaalt de categorie het venster (8 of 4 kwartalen) en het bedrag.</p>
        <template v-for="groep in categorieVerdeling" :key="groep.sector">
          <p class="pc-subkop">{{ sectorLabel(groep.sector) }}</p>
          <nldd-form-field
            v-for="item in groep.items"
            :key="`${groep.sector}:${item.naam}`"
            :label="`${categorieNaam(item.naam)} (%)`"
            :supporting-label="aandeelLabel(item, groep.totaal)"
          >
            <nldd-number-field
              :value="Math.round(item.waarde * 1000) / 10"
              :min="0"
              :max="100"
              :step="1"
              size="sm"
              @input="debounced(`c:${groep.sector}:${item.naam}`, () => setCategorie(groep.sector, item.naam, num($event) / 100))"
            ></nldd-number-field>
          </nldd-form-field>
        </template>
      </div>
    </details>

    <details class="pc-group">
      <summary>
        Scholen
        <nldd-badge v-if="gewijzigd('scholen')" size="sm" color="warning" text="aangepast"></nldd-badge>
      </summary>
      <div class="pc-fields">
        <p class="pc-hint">{{ HERKOMST.scholen }} Minder scholen betekent meer nieuwkomers per school, dus minder scholen onder de drempel van vier (art. 34 lid 2).</p>
        <nldd-form-field
          v-for="sector in ['po', 'vo']"
          :key="sector"
          :label="`Scholen met nieuwkomers, ${sector}`"
          :supporting-label="`van ${number(basis?.scholen?.[sector]?.aantal_scholen_totaal ?? 0)} scholen in totaal`"
        >
          <nldd-number-field
            :value="effectief?.scholen?.[sector]?.aantal ?? 0"
            :min="1"
            :step="50"
            size="sm"
            @input="debounced(`s:${sector}`, () => setScholen(sector, num($event)))"
          ></nldd-number-field>
        </nldd-form-field>
      </div>
    </details>

    <details class="pc-group">
      <summary>
        Gedrag en oordeel
        <nldd-badge v-if="gewijzigd('fracties')" size="sm" color="warning" text="aangepast"></nldd-badge>
      </summary>
      <div class="pc-fields">
        <nldd-form-field
          v-for="knop in FRACTIE_KNOPPEN"
          :key="knop.key"
          :label="`${knop.label} (%)`"
          :supporting-label="knop.uitleg"
        >
          <nldd-number-field
            :value="Math.round((effectief?.[knop.key] ?? 0) * 1000) / 10"
            :min="0"
            :max="100"
            :step="1"
            size="sm"
            @input="debounced(`f:${knop.key}`, () => setFractie(knop.key, num($event) / 100))"
          ></nldd-number-field>
        </nldd-form-field>
      </div>
    </details>

    <details class="pc-group">
      <summary>Steekproef en herberekenen</summary>
      <div class="pc-fields">
        <nldd-form-field
          label="Aantal records (N)"
          supporting-label="Meer records maakt de stand per jaar rustiger en de simulatie trager; de weging naar de echte instroom verandert niet."
        >
          <nldd-number-field
            :value="n"
            :min="200"
            :max="5000"
            :step="100"
            size="sm"
            @input="n = $event.detail?.value ?? n"
          ></nldd-number-field>
        </nldd-form-field>
        <nldd-form-field
          label="Seed"
          supporting-label="Technisch: het startgetal van de toevalsgenerator. Een andere seed trekt een andere steekproef uit dezelfde verdelingen."
        >
          <nldd-number-field
            :value="seed"
            :min="0"
            :step="1"
            size="sm"
            @input="seed = $event.detail?.value ?? seed"
          ></nldd-number-field>
        </nldd-form-field>
        <nldd-switch-field
          label="Automatisch herberekenen bij een wijziging"
          :checked="autoRecompute ? true : undefined"
          @change="autoRecompute = $event.detail?.checked ?? autoRecompute"
        ></nldd-switch-field>
      </div>
    </details>
  </div>
</template>

<script setup>
import { computed } from 'vue';
import { useSimulation } from '../../composables/useSimulation.js';
import { usePopulatie, FRACTIE_KNOPPEN, HERKOMST } from '../../composables/usePopulatie.js';
import { number, decimal, percent } from '../../lib/format.js';
import { sectorLabel } from '../../lib/nieuwkomerFacts.js';

const { n, seed, autoRecompute, records } = useSimulation();
const {
  basis, effectief, overrides, hasOverrides, categorieVerdeling, instroomRijen, instroomTotaal,
  setInstroom, setCategorie, setScholen, setFractie, resetOverrides,
} = usePopulatie();

const CATEGORIE_NAMEN = {
  asielzoeker: 'Asielzoeker',
  overige_vreemdeling: 'Overige vreemdeling',
  ambigu_code: 'Ambigue code (bestuur beslist)',
  zonder_bsn: 'Zonder BSN (onderwijsnummer)',
};

function categorieNaam(naam) {
  return CATEGORIE_NAMEN[naam] ?? naam;
}

function gewijzigd(blok) {
  return Object.keys(overrides[blok] ?? {}).length > 0;
}

/** De verdeling telt zelden precies op tot 100 %; toon wat de generator er feitelijk van maakt. */
function aandeelLabel(item, totaal) {
  if (Math.abs(totaal - 1) < 0.005) return '';
  return `genormaliseerd ${percent(item.aandeel)} (de vier tellen op tot ${percent(totaal)})`;
}

const gewicht = computed(() => (records.value.length ? Number(records.value[0]?.gewicht ?? 0) : 0));

function mediaan(waarden) {
  if (!waarden.length) return 0;
  const s = [...waarden].sort((a, b) => a - b);
  const m = Math.floor(s.length / 2);
  return s.length % 2 ? s[m] : (s[m - 1] + s[m]) / 2;
}

function jaarVan(iso) {
  return Number(String(iso).slice(0, 4));
}

/** Leeftijd in jaren tussen twee ISO-datums. */
function leeftijdBij(geboortedatum, datum) {
  return (new Date(datum) - new Date(geboortedatum)) / (365.25 * 24 * 3600 * 1000);
}

/** Wat er feitelijk getrokken is: de steekproef zelf doorgemeten, niet de verdeling. */
const profiel = computed(() => {
  const recs = records.value;
  if (!recs?.length) return null;

  const sector = { po: 0, vo: 0 };
  const perCat = {};
  const leeftijden = { po: [], vo: [], totaal: [] };
  const wachttijden = { po: [], vo: [], totaal: [] };
  const onderVier = { po: 0, totaal: 0 };
  const scholen = { po: new Map(), vo: new Map() };
  let echt = 0;
  let jaarMin = Infinity;
  let jaarMax = -Infinity;

  for (const r of recs) {
    const s = r.sector === 'vo' ? 'vo' : 'po';
    sector[s]++;
    echt += Number(r.gewicht ?? 0);

    const cat = r.heeft_bsn === false ? 'zonder_bsn' : categorieVanRecord(r);
    perCat[cat] ??= { po: 0, vo: 0, totaal: 0 };
    perCat[cat][s]++;
    perCat[cat].totaal++;

    const leeftijd = leeftijdBij(r.geboortedatum, r.datum_vestiging_nederland);
    leeftijden[s].push(leeftijd);
    leeftijden.totaal.push(leeftijd);
    if (leeftijd < 4) {
      onderVier.totaal++;
      if (s === 'po') onderVier.po++;
    }

    const wacht = Math.round(
      (new Date(r.eerste_inschrijfdatum) - new Date(r.datum_vestiging_nederland)) / (24 * 3600 * 1000),
    );
    wachttijden[s].push(wacht);
    wachttijden.totaal.push(wacht);

    scholen[s].set(r.school_id, (scholen[s].get(r.school_id) ?? 0) + 1);

    const jaar = jaarVan(r.eerste_inschrijfdatum);
    if (jaar < jaarMin) jaarMin = jaar;
    if (jaar > jaarMax) jaarMax = jaar;
  }

  const drempel = (m) => {
    if (!m.size) return 0;
    let onder = 0;
    for (const aantal of m.values()) if (aantal < 4) onder++;
    return onder / m.size;
  };
  const alleScholen = scholen.po.size + scholen.vo.size;
  const onderTotaal = alleScholen
    ? (drempel(scholen.po) * scholen.po.size + drempel(scholen.vo) * scholen.vo.size) / alleScholen
    : 0;

  const volgorde = ['asielzoeker', 'overige_vreemdeling', 'ambigu_code', 'zonder_bsn'];
  const categorieen = volgorde
    .filter((naam) => perCat[naam])
    .map((naam) => ({
      naam,
      po: sector.po ? perCat[naam].po / sector.po : 0,
      vo: sector.vo ? perCat[naam].vo / sector.vo : 0,
      totaal: perCat[naam].totaal / recs.length,
    }));

  return {
    records: recs.length,
    echt: Math.round(echt),
    scholen: alleScholen,
    jaarBereik: jaarMin === jaarMax ? String(jaarMin) : `${jaarMin} tot en met ${jaarMax}`,
    sector,
    categorieen,
    onderVier: { po: sector.po ? onderVier.po / sector.po : 0, totaal: onderVier.totaal / recs.length },
    leeftijd: {
      po: mediaan(leeftijden.po),
      vo: mediaan(leeftijden.vo),
      totaal: mediaan(leeftijden.totaal),
    },
    wachttijd: {
      po: mediaan(wachttijden.po),
      vo: mediaan(wachttijden.vo),
      totaal: mediaan(wachttijden.totaal),
    },
    onderDrempel: { po: drempel(scholen.po), vo: drempel(scholen.vo), totaal: onderTotaal },
  };
});

/**
 * De categorie is in het record impliciet: de verblijfstitelcode bepaalt hem.
 * Codes die bij beide groepen voorkomen (art. 34 lid 11) zijn de ambigue.
 */
const AMBIGUE_CODES = new Set([21, 33, 34, 98]);
const ASIEL_CODES = new Set([26, 27, 32, 39, 44, 46, 50, 51, 52, 53, 54, 55, 56]);

function categorieVanRecord(r) {
  const code = r.verblijfstitel_code;
  if (code === null || code === undefined) return 'zonder_bsn';
  if (AMBIGUE_CODES.has(Number(code))) return 'ambigu_code';
  return ASIEL_CODES.has(Number(code)) ? 'asielzoeker' : 'overige_vreemdeling';
}

function num(event) {
  const v = event.detail?.value ?? Number(event.target?.value);
  return v === null || v === undefined || Number.isNaN(v) ? 0 : Number(v);
}

const timers = new Map();
function debounced(key, fn) {
  clearTimeout(timers.get(key));
  timers.set(key, setTimeout(fn, 400));
}
</script>

<style scoped>
.pop-controls { display: flex; flex-direction: column; gap: var(--primitives-space-12); }
.pc-head { display: flex; align-items: center; justify-content: space-between; gap: var(--primitives-space-8); }
.pc-uitleg { margin: 0; font-size: 0.9em; color: var(--semantics-content-secondary-color); }
.pc-stand { margin: 0; font-size: 0.85em; color: var(--semantics-content-secondary-color); }
.pc-profiel { display: flex; flex-direction: column; gap: var(--primitives-space-8); }
.pc-kop { margin: 0; font-size: 0.9em; font-weight: 600; }
.pc-subkop { margin: var(--primitives-space-4) 0 0; font-size: 0.85em; font-weight: 600; }
.pc-kern {
  display: grid;
  grid-template-columns: repeat(auto-fit, minmax(120px, 1fr));
  gap: var(--primitives-space-8);
  margin: 0;
}
.pc-kern dt { font-size: 0.8em; color: var(--semantics-content-secondary-color); }
.pc-kern dd { margin: 0; font-size: 1.1em; font-weight: 600; }
.pc-tabel { width: 100%; border-collapse: collapse; font-size: 0.85em; }
.pc-caption { text-align: left; font-size: 0.85em; color: var(--semantics-content-secondary-color); padding-bottom: var(--primitives-space-4); }
.pc-tabel th, .pc-tabel td { text-align: left; padding: var(--primitives-space-4) var(--primitives-space-8); border-bottom: 1px solid var(--semantics-dividers-color); }
.pc-tabel th[scope='row'] { font-weight: 400; }
.pc-tabel .num { text-align: right; font-variant-numeric: tabular-nums; }
.pc-bron { margin: 0; font-size: 0.8em; color: var(--semantics-content-secondary-color); }
.pc-group {
  border: 1px solid var(--semantics-dividers-color);
  border-radius: var(--semantics-surfaces-corner-radius);
  padding: var(--primitives-space-8) var(--primitives-space-12);
}
.pc-group summary { cursor: pointer; font-weight: 600; font-size: 0.9em; }
.pc-fields { display: flex; flex-direction: column; gap: var(--primitives-space-12); margin-top: var(--primitives-space-12); }
.pc-row { display: flex; gap: var(--primitives-space-12); flex-wrap: wrap; }
.pc-hint { margin: 0; font-size: 0.8em; color: var(--semantics-content-secondary-color); }
</style>
