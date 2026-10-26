/**
 * Synthetic populations for the simulation: citizens and businesses.
 *
 * A generated person or business is a set of register rows in the same shape
 * as `corpus/demo/profiles.yaml` (service -> table -> rows), so the ordinary
 * materialiser and bindings turn them into law inputs exactly as they do for
 * the personas. Columns the simulation does not model are copied from a
 * persona's row of the same table (the template), so a law that reads an
 * unmodelled column still finds a plausible value.
 *
 * The distributions follow the POC simulator (CBS-like age buckets, log-normal
 * income per bracket, rent by income), reduced to what the demo laws read.
 * Pure JS, seeded, no dependencies.
 */
import { createRandom } from './random';
import type { Random } from './random';

/** One register row, in the shape of `corpus/demo/profiles.yaml`: column -> value. */
export type RegisterRow = Record<string, unknown>;

/** Register rows per service and table: service -> table -> rows. */
export type TablesByService = Record<string, Record<string, RegisterRow[]>>;

/** `templateRow(service, table)`: a persona's row to copy unmodelled columns from. */
export type TemplateRow = (service: string, table: string) => RegisterRow;

/** `rowsFor(service, table)`: every row of that table. */
export type RowsFor = (service: string, table: string) => RegisterRow[];

/** The fields of `profiles.yaml` this module reads (its own keys). */
export interface ProfilesDoc {
  globalServices?: Record<string, Record<string, unknown> | null> | null;
  profiles?: Record<string, { sources?: Record<string, Record<string, unknown> | null> | null }> | null;
}

export type IncomeLevel = 'low' | 'middle' | 'high';
export type RentBand = 'low' | 'medium' | 'high';
export type SizeClass = 'small' | 'medium' | 'large';
export type BusinessType = 'horecabedrijf' | 'slijtersbedrijf' | 'overig';

/** The knobs of a citizen population (the simulation form). */
export interface CitizenParams {
  count: number;
  seed: number;
  /** Relative weight per age bucket ('18-30', '30-45', '45-67', '67-85', '85+'). */
  ageDistribution: Record<string, number>;
  /** Relative weight per income level. */
  incomeDistribution: Record<IncomeLevel, number>;
  zeroIncomePct: number;
  renterPct: number;
  studentPct: number;
  /** Monthly rent range in euros per rent band. */
  rentRanges: Record<RentBand, [number, number]>;
}

/** The knobs of a business population (the simulation form). */
export interface BusinessParams {
  count: number;
  seed: number;
  horecaPct: number;
  foodPct: number;
  /** Relative weight per size class. */
  sizeDistribution: Record<SizeClass, number>;
  terracePct: number;
  employeesPct: number;
}

/**
 * A form answer (`kind: claim` input) the simulation supplies for one subject.
 * `value` is left out of the record when it is `undefined`.
 */
export interface Claim {
  lawId: string;
  keyField: string;
  keyValue: string;
  input: string;
  value: unknown;
}

/**
 * One simulated citizen, as shown in the subject table, the breakdowns and
 * the export. The keys are Dutch because the export columns and the
 * breakdowns (stats.ts) use them as they are. Amounts in euros.
 */
export interface CitizenSubject {
  id: string;
  bsn: string;
  leeftijd: number;
  inkomen: number;
  vermogen: number;
  partner: boolean;
  kinderen: number;
  huurder: boolean;
  huur: number;
  student: boolean;
  zelfstandig: boolean;
  werkloos: boolean;
  gedetineerd: boolean;
  nederlands: boolean;
}

/** One simulated business; Dutch keys for the same reason as `CitizenSubject`. */
export interface BusinessSubject {
  id: string;
  kvk_nummer: string;
  type: BusinessType;
  voedsel: boolean;
  oppervlakte: number;
  grootte: SizeClass;
  rechtsvorm: string;
  werknemers: number;
  terras: boolean;
  terras_m2: number;
  elektriciteit_kwh: number;
  gas_m3: number;
  alcohol: boolean;
  leeftijd_eigenaar: number;
}

export type SimulationSubject = CitizenSubject | BusinessSubject;

/**
 * A business's application-form answers, keyed on the law parameters they
 * fill (law-format names). A type alias rather than an interface, so it is
 * assignable to a record of parameters.
 */
export type BusinessFormValues = {
  bsn: string;
  aangevraagde_categorie: string;
  activiteiten: string[];
  bereidt_of_serveert_voedsel: boolean;
  /** The law_paths this business applies for. */
  aanvragen: string[];
  terras_locatie: string;
  terras_oppervlakte: number;
  obstakelvrije_ruimte: number;
  seizoen: string;
  gewenste_openingstijd: number;
  gewenste_sluitingstijd_doordeweeks: number;
  gewenste_sluitingstijd_weekend: number;
  activiteitsdatum: string;
  activiteitsstarttijd: number;
};

/** A generated population. */
export interface Population<S extends SimulationSubject = SimulationSubject> {
  /** One plain record per subject (for tables and breakdowns). */
  subjects: S[];
  /** Register rows per service/table. */
  tables: TablesByService;
  /** `kind: claim` inputs (rent, childcare). */
  claims: Claim[];
  /** Key field -> every value it takes (bsn, kvk_nummer). */
  keyValues: Record<string, string[]>;
  /** Per kvk_nummer the application-form answers; businesses only. */
  formValues?: Record<string, BusinessFormValues>;
}

export type CitizenPopulation = Population<CitizenSubject>;
export type BusinessPopulation = Population<BusinessSubject> & { formValues: Record<string, BusinessFormValues> };

