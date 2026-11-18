/**
 * useSimulation - de populatiesimulatie in kolommen (huidig recht + gekozen
 * varianten), als module-singleton zodat /beleid en /school dezelfde
 * uitkomsten delen.
 *
 * Per kolom draait een eigen web worker met een eigen WasmEngine en de
 * YAML's van die kolom (lawStore.lawYamlsFor). Alle kolommen krijgen
 * dezelfde populatie (zelfde seed), zodat verschillen alleen uit de
 * regelgeving komen. De ruwe simulatie (simulate.js) wordt bewaard; de
 * metrics (metrics.js) zijn een computed over de simulatie én het
 * handelingenmodel, dus minuten/tarieven/budget bijstellen is gratis.
 */
import { ref, shallowRef, computed, watch } from 'vue';
import * as yaml from 'js-yaml';
import { b } from '../basePad.js';
import { generatePopulation } from '../sim/population.js';
import { aggregate, terugverdientijd, verschil } from '../sim/metrics.js';
import { useLawStore } from '../engine/lawStore.js';
import { useHandelingen } from './useHandelingen.js';
import { usePopulatie } from './usePopulatie.js';
import { persistedRef } from './useBewaardeStand.js';
import { patchDefinitionValue, readDefinitionValue } from '../lib/yamlPatch.js';
import { DEFAULT_JAREN } from '../lib/nieuwkomerFacts.js';

const { lawYamlsFor, lawDocsFor, version, variants, docPaths } = useLawStore();
const { handelingenFor, ensureVariantDoc, fetchHandelingen, overrides } = useHandelingen();
const { basis: popBasis, effectief: popEffectief, versie: popVersie } = usePopulatie();

export const IST_KEY = 'ist';
export const MAX_VARIANTEN = 3;

// ---- Instellingen ----------------------------------------------------------
// Bewaard, zodat een ververs je keuzes niet wegvaagt. De uitkomsten hieronder
// niet: die zijn afgeleid en worden opnieuw berekend.
const n = persistedRef('populatie.n', 2000); // acht instroomjaren; onder 2000 records wordt de stand per jaar te grillig
const seed = persistedRef('populatie.seed', 42);
const jaren = ref([...DEFAULT_JAREN]);
const index = ref({}); // jaar -> indexfactor op de bedragen (1.02 = +2%)
const autoRecompute = persistedRef('populatie.auto', true);
const selectedVariants = persistedRef('kolommen', []); // variant-id's, max MAX_VARIANTEN

// ---- Toestand --------------------------------------------------------------
const records = shallowRef([]);
const sims = shallowRef({}); // kolomkey -> sim (simulate.js-resultaat)
const running = ref({}); // kolomkey -> boolean
const progress = ref({}); // kolomkey -> 0..1
const errors = ref({}); // kolomkey -> string|null
const simVersion = ref(-1); // lawStore-versie waarmee de sims zijn gedraaid
const simPopulatie = ref(null); // { n, seed, aannames } van de populatie waarmee de sims zijn gedraaid
const populatieGewijzigd = computed(
  () => simPopulatie.value !== null
    && (simPopulatie.value.n !== n.value
      || simPopulatie.value.seed !== seed.value
      || simPopulatie.value.aannames !== popVersie.value),
);
const stale = computed(() => simVersion.value !== -1 && (simVersion.value !== version.value || populatieGewijzigd.value));
const anyRunning = computed(() => Object.values(running.value).some(Boolean));

let distributions = null;
const distributionsRef = shallowRef(null); // voor de plausibiliteitstoets in de UI
const workers = new Map(); // kolomkey -> { worker, abort }
let recomputeSeq = 0; // volgnummer van de laatste recompute; oudere runs schrijven niets meer terug

async function loadDistributions() {
  if (distributions) return distributions;
  const res = await fetch(b('/data/distributions.yaml'));
  if (!res.ok) throw new Error(`Verdelingen ophalen mislukt (${res.status})`);
  distributions = yaml.load(await res.text());
  distributionsRef.value = distributions;
  popBasis.value = distributions; // usePopulatie legt de sessie-aannames hierover heen
  // Indexatie van de bedragen per jaar (2027-2028 rekenen met de 2026-bedragen).
  if (!Object.keys(index.value).length && distributions?.bedragen_index_per_jaar) {
    index.value = Object.fromEntries(
      Object.entries(distributions.bedragen_index_per_jaar).map(([j, f]) => [Number(j), Number(f)]),
    );
  }
  return distributions;
}

