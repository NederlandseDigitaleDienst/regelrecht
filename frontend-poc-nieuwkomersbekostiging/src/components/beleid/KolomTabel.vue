<template>
  <div class="kolom-tabel">
    <div class="kt-head">
      <!-- De kolomkeuze staat bij de kolommen die hij vult, niet in het
           linkerpaneel waar alleen staat wat je wijzigt. -->
      <column-menu
        :variants="variants"
        :selected="selectedVariants"
        :max="MAX_VARIANTEN"
        :title-of="shortTitle"
        footer="Tonen, niet bewerken: wijzigen gaat op de werkversie."
        @update:selected="selectedVariants = $event"
      />
      <nldd-segmented-control size="sm" :value="periode" @change="periode = $event.detail?.value ?? periode">
        <nldd-segmented-control-item value="totaal" text="Totaal"></nldd-segmented-control-item>
        <nldd-segmented-control-item v-for="j in jaren" :key="j" :value="String(j)" :text="String(j)"></nldd-segmented-control-item>
      </nldd-segmented-control>
      <nldd-segmented-control size="sm" :value="detail" class="kt-detail" @change="detail = $event.detail?.value ?? detail">
        <nldd-segmented-control-item value="kern" text="Kern"></nldd-segmented-control-item>
        <nldd-segmented-control-item value="alles" text="Alle posten"></nldd-segmented-control-item>
      </nldd-segmented-control>
    </div>

    <nldd-table
      :columns="gridColumns"
      accessible-label="Vergelijking van kosten per kolom"
    >
      <nldd-inline-dialog slot="empty" text="Nog geen uitkomsten" supporting-text="Kies een populatie en klik op Doorrekenen."></nldd-inline-dialog>
      <nldd-table-row slot="header">
        <nldd-text-cell size="sm" text="Post" :supporting-text="periode === 'totaal' ? `totaal ${jaren[0]}–${jaren[jaren.length - 1]}` : `jaar ${periode}`"></nldd-text-cell>
        <nldd-text-cell
          v-for="col in columns"
          :key="col.key"
          size="sm"
          horizontal-alignment="right"
          :text="col.key === 'ist' ? 'Huidig recht' : col.short"
          :supporting-text="running?.[col.key] ? `bezig, ${Math.round((progress?.[col.key] ?? 0) * 100)} %` : (col.variantId ?? null) === werkversie ? (changeCount ? `werkversie · ${changeCount} bewerkt` : 'werkversie') : col.key === 'ist' ? 'huidig recht (basis)' : 'variant (branch)'"
        ></nldd-text-cell>
      </nldd-table-row>

      <nldd-table-row v-for="row in zichtbareRijen" :key="row.key" :class="{ 'kt-sub': row.sub, 'kt-total': row.total }">
        <nldd-text-cell size="sm" :text="row.sub ? `— ${row.label}` : row.label" :supporting-text="row.hint" :color="row.sub ? 'secondary' : 'content'"></nldd-text-cell>
        <nldd-text-cell
          v-for="col in columns"
          :key="col.key"
          size="sm"
          horizontal-alignment="right"
          :color="cellColor(row, col)"
        >
          <span class="kt-value" :class="{ 'kt-bold': row.total }">{{ cellValue(row, col) }}</span>
          <span slot="supporting-text" v-if="cellDelta(row, col)" class="kt-delta">{{ cellDelta(row, col) }}</span>
        </nldd-text-cell>
      </nldd-table-row>
    </nldd-table>
  </div>
</template>

<script setup>
import { ref, computed } from 'vue';
import ColumnMenu from '@regelrecht/frontend-shared/components/ColumnMenu.vue';
import { euroCompact, euroDelta, number, numberDelta, years, decimal, jaNee } from '../../lib/format.js';
import { useLawStore } from '../../engine/lawStore.js';
import { useSimulation, shortTitle, MAX_VARIANTEN } from '../../composables/useSimulation.js';

const { werkversie, changeCount, variants } = useLawStore();
const { selectedVariants } = useSimulation();

const props = defineProps({
  columns: { type: Array, required: true }, // [{key, short, variantId}]
  metricsByColumn: { type: Object, required: true },
  jaren: { type: Array, required: true },
  running: { type: Object, default: () => ({}) }, // kolomkey -> bool
  progress: { type: Object, default: () => ({}) }, // kolomkey -> 0..1
});

const periode = ref('totaal');
const gridColumns = computed(() => `minmax(220px, 1.4fr) repeat(${props.columns.length}, minmax(140px, 1fr))`);

function slice(m) {
  if (!m) return null;
  return periode.value === 'totaal' ? m.totaal : m.perJaar[Number(periode.value)];
}

const ist = computed(() => props.metricsByColumn.ist ?? null);

