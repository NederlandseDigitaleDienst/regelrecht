/**
 * Harmonisatie: uit de uitkomsten van een simulatie één vereenvoudigde regeling
 * afleiden die de bestaande regelingen zo goed mogelijk benadert.
 *
 * De vraag erachter is een beleidsvraag. Vier toeslagen met elk hun eigen
 * voorwaarden, drempels en afbouwpaden leveren samen een bedrag op dat niemand
 * meer kan uitleggen. Wat als je diezelfde uitkomst probeert te vangen in één
 * staffel? Hoe dicht kom je, en waar zit het verschil? Dat verschil is het
 * interessante: waar het model de wet niet kan volgen, zit vaak precies de
 * regel die niemand meer begrijpt.
 *
 * De POC deed dit met scikit-learn in een Python-subprocess (`synthesize/`,
 * aangestuurd door `web/routers/harmonize.py`). Deze demo heeft geen backend —
 * de engine draait als WASM in de browser — dus is het staffelmodel hier
 * opnieuw geschreven in JavaScript. Dat kan omdat het rekenwerk elementair is:
 * kwantielen, groepsgemiddelden en kleinste kwadraten. De beslisboom en het
 * parametrische model uit de POC leunen wél op sklearn en scipy
 * (DecisionTreeRegressor, differential_evolution) en zijn hier niet
 * overgenomen.
 *
 * De invoer is een gewone simulatierun (`simulation/runner.ts`): dezelfde
 * gegenereerde bevolking, dezelfde echte wetten, dezelfde engine. Er wordt
 * hier dus niets nagebootst — het model leert van wat de wet werkelijk deed.
 *
 * **Bedragen in dit bestand staan in hele euro's, niet in eurocent.** Dat wijkt
 * af van de rest van de corpus, waar geld altijd in centen staat, en het is
 * geen slordigheid: `runner.ts` deelt een uitvoer met een `amount`-spec al door
 * honderd (zie `reduceOutcome`), dus wat hier binnenkomt is al euro. Daarom
 * rondt `roundAmount: 1` af op hele euro's en niet op hele centen.
 *
 * Dat er in drijvende komma wordt gerekend kan hier, anders dan in een wet:
 * de trederanden worden op hele euro's afgerond en het model is een
 * benadering die per persoon toch al honderden euro's mis kan zitten. Nagegaan
 * op een echte staffel: de randen komen exact op hun gehele bedrag uit, de
 * interpolatie ertussen loopt monotoon, en de grootste stap is € 0,44 per euro
 * inkomen. Een bedrag dat iemand écht krijgt komt uit de engine, niet hieruit.
 */
import { intlLocale } from '../data/format';
import { t } from '../i18n/index.js';
import type { LawResult } from './stats';

// ---- types --------------------------------------------------------------------

/**
 * The subject fields the features read, of either audience. Every field is
 * optional because a feature falls back to 0 when it is absent. The keys are
 * those of the subjects `population.ts` generates.
 */
export interface HarmonizeFacts {
  inkomen?: number;
  leeftijd?: number;
  huur?: number;
  kinderen?: number;
  partner?: boolean;
  huurder?: boolean;
  student?: boolean;
  oppervlakte?: number;
  werknemers?: number;
  type?: string;
  voedsel?: boolean;
  terras?: boolean;
}

/** A characteristic of a subject the model can bracket on (a number) or group by (a yes/no as 1/0). */
export interface HarmonizeFeature {
  key: string;
  labelKey: string;
  kind: 'number' | 'boolean';
  of: (subject: HarmonizeFacts) => number;
}

/** The part of a simulation run `trainingData` reads. */
export interface HarmonizeRun<S extends HarmonizeFacts = HarmonizeFacts> {
  kind: string;
  results: readonly { subject: S; laws: Record<string, LawResult> }[];
}

