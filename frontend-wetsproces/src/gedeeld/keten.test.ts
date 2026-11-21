// @vitest-environment node
import { describe, expect, it } from 'vitest';
import { indieningenTegen, isBesluit, ketens, openIndieningen, plat, waardeTekst } from './keten';
import type { Casus, Gram } from './types';

const casus: Casus = {
  id: 'voorbeeld',
  cel: 'gemeente',
  cel_naam: 'gemeente Voorbeeld',
  artikel: 'wet#1',
  besluit: 'wet#1',
  uitkomst: 'verleend',
  andere_cellen: [{ cel: 'rechtbank', cel_naam: 'Rechtbank Voorbeeld' }],
  indieningen: [
    { artikel: 'wet#1', besluit: 'wet#1', cel: 'gemeente', cel_naam: 'gemeente Voorbeeld', soort: 'AANVRAAG' },
    { artikel: 'awb#7:11', besluit: 'awb#7:11', cel: 'gemeente', cel_naam: 'gemeente Voorbeeld', soort: 'BEZWAAR' },
    { artikel: 'awb#8:70', besluit: 'awb#8:70', cel: 'rechtbank', cel_naam: 'Rechtbank Voorbeeld', soort: 'BEROEP' },
  ],
  vandaag: '2026-10-13',
};

const gram = (id: string, kind: string, basis: string, refers_to?: string): Gram => ({
  id,
  kind,
  basis,
  refers_to,
  recorded_on: '2026-10-13',
  recording_actor: 'gemeente',
});

const aanvraag = gram('a', 'AANVRAAG', 'wet#1');
const besluit = { ...gram('b', 'BESCHIKKING', 'wet#1', 'a'), outputs: { verleend: false } };
const bezwaar = gram('c', 'BEZWAAR', 'awb#7:11', 'b');
const advies = gram('d', 'EXECUTOGRAM', 'beleid#5');

describe('the records of a chronicle', () => {
  it('tell a decision from a submission and a recorded fact by the kinds the case knows', () => {
    expect([aanvraag, besluit, bezwaar, advies].map((g) => isBesluit(g, casus))).toEqual([false, true, false, false]);
  });

  it('leave in the work queue the submissions without a decision', () => {
    expect(openIndieningen([aanvraag, besluit, bezwaar, advies], casus).map((g) => g.id)).toEqual(['c']);
  });

  it('form chains of what answers to what, each record once', () => {
    const k = ketens([aanvraag, besluit, bezwaar, advies, aanvraag]);
    expect(k.map((x) => x.gram.id)).toEqual(['a', 'd']);
    expect(plat(k[0]!).map((r) => [r.gram.id, r.boven?.id])).toEqual([
      ['a', undefined],
      ['b', 'a'],
      ['c', 'b'],
    ]);
  });
});

describe('indieningenTegen', () => {
  it('offers against a decision what refers to one and the same cell decides on', () => {
    expect(indieningenTegen(besluit, casus).map((i) => i.soort)).toEqual(['BEZWAAR']);
  });

  it('takes the kind of an application from the case, whatever it is called', () => {
    const anders: Casus = {
      ...casus,
      indieningen: [
        ...casus.indieningen.map((i) => (i.soort === 'AANVRAAG' ? { ...i, soort: 'VERZOEK' } : i)),
        { artikel: 'verordening#2', besluit: 'verordening#2', cel: 'gemeente', cel_naam: 'gemeente Voorbeeld', soort: 'VERZOEK' },
      ],
    };
    expect(indieningenTegen(besluit, anders).map((i) => i.soort)).toEqual(['BEZWAAR']);
  });

  it('offers nothing when the case names no submission of its own article', () => {
    expect(indieningenTegen(besluit, { ...casus, artikel: 'wet#9' })).toEqual([]);
  });

  it('offers an appeal to the court against the decision on an objection, and no second objection', () => {
    const opBezwaar = gram('e', 'BESCHIKKING', 'awb#7:11', 'c');
    expect(indieningenTegen(opBezwaar, casus).map((i) => i.soort)).toEqual(['BEROEP']);
  });

  it('offers against the decision on an objection what every other cell decides on', () => {
    // A case with a second outside cell that decides on a submission: it is
    // offered too, as the rule says; nothing in the case data has one yet.
    const tweeHoven: Casus = {
      ...casus,
      indieningen: [
        ...casus.indieningen,
        { artikel: 'wet#9', besluit: 'wet#9', cel: 'hof', cel_naam: 'Hof Voorbeeld', soort: 'HOGER_BEROEP' },
      ],
    };
    const opBezwaar = gram('e', 'BESCHIKKING', 'awb#7:11', 'c');
    expect(indieningenTegen(opBezwaar, tweeHoven).map((i) => i.soort)).toEqual(['BEROEP', 'HOGER_BEROEP']);
  });

  it("offers nothing against the court's ruling", () => {
    const uitspraak = { ...gram('g', 'UITSPRAAK', 'awb#8:70', 'f'), recording_actor: 'rechtbank' };
    expect(indieningenTegen(uitspraak, casus)).toEqual([]);
  });
});

describe('waardeTekst', () => {
  it('names a decision a record holds by its article, never by its key', () => {
    expect(waardeTekst({ basis: 'algemene_wet_bestuursrecht#7:11', fields: {} })).toBe('een besluit onder Awb artikel 7:11');
  });

  it('names a record a field refers to, never by its id', () => {
    const id = '01a1fe00-0c63-707b-9459-f2cee1184407';
    expect(waardeTekst(id, (x) => (x === id ? 'besluit (beschikking) van 13 oktober 2026' : undefined))).toBe(
      'besluit (beschikking) van 13 oktober 2026',
    );
    expect(waardeTekst(id)).toBe('een eerder vastgelegd stuk');
  });

  it('writes a date as a screen does, and a yes or no in words', () => {
    expect(waardeTekst('2026-10-13')).toBe('13 oktober 2026');
    expect(waardeTekst(true)).toBe('ja');
  });
});
