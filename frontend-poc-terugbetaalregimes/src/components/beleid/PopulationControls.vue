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
      De steekproef wordt één keer getrokken en daarna hergebruikt: dezelfde {{ number(n) }} debiteuren staan in elke
      kolom en bij elke herberekening, dus verschillen tussen kolommen komen alleen uit de regelgeving. Elk record
      staat voor {{ number(Math.round(gewicht)) }} echte debiteuren.
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
          <dt>Echte debiteuren</dt>
          <dd>{{ number(profiel.echt) }}</dd>
        </div>
        <div>
          <dt>Mediane schuld</dt>
          <dd>{{ euroCompact(profiel.schuldMediaan) }}</dd>
        </div>
        <div>
          <dt>Mediaan inkomen</dt>
          <dd>{{ euroCompact(profiel.inkomenMediaan) }}</dd>
        </div>
      </dl>

      <table class="pc-tabel">
        <caption class="pc-caption">Verdeling over de terugbetaalregimes</caption>
        <thead>
          <tr>
            <th scope="col">Regime</th>
            <th scope="col" class="num">Records</th>
            <th scope="col" class="num">Aandeel</th>
            <th scope="col" class="num">Mediane schuld</th>
            <th scope="col" class="num">Draagkrachtmeting</th>
          </tr>
        </thead>
        <tbody>
          <tr v-for="rij in profiel.regimes" :key="rij.regime">
            <th scope="row">
              <nldd-tag :color="regimeColor(rij.regime)" size="sm" :text="regimeLabel(rij.regime)"></nldd-tag>
            </th>
            <td class="num">{{ number(rij.aantal) }}</td>
            <td class="num">{{ percent(rij.aandeel) }}</td>
            <td class="num">{{ euroCompact(rij.schuldMediaan) }}</td>
            <td class="num">{{ percent(rij.draagkracht) }}</td>
          </tr>
        </tbody>
      </table>

      <dl class="pc-kern">
        <div>
          <dt>Met partner</dt>
          <dd>{{ percent(profiel.metPartner) }}</dd>
        </div>
        <div>
          <dt>Partner niet meetellen</dt>
          <dd>{{ percent(profiel.optOut) }}</dd>
        </div>
        <div>
          <dt>Mediane leeftijd</dt>
          <dd>{{ number(profiel.leeftijdMediaan) }}</dd>
        </div>
      </dl>

      <p class="pc-bron">
        Herkomst: CBS StatLine (debiteuren, schuld, inkomen, huishoudens), de Stand van DUO 2023 en de Stand van de
        Uitvoering OCW 2026. Waar geen openbaar cijfer bestaat staat een aanname; die staan hieronder als knop.
      </p>
    </section>
    <p v-else class="pc-stand">Nog geen steekproef getrokken; reken huidig recht door.</p>

    <!-- AANNAMES BIJSTELLEN --------------------------------------------------->
    <details class="pc-group">
      <summary>
        Omvang en regimeverdeling
        <nldd-badge v-if="gewijzigd('totaal') || gewijzigd('regimeVerdeling')" size="sm" color="warning" text="aangepast"></nldd-badge>
      </summary>
      <div class="pc-fields">
        <p class="pc-hint">{{ HERKOMST.totaal }}</p>
        <nldd-form-field label="Aflossende debiteuren" supporting-label="het aantal waar de steekproef naar weegt">
          <nldd-number-field
            :value="totaalDebiteuren"
            :min="1000"
            :step="50000"
            size="sm"
            @input="debounced('td', $event, (value) => setTotaalDebiteuren(value))"
          ></nldd-number-field>
        </nldd-form-field>
        <p class="pc-hint">{{ HERKOMST.regime }} Het regime zelf volgt in de wet uit het cohort; deze verdeling stuurt alleen welke cohortparameters een record trekt.</p>
        <nldd-form-field
          v-for="rij in regimeVerdeling"
          :key="rij.regime"
          :label="`${regimeLabel(rij.regime)} (%)`"
          :supporting-label="aandeelLabel(rij)"
        >
          <nldd-number-field
            :value="Math.round(rij.waarde * 1000) / 10"
            :min="0"
            :max="100"
            :step="1"
            size="sm"
            @input="debounced(`r:${rij.regime}`, $event, (value) => setRegimeAandeel(rij.regime, value / 100))"
          ></nldd-number-field>
        </nldd-form-field>
      </div>
    </details>

    <details class="pc-group">
      <summary>
        Draagkrachtmeting per regime
        <nldd-badge v-if="gewijzigd('draagkracht')" size="sm" color="warning" text="aangepast"></nldd-badge>
      </summary>
      <div class="pc-fields">
        <p class="pc-hint">{{ HERKOMST.draagkracht }}</p>
        <nldd-form-field
          v-for="regime in REGIME_ORDER"
          :key="regime"
          :label="`${regimeLabel(regime)} (%)`"
          supporting-label="aandeel dat een draagkrachtmeting heeft"
        >
          <nldd-number-field
            :value="Math.round((effectief?.regimes?.[regime]?.draagkrachtmeting_prior ?? 0) * 1000) / 10"
            :min="0"
            :max="100"
            :step="5"
            size="sm"
            @input="debounced(`d:${regime}`, $event, (value) => setDraagkrachtPrior(regime, value / 100))"
          ></nldd-number-field>
        </nldd-form-field>
      </div>
    </details>

    <details class="pc-group">
      <summary>
        Huishoudens en gedrag
        <nldd-badge v-if="gewijzigd('huishouden') || gewijzigd('gedrag') || gewijzigd('inkomen')" size="sm" color="warning" text="aangepast"></nldd-badge>
      </summary>
      <div class="pc-fields">
        <p class="pc-hint">{{ HERKOMST.huishouden }} Wie een partner heeft, telt dat inkomen mee in de draagkracht, tenzij hij daarvoor kiest.</p>
        <nldd-form-field
          v-for="rij in huishoudVerdeling"
          :key="rij.type"
          :label="`${HUISHOUD_LABELS[rij.type] ?? rij.type} (%)`"
          :supporting-label="aandeelLabel(rij)"
        >
          <nldd-number-field
            :value="Math.round(rij.waarde * 1000) / 10"
            :min="0"
            :max="100"
            :step="1"
            size="sm"
            @input="debounced(`h:${rij.type}`, $event, (value) => setHuishouden(rij.type, value / 100))"
          ></nldd-number-field>
        </nldd-form-field>
        <p class="pc-hint">{{ HERKOMST.gedrag }}</p>
        <nldd-form-field
          label="Partner niet laten meetellen (%)"
          supporting-label="van de SF15-oud-debiteuren met een partner; verlengt de aflosfase"
        >
          <nldd-number-field
            :value="Math.round((effectief?.gedrag?.partner_opt_out_prior ?? 0) * 1000) / 10"
            :min="0"
            :max="100"
            :step="1"
            size="sm"
            @input="debounced('optout', $event, (value) => setGedrag('partner_opt_out_prior', value / 100))"
          ></nldd-number-field>
        </nldd-form-field>
        <p class="pc-hint">{{ HERKOMST.inkomen }}</p>
        <nldd-form-field label="Inkomensgroei per jaar (%)" supporting-label="reële groei over de hele looptijd">
          <nldd-number-field
            :value="Math.round((effectief?.inkomen?.groei_per_jaar ?? 0) * 1000) / 10"
            :min="-10"
            :max="20"
            :step="0.5"
            size="sm"
            @input="debounced('groei', $event, (value) => setInkomensgroei(value / 100))"
          ></nldd-number-field>
        </nldd-form-field>
      </div>
    </details>

    <details class="pc-group">
      <summary>Steekproef en herberekenen</summary>
      <div class="pc-fields">
        <nldd-form-field
          label="Aantal records (N)"
          supporting-label="Elke debiteur loopt tot 35 jaar door de wet, dus meer records kost merkbaar meer rekentijd. De weging naar het echte aantal debiteuren verandert niet."
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
import { usePopulation } from '../../composables/usePopulation.js';
import {
  usePopulatieAannames, REGIME_ORDER, HUISHOUD_LABELS, HERKOMST,
} from '../../composables/usePopulatieAannames.js';
import { number, euroCompact, percent, regimeLabel, regimeColor } from '../../lib/format.js';
import { regimeVanCohort } from '../../lib/regimeFacts.js';