/** A generated person before it becomes register rows. */
interface Person {
  bsn: string;
  birthDate: string;
  age: number;
  /** Eurocent per year. */
  income: number;
  netWorth: number;
  isStudent: boolean;
  rents: boolean;
  rent: number;
  serviceCosts: number;
  eligibleServiceCosts: number;
  detained: boolean;
  dutch: boolean;
  children: Child[];
  selfEmployed: boolean;
  hours: number;
  unemployed: boolean;
  childcare: { type: 'dagopvang' | 'bso'; hoursPerYear: number; hourlyRate: number } | null;
  workYears: number;
  residenceYears: number;
  partnerBsn: string | null;
  street: string;
  houseNumber: string;
  postcode: string;
}

interface Child {
  bsn: string;
  geboortedatum: string;
  age: number;
  zorgbehoefte: boolean;
}

interface PersonIds {
  bsn: () => string;
  childBsn: () => string;
}

/**
 * The largest population one run generates, citizens or businesses.
 *
 * Every subject goes through every law in the browser, so this is a ceiling on
 * waiting time, not on realism. The count field in SimulatieView and the
 * simulation slide in demo-config.yaml both follow it; `slides.test.ts` fails
 * when the slide names another number.
 */
export const MAX_POPULATION = 2000;

export const CITIZEN_DEFAULTS: Readonly<CitizenParams> = Object.freeze<CitizenParams>({
  count: 50,
  seed: 42,
  ageDistribution: { '18-30': 18, '30-45': 25, '45-67': 32, '67-85': 20, '85+': 5 },
  incomeDistribution: { low: 30, middle: 50, high: 20 },
  zeroIncomePct: 5,
  renterPct: 43,
  studentPct: 40,
  rentRanges: { low: [477, 600], medium: [600, 750], high: [750, 800] },
});

export const BUSINESS_DEFAULTS: Readonly<BusinessParams> = Object.freeze<BusinessParams>({
  count: 50,
  seed: 42,
  horecaPct: 40,
  foodPct: 55,
  sizeDistribution: { small: 50, medium: 30, large: 20 },
  terracePct: 70,
  employeesPct: 60,
});

const AGE_RANGES: Record<string, [number, number]> = { '18-30': [18, 30], '30-45': [30, 45], '45-67': [45, 67], '67-85': [67, 85], '85+': [85, 100] };
// Log-normal parameters of the underlying normal, in euros per year.
const INCOME_PARAMS: Record<IncomeLevel, [number, number]> = { low: [9.9, 0.5], middle: [10.7, 0.3], high: [11.3, 0.45] };
const STREETS = ['Kalverstraat', 'Ceintuurbaan', 'Javastraat', 'Overtoom', 'Meeuwenlaan', 'Rozengracht', 'Linnaeusstraat'];
const ROTTERDAM_STREETS = ['Nieuwe Binnenweg', 'Witte de Withstraat', 'Meent', 'Zwart Janstraat', 'Oude Binnenweg', 'Pannekoekstraat'];

/** `templateRow(service, table)`: any persona's row for that table, or {}. */
export function templatesFromProfiles(profilesDoc: ProfilesDoc | null | undefined): TemplateRow {
  const first = new Map<string, RegisterRow>();
  const add = (service: string, byTable: Record<string, unknown> | null | undefined) => {
    for (const [table, rows] of Object.entries(byTable ?? {})) {
      const key = `${service} ${table}`;
      if (!first.has(key) && Array.isArray(rows) && rows.length) first.set(key, rows[0] as RegisterRow);
    }
  };
  for (const [service, byTable] of Object.entries(profilesDoc?.globalServices ?? {})) add(service, byTable);
  for (const profile of Object.values(profilesDoc?.profiles ?? {})) {
    for (const [service, byTable] of Object.entries(profile.sources ?? {})) add(service, byTable);
  }
  return (service, table) => first.get(`${service} ${table}`) ?? {};
}

function isoDate(y: number, m: number, d: number): string {
  return `${y}-${String(m).padStart(2, '0')}-${String(d).padStart(2, '0')}`;
}

function yearOf(iso: string): number {
  return Number(String(iso).slice(0, 4));
}

function ageAt(birthIso: string, refIso: string): number {
  const b = new Date(`${birthIso}T00:00:00Z`);
  const r = new Date(`${refIso}T00:00:00Z`);
  let age = r.getUTCFullYear() - b.getUTCFullYear();
  if (r.getUTCMonth() < b.getUTCMonth() || (r.getUTCMonth() === b.getUTCMonth() && r.getUTCDate() < b.getUTCDate())) age -= 1;
  return age;
}

function tableSet() {
  const tables: TablesByService = {};
  return {
    tables,
    add(service: string, table: string, row: RegisterRow) {
      const byTable = (tables[service] ??= {});
      (byTable[table] ??= []).push(row);
    },
  };
}

// ---------------------------------------------------------------------------
// Citizens
// ---------------------------------------------------------------------------

