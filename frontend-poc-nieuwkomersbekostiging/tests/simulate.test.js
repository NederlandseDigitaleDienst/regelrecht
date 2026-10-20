import { describe, it, expect } from 'vitest';
import { simulate, evaluateLeerlingTimeline } from '../src/sim/simulate.js';
import { aggregate, countAanleidingen, terugverdientijd } from '../src/sim/metrics.js';
import { generatePopulation } from '../src/sim/population.js';
import { peildataTussen } from '../src/lib/nieuwkomerFacts.js';
import {
  createStubEngine,
  BEDRAG_ASIEL_JAAR,
  BEDRAG_OVERIG_JAAR,
  TOESLAG_EERSTE_KEER,
  VO_EERSTE_KWARTAAL,
  STUB_DISTRIBUTIONS,
} from './stubEngine.js';

const basis = {
  is_vreemdeling: true,
  heeft_bsn: true,
  woonachtig_in_nederland: true,
  werkelijk_schoolgaand: true,
  internationaal_georienteerd: false,
  in_telling_1_februari: false,
  oordeel_bevoegd_gezag: null,
  gewicht: 1,
};

function po(bsn, school, extra) {
  return { ...basis, bsn, sector: 'po', schoolsoort: 'basisschool', school_id: school, ...extra };
}
function vo(bsn, school, extra) {
  return { ...basis, bsn, sector: 'vo', schoolsoort: 'vo', school_id: school, ...extra };
}

/** C1 uit de visual: asielzoeker, ouder dan vier, 8 kwartalen vanaf 1-7-2024. */
const C1 = po('C1', '05AB', {
  geboortedatum: '2015-03-10', verblijfstitel_code: 26,
  datum_vestiging_nederland: '2024-02-01', eerste_inschrijfdatum: '2024-05-01',
});
/** C3: asielzoeker, geboren 2020-09-01, twee kwartalen aftrek → 6 kwartalen. */
const C3 = po('C3', '05AB', {
  geboortedatum: '2020-09-01', verblijfstitel_code: 27,
  datum_vestiging_nederland: '2024-02-01', eerste_inschrijfdatum: '2024-09-01',
});

const HANDELINGEN = {
  tarieven: { schooladministratie: 4500, duo_medewerker: 6500 },
  handelingen: [
    { id: 'bestand', partij: 'school', sector: 'po', aanleiding: 'per_school_peildatum', minuten: 60, tarief: 'schooladministratie' },
    { id: 'aanvraag', partij: 'school', sector: 'po', aanleiding: 'per_aanvraag', minuten: 30, tarief: 'schooladministratie' },
    { id: 'beoordelen', partij: 'duo', sector: 'po', aanleiding: 'per_aanvraag', minuten: 20, tarief: 'duo_medewerker' },
    { id: 'bezwaar', partij: 'duo', sector: 'po', aanleiding: 'per_bezwaar', minuten: 120, tarief: 'duo_medewerker' },
    { id: 'ambigu', partij: 'school', sector: 'po', aanleiding: 'per_ambigu_leerling', minuten: 10, tarief: 'schooladministratie' },
  ],
  fracties: { bezwaar_per_aanvraag: 0.5 },
  investeringen: [{ id: 'it', partij: 'duo', bedrag: 100000, afschrijving_jaren: 2, vanaf_jaar: 2026 }],
  budget: { regeling_po: 10_000_000, regeling_vo: 10_000_000, uitvoering: 1 },
};

/**
 * De kwartalen en de aftrek zelf rekent de stub-engine na; dat zij kloppen
 * met de regeling staat tegen de echte engine in nieuwkomers_po.feature
 * (casus C1 en C3). Wat hier getest wordt is de tijdlijn eromheen:
 * evaluateLeerlingTimeline loopt elke peildatum af en neemt de engine-velden
 * per peildatum over.
 */