/** One subject in the training table: feature values by key, and the summed amount in euro. */
export interface TrainingRow<S = HarmonizeFacts> {
  values: Record<string, number>;
  eligible: boolean;
  amount: number;
  subject: S;
}

export interface TrainingData<S = HarmonizeFacts> {
  rows: TrainingRow<S>[];
  features: HarmonizeFeature[];
}

/** One step of a bracket table: the amount runs linearly from `lower` to `upper`. */
export interface BracketStep {
  lower: number;
  upper: number;
  amountAtLower: number;
  amountAtUpper: number;
  count: number;
}

/** One bracket table, for the subjects whose group features equal `filter`; no keys means the rest. */
export interface BracketGroup {
  filter: Record<string, number>;
  keys: string[];
  steps: BracketStep[];
  count: number;
}

export interface BracketModelConfig {
  brackets: number;
  roundAmount: number;
  minGroupSize: number;
  maxGroupKeys: number;
  minEta: number;
}

/** The options `trainBracketModel` takes: the config plus the feature to bracket on. */
export interface BracketModelOptions extends Partial<BracketModelConfig> {
  primary?: string;
}

/** How much of the spread in amounts a feature explains. */
export interface FeatureInfluence {
  feature: HarmonizeFeature;
  eta: number;
  threshold: number | null;
}

/** One of the subjects the model misses the most. */
export interface WorstPrediction {
  values: Record<string, number>;
  actual: number;
  predicted: number;
  error: number;
}

export interface ModelMetrics {
  r2: number;
  mae: number;
  meanAmount: number;
  worst: WorstPrediction[];
}

export interface BracketModel {
  primary: HarmonizeFeature;
  groupKeys: HarmonizeFeature[];
  boundaries: number[];
  groups: BracketGroup[];
  influence: FeatureInfluence[];
  config: BracketModelConfig & BracketModelOptions;
  metrics: ModelMetrics;
}

/** What `predict` and `evaluateModel` read of a model. */
export interface PredictableModel {
  primary: { key: string };
  groups: readonly {
    filter: Record<string, number>;
    keys: readonly string[];
    steps: readonly Pick<BracketStep, 'lower' | 'upper' | 'amountAtLower' | 'amountAtUpper'>[];
  }[];
}

/** What `describeModel` reads of a model. */
export interface DescribableModel {
  groupKeys: readonly { key: string; labelKey?: string }[];
  groups: readonly Pick<BracketGroup, 'filter' | 'keys' | 'steps' | 'count'>[];
}

/** One row of the readable bracket table. */
export interface DescribedGroup {
  group: string;
  count: number;
  steps: { range: string; from: string; to: string; count: number }[];
}

// ---- elementaire statistiek ------------------------------------------------

export function mean(values: readonly number[]): number {
  const xs = values.filter((v) => Number.isFinite(v));
  return xs.length ? xs.reduce((s, v) => s + v, 0) / xs.length : 0;
}

/**
 * Het p-kwantiel met lineaire interpolatie tussen de omliggende waarnemingen,
 * zoals numpy.quantile het standaard doet — de POC's grenzen komen daarvandaan.
 */
export function quantile(sorted: readonly number[], p: number): number {
  if (!sorted.length) return 0;
  if (sorted.length === 1) return sorted[0]!;
  // p is clamped to [0, 1], so lower and upper are both within the array.
  const pos = (sorted.length - 1) * Math.min(1, Math.max(0, p));
  const lower = Math.floor(pos);
  const upper = Math.ceil(pos);
  if (lower === upper) return sorted[lower]!;
  return sorted[lower]! + (pos - lower) * (sorted[upper]! - sorted[lower]!);
}

/**
 * Eta-kwadraat: welk deel van de spreiding in de bedragen wordt verklaard
 * doordat je de mensen in groepen deelt. 0 betekent dat de groep niets zegt,
 * 1 dat de groep alles bepaalt. Hiermee wordt gekozen wélke kenmerken een
 * eigen staffel verdienen.
 */