/** Kolommen: huidig recht + de gekozen varianten (in keuzevolgorde). */
const columns = computed(() => [
  { key: IST_KEY, variantId: null, title: 'Huidig recht', short: 'Ist' },
  ...selectedVariants.value
    .map((id) => variants.value.find((v) => v.id === id))
    .filter(Boolean)
    .map((v) => ({ key: v.id, variantId: v.id, title: v.title, short: shortTitle(v) })),
]);

export function shortTitle(v) {
  // "Variant nk-1: ambtshalve po (…)" → "nk-1: ambtshalve po"
  const m = v.title.match(/^variant\s+([\w-]+)\s*:\s*(.*)$/i);
  if (!m) return v.title.split('(')[0].trim();
  return `${m[1]}: ${m[2].split('(')[0].trim()}`;
}

function toggleVariant(id) {
  const list = selectedVariants.value;
  if (list.includes(id)) {
    selectedVariants.value = list.filter((x) => x !== id);
  } else if (list.length < MAX_VARIANTEN) {
    selectedVariants.value = [...list, id];
  }
}

// ---- Workers ----------------------------------------------------------------

function terminate(key) {
  const entry = workers.get(key);
  if (entry) {
    entry.worker.terminate();
    workers.delete(key);
    running.value = { ...running.value, [key]: false };
    entry.abort(); // de wachtende aanroeper krijgt een afbreking, geen eeuwig hangende promise
  }
}

/**
 * Draai één kolom in een verse worker (een lopende run voor dezelfde kolom
 * wordt afgebroken). Resolvet met het sim-resultaat.
 */
function runInWorker(key, laws, recs, options) {
  terminate(key);
  const w = new Worker(new URL('../sim/simWorker.js', import.meta.url), { type: 'module' });
  running.value = { ...running.value, [key]: true };
  progress.value = { ...progress.value, [key]: 0 };
  errors.value = { ...errors.value, [key]: null };
  return new Promise((resolve, reject) => {
    const abort = () => reject(Object.assign(new Error('afgebroken door een nieuwere run'), { afgebroken: true }));
    workers.set(key, { worker: w, abort });
    w.onmessage = (event) => {
      const msg = event.data;
      if (msg.type === 'progress') {
        progress.value = { ...progress.value, [key]: msg.done / msg.total };
      } else if (msg.type === 'result') {
        if (workers.get(key)?.worker === w) workers.delete(key);
        running.value = { ...running.value, [key]: false };
        progress.value = { ...progress.value, [key]: 1 };
        w.terminate();
        resolve(msg.sim);
      } else if (msg.type === 'error') {
        if (workers.get(key)?.worker === w) workers.delete(key);
        running.value = { ...running.value, [key]: false };
        errors.value = { ...errors.value, [key]: msg.message };
        w.terminate();
        reject(new Error(msg.message));
      }
    };
    w.onerror = (e) => {
      running.value = { ...running.value, [key]: false };
      errors.value = { ...errors.value, [key]: String(e?.message ?? e) };
      reject(e);
    };
    // Alleen kloonbare data over de draad: Vue-proxies (bijv. jaren.value) en
    // extra array-properties geven anders een DataCloneError.
    const plain = JSON.parse(JSON.stringify({ laws, records: recs, options }));
    try {
      w.postMessage({ type: 'simulate', ...plain });
    } catch (e) {
      running.value = { ...running.value, [key]: false };
      errors.value = { ...errors.value, [key]: String(e?.message ?? e) };
      w.terminate();
      reject(e);
    }
  });
}

/** Genereer de populatie (deterministisch) als n/seed veranderd zijn. */
async function ensureRecords() {
  await loadDistributions();
  const dist = popEffectief.value ?? distributions;
  if (
    records.value.length !== n.value
    || records.value._seed !== seed.value
    || records.value._aannames !== popVersie.value
  ) {
    const recs = generatePopulation(dist, n.value, seed.value);
    recs._seed = seed.value;
    recs._aannames = popVersie.value;
    records.value = recs;
  }
  return records.value;
}

/** Draai alle kolommen (parallel). */
// Invoer van de laatste simulatie per kolom: een bewerking in de werkversie
// bumpt de store-versie, maar de andere kolommen zijn vast; die hoeven dan
// niet opnieuw te draaien.
const simInputs = new Map(); // key -> { recs, lawsText, jarenText }

