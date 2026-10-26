import { describe, expect, it } from 'vitest';
import { createRandom } from './random';
import { BUSINESS_DEFAULTS, CITIZEN_DEFAULTS, generateBusinesses, generateCitizens, rowsForTables, templatesFromProfiles } from './population';
import type { Population, RegisterRow } from './population';

/** The rows of one generated table; throws when the population has none. */
function rowsOf(pop: Population, service: string, table: string): RegisterRow[] {
  const rows = pop.tables[service]?.[table];
  if (!rows) throw new Error(`no table ${service}.${table}`);
  return rows;
}

/** The first row that matches; throws when there is none. */
function rowWhere(rows: RegisterRow[], pred: (row: RegisterRow) => boolean): RegisterRow {
  const row = rows.find(pred);
  if (!row) throw new Error('no matching row');
  return row;
}

/** A column that must hold a number. */
function num(value: unknown): number {
  if (typeof value !== 'number') throw new Error(`not a number: ${String(value)}`);
  return value;
}

const REF = '2025-06-01';

describe('createRandom', () => {
  it('is deterministic per seed and uniform in [0,1)', () => {
    const a = createRandom(7);
    const b = createRandom(7);
    const xs = Array.from({ length: 1000 }, () => a.next());
    expect(xs.map(() => b.next())).toEqual(xs);
    expect(Math.min(...xs)).toBeGreaterThanOrEqual(0);
    expect(Math.max(...xs)).toBeLessThan(1);
    const m = xs.reduce((s, v) => s + v, 0) / xs.length;
    expect(m).toBeGreaterThan(0.4);
    expect(m).toBeLessThan(0.6);
  });

  it('weighted() respects zero weights', () => {
    const rng = createRandom(1);
    for (let i = 0; i < 50; i += 1) expect(rng.weighted(['a', 'b', 'c'], [0, 1, 0])).toBe('b');
  });
});

describe('generateCitizens', () => {
  const pop = generateCitizens({ ...CITIZEN_DEFAULTS, count: 40, seed: 3 }, REF);

  it('produces the requested number of adults with unique test-range BSNs', () => {
    expect(pop.subjects).toHaveLength(40);
    const bsns = pop.subjects.map((s) => s.bsn);
    expect(new Set(bsns).size).toBe(40);
    for (const bsn of bsns) expect(bsn).toMatch(/^999\d{6}$/);
    for (const s of pop.subjects) expect(s.leeftijd).toBeGreaterThanOrEqual(18);
  });

  it('is reproducible for the same seed and differs for another', () => {
    const again = generateCitizens({ ...CITIZEN_DEFAULTS, count: 40, seed: 3 }, REF);
    expect(again.subjects).toEqual(pop.subjects);
    const other = generateCitizens({ ...CITIZEN_DEFAULTS, count: 40, seed: 4 }, REF);
    expect(other.subjects).not.toEqual(pop.subjects);
  });

  it('writes one row per person in the core registers, consistent with the subject', () => {
    const personen = rowsOf(pop, 'RvIG', 'personen');
    expect(personen).toHaveLength(40);
    const box1 = rowsOf(pop, 'BELASTINGDIENST', 'box1');
    for (const s of pop.subjects) {
      const row = rowWhere(box1, (r) => r.bsn === s.bsn);
      const total = num(row.loon_uit_dienstbetrekking) + num(row.winst_uit_onderneming) + num(row.uitkeringen_en_pensioenen);
      expect(total).toBe(Math.round(s.inkomen * 100));
    }
    expect(pop.keyValues.bsn).toEqual(pop.subjects.map((s) => s.bsn));
  });

  it('gives every self-employed citizen their own KVK registration', () => {
    const rows = rowsOf(pop, 'KVK', 'inschrijvingen');
    // 40 adults at a 12% chance: the seed has to produce several to prove anything.
    expect(rows.length).toBeGreaterThan(1);
    const numbers = rows.map((r) => r.kvk_nummer);
    expect(new Set(numbers).size).toBe(rows.length);
    for (const r of rows) expect(r.kvk_nummer).toMatch(/^7\d{7}$/);
  });

  it('pairs partners symmetrically and lists children on both sides', () => {
    const relaties = rowsOf(pop, 'RvIG', 'relaties');
    for (const r of relaties) {
      if (!r.partner_bsn) continue;
      const other = rowWhere(relaties, (x) => x.bsn === r.partner_bsn);
      expect(other.partner_bsn).toBe(r.bsn);
    }
    const parents = pop.subjects.filter((s) => s.kinderen > 0);
    for (const p of parents) {
      const kb = rowWhere(rowsOf(pop, 'SVB', 'algemene_kinderbijslagwet'), (r) => r.ouder_bsn === p.bsn);
      expect(kb.aantal_kinderen).toBe(p.kinderen);
    }
  });

  it('turns rent into huurtoeslag form answers for renters only', () => {
    const renters = pop.subjects.filter((s) => s.huurder);
    const rentClaims = pop.claims.filter((c) => c.lawId === 'wet_op_de_huurtoeslag' && c.input === 'huurprijs');
    expect(rentClaims).toHaveLength(renters.length);
    for (const c of rentClaims) expect(c.value).toBeGreaterThanOrEqual(CITIZEN_DEFAULTS.rentRanges.low[0] * 100);
  });

  it('copies unmodelled columns from a persona template row', () => {
    const templateRow = templatesFromProfiles({
      profiles: { '1': { sources: { RVZ: { verzekeringen: [{ bsn: '1', polis_status: 'ACTIEF', verdrag_status: 'GEEN', extra_kolom: 'x' }] } } } },
    });
    const withTemplate = generateCitizens({ count: 3, seed: 1 }, REF, templateRow);
    const [first] = rowsOf(withTemplate, 'RVZ', 'verzekeringen');
    if (!first) throw new Error('no verzekeringen row');
    expect(first.extra_kolom).toBe('x');
    expect(first.bsn).not.toBe('1');
  });

  it('honours the demographic knobs', () => {
    const old = generateCitizens({ count: 60, seed: 9, ageDistribution: { '18-30': 0, '30-45': 0, '45-67': 0, '67-85': 100, '85+': 0 } }, REF);
    for (const s of old.subjects) expect(s.leeftijd).toBeGreaterThanOrEqual(66);
    const owners = generateCitizens({ count: 60, seed: 9, renterPct: 0, ageDistribution: { '18-30': 0, '30-45': 0, '45-67': 100, '67-85': 0, '85+': 0 }, incomeDistribution: { low: 0, middle: 0, high: 100 } }, REF);
    expect(owners.subjects.filter((s) => s.huurder).length).toBeLessThan(owners.subjects.length / 2);
  });
});