export function etaSquared(groups: readonly (readonly number[])[], overallMean: number, ssTotal: number): number {
  if (!ssTotal) return 0;
  let ssBetween = 0;
  for (const values of groups) {
    if (!values.length) continue;
    const m = mean(values);
    ssBetween += values.length * (m - overallMean) ** 2;
  }
  return ssBetween / ssTotal;
}

function sumSquares(values: readonly number[], m: number): number {
  return values.reduce((s, v) => s + (v - m) ** 2, 0);
}

// ---- kenmerken uit een simulatierun ----------------------------------------

/**
 * De kenmerken waarop geharmoniseerd kan worden, per publiek. Dit zijn de
 * eigenschappen die de bevolkingsgenerator van elk subject vastlegt
 * (`simulation/population.ts`), vertaald naar de vorm die het model gebruikt:
 * een getal om op te staffelen, of een ja/nee om op te groeperen.
 */
export const CITIZEN_FEATURES: HarmonizeFeature[] = [
  { key: 'inkomen', labelKey: 'harm.feature.income', kind: 'number', of: (s) => s.inkomen ?? 0 },
  { key: 'leeftijd', labelKey: 'harm.feature.age', kind: 'number', of: (s) => s.leeftijd ?? 0 },
  { key: 'huur', labelKey: 'harm.feature.rent_per_month', kind: 'number', of: (s) => s.huur ?? 0 },
  { key: 'kinderen', labelKey: 'harm.feature.number_of_children', kind: 'number', of: (s) => s.kinderen ?? 0 },
  { key: 'heeft_partner', labelKey: 'harm.feature.has_partner', kind: 'boolean', of: (s) => (s.partner ? 1 : 0) },
  { key: 'heeft_kinderen', labelKey: 'harm.feature.has_children', kind: 'boolean', of: (s) => ((s.kinderen ?? 0) > 0 ? 1 : 0) },
  { key: 'huurder', labelKey: 'harm.feature.rents_a_home', kind: 'boolean', of: (s) => (s.huurder ? 1 : 0) },
  { key: 'student', labelKey: 'harm.feature.is_student', kind: 'boolean', of: (s) => (s.student ? 1 : 0) },
];

export const BUSINESS_FEATURES: HarmonizeFeature[] = [
  { key: 'oppervlakte', labelKey: 'harm.feature.floor_area', kind: 'number', of: (s) => s.oppervlakte ?? 0 },
  { key: 'werknemers', labelKey: 'harm.feature.number_of_employees', kind: 'number', of: (s) => s.werknemers ?? 0 },
  { key: 'horeca', labelKey: 'harm.feature.is_hospitality', kind: 'boolean', of: (s) => (s.type && s.type !== 'overig' ? 1 : 0) },
  { key: 'voedsel', labelKey: 'harm.feature.prepares_food', kind: 'boolean', of: (s) => (s.voedsel ? 1 : 0) },
  { key: 'terras', labelKey: 'harm.feature.has_terrace', kind: 'boolean', of: (s) => (s.terras ? 1 : 0) },
];

/**
 * Hoe een kenmerk heet op het scherm, in de taal die aan staat.
 *
 * Het kenmerk draagt een sleutel en geen zin, net als de dimensies in stats.ts:
 * `key` is waar de code op werkt en blijft zoals hij is.
 */
export function featureLabel(feature: { labelKey?: string } | null | undefined): string {
  return feature?.labelKey ? t(feature.labelKey) : '';
}

export function featuresFor(kind: string): HarmonizeFeature[] {
  return kind === 'ondernemers' ? BUSINESS_FEATURES : CITIZEN_FEATURES;
}