const { n, seed, autoRecompute, records } = usePopulation();
const {
  effectief, overrides, hasOverrides, regimeVerdeling, huishoudVerdeling, totaalDebiteuren,
  setTotaalDebiteuren, setRegimeAandeel, setDraagkrachtPrior, setHuishouden, setGedrag,
  setInkomensgroei, resetOverrides,
} = usePopulatieAannames();

function gewijzigd(blok) {
  return Object.keys(overrides[blok] ?? {}).length > 0;
}

/** De verdeling telt zelden precies op tot 100 %; toon wat de generator ervan maakt. */
function aandeelLabel(rij) {
  if (Math.abs(rij.aandeel - rij.waarde) < 0.0005) return '';
  return `genormaliseerd ${percent(rij.aandeel)}`;
}

const gewicht = computed(() => Number(records.value[0]?.gewicht ?? 0));

function mediaan(waarden) {
  if (!waarden.length) return 0;
  const s = [...waarden].sort((a, b) => a - b);
  const m = Math.floor(s.length / 2);
  return s.length % 2 ? s[m] : (s[m - 1] + s[m]) / 2;
}

/** Wat er feitelijk getrokken is: de steekproef doorgemeten, niet de verdeling. */
const profiel = computed(() => {
  const recs = records.value;
  if (!recs?.length) return null;

  const perRegime = new Map();
  const schulden = [];
  const inkomens = [];
  const leeftijden = [];
  let echt = 0;
  let metPartner = 0;
  let optOut = 0;

  for (const r of recs) {
    const regime = regimeVanCohort(r);
    if (!perRegime.has(regime)) perRegime.set(regime, { aantal: 0, schulden: [], draagkracht: 0 });
    const g = perRegime.get(regime);
    g.aantal++;
    g.schulden.push(Number(r.schuld ?? 0));
    if (r.keuzes?.draagkrachtAangevraagd) g.draagkracht++;

    echt += Number(r.gewicht ?? 0);
    schulden.push(Number(r.schuld ?? 0));
    inkomens.push(Number(r.inkomen ?? 0));
    leeftijden.push(Number(r.geboortejaar ? 2026 - r.geboortejaar : 0));
    if (r.heeft_partner) {
      metPartner++;
      if (r.keuzes?.partnerMeetellen === false) optOut++;
    }
  }

  return {
    records: recs.length,
    echt: Math.round(echt),
    schuldMediaan: mediaan(schulden),
    inkomenMediaan: mediaan(inkomens),
    leeftijdMediaan: mediaan(leeftijden),
    metPartner: metPartner / recs.length,
    // Het opt-out-aandeel slaat op wie een partner heeft; zonder partner speelt het niet.
    optOut: metPartner ? optOut / metPartner : 0,
    regimes: REGIME_ORDER.filter((r) => perRegime.has(r)).map((regime) => {
      const g = perRegime.get(regime);
      return {
        regime,
        aantal: g.aantal,
        aandeel: g.aantal / recs.length,
        schuldMediaan: mediaan(g.schulden),
        draagkracht: g.aantal ? g.draagkracht / g.aantal : 0,
      };
    }),
  };
});