describe('generateBusinesses', () => {
  const pop = generateBusinesses({ ...BUSINESS_DEFAULTS, count: 30, seed: 5 }, REF);

  it('produces businesses keyed on kvk with an owner keyed on bsn', () => {
    expect(pop.subjects).toHaveLength(30);
    expect(pop.keyValues.kvk_nummer).toHaveLength(30);
    expect(pop.keyValues.bsn).toHaveLength(30);
    for (const s of pop.subjects) {
      expect(rowsOf(pop, 'KVK', 'organisaties').find((r) => r.kvk_nummer === s.kvk_nummer)).toBeTruthy();
      const form = pop.formValues[s.kvk_nummer];
      if (!form) throw new Error(`no form values for ${s.kvk_nummer}`);
      expect(form.bereidt_of_serveert_voedsel).toBe(s.voedsel);
      expect(form.terras_oppervlakte).toBe(s.terras_m2);
    }
  });

  it('keeps the premises consistent between municipality and cadastre', () => {
    const vestigingen = rowsOf(pop, 'GEMEENTE_ROTTERDAM', 'vestigingen');
    for (const v of vestigingen) {
      expect(rowsOf(pop, 'KADASTER', 'bag_verblijfsobjecten').some((b) => b.adres === v.adres)).toBe(true);
      expect(rowsOf(pop, 'GEMEENTE_ROTTERDAM', 'terrassenbeleid').some((b) => b.adres === v.adres)).toBe(true);
    }
    for (const s of pop.subjects) if (s.terras) expect(s.type).toBe('horecabedrijf');
    for (const b of rowsOf(pop, 'GEMEENTE_ROTTERDAM', 'terrassenbeleid')) expect(b).toMatchObject({ max_sluitingstijd_doordeweeks: 23, max_sluitingstijd_weekend: 23 });
  });

  it('follows the type and size knobs', () => {
    const horeca = generateBusinesses({ count: 40, seed: 2, horecaPct: 100, sizeDistribution: { small: 0, medium: 0, large: 100 } }, REF);
    for (const s of horeca.subjects) {
      expect(s.type).not.toBe('overig');
      expect(s.oppervlakte).toBeGreaterThan(100);
    }
  });
});

describe('rowsForTables', () => {
  it('concatenates rows across table sets and returns [] for unknown tables', () => {
    const rowsFor = rowsForTables({ CBS: { levensverwachting: [{ jaar: 2025 }] } }, { CBS: { levensverwachting: [{ jaar: 2026 }] }, X: { y: [{}] } });
    expect(rowsFor('CBS', 'levensverwachting')).toHaveLength(2);
    expect(rowsFor('X', 'y')).toHaveLength(1);
    expect(rowsFor('Q', 'z')).toEqual([]);
  });
});
