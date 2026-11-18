<template>
  <div class="kt">
    <div class="kt-head">
      <!-- De kolomkeuze staat bij de kolommen die hij vult, niet in het
           linkerpaneel waar alleen staat wat je wijzigt. -->
      <column-menu
        :variants="variants"
        :selected="selectedVariants"
        :max="MAX_VARIANTEN"
        :title-of="kortTitel"
        footer="Tonen, niet bewerken. Geldt ook voor de persona's."
        @update:selected="selectedVariants = $event"
      />
      <nldd-segmented-control size="sm" :value="detail" @change="detail = $event.detail?.value ?? detail">
        <nldd-segmented-control-item value="kern" text="Kern"></nldd-segmented-control-item>
        <nldd-segmented-control-item value="alles" text="Alle posten"></nldd-segmented-control-item>
      </nldd-segmented-control>
    </div>

    <nldd-table
      :columns="gridColumns"
      accessible-label="Posten per kolom"
    >
      <nldd-inline-dialog slot="empty" text="Nog geen uitkomsten" supporting-text="Klik op Doorrekenen om de kolommen te vullen."></nldd-inline-dialog>
      <nldd-table-row slot="header">
        <nldd-text-cell size="sm" text="Post" supporting-text="over de hele looptijd"></nldd-text-cell>
        <nldd-text-cell
          v-for="col in columns"
          :key="col.key"
          size="sm"
          horizontal-alignment="right"
          :text="col.titel"
          :supporting-text="kolomOnderschrift(col)"
        ></nldd-text-cell>
      </nldd-table-row>

      <template v-for="groep in zichtbareGroepen" :key="groep.titel">
        <nldd-table-row class="kt-groep">
          <nldd-text-cell size="sm" :text="`**${groep.titel}**`" :supporting-text="groep.hint"></nldd-text-cell>
          <nldd-text-cell v-for="col in columns" :key="col.key" size="sm" text=""></nldd-text-cell>
        </nldd-table-row>

        <nldd-table-row v-for="rij in groep.rijen" :key="rij.key" :class="{ 'kt-sub': rij.sub }">
          <nldd-text-cell
            size="sm"
            :text="rij.sub ? `— ${rij.label}` : rij.label"
            :supporting-text="rij.hint"
            :color="rij.sub ? 'secondary' : 'content'"
          ></nldd-text-cell>
          <nldd-text-cell
            v-for="col in columns"
            :key="col.key"
            size="sm"
            horizontal-alignment="right"
          >
            <span class="kt-value">{{ metricsByColumn[col.key] ? toon(rij, metricsByColumn[col.key]) : (running[col.key] ? 'rekent…' : '—') }}</span>
            <span v-if="verschilTekst(rij, col)" slot="supporting-text" class="kt-delta" :class="richting(rij, col)">
              {{ verschilTekst(rij, col) }}
            </span>
          </nldd-text-cell>
        </nldd-table-row>
      </template>
    </nldd-table>

    <p class="kt-bron">
      Bedragen zijn de contante som over de hele looptijd van de steekproef, gewogen naar
      {{ number(baseline?.aantalDebiteuren ?? 0) }} debiteuren. De pijl vergelijkt met huidig recht.
      De uitvoeringskosten bij DUO zijn handelingen × minuten × tarief (HOT 2026); de minuten zijn aannames.
      De tijd van debiteuren staat in uren en niet in euro's: er bestaat geen tarief voor de tijd van een burger,
      en er een op plakken zou een uitspraak zijn die dit model niet doet.
    </p>
    <p v-if="ramingRijen.length" class="kt-bron">
      Raming Stand van de Uitvoering OCW 2026 ter vergelijking:
      <template v-for="(r, i) in ramingRijen" :key="r.id">{{ i ? ' · ' : '' }}{{ r.label }} {{ r.raming }}</template>.
      Die raming is een andere grootheid dan de simulatie hierboven (kosten voor het Rijk volgens de brief,
      tegenover wat deze populatie over haar looptijd betaalt), dus lees hem als orde van grootte.
    </p>
  </div>
</template>

<!--
  De twee rekeningen naast elkaar, per kolom: wat de regeling doet met de
  OCW-begroting (afboeking en ontvangsten) en wat de debiteuren ervan merken,
  plus de volumes die bij DUO op het bureau landen.
-->

<script setup>
import { ref, computed } from 'vue';
import ColumnMenu from '@regelrecht/frontend-shared/components/ColumnMenu.vue';
import { useLawStore, kortTitel } from '../../engine/lawStore.js';
import { number, euroCompact, percent, regimeLabel, urenCompact } from '../../lib/format.js';
import { KOLOM_BASIS, usePopulation, MAX_VARIANTEN } from '../../composables/usePopulation.js';
import { VARIANT_RAMING } from '../../lib/regimeFacts.js';

