import { describe, it, expect } from 'vitest';
import { generatePopulation } from './population.js';
import { STUB_DISTRIBUTIONS } from '../test-utils/stubEngine.js';

const VELDEN = [
  'bsn', 'geboortedatum', 'is_vreemdeling', 'verblijfstitel_code', 'heeft_bsn',
  'datum_vestiging_nederland', 'eerste_inschrijfdatum', 'woonachtig_in_nederland',
  'werkelijk_schoolgaand', 'sector', 'schoolsoort', 'internationaal_georienteerd',
  'school_id', 'in_telling_1_februari', 'oordeel_bevoegd_gezag', 'gewicht',
];

describe('populatie', () => {
  it('is deterministisch en volgt het recordschema', () => {
    const a = generatePopulation(STUB_DISTRIBUTIONS, 300, 7);
    const b = generatePopulation(STUB_DISTRIBUTIONS, 300, 7);
    expect(a).toEqual(b);
    expect(a).toHaveLength(300);
    for (const veld of VELDEN) expect(a[0]).toHaveProperty(veld);
    expect(generatePopulation(STUB_DISTRIBUTIONS, 300, 8)).not.toEqual(a);
  });

  it('schaalt het scholenbestand op de instroom per jaar, zodat de drempel bijt', () => {
    const n = 2000;
    const pop = generatePopulation(STUB_DISTRIBUTIONS, n, 42);

    // 6 instroomjaren, po-instroom gemiddeld ca. 11.250 per jaar; 3000 scholen
    // x (records po / instroom per jaar) geeft enkele honderden scholen, niet
    // enkele tientallen. Schalen tegen de instroom over alle jaren zou het
    // aantal door zes delen en elke school onbedoeld vol stoppen.
    const perSchool = new Map();
    for (const r of pop) perSchool.set(r.school_id, (perSchool.get(r.school_id) ?? 0) + 1);
    expect(perSchool.size).toBeGreaterThan(100);

    // Met een lange staart hoort een flink deel van de scholen onder de
    // drempel van vier te blijven (art. 34 lid 2); anders is de drempel dood.
    const onder = [...perSchool.values()].filter((aantal) => aantal < 4).length;
    expect(onder / perSchool.size).toBeGreaterThan(0.2);
  });

  it('weegt naar de totale instroom en verdeelt over sectoren naar rato', () => {
    const n = 2000;
    const pop = generatePopulation(STUB_DISTRIBUTIONS, n, 42);
    const totaal = 12000 + 10000 + 11000 + 9000 + 11500 + 9500 + 3 * (11000 + 9000);
    expect(pop[0].gewicht).toBeCloseTo(totaal / n, 6);
    const po = pop.filter((r) => r.sector === 'po').length / n;
    expect(po).toBeGreaterThan(0.5);
    expect(po).toBeLessThan(0.6);
  });

  it('zet de datums in de juiste volgorde: geboorte < vestiging ≤ eerste inschrijving', () => {
    const pop = generatePopulation(STUB_DISTRIBUTIONS, 500, 3);
    for (const r of pop) {
      expect(r.geboortedatum < r.datum_vestiging_nederland).toBe(true);
      expect(r.datum_vestiging_nederland <= r.eerste_inschrijfdatum).toBe(true);
      expect(r.eerste_inschrijfdatum.slice(0, 4) >= '2023').toBe(true);
    }
  });

  it('koppelt BSN, code en oordeel aan de categorie', () => {
    const pop = generatePopulation(STUB_DISTRIBUTIONS, 2000, 11);
    const zonderBsn = pop.filter((r) => !r.heeft_bsn);
    expect(zonderBsn.length).toBeGreaterThan(30);
    for (const r of zonderBsn) {
      expect(r.verblijfstitel_code).toBeNull();
      expect(['ASIELZOEKER', 'OVERIGE_VREEMDELING']).toContain(r.oordeel_bevoegd_gezag);
      expect(r.bsn.startsWith('ON')).toBe(true);
    }
    const ambigu = pop.filter((r) => r.heeft_bsn && [21, 33, 34, 98].includes(r.verblijfstitel_code));
    expect(ambigu.length).toBeGreaterThan(30);
    const eenduidig = pop.filter((r) => r.heeft_bsn && ![21, 33, 34, 98].includes(r.verblijfstitel_code));
    for (const r of eenduidig) expect(r.oordeel_bevoegd_gezag).toBeNull();
    // At alleen in het po, bij asielzoekers.
    for (const r of pop.filter((r) => r.in_telling_1_februari)) expect(r.sector).toBe('po');
  });

  it('geeft de nieuwkomers per school een lange staart', () => {
    const pop = generatePopulation(STUB_DISTRIBUTIONS, 3000, 5);
    const perSchool = new Map();
    for (const r of pop.filter((r) => r.sector === 'po')) {
      perSchool.set(r.school_id, (perSchool.get(r.school_id) ?? 0) + 1);
    }
    const counts = [...perSchool.values()].sort((a, b) => a - b);
    const mediaan = counts[Math.floor(counts.length / 2)];
    const max = counts[counts.length - 1];
    const gemiddelde = counts.reduce((a, b) => a + b, 0) / counts.length;
    // Lange staart: de grootste school heeft veel meer dan de mediaan, en de
    // mediaan ligt onder het gemiddelde (rechtsscheef).
    expect(max).toBeGreaterThan(5 * mediaan);
    expect(mediaan).toBeLessThan(gemiddelde);
    // Het aantal gesimuleerde scholen schaalt mee met n / echte leerlingen.
    expect(perSchool.size).toBeGreaterThan(50);
    expect(perSchool.size).toBeLessThan(3000);
  });
});
