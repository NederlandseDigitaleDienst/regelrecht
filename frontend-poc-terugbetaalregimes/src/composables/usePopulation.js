/**
 * usePopulation - de populatiesimulatie in kolommen: huidig recht (de
 * onbewerkte basis uit main) plus de beleidsvarianten die je erbij kiest.
 * Module-singleton, zodat de beleidsview, de doorrekenknop, de
 * populatie-instellingen en de personaview dezelfde stand delen.
 *
 * Elke kolom draait in een eigen web worker met een eigen WasmEngine en de
 * YAML's van die kolom. Alle kolommen krijgen dezelfde populatie (één
 * steekproef, dezelfde seed), zodat verschillen alleen uit de regelgeving
 * komen. Een kolom waarvan de invoer niet veranderde wordt niet opnieuw
 * gesimuleerd, en kolommen met identieke wetteksten rekenen één keer.
 *
 * De werkversie is een van de kolommen: zodra je hem bewerkt, wijkt die
 * kolom af van de branch waar hij vandaan komt.
 */
import { ref, shallowRef, reactive, computed, watch } from 'vue';
import * as yaml from 'js-yaml';
import { generatePopulation } from '../sim/population.js';
import { aggregate } from '../sim/metrics.js';
import { useLawStore, kortTitel } from '../engine/lawStore.js';
import { usePopulatieAannames } from './usePopulatieAannames.js';
import { persistedRef } from './useBewaardeStand.js';
import { b } from '../basePad.js';

const { baseLawYamls, lawYamlsFor, version, variants, werkversie, werkversieLabel } = useLawStore();
const { basis: aannameBasis, effectief: aannameEffectief, versie: aannameVersie } = usePopulatieAannames();

const SIM_OPTIONS = { startJaar: 2026 };

export const KOLOM_BASIS = 'basis';
export const MAX_VARIANTEN = 3;

// ---- Instellingen ----------------------------------------------------------
// Bewaard, zodat een ververs je keuzes niet wegvaagt. De uitkomsten hieronder
// niet: die zijn afgeleid en worden opnieuw berekend.
const n = persistedRef('populatie.n', 500); // elke debiteur loopt tot 35 jaar door de wet; 500 records rekenen in een halve minuut
const seed = persistedRef('populatie.seed', 42);
const autoRecompute = persistedRef('populatie.auto', true);
const selectedVariants = persistedRef('kolommen', []); // variant-id's die als kolom naast huidig recht staan

// ---- Toestand --------------------------------------------------------------
const records = shallowRef([]);
const results = shallowRef({}); // kolomkey -> [{ record, totals }]
const running = ref({});
const progress = ref({});
const errors = ref({});
const simVersion = ref(-1); // lawStore-versie waarmee de sims zijn gedraaid
const simPopulatie = ref(null); // { n, seed, aannames } van de populatie waarmee de sims zijn gedraaid
const populatieGewijzigd = computed(
  () => simPopulatie.value !== null
    && (simPopulatie.value.n !== n.value
      || simPopulatie.value.seed !== seed.value
      || simPopulatie.value.aannames !== aannameVersie.value),
);
const stale = computed(() => simVersion.value !== -1 && (simVersion.value !== version.value || populatieGewijzigd.value));
const anyRunning = computed(() => Object.values(running.value).some(Boolean));

let distributions = null;
const distributionsRef = shallowRef(null);
const workers = new Map(); // kolomkey -> { worker, abort }
let recomputeSeq = 0; // volgnummer van de laatste recompute; oudere runs schrijven niets meer terug
const simInputs = new Map(); // kolomkey -> { recs, lawsText }

/**
 * De kolommen: huidig recht staat altijd links, daarna de gekozen varianten
 * in keuzevolgorde. De werkversie krijgt haar bewerkingen mee; de andere
 * kolommen tonen de branch zoals die is.
 */
const columns = computed(() => [
  {
    key: KOLOM_BASIS,
    variantId: null,
    titel: 'Huidig recht',
    kort: 'Huidig recht',
    code: 'nu',
    isWerkversie: werkversie.value === null,
  },
  ...selectedVariants.value
    .map((id) => variants.value.find((v) => v.id === id))
    .filter(Boolean)
    .map((v) => ({
      key: v.id,
      variantId: v.id,
      titel: `variant ${kortTitel(v)}`,
      kort: `variant ${kortTitel(v)}`,
      // Alleen het variantnummer (a1, nk-2). In een tabel waar dezelfde
      // kolommen per maat terugkomen is de volle naam onleesbaar breed; het
      // nummer is bovendien waar mensen in de zaal naar verwijzen.
      code: v.id.split('-')[0],
      isWerkversie: werkversie.value === v.id,
    })),
]);

function toggleVariant(id) {
  const list = selectedVariants.value;
  if (list.includes(id)) selectedVariants.value = list.filter((x) => x !== id);
  else if (list.length < MAX_VARIANTEN) selectedVariants.value = [...list, id];
}