function generatePerson(rng: Random, params: CitizenParams, referenceDate: string, ids: PersonIds, birthYearRange: [number, number] | null = null): Person {
  const refYear = yearOf(referenceDate);
  let range = birthYearRange;
  if (!range) {
    const labels = Object.keys(params.ageDistribution);
    const bucket: string | undefined = rng.weighted(labels, labels.map((l) => Number(params.ageDistribution[l]) || 0));
    const [lo, hi] = (bucket !== undefined ? AGE_RANGES[bucket] : undefined) ?? [30, 45];
    range = [refYear - hi, refYear - lo];
  }
  const birthDate = isoDate(rng.int(range[0], range[1]), rng.int(1, 12), rng.int(1, 28));
  const age = Math.max(0, ageAt(birthDate, referenceDate));

  const levelWeights: [number, number, number] = age < 30 ? [0.6, 0.3, 0.1] : age < 45 ? [0.3, 0.5, 0.2] : age < 67 ? [0.25, 0.45, 0.3] : [0.5, 0.4, 0.1];
  const dist = params.incomeDistribution;
  const levels: IncomeLevel[] = ['low', 'middle', 'high'];
  const level = rng.weighted(levels, levels.map((k, i) => levelWeights[i]! * (Number(dist[k]) || 0)));
  let income = 0; // eurocent per year
  if (!rng.chance(params.zeroIncomePct / 100)) {
    const [mu, sigma] = INCOME_PARAMS[level];
    income = Math.min(Math.max(Math.round(rng.lognormal(mu, sigma)), 0), 200000) * 100;
  }
  if (age >= 67) income = Math.min(Math.max(income, 15000 * 100), 50000 * 100);

  const isStudent = age < 30 && rng.chance(params.studentPct / 100);
  if (isStudent) income = Math.min(Math.max(income, 5000 * 100), 20000 * 100);

  let worthMultiplier = Math.max(0.5, Math.min(age / 25, 8));
  if (isStudent) worthMultiplier *= 0.4;
  const netWorth = Math.round(Math.min(Math.max(income * worthMultiplier * rng.uniform(0.8, 1.2), 0), 2000000 * 100));

  let rentProbability = params.renterPct / 100;
  if (age < 30) rentProbability = 0.8;
  else if (age < 40) rentProbability = 0.55;
  else if (age > 67) rentProbability = 0.35;
  if (income < 2500000) rentProbability += 0.2;
  const rents = rentProbability > rng.next();
  let rent = 0;
  let serviceCosts = 0;
  let eligibleServiceCosts = 0;
  if (rents) {
    const band = income < 2000000 ? 'low' : income < 4000000 ? 'medium' : 'high';
    const [lo, hi] = params.rentRanges[band];
    rent = rng.int(Number(lo), Number(hi)) * 100;
    serviceCosts = rng.int(40, 150) * 100;
    eligibleServiceCosts = Math.min(serviceCosts, 4800);
  }

  const detained = rng.chance(0.002);
  const dutch = rng.chance(0.9);
  const children: Child[] = [];
  if (age >= 23 && age <= 55 && rng.chance(Math.min(0.7, (age - 23) * 0.03))) {
    const n = rng.weighted([1, 2, 3, 4], [0.4, 0.4, 0.15, 0.05]);
    for (let i = 0; i < n; i += 1) {
      const childAge = rng.int(0, Math.min(20, age - 23));
      children.push({
        bsn: ids.childBsn(),
        geboortedatum: isoDate(refYear - childAge - 1, rng.int(1, 12), rng.int(1, 28)),
        age: childAge,
        zorgbehoefte: rng.chance(0.05),
      });
    }
  }
  const selfEmployed = !isStudent && age >= 18 && age < 67 && rng.chance(0.12);
  let hours = 0;
  if (isStudent) hours = rng.pick([0, 8, 12, 16, 20]);
  else if (age >= 67 || detained) hours = 0;
  else if (income < 500000) hours = rng.pick([0, 0, 0, 8, 12]);
  else if (income < 2000000) hours = rng.int(12, 28);
  else if (income < 4000000) hours = rng.int(28, 40);
  else hours = rng.int(32, 45);
  const unemployed = !isStudent && age >= 18 && age < 67 && income > 0 && hours === 0;

  const youngChildren = children.filter((c) => c.age < 12);
  let childcare: Person['childcare'] = null;
  if (youngChildren.length && hours >= 12) {
    const youngest = Math.min(...youngChildren.map((c) => c.age));
    const type: 'dagopvang' | 'bso' = youngest < 4 ? 'dagopvang' : 'bso';
    const days = Math.min(5, Math.max(2, Math.floor(hours / 8)));
    childcare = { type, hoursPerYear: days * (type === 'dagopvang' ? 10 : 4) * 46, hourlyRate: type === 'dagopvang' ? rng.int(800, 920) : rng.int(700, 780) };
  }

  return {
    bsn: ids.bsn(),
    birthDate,
    age,
    income,
    netWorth,
    isStudent,
    rents,
    rent,
    serviceCosts,
    eligibleServiceCosts,
    detained,
    dutch,
    children,
    selfEmployed,
    hours,
    unemployed,
    childcare,
    workYears: Math.min(Math.max(0, Math.round((age - 18) * rng.uniform(0.5, 0.9))), 50),
    residenceYears: Math.min(Math.max(0, Math.round((age - 15) * rng.uniform(0.8, 1.0))), 50),
    partnerBsn: null,
    street: rng.pick(STREETS),
    houseNumber: String(rng.int(1, 250)),
    postcode: `${rng.int(1011, 1109)}${rng.pick(['AB', 'CD', 'EF', 'GH', 'JK'])}`,
  };
}

function pairPeople(people: Person[], rng: Random): void {
  for (let i = 0; i < people.length - 1; i += 1) {
    // i + 1 < people.length by the loop bound.
    const p = people[i]!;
    if (p.partnerBsn) continue;
    const q = people[i + 1]!;
    if (q.partnerBsn) continue;
    const prob = p.age < 25 ? 0.1 : p.age < 35 ? 0.4 : p.age < 60 ? 0.7 : 0.6;
    if (Math.abs(p.age - q.age) <= 12 && rng.chance(prob)) {
      p.partnerBsn = q.bsn;
      q.partnerBsn = p.bsn;
      q.street = p.street;
      q.houseNumber = p.houseNumber;
      q.postcode = p.postcode;
    }
  }
}