const props = defineProps({
  columns: { type: Array, default: () => [] },
  metricsByColumn: { type: Object, default: () => ({}) },
  running: { type: Object, default: () => ({}) },
});

const { hasChanges, variants } = useLawStore();
const { selectedVariants } = usePopulation();
const detail = ref('kern');

/** Kolombreedtes: de postkolom breed, de waardekolommen gelijk verdeeld. */
const gridColumns = computed(
  () => `minmax(200px, 1.6fr) repeat(${props.columns.length}, minmax(120px, 1fr))`,
);

/** Onder de kolomkop: wat deze kolom is, en of hij nog rekent. */
function kolomOnderschrift(col) {
  if (props.running?.[col.key]) return 'bezig met rekenen';
  if (col.isWerkversie) return hasChanges.value ? 'werkversie · bewerkt' : 'werkversie';
  return col.key === KOLOM_BASIS ? 'huidig recht (basis)' : 'variant (branch)';
}

const baseline = computed(() => props.metricsByColumn[KOLOM_BASIS] ?? null);

const REGIME_ORDER = ['SF15_OUD', 'SF15_NIEUW', 'SF15_LLLK', 'SF35'];

/**
 * Rijen per groep. `get` haalt de waarde uit een metrics-object, `kind`
 * bepaalt de opmaak en `beter` of een stijging goed nieuws is (voor de kleur
 * van de pijl): minder betalingsproblemen is beter, meer ontvangsten ook,
 * maar meer kwijtschelding kost geld.
 */
const GROEPEN = [
  {
    titel: 'OCW-begroting',
    hint: 'wat de regeling kost en oplevert over de looptijd',
    rijen: [
      { key: 'kwijt', label: 'Kwijtschelding', hint: 'restschuld die aan het eind wordt afgeboekt', kind: 'euro', beter: 'lager', kern: true, get: (m) => m.kwijtgescholdenTotaal },
      { key: 'betaald', label: 'Ontvangen aflossingen', hint: 'som van alle termijnen over de looptijd', kind: 'euro', beter: 'hoger', kern: true, get: (m) => m.betaaldTotaal },
      { key: 'rente', label: 'waarvan rente', sub: true, kind: 'euro', beter: 'hoger', get: (m) => m.renteTotaal },
      { key: 'uitstaand', label: 'Uitstaande schuld bij start', sub: true, kind: 'euro', beter: 'neutraal', get: (m) => m.startSchuldTotaal },
    ],
  },
  {
    titel: 'Debiteuren',
    hint: 'wat mensen ervan merken',
    rijen: [
      { key: 'prob', label: 'Met betalingsproblemen', hint: 'maandbedrag boven de Nibud-afloscapaciteit', kind: 'aantal', beter: 'lager', kern: true, get: (m) => m.aantalBetalingsprobleem },
      { key: 'probpct', label: 'aandeel van alle debiteuren', sub: true, kind: 'pct', beter: 'lager', get: (m) => m.pctBetalingsprobleem },
      { key: 'levens', label: 'Levenslang debiteur', hint: 'schuld loopt door tot het einde van de aflosfase', kind: 'aantal', beter: 'lager', kern: true, get: (m) => m.aantalLevenslang },
      { key: 'debiteuren', label: 'Debiteuren in de populatie', sub: true, kind: 'aantal', beter: 'neutraal', get: (m) => m.aantalDebiteuren },
    ],
  },
  {
    titel: 'Per terugbetaalregime',
    hint: 'aandeel met betalingsproblemen',
    rijen: REGIME_ORDER.map((r) => ({
      key: `reg:${r}`,
      label: regimeLabel(r),
      kind: 'pct',
      beter: 'lager',
      get: (m) => m.regimes?.[r]?.pctBetalingsprobleem ?? null,
    })),
  },
  {
    titel: 'Uitvoering',
    hint: 'volumes, kosten bij DUO en tijd van debiteuren',
    rijen: [
      // De twee eenheden staan bewust apart. DUO's tijd is loonkosten en telt
      // in euro's; de tijd van een debiteur heeft geen tarief en telt in uren.
      { key: 'ul_duo', label: 'Uitvoeringskosten DUO', hint: 'handelingen × minuten × tarief (HOT 2026)', kind: 'euro', beter: 'lager', kern: true, get: (m) => m.uitvoeringslast?.kostenTotaal ?? null },
      { key: 'ul_burger', label: 'Tijd van debiteuren', hint: 'aanvragen en achterstanden; bewust niet in euro’s uitgedrukt', kind: 'uren', beter: 'lager', kern: true, get: (m) => m.uitvoeringslast?.urenBurger ?? null },
      { key: 'dk', label: 'Draagkrachtmetingen', hint: 'debiteuren met een meting', kind: 'aantal', beter: 'neutraal', kern: true, get: (m) => m.uitvoering?.draagkrachtmetingen ?? null },
      { key: 'oo', label: 'Partner-opt-outs', hint: 'partnerinkomen niet laten meetellen', kind: 'aantal', beter: 'neutraal', get: (m) => m.uitvoering?.partnerOptOuts ?? null },
      // Peiljaarverleggingen (artikel 6.12) stond hier en telde altijd 0:
      // `population.js` zet die keuze op elk record hard op false en
      // `distributions.yaml` heeft er geen prior voor, anders dan voor
      // draagkrachtmetingen en partner-opt-outs. Een rij die per constructie
      // nul toont, leest als "dit gebeurt niet" in plaats van "dit wordt niet
      // gesimuleerd". Terug zodra er een onderbouwd percentage is: een
      // `peiljaarverlegging_prior` erbij, samplen zoals de andere twee, en
      // deze rij terugzetten.
    ],
  },
];