describe('persona-tijdlijn', () => {
  it('loopt de peildata af en neemt per peildatum telt, jaar en bedrag over (C1)', () => {
    const engine = createStubEngine();
    const tl = evaluateLeerlingTimeline(engine, C1, peildataTussen('2024-04-01', '2026-10-01'));
    const tellend = tl.filter((t) => t.telt).map((t) => t.peildatum);
    expect(tellend).toEqual([
      '2024-07-01', '2024-10-01', '2025-01-01', '2025-04-01',
      '2025-07-01', '2025-10-01', '2026-01-01', '2026-04-01',
    ]);
    expect(tl.filter((t) => t.bekostigingsjaar === 1)).toHaveLength(4);
    expect(tl.filter((t) => t.bekostigingsjaar === 2)).toHaveLength(4);
    expect(tl.find((t) => t.peildatum === '2024-07-01').bedrag).toBe(Math.round(BEDRAG_ASIEL_JAAR / 4));
  });

  it('neemt de aftrek van de engine over en telt pas vanaf de eerste tellende peildatum (C3)', () => {
    const engine = createStubEngine();
    const tl = evaluateLeerlingTimeline(engine, C3, peildataTussen('2024-07-01', '2026-10-01'));
    expect(tl[0].aftrek_kwartalen).toBe(2);
    expect(tl[0].bekostigbare_kwartalen).toBe(6);
    expect(tl.filter((t) => t.telt)).toHaveLength(6);
    expect(tl.find((t) => t.telt).peildatum).toBe('2024-10-01');
  });
});