/**
 * Welke van de gekozen wetten geld kósten in plaats van opleveren.
 *
 * Een belasting en een toeslag bij elkaar optellen alsof ze hetzelfde teken
 * hebben levert een onzinnig bedrag op: € 4.000 belasting plus € 1.600
 * zorgtoeslag is niet € 5.600 waar iemand recht op heeft. Welke wet een
 * belasting is staat in `simulation.disposable_income` in demo-config.yaml
 * (`kind: tax`), dezelfde bron die de besteedbaar-inkomenberekening gebruikt;
 * de wet zelf zegt het niet.
 */
export function taxLawIds(
  corpus: { config?: { simulation?: { disposable_income?: readonly { law: string; kind?: string }[] | null } | null } | null } | null | undefined,
): Set<string> {
  const components = corpus?.config?.simulation?.disposable_income ?? [];
  return new Set(components.filter((c) => c.kind === 'tax').map((c) => c.law));
}

/**
 * Een simulatierun omzetten naar de tabel waarop geleerd wordt: per subject de
 * kenmerken, of er recht bestond op één van de gekozen wetten, en het totaal
 * van wat die wetten netto opleverden.
 *
 * Meerdere wetten optellen is precies de vraag die harmonisatie stelt: wat de
 * burger overhoudt is de som, niet de losse regelingen. Een belasting telt
 * daarin negatief mee.
 *
 * @param run        een run van `runSimulation`
 * @param lawIds   de wetten die samengenomen worden
 * @param taxes  wetten die geld kosten (zie `taxLawIds`)
 */
export function trainingData<S extends HarmonizeFacts>(run: HarmonizeRun<S>, lawIds: readonly string[], taxes: ReadonlySet<string> = new Set()): TrainingData<S> {
  const features = featuresFor(run.kind);
  const rows: TrainingRow<S>[] = [];
  for (const result of run.results) {
    const values: Record<string, number> = {};
    for (const f of features) values[f.key] = f.of(result.subject);
    let eligible = false;
    let amount = 0;
    let usable = false;
    for (const id of lawIds) {
      const law = result.laws[id];
      if (!law?.ok) continue;
      usable = true;
      // Een wet zonder voorwaardenuitvoer (een belasting) geldt voor iedereen;
      // 'onbekend' is geen ja.
      if (law.met === true || law.met === null) eligible = true;
      if (typeof law.amount === 'number') amount += taxes.has(id) ? -law.amount : law.amount;
    }
    // Een subject waarvoor geen enkele gekozen wet kon rekenen zegt niets over
    // de vorm van de regeling en zou het gemiddelde vertekenen.
    if (!usable) continue;
    rows.push({ values, eligible, amount, subject: result.subject });
  }
  return { rows, features };
}

// ---- staffelmodel ----------------------------------------------------------

const DEFAULTS: BracketModelConfig = {
  brackets: 5,
  /** Bedragen afronden op hele euro's: een staffel met centen leest niemand. */
  roundAmount: 1,
  minGroupSize: 30,
  maxGroupKeys: 2,
  /** Onder deze verklaarde spreiding is een eigen staffel de complexiteit niet waard. */
  minEta: 0.05,
};

function roundTo(value: number, step: number): number {
  return step > 0 ? Math.round(value / step) * step : value;
}

/**
 * De grens van elke staffeltrede: kwantielen van het gekozen kenmerk, afgerond
 * op een leesbaar getal. Een grens van € 31.284 is geen wet; € 31.000 wel.
 */
function bracketBoundaries(values: readonly number[], count: number): number[] {
  const sorted = [...values].sort((a, b) => a - b);
  const span = (sorted[sorted.length - 1] ?? 0) - (sorted[0] ?? 0);
  // De afronding volgt de schaal: leeftijd op jaren, inkomen op duizendtallen.
  const step = span > 20000 ? 1000 : span > 2000 ? 100 : span > 200 ? 10 : 1;
  const raw = [];
  for (let i = 0; i <= count; i += 1) raw.push(quantile(sorted, i / count));
  const rounded = [...new Set(raw.map((b) => roundTo(b, step)))].sort((a, b) => a - b);
  return rounded.length >= 2 ? rounded : [sorted[0] ?? 0, (sorted[sorted.length - 1] ?? 0) + 1];
}