async function recompute() {
  const mijn = ++recomputeSeq;
  await fetchHandelingen();
  const recs = await ensureRecords();
  const ver = version.value;
  const cols = columns.value;
  await Promise.all(cols.map((c) => ensureVariantDoc(c.variantId)));
  const results = await Promise.allSettled(
    cols.map(async (col) => {
      const laws = await lawYamlsFor(col.variantId);
      const lawsText = laws.join('\n---\n');
      const jarenText = JSON.stringify(jaren.value);
      const vorige = simInputs.get(col.key);
      if (vorige && sims.value[col.key] && vorige.recs === recs && vorige.lawsText === lawsText && vorige.jarenText === jarenText) {
        return [col.key, sims.value[col.key]];
      }
      const sim = await runInWorker(col.key, laws, recs, { jaren: jaren.value });
      simInputs.set(col.key, { recs, lawsText, jarenText });
      return [col.key, sim];
    }),
  );
  // Een nieuwere recompute is inmiddels gestart (snelle opeenvolgende
  // wijzigingen): die bepaalt de stand, deze uitkomsten zijn achterhaald.
  if (mijn !== recomputeSeq) return;
  const next = { ...sims.value };
  for (const r of results) {
    if (r.status === 'fulfilled') next[r.value[0]] = r.value[1];
  }
  // Kolommen die niet meer gekozen zijn opruimen.
  for (const key of Object.keys(next)) {
    if (!cols.some((c) => c.key === key)) delete next[key];
  }
  sims.value = next;
  simVersion.value = ver;
  simPopulatie.value = { n: recs.length, seed: recs._seed, aannames: recs._aannames };
}

/** Draai alleen de kolommen die nog geen sim hebben (bij het bijkiezen van een variant). */
async function recomputeMissing() {
  await fetchHandelingen();
  // Een nieuwe populatie moet in álle kolommen staan, anders vergelijk je verschillende leerlingen.
  if (populatieGewijzigd.value) return recompute();
  const recs = await ensureRecords();
  const missing = columns.value.filter((c) => !sims.value[c.key] && !running.value[c.key]);
  if (!missing.length) return;
  await Promise.all(missing.map((c) => ensureVariantDoc(c.variantId)));
  const results = await Promise.allSettled(
    missing.map(async (col) => {
      const laws = await lawYamlsFor(col.variantId);
      const sim = await runInWorker(col.key, laws, recs, { jaren: jaren.value });
      simInputs.set(col.key, { recs, lawsText: laws.join('\n---\n'), jarenText: JSON.stringify(jaren.value) });
      return [col.key, sim];
    }),
  );
  const next = { ...sims.value };
  for (const r of results) {
    if (r.status === 'fulfilled') next[r.value[0]] = r.value[1];
  }
  sims.value = next;
}

// ---- Metrics (reactief op sims én handelingen-overrides) --------------------

const metricsByColumn = computed(() => {
  // afhankelijkheden: sims, overrides, index
  overrides.minuten; overrides.tarieven; overrides.fracties; overrides.investeringen; overrides.budget;
  const out = {};
  for (const col of columns.value) {
    const sim = sims.value[col.key];
    if (!sim) continue;
    const handelingen = handelingenFor(col.variantId);
    out[col.key] = aggregate(sim, handelingen ?? {}, { index: index.value });
  }
  const ist = out[IST_KEY] ?? null;
  for (const col of columns.value) {
    const m = out[col.key];
    if (!m) continue;
    m.terugverdientijd_jaren = col.key === IST_KEY ? null : terugverdientijd(m, ist);
    m.verschil = col.key === IST_KEY ? null : verschil(m, ist);
  }
  return out;
});

const istMetrics = computed(() => metricsByColumn.value[IST_KEY] ?? null);

// ---- Budgetneutraal-oplosser -------------------------------------------------

const solving = ref(false);
const solveLog = ref([]);

/**
 * Zoek de waarde van één definitie in een variant-document waarbij de
 * regeling-uitgave (doelpost: 'regeling_po' | 'regeling_vo' | 'regeling_totaal',
 * totaal over de horizon) gelijk is aan die onder huidig recht. Secant-
 * iteratie (uitgaven zijn vrijwel lineair in een bedrag), met bisectie als
 * vangnet. Elke iteratie is een volledige simulatie van de variantkolom.
 *
 * @returns {{ value, spend, target, iterations }} of gooit bij geen convergentie
 */