describe('simulate + aggregate', () => {
  it('past de drempel toe op schoolniveau en telt de aanvraag-handelingen', () => {
    const engine = createStubEngine();
    // School A: drie eerstejaars (onder de drempel). School B: vijf, eerste keer.
    const records = [
      ...['a1', 'a2', 'a3'].map((b) => po(b, 'A', {
        geboortedatum: '2016-01-01', verblijfstitel_code: 26,
        datum_vestiging_nederland: '2025-01-15', eerste_inschrijfdatum: '2025-03-01',
      })),
      ...['b1', 'b2', 'b3', 'b4'].map((b) => po(b, 'B', {
        geboortedatum: '2016-01-01', verblijfstitel_code: 26,
        datum_vestiging_nederland: '2025-01-15', eerste_inschrijfdatum: '2025-03-01',
      })),
      po('b5', 'B', {
        geboortedatum: '2016-01-01', verblijfstitel_code: 22,
        datum_vestiging_nederland: '2025-01-15', eerste_inschrijfdatum: '2025-03-01',
      }),
    ];
    const sim = simulate(engine, records, { jaren: [2025, 2026] });
    expect(sim.fouten.aantal).toBe(0);
    expect(sim.scholen).toHaveLength(2);

    const A = sim.scholen.find((s) => s.school_id === 'A');
    const B = sim.scholen.find((s) => s.school_id === 'B');
    const A_apr = A.perPeildatum.find((p) => p.peildatum === '2025-04-01');
    const B_apr = B.perPeildatum.find((p) => p.peildatum === '2025-04-01');
    expect(A_apr.Ap).toBe(3);
    expect(A_apr.voldoet_aan_drempel).toBe(false);
    expect(A_apr.bedrag_totaal).toBe(0);
    expect(A_apr.aanvraag).toBe(false);
    expect(B_apr.Ap).toBe(4);
    expect(B_apr.Vp).toBe(1);
    expect(B_apr.voldoet_aan_drempel).toBe(true);
    expect(B_apr.eerste_keer).toBe(true);
    expect(B_apr.bedrag_eerste_opvang).toBe(Math.round((4 * BEDRAG_ASIEL_JAAR + BEDRAG_OVERIG_JAAR) / 4));
    expect(B_apr.toeslag_eerste_keer).toBe(TOESLAG_EERSTE_KEER);
    expect(B_apr.aanvraag).toBe(true);
    // De toeslag is eenmalig.
    expect(B.perPeildatum.filter((p) => p.toeslag_eerste_keer > 0)).toHaveLength(1);

    const counts = countAanleidingen(sim);
    // B vraagt aan op elke peildatum dat er iets telt: 4 eerstejaars-kwartalen
    // in 2025 (apr..okt = 3) + 2026 (jan = 1), daarna tweedejaars (art. 35).
    expect(counts.po.per_aanvraag[2025]).toBe(3);
    expect(counts.po.per_aanvraag_1_januari[2026]).toBe(1);
    expect(counts.po.per_school_peildatum[2025]).toBe(6); // A en B, drie peildata elk

    const m = aggregate(sim, HANDELINGEN);
    const y = m.perJaar[2025];
    expect(y.regeling_po.totaal).toBeGreaterThan(0);
    expect(y.regeling_vo.totaal).toBe(0);
    expect(y.handelingen.bestand.aantal).toBe(6);
    expect(y.handelingen.bestand.kosten).toBeCloseTo(6 * 1 * 4500, 6);
    expect(y.handelingen.aanvraag.aantal).toBe(3);
    expect(y.handelingen.beoordelen.kosten).toBeCloseTo(3 * (20 / 60) * 6500, 6);
    expect(y.handelingen.bezwaar.aantal).toBeCloseTo(1.5, 6);
    expect(y.uitvoeringslast.school).toBeCloseTo(y.handelingen.bestand.kosten + y.handelingen.aanvraag.kosten, 6);
    expect(y.uitvoeringslast.duo).toBeCloseTo(y.handelingen.beoordelen.kosten + y.handelingen.bezwaar.kosten, 6);
    expect(y.investering).toBe(0);
    expect(m.perJaar[2026].investering).toBe(50000);
    expect(y.binnen_budget.regeling_po).toBe(true);
    expect(y.binnen_budget.uitvoering).toBe(false);
    expect(m.totaal.binnen_budget.uitvoering).toBe(false);
    expect(y.scholen_onder_drempel).toBe(1);
    expect(y.leerlingen_bekostigd.po).toBe(5);
    expect(y.leerlingen_onder_drempel).toBe(3);
    expect(m.totaal.regeling_po.totaal).toBeCloseTo(y.regeling_po.totaal + m.perJaar[2026].regeling_po.totaal, 6);
  });

  it('vo: ambtshalve, geen aanvraag, eerste categorie tot de teldatum', () => {
    const engine = createStubEngine();
    const records = Array.from({ length: 12 }, (_, i) => vo(`v${i}`, 'V1', {
      geboortedatum: '2011-05-05', verblijfstitel_code: 26,
      datum_vestiging_nederland: '2025-02-01', eerste_inschrijfdatum: '2025-03-01',
    }));
    const sim = simulate(engine, records, { jaren: [2025, 2026] });
    const V = sim.scholen[0];
    const apr = V.perPeildatum.find((p) => p.peildatum === '2025-04-01');
    expect(apr.nieuwkomers_vo).toBe(12);
    expect(apr.eerste_cat_vo).toBe(12);
    expect(apr.bedrag_leerlingen_vo).toBe(12 * VO_EERSTE_KWARTAAL);
    expect(apr.voorbereidingskosten).toBeGreaterThan(0);
    expect(apr.aanvraag).toBe(false);
    expect(apr.aanvraag_voorbereidingskosten).toBe(true);
    const jan26 = V.perPeildatum.find((p) => p.peildatum === '2026-01-01');
    expect(jan26.tweede_cat_vo).toBe(12);
    const m = aggregate(sim, HANDELINGEN);
    expect(m.perJaar[2025].regeling_vo.per_categorie.EERSTE).toBe(3 * 12 * VO_EERSTE_KWARTAAL);
    expect(m.perJaar[2025].handelingen.bestand.aantal).toBe(0);
    expect(m.aantallen.vo.per_aanvraag_voorbereidingskosten[2025]).toBe(1);
  });

  it('draait een synthetische populatie gewogen door en berekent de terugverdientijd', () => {
    const engine = createStubEngine();
    const records = generatePopulation(STUB_DISTRIBUTIONS, 200, 42);
    const sim = simulate(engine, records, { jaren: [2025, 2026] });
    expect(sim.fouten.aantal).toBe(0);
    const ist = aggregate(sim, HANDELINGEN);
    expect(ist.perJaar[2025].regeling_po.totaal).toBeGreaterThan(0);
    expect(ist.perJaar[2025].regeling_vo.totaal).toBeGreaterThan(0);
    expect(ist.perJaar[2025].leerlingen_bekostigd.totaal).toBeGreaterThan(1000);
    expect(ist.stabiliteit.totaal).toBeGreaterThan(0);

    // Variant: aanvraag-stappen vervallen (alsof aanvraag_vereist false is), investering blijft.
    const zonderAanvraag = {
      ...HANDELINGEN,
      handelingen: HANDELINGEN.handelingen.filter((h) => !h.aanleiding.startsWith('per_aanvraag') && h.aanleiding !== 'per_bezwaar'),
    };
    const variant = aggregate(sim, zonderAanvraag);
    expect(variant.totaal.uitvoeringslast.totaal).toBeLessThan(ist.totaal.uitvoeringslast.totaal);
    const tvt = terugverdientijd(variant, ist);
    expect(tvt).toBeGreaterThan(0);
    expect(terugverdientijd(ist, ist)).toBeNull();
    expect(terugverdientijd({ ...ist, investering_totaal: 0 }, ist)).toBe(0);
  });
});