async function loadDistributions() {
  if (distributions) return distributions;
  const res = await fetch(b('/data/distributions.yaml'));
  if (!res.ok) throw new Error(`Verdelingen ophalen mislukt (${res.status})`);
  distributions = yaml.load(await res.text());
  distributionsRef.value = distributions;
  aannameBasis.value = distributions; // de sessie-aannames liggen hierover heen
  return distributions;
}

/**
 * Het uitvoeringslastmodel: handelingen met minuten en tarieven.
 *
 * Blijft null als het bestand er niet is of niet laadt. Dat is met opzet geen
 * lege standaard: een uitvoeringslast van nul euro is een bewering, en
 * "onbekend" is hier de eerlijke uitkomst.
 */
const handelingenBasis = shallowRef(null);
async function loadHandelingen() {
  if (handelingenBasis.value) return handelingenBasis.value;
  try {
    const res = await fetch(b('/data/handelingen.yaml'));
    if (!res.ok) return null;
    handelingenBasis.value = yaml.load(await res.text());
  } catch {
    handelingenBasis.value = null;
  }
  return handelingenBasis.value;
}

/**
 * Bijstellingen op het model, voor deze sessie.
 *
 * Elke minuut in handelingen.yaml is een aanname en staat daar ook zo
 * gemarkeerd; DUO kan ze ijken. Dan moet je er ook aan kunnen draaien zonder
 * het bestand te wijzigen, en zien wat het met de uitkomst doet.
 *
 * Bewust niet bewaard: dit zijn geen keuzes zoals de kolommen of de werkversie,
 * maar een "wat als" binnen één gesprek. Een ververs zet het model terug op
 * wat er in het dossier staat, en dat is het cijfer waar iedereen het over
 * heeft.
 */
const handelingenOverrides = reactive({
  minuten: {}, // handeling-id -> minuten
  tarieven: {}, // tariefnaam -> eurocent per uur
});

/** Het model zoals er nu mee gerekend wordt: het dossier plus de bijstellingen. */
const handelingenModel = computed(() => {
  const doc = handelingenBasis.value;
  if (!doc) return null;
  return {
    ...doc,
    tarieven: { ...(doc.tarieven ?? {}), ...handelingenOverrides.tarieven },
    handelingen: (doc.handelingen ?? []).map((h) => ({
      ...h,
      minuten: handelingenOverrides.minuten[h.id] ?? h.minuten,
    })),
  };
});

const handelingenGewijzigd = computed(
  () => Object.keys(handelingenOverrides.minuten).length > 0
    || Object.keys(handelingenOverrides.tarieven).length > 0,
);

function stelMinutenIn(id, minuten) {
  // An emptied number field reports null; Number(null) would store 0 minutes.
  if (minuten === null || minuten === undefined || minuten === '') return;
  const n = Number(minuten);
  if (!Number.isFinite(n) || n < 0) return;
  handelingenOverrides.minuten[id] = n;
}

function stelTariefIn(naam, eurocentPerUur) {
  const n = Number(eurocentPerUur);
  if (!Number.isFinite(n) || n < 0) return;
  handelingenOverrides.tarieven[naam] = n;
}

/** Alles terug naar het model uit het dossier. */
function herstelHandelingen() {
  for (const groep of Object.values(handelingenOverrides)) {
    for (const sleutel of Object.keys(groep)) delete groep[sleutel];
  }
}

function terminate(key) {
  const entry = workers.get(key);
  if (entry) {
    entry.worker.terminate();
    workers.delete(key);
    running.value = { ...running.value, [key]: false };
    entry.abort();
  }
}

/** Draai één kolom in een verse worker; een lopende run voor dezelfde kolom wordt afgebroken. */
function runInWorker(key, laws, recs) {
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
        resolve(msg.results);
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
    const plain = JSON.parse(JSON.stringify({ laws, records: recs }));
    try {
      w.postMessage({ type: 'simulate', ...plain, choicesOverride: null, options: SIM_OPTIONS });
    } catch (e) {
      running.value = { ...running.value, [key]: false };
      errors.value = { ...errors.value, [key]: String(e?.message ?? e) };
      w.terminate();
      reject(e);
    }
  });
}

/** Genereer de populatie (deterministisch) alleen als N, de seed of een aanname veranderd is. */
async function ensureRecords() {
  await loadDistributions();
  // Het uitvoeringslastmodel hoort bij dezelfde doorrekening; ontbreekt het,
  // dan blijft de uitvoeringslast leeg en rekent de rest gewoon door.
  await loadHandelingen();
  const dist = aannameEffectief.value ?? distributions;
  if (
    records.value.length !== n.value
    || records.value._seed !== seed.value
    || records.value._aannames !== aannameVersie.value
  ) {
    const recs = generatePopulation(dist, n.value, seed.value);
    recs._seed = seed.value;
    recs._aannames = aannameVersie.value;
    records.value = recs;
  }
  return records.value;
}