const zichtbareGroepen = computed(() => GROEPEN
  .map((g) => ({ ...g, rijen: g.rijen.filter((r) => detail.value === 'alles' || r.kern || g.titel === 'Per terugbetaalregime') }))
  .filter((g) => g.rijen.length));

function toon(rij, m) {
  const v = rij.get(m);
  if (v === null || v === undefined) return '—';
  if (rij.kind === 'euro') return euroCompact(v);
  if (rij.kind === 'pct') return percent(v, 1);
  if (rij.kind === 'uren') return urenCompact(v);
  return number(v);
}

function verschil(rij, col) {
  if (col.key === KOLOM_BASIS || !baseline.value) return null;
  const m = props.metricsByColumn[col.key];
  if (!m) return null;
  const a = rij.get(m);
  const b = rij.get(baseline.value);
  if (a === null || b === null || a === undefined || b === undefined) return null;
  const d = a - b;
  return Math.abs(d) < 1e-9 ? null : d;
}

function verschilTekst(rij, col) {
  const d = verschil(rij, col);
  if (d === null) return '';
  const teken = d > 0 ? '+' : '−';
  const abs = Math.abs(d);
  if (rij.kind === 'euro') return `${teken}${euroCompact(abs)}`;
  if (rij.kind === 'pct') return `${teken}${percent(abs, 1)}`;
  if (rij.kind === 'uren') return `${teken}${urenCompact(abs)}`;
  return `${teken}${number(abs)}`;
}

/** Groen als het de goede kant op gaat, rood als het de verkeerde kant op gaat. */
function richting(rij, col) {
  const d = verschil(rij, col);
  if (d === null || rij.beter === 'neutraal') return '';
  const goed = rij.beter === 'lager' ? d < 0 : d > 0;
  return goed ? 'kt-goed' : 'kt-slecht';
}

/** De PDF-raming per gekozen variant, als externe toets naast de simulatie. */
const ramingRijen = computed(() => props.columns
  .filter((c) => c.variantId && VARIANT_RAMING[c.variantId])
  .map((c) => ({ id: c.variantId, label: c.titel, raming: VARIANT_RAMING[c.variantId] })));
</script>

<style scoped>
.kt { display: flex; flex-direction: column; gap: var(--primitives-space-8); }
/* De kolomkeuze links, de detailkeuze rechts: wat er in de tabel staat
   tegenover hoe je ernaar kijkt. `auto` op de marge houdt die tweedeling ook
   heel als de rij afbreekt op een smal scherm. */
.kt-head { display: flex; gap: var(--primitives-space-12); flex-wrap: wrap; align-items: center; }
.kt-head > nldd-segmented-control { margin-left: auto; }
/* nldd-table-row zet geen ::part, dus de groepsrij valt op aan zijn eigen
   inhoud (vetgedrukte kop, lege waardekolommen), niet aan een achtergrond. */
.kt-value { font-variant-numeric: tabular-nums; }
.kt-delta { font-variant-numeric: tabular-nums; }
.kt-goed { color: var(--semantics-content-success-color); }
.kt-slecht { color: var(--semantics-content-critical-color); }
.kt-bron { margin: 0; font-size: 0.8em; color: var(--semantics-content-secondary-color); }
</style>
