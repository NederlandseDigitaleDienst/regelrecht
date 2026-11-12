<template>
  <div class="bn">
    <p class="bn-hint">
      De vraag hier is: <em>wat kan ik veranderen zonder dat het meer geld kost?</em> Je kiest één bedrag uit de
      regeling en één post die gelijk moet blijven. De oplosser probeert een waarde, rekent de hele kolom door,
      kijkt hoe ver de uitgave van huidig recht af zit, en probeert opnieuw tot hij er binnen een half procent bij zit.
      Je krijgt een bedrag terug; toepassen doe je zelf.
    </p>

    <nldd-banner v-if="!werkversie" variant="accent">
      Hiervoor is een variant nodig om aan te draaien. Huidig recht is het ijkpunt waar de uitgave gelijk aan moet
      blijven, dus daarin een budgetneutrale waarde zoeken zou betekenen dat je hem met zichzelf vergelijkt.
      Kies een variant als werkversie, of bewaar je eigen bewerkingen eerst als variant.
      <span v-if="changeCount === 0 && variantColumns.length" class="bn-row">
        <nldd-button v-for="c in variantColumns" :key="c.key" size="sm" appearance="secondary" :text="`Werk in ${c.short}`" @click="setWerkversie(c.variantId)"></nldd-button>
      </span>
    </nldd-banner>

    <template v-else>
      <nldd-form-field label="Waaraan draaien" :supporting-label="`een bedrag uit ${werkversieLabel}; dit is wat de oplosser verandert`">
        <nldd-dropdown :key="`${werkversie}:${definities.length}:${defKey}`" size="sm" @change="defKey = $event.target.value ?? defKey">
          <select :value="defKey">
            <option v-for="d in definities" :key="d.key" :value="d.key">{{ d.label }}</option>
          </select>
        </nldd-dropdown>
      </nldd-form-field>

      <nldd-form-field label="Wat gelijk moet blijven" :supporting-label="doelpostUitleg">
        <nldd-segmented-control size="sm" :value="doelpost" @change="doelpost = $event.detail?.value ?? doelpost">
          <nldd-segmented-control-item value="regeling_po" text="Regeling po"></nldd-segmented-control-item>
          <nldd-segmented-control-item value="regeling_vo" text="Regeling vo"></nldd-segmented-control-item>
          <nldd-segmented-control-item value="regeling_totaal" text="Totaal"></nldd-segmented-control-item>
        </nldd-segmented-control>
      </nldd-form-field>

      <div class="bn-row">
        <nldd-button
          :text="solving ? 'Zoeken…' : 'Zoek budgetneutrale waarde'"
          start-icon="graph"
          appearance="secondary"
          :disabled="solving || !defKey || !istMetrics ? true : undefined"
          @click="solve"
        ></nldd-button>
        <nldd-activity-indicator v-if="solving" size="20" timing="instant"></nldd-activity-indicator>
      </div>

      <nldd-banner v-if="error" variant="critical">{{ error }}</nldd-banner>

      <!-- Met het verschil per poging erbij is te zien dat de oplosser
           toeloopt; alleen bedrag en uitgave lieten dat niet zien, en dan
           oogt een reeks getallen als willekeur. -->
      <div v-if="solveLog.length" class="bn-log">
        <div class="bn-kop">
          <span class="bn-i"></span>
          <span>probeersel</span>
          <span></span>
          <span>uitgave</span>
          <span class="bn-af">verschil met huidig recht</span>
        </div>
        <div v-for="(step, i) in solveLog" :key="i" class="bn-step">
          <span class="bn-i">{{ i + 1 }}</span>
          <span>{{ euro(step.value) }}</span>
          <nldd-icon icon="arrow-right" size="16"></nldd-icon>
          <span>{{ euroCompact(step.spend) }}</span>
          <span class="bn-af" :class="{ 'bn-raak': raak(step) }">{{ afwijking(step) }}</span>
        </div>
        <p v-if="doelUitgave" class="bn-hint">
          Huidig recht geeft {{ euroCompact(doelUitgave) }} uit aan deze post; daar moet de werkversie op uitkomen.
        </p>
      </div>

      <nldd-inline-dialog v-if="result" :variant="result.converged === false ? 'alert' : 'success'" :text="resultTekst">
        <nldd-button
          slot="actions"
          :text="`Toepassen in ${werkversieLabel}`"
          size="sm"
          appearance="primary"
          @click="apply"
        ></nldd-button>
      <p v-if="applied" class="bn-hint">{{ applied }}</p>
      </nldd-inline-dialog>
    </template>
  </div>