/**
 * Kleinste kwadraten binnen één trede, geëvalueerd op de beide grenzen.
 *
 * Niet afgekapt op nul: als er een belasting in de som zit is het nettobedrag
 * negatief, en dat is een echte uitkomst — iemand die per saldo betaalt, niet
 * iemand die niets krijgt.
 */
/** A subject reduced to the bracketed feature and its amount. */
interface Point {
  x: number;
  amount: number;
}

function fitSegment(points: readonly Point[], lower: number, upper: number): [number, number] {
  if (!points.length) return [0, 0];
  if (points.length < 3) {
    const m = mean(points.map((p) => p.amount));
    return [m, m];
  }
  const xs = points.map((p) => p.x);
  const ys = points.map((p) => p.amount);
  const mx = mean(xs);
  const my = mean(ys);
  let num = 0;
  let den = 0;
  for (let i = 0; i < xs.length; i += 1) {
    // i is below xs.length, and ys has the same length.
    num += (xs[i]! - mx) * (ys[i]! - my);
    den += (xs[i]! - mx) ** 2;
  }
  const slope = den > 0 ? num / den : 0;
  const at = (x: number) => my + slope * (x - mx);
  return [at(lower), at(upper)];
}

/**
 * De treden van één staffel: één bedrag per trederand, gedeeld door de treden
 * aan weerszijden.
 *
 * Elke trede apart fitten en de twee randwaarden los opschrijven gaf een
 * staffel die op papier doorloopt maar in werkelijkheid springt: bij een wet
 * die op één inkomensgrens wegvalt, scheelde het € 512 tussen de laatste euro
 * van de ene trede en de eerste van de volgende. Dat is precies het
 * cliff-effect dat een staffel juist zou wegnemen.
 *
 * Nu levert elke trede haar eigen schatting voor beide randen, en waar twee
 * treden dezelfde rand raken wordt het gemiddelde genomen. Het bedrag is
 * daarmee per definitie doorlopend; de prijs is dat een echte klif in de
 * gegevens wordt uitgesmeerd over de twee treden eromheen. Dat is de juiste
 * ruil: dat de vereenvoudiging een klif niet kan volgen, hoort te blijken uit
 * "Waar het misgaat", niet uit een sprong die het model zelf introduceert.
 *
 * Bedragen worden niet op nul afgekapt: zit er een belasting in de som, dan is
 * een negatief nettobedrag een echte uitkomst.
 */
function stepsFor(points: readonly Point[], boundaries: readonly number[], config: BracketModelConfig): BracketStep[] {
  const n = boundaries.length - 1;
  // Per trede de eigen schatting op beide randen.
  const fits: { lower: number; upper: number; count: number; fit: [number, number] }[] = [];
  for (let i = 0; i < n; i += 1) {
    // i + 1 <= n = boundaries.length - 1, so both are within the array.
    const lower = boundaries[i]!;
    const upper = boundaries[i + 1]!;
    const inBracket = points.filter((p) => p.x >= lower && p.x <= upper);
    fits.push({ lower, upper, count: inBracket.length, fit: fitSegment(inBracket.length >= 3 ? inBracket : points, lower, upper) });
  }
  // Eén waarde per rand: het gemiddelde van wat de treden eromheen zeggen.
  const atBoundary: number[] = [];
  for (let b = 0; b <= n; b += 1) {
    // fits has n entries: b - 1 is one when b > 0, b is one when b < n.
    const left = b > 0 ? fits[b - 1]!.fit[1] : null;
    const right = b < n ? fits[b]!.fit[0] : null;
    const both = [left, right].filter((v) => v !== null);
    atBoundary.push(both.reduce((s, v) => s + v, 0) / both.length);
  }
  return fits.map((f, i) => ({
    lower: f.lower,
    upper: f.upper,
    // atBoundary has n + 1 entries for the n fits.
    amountAtLower: roundTo(atBoundary[i]!, config.roundAmount),
    amountAtUpper: roundTo(atBoundary[i + 1]!, config.roundAmount),
    count: f.count,
  }));
}

