import { describe, expect, it } from 'vitest';
import { indieningenTegen, isBesluit, ketens, openIndieningen, plat } from './keten';
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

  it('offers nothing of that kind against the decision on it', () => {
    const opBezwaar = gram('e', 'BESCHIKKING', 'awb#7:11', 'c');
    expect(indieningenTegen(opBezwaar, casus)).toEqual([]);
  });
});