</template>

<script setup>
import { ref, computed, watch } from 'vue';
import { useSimulation } from '../../composables/useSimulation.js';
import { useLawStore } from '../../engine/lawStore.js';
import { listDefinitions } from '../../lib/yamlPatch.js';
import { euro, euroCompact } from '../../lib/format.js';
import { LAW_PO } from '../../lib/nieuwkomerFacts.js';

const { columns, istMetrics, solving, solveLog, solveBudgetneutraal } = useSimulation();
const { lawDocsFor, docPaths, editableDocs, applyDefinitionChange, werkversie, werkversieLabel, changeCount, setWerkversie, version } = useLawStore();

const defKey = ref(null);
const doelpost = ref('regeling_po');
const definities = ref([]);
const result = ref(null);
const error = ref(null);
const applied = ref(null);

const variantColumns = computed(() => columns.value.filter((c) => c.variantId));

// De definities van de werkversie; opnieuw als de werkversie wisselt of als
// een document verandert (bijvoorbeeld na Toepassen: dan klopt het bedrag in
// het label weer).
watch([werkversie, version], async ([id]) => {
  const vorige = defKey.value;
  definities.value = [];
  defKey.value = null;
  if (!id) { result.value = null; applied.value = null; return; }
  // De documenten van de werkversie (inclusief het nieuwe versiebestand van de
  // variant), alleen bewerkbare wetten en per wet alleen de nieuwste versie:
  // aan een oudere versie draaien verandert hooguit één jaar.
  const kolomDocs = await lawDocsFor(id);
  const editableIds = new Set(editableDocs.value.map((d) => d.entry.id));
  const nieuwste = new Map();
  for (const d of kolomDocs) {
    if (!editableIds.has(d.entry.id)) continue;
    const huidige = nieuwste.get(d.entry.id);
    if (!huidige || String(d.entry.valid_from) > String(huidige.entry.valid_from)) nieuwste.set(d.entry.id, d);
  }
  const volgorde = [...nieuwste.values()].sort((a, b) => (a.entry.id === LAW_PO ? -1 : b.entry.id === LAW_PO ? 1 : 0));
  const list = [];
  for (const d of volgorde) {
    const versie = String(d.entry.valid_from ?? '').slice(0, 4);
    for (const def of listDefinitions(d.yaml)) {
      if (def.unit !== 'eurocent') continue;
      // Nulconstanten (geen_bedrag, geen_toeslag) zijn geen knoppen.
      if (!(Number(def.value) > 0)) continue;
      list.push({
        key: `${d.path}::${def.article}::${def.name}`,
        path: d.path,
        article: def.article,
        name: def.name,
        label: `art. ${def.article} · ${def.name} (${euro(def.value)}) · ${(d.entry.name ?? d.path).slice(0, 36)} ${versie}`,
      });
    }
  }
  definities.value = list;
  // Zelfde keuze houden als die nog bestaat; anders standaard een tarief per
  // leerling, niet een eenmalige toeslag.
  defKey.value = (list.find((d) => d.key === vorige) ?? list.find((d) => d.name.startsWith('bedrag_per_')) ?? list[0])?.key ?? null;
}, { immediate: true });

const gekozen = computed(() => definities.value.find((d) => d.key === defKey.value) ?? null);

const resultTekst = computed(() => {
  const r = result.value;
  if (!r) return '';
  const kern = `${gekozen.value?.name} = ${euro(r.value)} geeft ${euroCompact(r.spend)}, tegenover ${euroCompact(r.target)} onder huidig recht`;
  if (r.converged === false) {
    // "Niet geconvergeerd na 8 iteraties" zegt niets tegen wie de oplosser
    // niet kent. Wat het betekent en wat je eraan doet, hoort erbij.
    return `${kern}. De oplosser kwam er in ${r.iterations} pogingen niet dichtbij genoeg: dit bedrag komt het `
      + 'meest in de buurt. Meestal betekent dat deze knop de post niet genoeg beweegt. Probeer een ander bedrag, '
      + 'of een andere post om gelijk te houden.';
  }
  return `${kern}. Gevonden in ${r.iterations} ${r.iterations === 1 ? 'poging' : 'pogingen'}.`;
});