/**
 * Een staffelmodel leren.
 *
 * Eerst wordt gekozen waarop gestaffeld wordt (het kenmerk met de meeste
 * invloed op het bedrag, standaard het inkomen), daarna welke ja/nee-kenmerken
 * een eigen staffel krijgen, en ten slotte per groep het bedrag op elke
 * trederand. Tussen twee randen loopt het bedrag rechtlijnig, dus er zit nooit
 * een sprong in: precies één euro meer verdienen kost nooit honderd euro
 * toeslag.
 *
 * @param data  uit `trainingData`
 */
export function trainBracketModel(data: TrainingData<unknown>, options: BracketModelOptions = {}): BracketModel {
  const config: BracketModelConfig & BracketModelOptions = { ...DEFAULTS, ...options };
  const { rows, features } = data;
  if (rows.length < 10) throw new Error(t('harm.error.too_little_data'));

  const amounts = rows.map((r) => r.amount);
  const overallMean = mean(amounts);
  const ssTotal = sumSquares(amounts, overallMean);

  // Welke kenmerken doen ertoe? Voor een getal wordt de beste tweedeling
  // gezocht, voor een ja/nee is de deling al gegeven.
  const influence: FeatureInfluence[] = [];
  for (const f of features) {
    if (f.kind === 'boolean') {
      const yes = rows.filter((r) => r.values[f.key] === 1).map((r) => r.amount);
      const no = rows.filter((r) => r.values[f.key] !== 1).map((r) => r.amount);
      if (yes.length < config.minGroupSize || no.length < config.minGroupSize) continue;
      influence.push({ feature: f, eta: etaSquared([yes, no], overallMean, ssTotal), threshold: null });
    } else {
      const { threshold, eta } = bestSplit(rows, f.key, overallMean, ssTotal, config.minGroupSize);
      influence.push({ feature: f, eta, threshold });
    }
  }
  influence.sort((a, b) => b.eta - a.eta);

  // Waarop staffelen: het opgegeven kenmerk, anders het invloedrijkste getal.
  const primary =
    features.find((f) => f.key === options.primary && f.kind === 'number') ??
    influence.find((i) => i.feature.kind === 'number')?.feature ??
    features.find((f) => f.kind === 'number');
  if (!primary) throw new Error(t('harm.error.no_numeric_feature'));

  // Waarop groeperen: de invloedrijkste ja/nee-kenmerken, behalve het kenmerk
  // waarop al gestaffeld wordt.
  const groupKeys = influence
    .filter((i) => i.feature.kind === 'boolean' && i.feature.key !== primary.key && i.eta >= config.minEta)
    .slice(0, config.maxGroupKeys)
    .map((i) => i.feature);

  // primary is one of `features`, and `trainingData` gives every row a value for each of them.
  const boundaries = bracketBoundaries(rows.map((r) => r.values[primary.key]!), config.brackets);

  // Elke combinatie van de groepskenmerken krijgt een eigen staffel, mits er
  // genoeg mensen in zitten om iets zinnigs over te zeggen.
  const combos = groupCombinations(groupKeys);
  const groups: BracketGroup[] = [];
  const covered: Record<string, number>[] = [];
  for (const combo of combos) {
    const members = rows.filter((r) => groupKeys.every((f) => r.values[f.key] === combo[f.key]));
    if (members.length < Math.max(config.minGroupSize, 5 * (boundaries.length - 1))) continue;
    covered.push(combo);
    groups.push({
      filter: combo,
      keys: groupKeys.map((f) => f.key),
      steps: stepsFor(members.map((r) => ({ x: r.values[primary.key]!, amount: r.amount })), boundaries, config),
      count: members.length,
    });
  }

  // En altijd een staffel voor de rest: iedereen wiens combinatie te klein was
  // om er een te verdienen.
  //
  // Zonder deze viel zo iemand bij `predict` terug op de eerste groep in de
  // lijst, wat een wíllekeurige andere combinatie is: wie een partner heeft en
  // niet huurt kreeg dan het bedrag van wie geen partner heeft en wel huurt.
  // Dat telde ook mee in de gemiddelde afwijking, terwijl de tabel er geen
  // regel voor toonde — de staffel was dus geen totale functie over de
  // bevolking. Nu is er één regel die zegt wat er met de rest gebeurt.
  const rest = rows.filter((r) => !covered.some((c) => groupKeys.every((f) => r.values[f.key] === c[f.key])));
  if (!groups.length || rest.length) {
    groups.push({
      filter: {},
      keys: [],
      steps: stepsFor((rest.length ? rest : rows).map((r) => ({ x: r.values[primary.key]!, amount: r.amount })), boundaries, config),
      count: rest.length || rows.length,
    });
  }

  const model = { primary, groupKeys, boundaries, groups, influence, config };
  return { ...model, metrics: evaluateModel(model, rows) };
}