/**
 * The number in a number field, or null while the field is empty. Since design
 * system 0.18 an emptied field reports null; Number(null) would read that as 0.
 */
function num(event) {
  const v = event.detail?.value ?? event.target?.value;
  if (v === null || v === undefined || v === '') return null;
  const n = Number(v);
  return Number.isNaN(n) ? null : n;
}

const timers = new Map();
// An empty field is no answer yet: the value it had stays until a number is typed.
function debounced(key, event, apply) {
  const n = num(event);
  if (n === null) return;
  clearTimeout(timers.get(key));
  timers.set(key, setTimeout(() => apply(n), 400));
}
</script>

<style scoped>
.pop-controls { display: flex; flex-direction: column; gap: var(--primitives-space-12); }
.pc-head { display: flex; align-items: center; justify-content: space-between; gap: var(--primitives-space-8); }
.pc-uitleg { margin: 0; font-size: 0.9em; color: var(--semantics-content-secondary-color); }
.pc-stand { margin: 0; font-size: 0.85em; color: var(--semantics-content-secondary-color); }
.pc-profiel { display: flex; flex-direction: column; gap: var(--primitives-space-8); }
.pc-kop { margin: 0; font-size: 0.9em; font-weight: 600; }
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
.pc-hint { margin: 0; font-size: 0.8em; color: var(--semantics-content-secondary-color); }
</style>