const doelpostUitleg = computed(() => ({
  regeling_po: 'de uitgave aan de po-regeling blijft gelijk aan huidig recht',
  regeling_vo: 'de uitgave aan de vo-regeling blijft gelijk aan huidig recht',
  regeling_totaal: 'po en vo samen blijven gelijk aan huidig recht',
}[doelpost.value] ?? ''));

/** Wat huidig recht aan de gekozen post uitgeeft: het ijkpunt van de zoektocht. */
const doelUitgave = computed(() => {
  const ist = istMetrics.value;
  if (!ist) return null;
  return doelpost.value === 'regeling_totaal'
    ? ist.totaal.regeling_totaal
    : ist.totaal[doelpost.value]?.totaal ?? null;
});

/** Hoe ver deze poging van huidig recht af zat, met teken. */
function afwijking(step) {
  const doel = doelUitgave.value;
  if (!doel) return '';
  const verschil = step.spend - doel;
  const pct = (verschil / doel) * 100;
  const teken = verschil > 0 ? '+' : '−';
  return `${teken} ${euroCompact(Math.abs(verschil))} (${teken}${Math.abs(pct).toFixed(1)}%)`;
}

/** Zat deze poging binnen de tolerantie van een half procent? */
function raak(step) {
  const doel = doelUitgave.value;
  return !!doel && Math.abs(step.spend - doel) <= 0.005 * doel;
}

async function solve() {
  const d = gekozen.value;
  if (!d) return;
  error.value = null;
  result.value = null;
  try {
    result.value = await solveBudgetneutraal({
      variantId: werkversie.value,
      lawPath: d.path,
      article: d.article,
      name: d.name,
      doelpost: doelpost.value,
    });
  } catch (e) {
    error.value = String(e?.message ?? e);
  }
}

async function apply() {
  const d = gekozen.value;
  if (!d || !result.value) return;
  error.value = null;
  applied.value = null;
  try {
    // De werkversie staat in de store; de waarde wordt daar een bewerking.
    if (!docPaths.value.includes(d.path)) throw new Error(`Document ${d.path} niet gevonden in de werkversie`);
    applyDefinitionChange(d.path, d.article, d.name, result.value.value);
    applied.value = `${d.name} = ${euro(result.value.value)} gezet als bewerking in ${werkversieLabel.value}; de kolommen rekenen opnieuw. Terugdraaien: Terugzetten; vastleggen: Bewaar als variant.`;
  } catch (e) {
    error.value = String(e?.message ?? e);
  }
}
</script>

<style scoped>
.bn { display: flex; flex-direction: column; gap: var(--primitives-space-12); }
.bn-hint { margin: 0; font-size: 0.85em; color: var(--semantics-content-secondary-color); }
.bn-row { display: flex; align-items: center; gap: var(--primitives-space-8); }
.bn-log {
  display: flex; flex-direction: column; gap: 2px;
  font-size: 0.85em; font-variant-numeric: tabular-nums;
  padding: var(--primitives-space-8) var(--primitives-space-12);
  border-radius: var(--semantics-surfaces-corner-radius);
  background: var(--semantics-surfaces-tinted-background-color);
  border: 1px solid var(--semantics-dividers-color);
}
.bn-step, .bn-kop { display: flex; align-items: center; gap: 6px; }
.bn-kop { font-size: 0.9em; color: var(--semantics-content-secondary-color); padding-bottom: 2px; }
.bn-i { width: 1.5em; color: var(--semantics-content-secondary-color); }
.bn-af { margin-left: auto; color: var(--semantics-content-secondary-color); }
.bn-raak { color: var(--semantics-content-success-color); font-weight: 600; }
.bn-log .bn-hint { margin-top: var(--primitives-space-8); }
</style>