/** De beste tweedeling van een numeriek kenmerk, op verklaarde spreiding. */
function bestSplit(
  rows: readonly Pick<TrainingRow<unknown>, 'values' | 'amount'>[],
  key: string,
  overallMean: number,
  ssTotal: number,
  minGroupSize: number,
): { threshold: number | null; eta: number } {
  // Every row carries a value for every feature key (`trainingData` fills them all).
  const values = rows.map((r) => r.values[key]!);
  const sorted = [...values].sort((a, b) => a - b);
  // trainBracketModel refuses fewer than ten rows, so sorted is not empty.
  const span = sorted[sorted.length - 1]! - sorted[0]!;
  const step = span > 20000 ? 1000 : span > 2000 ? 100 : span > 200 ? 10 : 1;
  const candidates = new Set<number>();
  for (let p = 0.1; p <= 0.9; p += 0.1) candidates.add(roundTo(quantile(sorted, p), step));
  let best: { threshold: number | null; eta: number } = { threshold: null, eta: 0 };
  for (const t of candidates) {
    const below = rows.filter((r) => r.values[key]! <= t).map((r) => r.amount);
    const above = rows.filter((r) => r.values[key]! > t).map((r) => r.amount);
    if (below.length < minGroupSize || above.length < minGroupSize) continue;
    const eta = etaSquared([below, above], overallMean, ssTotal);
    if (eta > best.eta) best = { threshold: t, eta };
  }
  return best;
}

/** Alle combinaties van ja/nee over de groepskenmerken. */
function groupCombinations(keys: readonly HarmonizeFeature[]): Record<string, number>[] {
  let combos: Record<string, number>[] = [{}];
  for (const f of keys) {
    const next: Record<string, number>[] = [];
    for (const c of combos) {
      next.push({ ...c, [f.key]: 0 });
      next.push({ ...c, [f.key]: 1 });
    }
    combos = next;
  }
  return combos;
}

/**
 * Wat het model voor één persoon zou toekennen.
 *
 * Elke trede wordt apart gefit, dus het bedrag op de bovengrens van de ene
 * trede hoeft niet tot op de cent gelijk te zijn aan dat op de ondergrens van
 * de volgende. Gemeten op een sterk gebogen functie blijft dat verschil binnen
 * de afronding op hele euro's (hooguit € 2), dus als sprong is het niet
 * zichtbaar; op de grens zelf wint de eerste trede die past.
 */