/** De wetten van een kolom: huidig recht is de onbewerkte basis uit main. */
async function lawsFor(col) {
  if (col.key === KOLOM_BASIS) {
    // De werkversie is huidig recht: dan mét de bewerkingen, anders de basis.
    return col.isWerkversie ? lawYamlsFor(null) : baseLawYamls();
  }
  return lawYamlsFor(col.variantId);
}

/** Draai alle kolommen (parallel); ongewijzigde invoer wordt hergebruikt. */
async function recompute() {
  const mijn = ++recomputeSeq;
  const recs = await ensureRecords();
  const ver = version.value;
  const cols = columns.value;
  const lopend = new Map(); // lawsText -> promise; identieke wetteksten rekenen één keer
  const uitkomsten = await Promise.allSettled(
    cols.map(async (col) => {
      const laws = await lawsFor(col);
      const lawsText = laws.join('\n---\n');
      const vorige = simInputs.get(col.key);
      if (vorige && results.value[col.key] && vorige.recs === recs && vorige.lawsText === lawsText) {
        return [col.key, results.value[col.key]];
      }
      let run = lopend.get(lawsText);
      if (!run) {
        run = runInWorker(col.key, laws, recs);
        lopend.set(lawsText, run);
      }
      const res = await run;
      simInputs.set(col.key, { recs, lawsText });
      return [col.key, res];
    }),
  );
  // Een nieuwere recompute is inmiddels gestart: die bepaalt de stand.
  if (mijn !== recomputeSeq) return;
  const next = { ...results.value };
  for (const r of uitkomsten) if (r.status === 'fulfilled') next[r.value[0]] = r.value[1];
  // Kolommen die niet meer gekozen zijn opruimen.
  for (const key of Object.keys(next)) {
    if (!cols.some((c) => c.key === key)) delete next[key];
  }
  results.value = next;
  simVersion.value = ver;
  simPopulatie.value = { n: recs.length, seed: recs._seed, aannames: recs._aannames };
}

/** Draai alleen de kolommen die nog geen uitkomst hebben (bij het bijkiezen van een variant). */
async function recomputeMissing() {
  if (populatieGewijzigd.value) return recompute();
  const recs = await ensureRecords();
  const missing = columns.value.filter((c) => !results.value[c.key] && !running.value[c.key]);
  if (!missing.length) return;
  const uitkomsten = await Promise.allSettled(
    missing.map(async (col) => {
      const laws = await lawsFor(col);
      const res = await runInWorker(col.key, laws, recs);
      simInputs.set(col.key, { recs, lawsText: laws.join('\n---\n') });
      return [col.key, res];
    }),
  );
  const next = { ...results.value };
  for (const r of uitkomsten) if (r.status === 'fulfilled') next[r.value[0]] = r.value[1];
  results.value = next;
}

// ---- Metrics -----------------------------------------------------------------

const metricsByColumn = computed(() => {
  const out = {};
  for (const col of columns.value) {
    const res = results.value[col.key];
    if (res) out[col.key] = aggregate(res, handelingenModel.value);
  }
  return out;
});

/** Huidig recht: de vaste vergelijkingskolom. */
const baselineMetrics = computed(() => metricsByColumn.value[KOLOM_BASIS] ?? null);

/** De werkversie-kolom, voor de schermen die één uitkomst tonen. */
const werkversieKey = computed(() => werkversie.value ?? KOLOM_BASIS);
const metrics = computed(() => metricsByColumn.value[werkversieKey.value] ?? baselineMetrics.value);

// ---- Automatisch herberekenen -----------------------------------------------
// Zolang de view nog nooit heeft doorgerekend, start hij zelf; daarna volgt
// elke wijziging, ook als de eerste run nog loopt (die wordt dan afgebroken).

let autoTimer = null;
function planRecompute() {
  if (!autoRecompute.value || recomputeSeq === 0) return;
  clearTimeout(autoTimer);
  autoTimer = setTimeout(() => recompute().catch(() => {}), 600);
}

watch(version, planRecompute);
watch([n, seed, aannameVersie], planRecompute);
watch(selectedVariants, () => {
  if (recomputeSeq === 0) return; // nog nooit gedraaid: de view start zelf
  recomputeMissing().catch(() => {});
});

function dispose() {
  for (const key of [...workers.keys()]) terminate(key);
}

export function usePopulation() {
  return {
    n,
    seed,
    autoRecompute,
    selectedVariants,
    columns,
    records,
    distributions: distributionsRef,
    handelingenModel,
    handelingenBasis,
    handelingenGewijzigd,
    stelMinutenIn,
    stelTariefIn,
    herstelHandelingen,
    running,
    anyRunning,
    progress,
    errors,
    stale,
    simVersion,
    simPopulatie,
    populatieGewijzigd,
    metricsByColumn,
    metrics,
    baselineMetrics,
    werkversieKey,
    werkversieLabel,
    toggleVariant,
    ensureRecords,
    recompute,
    recomputeMissing,
    dispose,
  };
}