const rows = computed(() => [
  { key: 'po', label: 'Regeling po', hint: 'eerste opvang, tweede jaar, toeslag', kind: 'euro', get: (s) => s.regeling_po.totaal },
  { key: 'po_a', label: 'Asielzoekers 1e jaar', sub: true, kind: 'euro', get: (s) => s.regeling_po.per_categorie.ASIELZOEKER },
  { key: 'po_v', label: 'Overige vreemdelingen', sub: true, kind: 'euro', get: (s) => s.regeling_po.per_categorie.OVERIGE_VREEMDELING },
  { key: 'po_2', label: 'Tweede jaar asielzoekers', sub: true, kind: 'euro', get: (s) => s.regeling_po.tweede_jaar },
  { key: 'po_t', label: 'Toeslag eerste keer', sub: true, kind: 'euro', get: (s) => s.regeling_po.toeslag },
  { key: 'vo', label: 'Regeling vo', hint: 'eerste opvang nieuwkomers vo', kind: 'euro', get: (s) => s.regeling_vo.totaal },
  { key: 'vo_1', label: 'Eerste categorie', sub: true, kind: 'euro', get: (s) => s.regeling_vo.per_categorie.EERSTE },
  { key: 'vo_2', label: 'Tweede categorie', sub: true, kind: 'euro', get: (s) => s.regeling_vo.per_categorie.TWEEDE },
  { key: 'vo_v', label: 'Voorbereidingskosten', sub: true, kind: 'euro', get: (s) => s.regeling_vo.voorbereidingskosten },
  { key: 'reg', label: 'Regeling totaal', kind: 'euro', total: true, get: (s) => s.regeling_totaal },
  { key: 'gr', label: 'Uitbetaald op eigen opgave', hint: 'zonder verifieerbare grondslag; de ADR merkt dit als onzeker aan', kind: 'euro', get: (s) => s.grondslag?.zonder_verifieerbare_grondslag ?? 0 },
  { key: 'gr_o', label: 'waarvan op bestuursoordeel', sub: true, kind: 'euro', get: (s) => s.grondslag?.op_oordeel ?? 0 },
  { key: 'tl', label: 'Gemist door te late aanvraag', hint: 'aandeel te laat × bedrag op aanvraag; niet uitbetaald aan scholen', kind: 'euro', get: (s) => s.grondslag?.gemist_te_laat ?? 0 },
  { key: 'ul_s', label: 'Uitvoeringslast school', hint: 'handelingen × minuten × tarief', kind: 'euro', get: (s) => s.uitvoeringslast.school },
  { key: 'ul_d', label: 'Uitvoeringslast DUO', kind: 'euro', get: (s) => s.uitvoeringslast.duo },
  { key: 'ul_o', label: 'Uitvoeringslast overig', hint: 'accountant, OCW', kind: 'euro', get: (s) => s.uitvoeringslast.overig, hideIfZero: true },
  { key: 'inv', label: 'Investering', hint: 'afschrijving in de periode', kind: 'euro', get: (s) => s.investering },
  { key: 'uitv', label: 'Uitvoering totaal', kind: 'euro', total: true, get: (s) => s.uitvoering_totaal },
  { key: 'alles', label: 'Regeling + uitvoering', kind: 'euro', total: true, get: (s) => s.regeling_totaal + s.uitvoering_totaal },
  { key: 'b_all', label: 'Binnen budget', hint: 'regeling po · regeling vo · uitvoering', kind: 'bool3', kern: true, get: (s) => [s.binnen_budget.regeling_po, s.binnen_budget.regeling_vo, s.binnen_budget.uitvoering] },
  { key: 'b_po', label: 'Binnen budget regeling po', kind: 'bool', get: (s) => s.binnen_budget.regeling_po },
  { key: 'b_vo', label: 'Binnen budget regeling vo', kind: 'bool', get: (s) => s.binnen_budget.regeling_vo },
  { key: 'b_u', label: 'Binnen budget uitvoering', kind: 'bool', get: (s) => s.binnen_budget.uitvoering },
  { key: 'tvt', label: 'Terugverdientijd', hint: 'investering ÷ jaarlijkse besparing uitvoeringslast', kind: 'years', get: (s, m) => m.terugverdientijd_jaren },
  { key: 'll', label: 'Leerlingen bekostigd', hint: 'gemiddeld per jaar, po + vo', kind: 'count', get: (s, m) => perJaarGemiddeld(m, (x) => x.leerlingen_bekostigd.totaal) },
  { key: 'll_po', label: 'waarvan po', sub: true, kind: 'count', get: (s, m) => perJaarGemiddeld(m, (x) => x.leerlingen_bekostigd.po) },
  { key: 'll_vo', label: 'waarvan vo', sub: true, kind: 'count', get: (s, m) => perJaarGemiddeld(m, (x) => x.leerlingen_bekostigd.vo) },
  { key: 'll_d', label: 'Leerlingen onder de drempel', hint: 'tellen wel, school krijgt niets', kind: 'count', get: (s, m) => perJaarGemiddeld(m, (x) => x.leerlingen_onder_drempel) },
  { key: 'sch_d', label: 'Scholen onder de drempel', kind: 'count', get: (s, m) => perJaarGemiddeld(m, (x) => x.scholen_onder_drempel) },
  { key: 'stab', label: 'Stabiliteit per school', hint: 'variatiecoëfficiënt jaar op jaar; lager is stabieler', kind: 'cv', get: (s, m) => m.stabiliteit?.totaal },
].filter((r) => !r.hideIfZero || props.columns.some((c) => (slice(props.metricsByColumn[c.key])?.uitvoeringslast.overig ?? 0) > 0)));