export function predict(model: PredictableModel, values: Record<string, number | undefined>): number {
  const group =
    model.groups.find((g) => g.keys.every((k) => values[k] === g.filter[k])) ??
    model.groups.find((g) => g.keys.length === 0) ??
    model.groups[0];
  if (!group) return 0;
  const x = values[model.primary.key] ?? 0;
  const steps = group.steps;
  if (!steps.length) return 0;
  // steps is not empty, so its first and last entries exist.
  const first = steps[0]!;
  if (x <= first.lower) return first.amountAtLower;
  const last = steps[steps.length - 1]!;
  if (x >= last.upper) return last.amountAtUpper;
  for (const s of steps) {
    if (x >= s.lower && x <= s.upper) {
      const width = s.upper - s.lower;
      if (width === 0) return s.amountAtLower;
      const t = (x - s.lower) / width;
      return s.amountAtLower + t * (s.amountAtUpper - s.amountAtLower);
    }
  }
  return last.amountAtUpper;
}

/**
 * Hoe goed benadert het model de bestaande wetten?
 *
 * R² is het deel van de spreiding dat het model verklaart; MAE het gemiddelde
 * verschil in euro's. Die tweede is de eerlijkste: een R² van 0,9 klinkt goed,
 * maar € 40 per maand ernaast is voor wie het aangaat gewoon € 40.
 */
export function evaluateModel<R extends { values: Record<string, number>; amount: number }>(model: PredictableModel, rows: readonly R[]): ModelMetrics {
  if (!rows.length) return { r2: 0, mae: 0, meanAmount: 0, worst: [] };
  const errors: (R & { predicted: number; error: number })[] = [];
  let ssResidual = 0;
  const actual = rows.map((r) => r.amount);
  const m = mean(actual);
  for (const row of rows) {
    const predicted = predict(model, row.values);
    const error = predicted - row.amount;
    ssResidual += error ** 2;
    errors.push({ ...row, predicted, error });
  }
  const ssTotal = sumSquares(actual, m);
  errors.sort((a, b) => Math.abs(b.error) - Math.abs(a.error));
  return {
    r2: ssTotal > 0 ? 1 - ssResidual / ssTotal : 0,
    mae: mean(errors.map((e) => Math.abs(e.error))),
    meanAmount: m,
    // Waar het model er het verst naast zit: daar zit de regel die de staffel
    // niet kan volgen, en dat is het interessantste deel van de uitkomst.
    worst: errors.slice(0, 5).map((e) => ({ values: e.values, actual: e.amount, predicted: e.predicted, error: e.error })),
  };
}

/**
 * Het model als leesbare regeling, in de vorm waarin een wet het zou zeggen.
 * Geen YAML zoals de POC genereerde — die was bedoeld om weer uit te voeren;
 * dit is bedoeld om te lezen en te bespreken.
 */
export function describeModel(model: DescribableModel, formatAmount: (v: number) => string = (v) => `€ ${v.toFixed(0)}`): DescribedGroup[] {
  const lines: DescribedGroup[] = [];
  for (const group of model.groups) {
    // Een groep zonder kenmerken is de rest: iedereen wiens combinatie te
    // klein was voor een eigen staffel. Staat er maar één groep, dan is dat
    // gewoon iedereen.
    const label = group.keys.length
      ? group.keys.map((k) => `${labelOf(model, k)}: ${t(group.filter[k] ? 'harm.group.yes' : 'harm.group.no')}`).join(', ')
      : model.groups.length > 1
        ? t('harm.group.rest')
        : t('harm.group.everyone');
    lines.push({
      group: label,
      count: group.count,
      steps: group.steps.map((s) => ({
        range: `${formatNumber(s.lower)} – ${formatNumber(s.upper)}`,
        from: formatAmount(s.amountAtLower),
        to: formatAmount(s.amountAtUpper),
        count: s.count,
      })),
    });
  }
  return lines;
}

function labelOf(model: DescribableModel, key: string): string {
  const feature = model.groupKeys.find((f) => f.key === key);
  return feature ? featureLabel(feature) : key;
}

function formatNumber(v: number): string {
  return new Intl.NumberFormat(intlLocale(), { maximumFractionDigits: 0 }).format(v);
}