/**
 * @returns subjects: one plain record per person (for tables and breakdowns);
 *   tables: register rows per service/table; claims: `kind: claim` inputs
 *   (rent, childcare) as { lawId, keyField, keyValue, input, value }.
 */
export function generateCitizens(userParams: Partial<CitizenParams> | null | undefined, referenceDate: string, templateRow: TemplateRow = () => ({})): CitizenPopulation {
  const params: CitizenParams = { ...CITIZEN_DEFAULTS, ...userParams };
  const rng = createRandom(params.seed);
  let bsnCounter = 0;
  let childCounter = 0;
  const ids = {
    bsn: () => `9995${String((bsnCounter += 1)).padStart(5, '0')}`,
    childBsn: () => `9996${String((childCounter += 1)).padStart(5, '0')}`,
  };
  const count = Math.max(1, Math.min(MAX_POPULATION, Math.floor(Number(params.count) || 1)));
  const people: Person[] = [];
  for (let i = 0; i < count; i += 1) people.push(generatePerson(rng, params, referenceDate, ids));
  pairPeople(people, rng);
  const byBsn = new Map(people.map((p) => [p.bsn, p]));

  const t = tableSet();
  const claims: Claim[] = [];
  const T = (service: string, table: string, row: RegisterRow) => t.add(service, table, { ...templateRow(service, table), ...row });
  const refYear = yearOf(referenceDate);

  for (const p of people) {
    const partner = p.partnerBsn ? byBsn.get(p.partnerBsn) : null;
    const address = `${p.street} ${p.houseNumber}, ${p.postcode} Amsterdam`;
    const kinderen = p.children.map((c, i) => ({ bsn: c.bsn, geboortedatum: c.geboortedatum, naam: `Kind ${i + 1}` }));
    T('RvIG', 'personen', {
      bsn: p.bsn,
      geboortedatum: p.birthDate,
      land_verblijf: 'NEDERLAND',
      nationaliteit: p.dutch ? 'NEDERLANDS' : 'BUITENLANDS',
      age: p.age,
      has_dutch_nationality: p.dutch,
      has_partner: !!partner,
      residence_address: address,
      has_fixed_address: true,
      household_size: 1 + (partner ? 1 : 0) + p.children.length,
    });
    T('RvIG', 'relaties', {
      bsn: p.bsn,
      partnerschap_type: partner ? 'HUWELIJK' : 'GEEN',
      partner_bsn: partner ? partner.bsn : null,
      has_partner: !!partner,
      kinderen,
    });
    T('RvIG', 'verblijfplaats', { bsn: p.bsn, straat: p.street, huisnummer: p.houseNumber, postcode: p.postcode, woonplaats: 'Amsterdam', type: 'WOONADRES' });
    T('RvIG', 'children_data', { bsn: p.bsn, kinderen: p.children.map((c) => ({ geboortedatum: c.geboortedatum, kind_bsn: c.bsn, zorgbehoefte: c.zorgbehoefte })) });
    for (const c of p.children) {
      T('RvIG', 'gezag_relaties', {
        bsn_gezagdrager: p.bsn,
        bsn_kind: c.bsn,
        naam_kind: `Kind van ${p.bsn}`,
        geboortedatum_kind: c.geboortedatum,
        type_gezag: 'OUDERLIJK_GEZAG',
        datum_ingang: c.geboortedatum,
        datum_einde: null,
        status: 'ACTIEF',
        heeft_handlichting: false,
      });
    }
    if (!partner || p.bsn < partner.bsn) {
      T('RvIG', 'bewoners', { postcode: p.postcode, huisnummer: p.houseNumber, medebewoners: partner ? [{ bsn: partner.bsn, geboortedatum: partner.birthDate }] : [] });
    }

    const wages = p.selfEmployed || p.age >= 67 ? 0 : p.income;
    const profit = p.selfEmployed ? p.income : 0;
    const pension = p.age >= 67 ? p.income : 0;
    T('BELASTINGDIENST', 'box1', {
      bsn: p.bsn,
      loon_uit_dienstbetrekking: wages,
      uitkeringen_en_pensioenen: pension,
      winst_uit_onderneming: profit,
      resultaat_overige_werkzaamheden: 0,
      eigen_woning: p.rents ? 0 : -Math.round(p.income * 0.1),
    });
    T('BELASTINGDIENST', 'box2', { bsn: p.bsn, reguliere_voordelen: 0, vervreemdingsvoordelen: 0 });
    T('BELASTINGDIENST', 'box3', {
      bsn: p.bsn,
      spaargeld: Math.round(p.netWorth * 0.4),
      beleggingen: Math.round(p.netWorth * 0.1),
      onroerend_goed: p.rents ? 0 : Math.round(p.netWorth * 0.5),
      schulden: rng.chance(0.2) ? Math.round(p.income * 0.05) : 0,
      buitenlands_inkomen: 0,
    });
    const monthly = Math.round(p.income / 12);
    for (const table of ['monthly_income', 'maandelijks_inkomen']) T('BELASTINGDIENST', table, { bsn: p.bsn, bedrag: monthly });
    for (const table of ['assets', 'bezittingen']) T('BELASTINGDIENST', table, { bsn: p.bsn, bedrag: p.netWorth });
    for (const table of ['business_income', 'bedrijfsinkomen']) T('BELASTINGDIENST', table, { bsn: p.bsn, bedrag: p.selfEmployed ? monthly : 0 });
    T('BELASTINGDIENST', 'buitenlands_inkomen', { bsn: p.bsn, bedrag: 0, land: 'GEEN' });
    T('BELASTINGDIENST', 'buitenlands', { bsn: p.bsn, inkomen: 0 });
    T('BELASTINGDIENST', 'aftrekposten', { bsn: p.bsn, persoonsgebonden_aftrek: 0 });
    T('BELASTINGDIENST', 'belastingdienst_vermogen', { bsn: p.bsn, vermogen: p.netWorth });

    T('UWV', 'arbeidsverhoudingen', {
      bsn: p.bsn,
      dienstverband_type: wages > 0 && !p.unemployed ? 'VAST' : 'GEEN',
      verzekerd_ww: wages > 0,
      verzekerd_wia: wages > 0,
    });
    T('UWV', 'uwv_toetsingsinkomen', { bsn: p.bsn, toetsingsinkomen: p.income });
    T('UWV', 'uwv_werkgegevens', {
      bsn: p.bsn,
      gemiddeld_uren_per_week: p.unemployed ? rng.int(32, 40) : p.hours,
      huidige_uren_per_week: p.hours,
      gewerkte_weken_36: p.unemployed || p.hours > 0 ? rng.int(26, 36) : 0,
      arbeidsverleden_jaren: p.workYears,
      jaarloon: wages,
    });
    if (wages > 0 || p.unemployed) {
      T('UWV', 'dienstverbanden', {
        bsn: p.bsn,
        start_date: isoDate(Math.max(yearOf(p.birthDate) + 18, refYear - p.workYears), 1, 1),
        end_date: p.unemployed ? isoDate(refYear, 1, 15) : null,
        uren_per_week: p.unemployed ? 0 : p.hours,
      });
    }
    T('UWV', 'ziektewet', { bsn: p.bsn, heeft_ziektewet_uitkering: false });
    T('UWV', 'WIA', { bsn: p.bsn, heeft_wia_uitkering: false });

    T('RVZ', 'verzekeringen', { bsn: p.bsn, polis_status: p.detained ? 'INACTIEF' : 'ACTIEF', verdrag_status: 'GEEN', zorg_type: 'BASIS' });
    T('DUO', 'inschrijvingen', { bsn: p.bsn, onderwijssoort: p.isStudent ? rng.pick(['HBO', 'WO']) : 'GEEN', onderwijstype: p.isStudent ? rng.pick(['HBO', 'WO']) : 'GEEN' });
    T('DUO', 'studiefinanciering', { bsn: p.bsn, ontvangt_studiefinanciering: p.isStudent, aantal_studerend_gezin: p.isStudent ? rng.int(0, 2) : 0 });
    T('DUO', 'is_student', { bsn: p.bsn, waarde: p.isStudent });
    T('DUO', 'receives_study_grant', { bsn: p.bsn, waarde: p.isStudent });
    T('DJI', 'detenties', { bsn: p.bsn, is_gedetineerd: p.detained, status: p.detained ? 'GEDETINEERD' : 'VRIJ', inrichting_type: p.detained ? 'REGULIER' : 'GEEN' });
    T('DJI', 'is_detainee', { bsn: p.bsn, waarde: p.detained });
    T('DJI', 'detentie', { bsn: p.bsn, is_gedetineerd: p.detained });
    T('DJI', 'forensische_zorg', { bsn: p.bsn, zorgtype: null, juridische_titel: null });
    T('GEMEENTE_AMSTERDAM', 'werk_en_re_integratie', {
      bsn: p.bsn,
      arbeidsvermogen: rng.weighted(['VOLLEDIG', 'GEDEELTELIJK', 'MEDISCH_VOLLEDIG', 'GEEN'], [0.8, 0.1, 0.05, 0.05]),
      re_integratie_traject: rng.pick(['Werkstage', 'Ondernemerscoaching', 'Zelfstandigentraject', 'Geen']),
    });
    T('SVB', 'retirement_age', { bsn: p.bsn, leeftijd: 67 });
    T('SVB', 'algemene_ouderdomswet_gegevens', { bsn: p.bsn, pensioenleeftijd: 67 });
    T('SVB', 'algemene_kinderbijslagwet', {
      ouder_bsn: p.bsn,
      aantal_kinderen: p.children.length,
      kinderen_leeftijden: p.children.map((c) => c.age),
      ontvangt_kinderbijslag: p.children.some((c) => c.age < 18),
    });
    T('SVB', 'verzekerde_tijdvakken', { bsn: p.bsn, woonperiodes: p.residenceYears });
    T('IND', 'verblijfsvergunningen', { bsn: p.bsn, type: p.dutch ? 'NEDERLANDS' : 'PERMANENT', status: 'VERLEEND' });
    T('IND', 'residence_permit_type', { bsn: p.bsn, type: p.dutch ? 'NEDERLANDS' : 'PERMANENT' });
    T('IND', 'vreemdelingenwet', { bsn: p.bsn, verblijfsvergunning_type: p.dutch ? null : 'PERMANENT' });
    if (p.selfEmployed) {
      // Derived from the citizen's own BSN, so every self-employed citizen has
      // their own registration (the BSN is unique, `9995` plus five digits).
      const kvk = `7${p.bsn.slice(-7)}`;
      T('KVK', 'inschrijvingen', { bsn: p.bsn, kvk_nummer: kvk, handelsnaam: `Bedrijf ${p.bsn}`, rechtsvorm: 'EENMANSZAAK', status: 'ACTIEF', activiteit: rng.pick(['Thuiszorg', 'Webdesign', 'Consultancy', 'Bouw']) });
      T('KVK', 'functionarissen', { bsn: p.bsn, kvk_nummer: kvk, handelsnaam: `Bedrijf ${p.bsn}`, rechtsvorm: 'EENMANSZAAK', status: 'ACTIEF', functie: 'EIGENAAR', bevoegdheid: 'VOLLEDIG' });
      T('KVK', 'is_entrepreneur', { bsn: p.bsn, waarde: true });
      T('SZW', 'bbz_aanvraag', { bsn: p.bsn, type_zelfstandige: 'GEVESTIGD', bedrijf_levensvatbaar: rng.chance(0.7), jaren_ondernemerschap: rng.int(1, 10), uren_per_week: Math.max(p.hours, 24), beeindigingsdatum: null });
    } else {
      T('KVK', 'is_entrepreneur', { bsn: p.bsn, waarde: false });
    }

    // Inputs only the citizen supplies (`kind: claim` in the bindings): the
    // simulation fills them in as if every renter and every working parent
    // had answered the form.
    if (p.rents) {
      claims.push({ lawId: 'wet_op_de_huurtoeslag', keyField: 'bsn', keyValue: p.bsn, input: 'huurprijs', value: p.rent });
      claims.push({ lawId: 'wet_op_de_huurtoeslag', keyField: 'bsn', keyValue: p.bsn, input: 'servicekosten', value: p.serviceCosts });
      claims.push({ lawId: 'wet_op_de_huurtoeslag', keyField: 'bsn', keyValue: p.bsn, input: 'subsidiabele_servicekosten', value: p.eligibleServiceCosts });
    }
    if (p.childcare) {
      claims.push({ lawId: 'wet_kinderopvang', keyField: 'bsn', keyValue: p.bsn, input: 'kinderopvang_kvk', value: '87654321' });
      claims.push({ lawId: 'wet_kinderopvang', keyField: 'bsn', keyValue: p.bsn, input: 'aangegeven_uren', value: p.childcare.hoursPerYear });
      claims.push({ lawId: 'wet_kinderopvang', keyField: 'bsn', keyValue: p.bsn, input: 'verwachte_partner_uren', value: partner ? partner.hours * 46 : 0 });
    }
  }

  const subjects = people.map((p): CitizenSubject => ({
    id: p.bsn,
    bsn: p.bsn,
    leeftijd: p.age,
    inkomen: p.income / 100,
    vermogen: p.netWorth / 100,
    partner: !!p.partnerBsn,
    kinderen: p.children.length,
    huurder: p.rents,
    huur: p.rents ? p.rent / 100 : 0,
    student: p.isStudent,
    zelfstandig: p.selfEmployed,
    werkloos: p.unemployed,
    gedetineerd: p.detained,
    nederlands: p.dutch,
  }));
  return { subjects, tables: t.tables, claims, keyValues: { bsn: people.map((p) => p.bsn) } };
}