// Kernrijen: de twee rekeningen, het rechtmatigheidspunt, budget, leerlingen.
// De rest (uitsplitsingen, stabiliteit, drempel per school) staat onder "Alle posten".
const KERN = new Set(['po', 'vo', 'reg', 'gr', 'ul_s', 'ul_d', 'inv', 'uitv', 'alles', 'b_all', 'tvt', 'll', 'll_d']);
const detail = ref('kern');
const zichtbareRijen = computed(() => rows.value.filter((r) => (detail.value === 'alles' ? r.key !== 'b_all' : KERN.has(r.key))));

function perJaarGemiddeld(m, pick) {
  if (!m) return null;
  if (periode.value !== 'totaal') return pick(m.perJaar[Number(periode.value)]);
  const vals = m.jaren.map((j) => pick(m.perJaar[j]));
  return vals.reduce((a, b) => a + b, 0) / (vals.length || 1);
}

function valueOf(row, col) {
  const m = props.metricsByColumn[col.key];
  if (!m) return null;
  const s = slice(m);
  if (!s) return null;
  return row.get(s, m);
}

function cellValue(row, col) {
  const v = valueOf(row, col);
  if (v === null || v === undefined) return props.running?.[col.key] ? '…' : '—';
  if (row.kind === 'euro') return euroCompact(v);
  if (row.kind === 'bool') return jaNee(v);
  if (row.kind === 'bool3') return ['po', 'vo', 'uitv.'].map((naam, i) => `${v[i] === true ? '✓' : v[i] === false ? '✗' : '–'} ${naam}`).join('  ');
  if (row.kind === 'years') return col.key === 'ist' ? '—' : years(v);
  if (row.kind === 'cv') return decimal(v, 2);
  return number(v);
}

function cellDelta(row, col) {
  if (col.key === 'ist' || !ist.value) return '';
  if (row.kind === 'bool' || row.kind === 'bool3' || row.kind === 'years') return '';
  const v = valueOf(row, col);
  const b = valueOf(row, { key: 'ist' });
  if (v === null || v === undefined || b === null || b === undefined) return '';
  const d = v - b;
  if (row.kind === 'euro') return euroDelta(d);
  if (row.kind === 'cv') return Math.abs(d) < 0.005 ? '± 0' : `${d > 0 ? '+' : '−'} ${decimal(Math.abs(d), 2)}`;
  return numberDelta(d);
}

function cellColor(row, col) {
  const v = valueOf(row, col);
  if (row.kind === 'bool') return v === true ? 'success' : v === false ? 'critical' : 'secondary';
  // Een bool3-cel toont drie oordelen naast elkaar (po, vo, uitvoering). Eén
  // `false` kleurde de hele cel rood, inclusief de twee vinkjes ernaast: dat
  // leest als "hier is iets mis" terwijl er staat dat twee van de drie wel
  // binnen budget blijven. Alles goed is `success`, alles mis is `critical`,
  // en daartussen `warning` — de cel is dan gemengd, en dat is wat de kleur
  // moet zeggen.
  if (row.kind === 'bool3') {
    if (!Array.isArray(v)) return 'secondary';
    const oordelen = v.filter((x) => typeof x === 'boolean');
    if (!oordelen.length) return 'secondary';
    if (oordelen.every((x) => x === true)) return 'success';
    if (oordelen.every((x) => x === false)) return 'critical';
    return 'warning';
  }
  return 'content';
}
</script>

<style scoped>
.kolom-tabel { display: flex; flex-direction: column; gap: var(--primitives-space-12); }
/* De kolomkeuze links, de periode- en detailkeuze rechts: wat er in de tabel
   staat tegenover hoe je ernaar kijkt. `auto` op de eerste marge houdt die
   tweedeling ook heel als de rij afbreekt op een smal scherm. */
.kt-head { display: flex; gap: var(--primitives-space-12); flex-wrap: wrap; align-items: center; }
.kt-head > nldd-segmented-control:first-of-type { margin-left: auto; }
.kt-value { font-variant-numeric: tabular-nums; }
.kt-bold { font-weight: 700; }
.kt-delta { font-variant-numeric: tabular-nums; font-size: 0.85em; }
</style>