async function solveBudgetneutraal({ variantId, lawPath, article, name, doelpost = 'regeling_totaal', tolerance = 0.005, maxIter = 8 }) {
  const ist = istMetrics.value;
  if (!ist) throw new Error('Eerst huidig recht doorrekenen');
  const target = doelpost === 'regeling_totaal' ? ist.totaal.regeling_totaal : ist.totaal[doelpost].totaal;
  const recs = await ensureRecords();
  const docsKolom = await lawDocsFor(variantId);
  const laws = docsKolom.map((d) => d.yaml);
  const docIndex = docsKolom.findIndex((d) => d.path === lawPath);
  if (docIndex < 0) throw new Error(`Document ${lawPath} niet gevonden in deze kolom`);
  const key = `${variantId}::solve`;
  const handelingen = handelingenFor(variantId) ?? {};
  solving.value = true;
  solveLog.value = [];

  const spendAt = async (value) => {
    const patched = [...laws];
    patched[docIndex] = patchDefinitionValue(laws[docIndex], article, name, value);
    const sim = await runInWorker(key, patched, recs, { jaren: jaren.value });
    const m = aggregate(sim, handelingen, { index: index.value });
    const spend = doelpost === 'regeling_totaal' ? m.totaal.regeling_totaal : m.totaal[doelpost].totaal;
    solveLog.value = [...solveLog.value, { value, spend }];
    return spend;
  };

  try {
    let x0 = Number(readDefinitionValue(laws[docIndex], article, name));
    if (!Number.isFinite(x0)) throw new Error('Definitie heeft geen numerieke waarde');
    let s0 = await spendAt(x0);
    if (Math.abs(s0 - target) <= tolerance * target) {
      return { value: x0, spend: s0, target, iterations: 1 };
    }
    let x1 = x0 === 0 ? 100000 : x0 * (s0 > target ? 0.8 : 1.2);
    let s1 = await spendAt(x1);
    let lo = null;
    let hi = null;
    for (let i = 2; i < maxIter; i++) {
      if (Math.abs(s1 - target) <= tolerance * target) {
        return { value: Math.round(x1), spend: s1, target, iterations: i };
      }
      // Bracket bijhouden voor de bisectie-terugval.
      if ((s0 - target) * (s1 - target) < 0) {
        lo = s0 < target ? [x0, s0] : [x1, s1];
        hi = s0 < target ? [x1, s1] : [x0, s0];
      }
      let x2;
      if (s1 !== s0) x2 = x1 - ((s1 - target) * (x1 - x0)) / (s1 - s0);
      if (!Number.isFinite(x2) || x2 < 0 || (lo && hi && (x2 < Math.min(lo[0], hi[0]) || x2 > Math.max(lo[0], hi[0])))) {
        x2 = lo && hi ? (lo[0] + hi[0]) / 2 : Math.max(0, x1 * (s1 > target ? 0.7 : 1.4));
      }
      x0 = x1; s0 = s1;
      x1 = Math.round(x2);
      s1 = await spendAt(x1);
    }
    return { value: Math.round(x1), spend: s1, target, iterations: maxIter, converged: Math.abs(s1 - target) <= tolerance * target };
  } finally {
    solving.value = false;
    terminate(key);
  }
}

// ---- Automatisch herberekenen bij wetswijzigingen -----------------------------

let autoTimer = null;
// Zolang de view nog nooit heeft doorgerekend, start hij zelf; daarna volgt
// elke wijziging, ook als de eerste run nog loopt (die rekent dan met
// achterhaalde invoer en wordt afgebroken).
watch(version, () => {
  if (!autoRecompute.value || recomputeSeq === 0) return;
  clearTimeout(autoTimer);
  autoTimer = setTimeout(() => recompute().catch(() => {}), 600);
});

// Andere N, seed of aanname over de populatie: nieuwe steekproef, dus alle kolommen opnieuw.
watch([n, seed, popVersie], () => {
  if (!autoRecompute.value || recomputeSeq === 0) return;
  clearTimeout(autoTimer);
  autoTimer = setTimeout(() => recompute().catch(() => {}), 600);
});

watch(selectedVariants, () => {
  if (recomputeSeq === 0) return; // nog nooit gedraaid: de view start zelf
  recomputeMissing().catch(() => {});
});

function dispose() {
  for (const key of [...workers.keys()]) terminate(key);
}

export function useSimulation() {
  return {
    n,
    seed,
    jaren,
    index,
    autoRecompute,
    selectedVariants,
    columns,
    records,
    distributions: distributionsRef,
    sims,
    running,
    anyRunning,
    progress,
    errors,
    stale,
    simVersion,
    simPopulatie,
    populatieGewijzigd,
    metricsByColumn,
    istMetrics,
    solving,
    solveLog,
    toggleVariant,
    ensureRecords,
    recompute,
    recomputeMissing,
    solveBudgetneutraal,
    dispose,
  };
}