// ---------------------------------------------------------------------------
// Businesses
// ---------------------------------------------------------------------------

/**
 * @returns formValues: per kvk_nummer the application-form parameters a business law
 *   declares (terrace size, whether food is served), taken from the business.
 */
export function generateBusinesses(userParams: Partial<BusinessParams> | null | undefined, referenceDate: string, templateRow: TemplateRow = () => ({})): BusinessPopulation {
  const params: BusinessParams = { ...BUSINESS_DEFAULTS, ...userParams };
  const rng = createRandom(params.seed);
  const count = Math.max(1, Math.min(MAX_POPULATION, Math.floor(Number(params.count) || 1)));
  const t = tableSet();
  const T = (service: string, table: string, row: RegisterRow) => t.add(service, table, { ...templateRow(service, table), ...row });
  const refYear = yearOf(referenceDate);
  const subjects: BusinessSubject[] = [];
  const formValues: Record<string, BusinessFormValues> = {};
  const kvks: string[] = [];
  const bsns: string[] = [];

  for (let i = 1; i <= count; i += 1) {
    const kvk = `8${String(i).padStart(7, '0')}`;
    const bsn = `9997${String(i).padStart(5, '0')}`;
    const horeca = rng.chance(params.horecaPct / 100);
    const type: BusinessType = horeca ? (rng.chance(0.15) ? 'slijtersbedrijf' : 'horecabedrijf') : 'overig';
    const food = horeca ? rng.chance(0.9) : rng.chance(params.foodPct / 100 / 3);
    const sizeClasses: SizeClass[] = ['small', 'medium', 'large'];
    const sizeClass = rng.weighted(sizeClasses, sizeClasses.map((k) => Number(params.sizeDistribution[k]) || 0));
    const floor = sizeClass === 'small' ? rng.int(12, 49) : sizeClass === 'medium' ? rng.int(50, 100) : rng.int(101, 400);
    const electricity = Math.round(Math.min(500000, Math.max(5000, rng.lognormal(10.2, 0.8))));
    const gas = Math.round(Math.min(100000, Math.max(1000, rng.lognormal(9.5, 0.7))));
    const legalForm = rng.weighted(['BV', 'EENMANSZAAK', 'VOF', 'NV', 'STICHTING'], [35, 35, 15, 5, 10]);
    const employees = rng.chance(params.employeesPct / 100) ? (sizeClass === 'large' ? rng.int(10, 120) : sizeClass === 'medium' ? rng.int(2, 15) : rng.int(0, 4)) : 0;
    const terrace = type === 'horecabedrijf' && rng.chance(params.terracePct / 100);
    const terraceArea = terrace ? Math.round(floor * rng.uniform(0.2, 0.6) * 10) / 10 : 0;
    const ownerAge = rng.int(19, 70);
    const curatele = rng.chance(0.01);
    const vog = rng.chance(0.97);
    const svh = horeca ? rng.chance(0.85) : false;
    const incident = rng.chance(food ? 0.08 : 0.01);
    const cbsSelected = rng.chance(0.25);
    const alcohol = horeca && rng.chance(0.7);
    const woonfunctie = rng.chance(0.05);
    const street = rng.pick(ROTTERDAM_STREETS);
    const houseNumber = `${rng.int(1, 300)}${rng.chance(0.3) ? 'A' : ''}`;
    const postcode = `${rng.int(3011, 3089)}${rng.pick(['AB', 'CD', 'GC', 'TL', 'XY'])}`;
    const address = `${street} ${houseNumber}, ${postcode} Rotterdam`;
    const ownerBirth = isoDate(refYear - ownerAge - 1, rng.int(1, 12), rng.int(1, 28));
    kvks.push(kvk);
    bsns.push(bsn);

    // The business in the registers.
    T('KVK', 'organisaties', { kvk_nummer: kvk, rechtsvorm: legalForm, status: 'Actief', aantal_werknemers: employees, datum_telling: isoDate(refYear, 1, 1), datum_aanvang: isoDate(refYear - rng.int(0, 15), rng.int(1, 12), 1), vestigingsadres: address, sbi_code: horeca ? '56102' : food ? '47110' : '62010' });
    T('KVK', 'inschrijvingen', { bsn, bsn_eigenaar: bsn, kvk_nummer: kvk, handelsnaam: `Bedrijf ${i}`, rechtsvorm: legalForm, status: 'Actief', activiteit: horeca ? 'Horeca' : food ? 'Levensmiddelen' : 'Zakelijke dienstverlening', sbi_code: horeca ? '56102' : food ? '47110' : '62010', datum_aanvang: isoDate(refYear - 1, 1, 15), aantal_werknemers: employees });
    T('KVK', 'functionarissen', { bsn, kvk_nummer: kvk, handelsnaam: `Bedrijf ${i}`, rechtsvorm: legalForm, status: 'Actief', functie: 'EIGENAAR', bevoegdheid: 'VOLLEDIG' });
    T('KVK', 'vestigingen', { kvk_nummer: kvk, adres: address });
    T('KVK', 'jaarrekeningen', { kvk_nummer: kvk, rechtsvorm: legalForm, laatste_boekjaar_einde: isoDate(refYear - 1, 12, 31) });
    T('BELASTINGDIENST', 'werkgevers', { kvk_nummer: kvk, heeft_werknemers: employees > 0, totaal_bruto_loon: employees * 3200000 });
    T('CBS', 'cbs_enquetes', { kvk_nummer: kvk, is_geselecteerd_cbs_enquete: cbsSelected });

    // The premises.
    T('KADASTER', 'bag_verblijfsobjecten', { adres: address, gebruiksdoel: horeca ? 'bijeenkomstfunctie' : 'winkelfunctie', oppervlakte: floor, status: 'Actief', bouwjaar: rng.int(1890, 2015) });
    T('GEMEENTE_ROTTERDAM', 'horecagebiedsplannen', { adres: address, categorie: 'licht', gebied: 'Rotterdam West', categorie_toegestaan: rng.chance(0.9), ontwikkelruimte: rng.chance(0.85) });
    T('GEMEENTE_ROTTERDAM', 'omgevingsplan_toetsingen', { adres: address, categorie: 'licht', horeca_toegestaan: rng.chance(0.9) });
    T('GEMEENTE_ROTTERDAM', 'bgt_terraslocaties', { adres: address, locatie: 'voor', beschikbare_oppervlakte: Math.round(rng.uniform(10, 120)), functie_oppervlak: 'voetpad', is_openbare_weg: true });
    // Categorie licht (aangevraagde_categorie): terras tot 23.00 uur. De APV
    // noemt geen terrastijden; het horecabeleid vult art. 2:29 in
    // (Horecagebiedsplannen 2024-2027).
    T('GEMEENTE_ROTTERDAM', 'terrassenbeleid', { adres: address, seizoen: 'jaarrond', gebied: 'Rotterdam West', max_sluitingstijd_doordeweeks: 23, max_sluitingstijd_weekend: 23 });
    T('GEMEENTE_ROTTERDAM', 'precario_tarieven', { adres: address, gebruik: 'terras' });

    // The business at the municipality.
    T('GEMEENTE_ROTTERDAM', 'vestigingen', { kvk_nummer: kvk, adres: address, schenkt_alcohol: alcohol, heeft_terras: terrace });
    // Art. 2:28 lid 5 onder i t/m k: getoetst bij de verlening. Zonder deze
    // velden bleef de exploitatievergunning voor bijna iedereen onbekend.
    T('GEMEENTE_ROTTERDAM', 'vergunningen', { kvk_nummer: kvk, heeft_exploitatievergunning: horeca && rng.chance(0.8), categorie: '1', heeft_alcoholvergunning: alcohol && rng.chance(0.7), kvk_inschrijving_geldig: true, feitelijke_toestand_conform_aanvraag: true, voldoet_aan_horecabeleid: true });
    T('GEMEENTE_ROTTERDAM', 'inrichtingen', { kvk_nummer: kvk, vloeroppervlakte_horecalokaliteit: floor, type_bedrijf: type });
    T('GEMEENTE_ROTTERDAM', 'beheerders', { kvk_nummer: kvk, schenkt_alcohol: alcohol, bsn, heeft_vog: vog, leeftijd: ownerAge, is_onder_curatele: curatele, heeft_svh_diploma: svh, alle_hebben_vog: vog, alle_voldoen_leeftijd: ownerAge >= 21, geen_onder_curatele: !curatele });
    T('GEMEENTE_ROTTERDAM', 'leidinggevenden', { kvk_nummer: kvk, bsn, naam: `Eigenaar ${i}`, leeftijd: ownerAge, is_onder_curatele: curatele, heeft_svh_diploma: svh, is_van_slecht_levensgedrag: false, is_ingeschreven_svh_register: svh, aantal_voldoet_alle_eisen: ownerAge >= 21 && !curatele && svh ? 1 : 0 });
    T('GEMEENTE_ROTTERDAM', 'exploitatie_inschrijvingen', { kvk_nummer: kvk, bsn_eigenaar: bsn, aangevraagde_categorie: 'licht' });
    T('GEMEENTE_ROTTERDAM', 'geluidsklachten', { kvk_nummer: kvk, heeft_actieve_klachten: rng.chance(0.1) });
    T('GEMEENTE_ROTTERDAM', 'vergunningen_historie', { adres: address, bsn, kvk_nummer: kvk, vergunning_type: 'exploitatievergunning', intrekkingsdatum: null, intrekkingsreden: null, ingetrokken_slecht_levensgedrag: false, voorschriften_overtreden: false });
    T('GEMEENTE_ROTTERDAM', 'personen_vog', { bsn, heeft_geldige_vog: vog });

    // Inspectorates and registers.
    T('NVWA', 'nvwa_registratie', { kvk_nummer: kvk, is_levensmiddelenbedrijf: food, is_geregistreerd_nvwa: food && rng.chance(0.9), heeft_haccp_systeem: food && rng.chance(0.8), type_haccp_systeem: 'hygienecode' });
    T('NVWA', 'nvwa_meldplicht', { kvk_nummer: kvk, is_levensmiddelenbedrijf: food, heeft_actief_incident: incident });
    T('RVO', 'rvo_energiegegevens', { kvk_nummer: kvk, elektriciteit_kwh: electricity, gasverbruik_m3: gas, is_woonfunctie: woonfunctie });
    T('RVO', 'wpm_gegevens', { kvk_nummer: kvk, aantal_werknemers: employees, verstrekt_mobiliteitsvergoeding: employees >= 100 && rng.chance(0.6) });
    T('LBB', 'bibob_adviezen', { kvk_nummer: kvk, bsn, advies_uitgebracht: true, mate_van_gevaar: rng.chance(0.95) ? 'geen_gevaar' : 'ernstig_gevaar' });
    T('RECHTSPRAAK', 'curatele', { bsn, is_onder_curatele: curatele, datum_uitspraak: curatele ? isoDate(refYear - 1, 3, 1) : null });
    if (curatele) T('RECHTSPRAAK', 'curatele_registraties', { bsn_curandus: bsn, status: 'ACTIEF', datum_ingang: isoDate(refYear - 1, 3, 1), datum_einde: null });
    T('SVH', 'registraties', { bsn, is_geregistreerd: svh, naam: `Eigenaar ${i}` });

    // The owner as a person.
    T('RvIG', 'personen', { bsn, geboortedatum: ownerBirth, land_verblijf: 'NEDERLAND', nationaliteit: 'NEDERLANDS', age: ownerAge, has_dutch_nationality: true, has_partner: false, residence_address: address, has_fixed_address: true, household_size: 1 });
    T('RvIG', 'relaties', { bsn, partnerschap_type: 'GEEN', partner_bsn: null, has_partner: false, kinderen: [] });
    T('RvIG', 'verblijfplaats', { bsn, straat: street, huisnummer: houseNumber, postcode, woonplaats: 'Rotterdam', type: 'WOONADRES' });
    T('RvIG', 'personen_vog', { bsn, heeft_geldige_vog: vog });

    formValues[kvk] = {
      bsn,
      aangevraagde_categorie: 'licht',
      activiteiten: horeca ? ['eten_en_drinken'] : ['detailhandel'],
      bereidt_of_serveert_voedsel: food,
      // Wie een terras heeft, vraagt er een vergunning voor. Wat de APV
      // toekent, wordt in de runner een verleende zaak (zie caseSourceLaws).
      aanvragen: terrace ? ['algemene_plaatselijke_verordening/terrassen'] : [],
      terras_locatie: 'voor',
      terras_oppervlakte: terraceArea,
      obstakelvrije_ruimte: 1.8,
      seizoen: 'jaarrond',
      gewenste_openingstijd: 8,
      gewenste_sluitingstijd_doordeweeks: 23,
      gewenste_sluitingstijd_weekend: 23,
      activiteitsdatum: isoDate(refYear, 12, 31),
      activiteitsstarttijd: 20,
    };
    subjects.push({
      id: kvk,
      kvk_nummer: kvk,
      type: type,
      voedsel: food,
      oppervlakte: floor,
      grootte: sizeClass,
      rechtsvorm: legalForm,
      werknemers: employees,
      terras: terrace,
      terras_m2: terraceArea,
      elektriciteit_kwh: electricity,
      gas_m3: gas,
      alcohol,
      leeftijd_eigenaar: ownerAge,
    });
  }
  return { subjects, tables: t.tables, claims: [], keyValues: { kvk_nummer: kvks, bsn: bsns }, formValues };
}

/** Merge the demo's shared tables (CBS, KIESRAAD, JenV) with generated ones into a `rowsFor`. */
export function rowsForTables(...tableSets: Array<TablesByService | null | undefined>): RowsFor {
  return (service, table) => {
    const out: RegisterRow[] = [];
    for (const set of tableSets) {
      const rows = set?.[service]?.[table];
      if (Array.isArray(rows)) out.push(...rows);
    }
    return out;
  };
}
