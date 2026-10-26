/**
 * Wanneer een lopende aanvraag niet meer klopt met wat de wet nu zegt.
 *
 * De vergelijking is wetonafhankelijk: ze kent geen enkele regeling bij naam en
 * werkt op de uitkomsten die de wet zelf declareert. Dat is wat hier bewezen
 * wordt, met namen die net zo goed van een andere wet konden zijn.
 */
import { describe, expect, it } from 'vitest';
import { driftOf, resultsMatch, valuesMatch } from './caseDrift';

const decided = (claimedResult: Record<string, unknown>) => ({ id: 'zaak-1', status: 'DECIDED', claimedResult });
const evaluated = (outputs: Record<string, unknown>) => ({ ok: true, outputs });

describe('valuesMatch', () => {
  it('laat een cent verschil op een bedrag lopen', () => {
    // Een herberekening op een andere dag schuift een cent; dat heropent geen besluit.
    expect(valuesMatch(160825, 160826)).toBe(true);
  });

  it('ziet een verschil dat er werkelijk toe doet', () => {
    expect(valuesMatch(160825, 143200)).toBe(false);
  });

  it('houdt nul apart: alleen nul is nul', () => {
    expect(valuesMatch(0, 0)).toBe(true);
    expect(valuesMatch(0, 100)).toBe(false);
    expect(valuesMatch(100, 0)).toBe(false);
  });

  it('vergelijkt niet-getallen exact', () => {
    expect(valuesMatch(true, false)).toBe(false);
    expect(valuesMatch('WEEK', 'WEEK')).toBe(true);
    expect(valuesMatch([1, 2], [1, 2])).toBe(true);
  });
});

describe('resultsMatch', () => {
  it('vindt een uitkomst die er bij is gekomen of weggevallen', () => {
    expect(resultsMatch({ a: 1 }, { a: 1, b: 2 })).toBe(false);
  });

  it('is tevreden als alles gelijk is', () => {
    expect(resultsMatch({ a: 1, b: true }, { a: 1, b: true })).toBe(true);
  });
});

describe('driftOf', () => {
  it('meldt niets als de wet nog op hetzelfde uitkomt', () => {
    expect(driftOf(decided({ hoogte_toeslag: 160825 }), evaluated({ hoogte_toeslag: 160825 }))).toBeNull();
  });

  it('meldt het verschil als de uitkomst is veranderd', () => {
    const drift = driftOf(decided({ hoogte_toeslag: 160825 }), evaluated({ hoogte_toeslag: 143200 }));
    expect(drift?.changed).toEqual([{ name: 'hoogte_toeslag', claimed: 160825, current: 143200 }]);
  });

  it('werkt op elke wet, want het kent er geen een bij naam', () => {
    // Dezelfde vergelijking, andere regeling, ander soort uitkomst.
    const drift = driftOf(decided({ pensioenbedrag: 120000 }), evaluated({ pensioenbedrag: 0 }));
    expect(drift?.changed[0]?.name).toBe('pensioenbedrag');
    const ja_nee = driftOf(decided({ heeft_stemrecht: true }), evaluated({ heeft_stemrecht: false }));
    expect(ja_nee?.changed[0]).toEqual({ name: 'heeft_stemrecht', claimed: true, current: false });
  });

  it('zet het bedrag vooraan, want daar ging het besluit over', () => {
    const drift = driftOf(
      decided({ voldoet_aan_voorwaarden: true, subsidiebedrag: 30296 }),
      evaluated({ voldoet_aan_voorwaarden: false, subsidiebedrag: 0 }),
    );
    expect(drift?.changed.map((d) => d.name)).toEqual(['subsidiebedrag', 'voldoet_aan_voorwaarden']);
  });

  it('zwijgt over een uitkomst die de wet niet kon bepalen', () => {
    // Er staat een vraag open (RFC-036); er is niets veranderd aan wat iemand krijgt.
    const unknown = { __unknown: true, missing: ['inkomen'] };
    expect(driftOf(decided({ hoogte_toeslag: 160825 }), evaluated({ hoogte_toeslag: unknown }))).toBeNull();
  });

  it('zwijgt over een zaak die is ingetrokken', () => {
    const withdrawn = { id: 'zaak-1', status: 'WITHDRAWN', claimedResult: { hoogte_toeslag: 160825 } };
    expect(driftOf(withdrawn, evaluated({ hoogte_toeslag: 0 }))).toBeNull();
  });

  it('meldt ook op een zaak die nog in behandeling is', () => {
    // Ook daar geldt: de behandelaar beoordeelt iets dat inmiddels is ingehaald.
    const inReview = { id: 'zaak-1', status: 'IN_REVIEW', claimedResult: { hoogte_toeslag: 160825 } };
    expect(driftOf(inReview, evaluated({ hoogte_toeslag: 0 }))?.changed).toHaveLength(1);
  });

  it('zwijgt als er geen zaak is of de wet niet door te rekenen was', () => {
    expect(driftOf(null, evaluated({ a: 1 }))).toBeNull();
    const failed = { ok: false, error: 'stuk' };
    expect(driftOf(decided({ a: 1 }), failed)).toBeNull();
  });
});
