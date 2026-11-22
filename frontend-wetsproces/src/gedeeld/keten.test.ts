// @vitest-environment node
import { describe, expect, it } from 'vitest';
import { besluitNaam, herroeping, herroepingTekst, indieningenTegen, isBesluit, ketens, plat, termijn, waardeTekst, waarover } from './keten';
import type { Casus, Gram, Stand } from './types';

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
  // What each record is comes from the server; the screens only read it.
  const stand: Stand = {
    rollen: { a: 'indiening', b: 'besluit', c: 'indiening', d: 'vastlegging' },
    herroepen: { b: 'd' },
    antwoorden: {},
  };

  it('tell a decision from a submission and a recorded fact by what the server says', () => {
    expect([aanvraag, besluit, bezwaar, advies].map((g) => isBesluit(g, stand))).toEqual([false, true, false, false]);
  });

  it('name a decision a recorded fact revoked annulled, one a decision revoked revoked', () => {
    const afschrift = { ...gram('d', 'EXECUTOGRAM', 'awb#8:79', 'b'), fact: 'afschrift_van_de_uitspraak', received: { from: 'rechtbank', id: 'u' } };
    const h = herroeping('b', stand, [aanvraag, besluit, bezwaar, afschrift]);
    expect(h?.gram?.id).toBe('d');
    expect(h?.woord).toBe('Vernietigd');
    expect(herroepingTekst(h!, casus, stand)).toBe('Vernietigd door Rechtbank Voorbeeld (afschrift van de uitspraak van 13 oktober 2026)');
    // Revoked whether or not the revoking record is in the list at hand.
    expect(herroeping('b', stand, [])).toEqual({ door: 'd', gram: undefined, woord: 'Vernietigd' });
    expect(herroeping('b', { ...stand, rollen: { ...stand.rollen, d: 'besluit' } }, [])?.woord).toBe('Herroepen');
    expect(herroeping('a', stand, [aanvraag])).toBeUndefined();
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

  it('put a record that names another in a field under it, when nothing else places it', () => {
    // Record ids are UUIDs; a request to another party names the application it was made for.
    const id = (n: number) => `01a20000-0000-7000-8000-00000000000${n}`;
    const a = gram(id(1), 'AANVRAAG', 'wet#1');
    const verzoek = { ...gram(id(2), 'EXECUTOGRAM', 'beleid#9'), fields: { aanvraag: id(1), bsn: '999990032' } };
    const ontvangst = { ...gram(id(3), 'EXECUTOGRAM', 'beleid#9'), recording_actor: 'ggd', fields: { aanvraag: id(1) } };
    // A field naming a record outside the list, or a plain value, places nothing.
    const los = { ...gram(id(4), 'EXECUTOGRAM', 'beleid#5'), fields: { aanvraag: id(9), datum: '2026-10-13' } };
    // What a record refers to still comes first.
    const b = { ...gram(id(5), 'BESCHIKKING', 'wet#1', id(1)), fields: { gevolgd: id(4) } };
    const k = ketens([a, verzoek, b, ontvangst, los]);
    expect(k.map((x) => x.gram.id)).toEqual([id(1), id(4)]);
    expect(plat(k[0]!).map((r) => [r.gram.id, r.boven?.id])).toEqual([
      [id(1), undefined],
      [id(2), id(1)],
      [id(5), id(1)],
      [id(3), id(1)],
    ]);
    expect(waarover(verzoek, [a, verzoek])?.id).toBe(id(1));
    expect(waarover(los, [a, los])).toBeUndefined();
    // Only an earlier record: two that name each other stay in the chains, the first on top.
    const x = { ...gram(id(6), 'EXECUTOGRAM', 'beleid#9'), fields: { ander: id(7) } };
    const y = { ...gram(id(7), 'EXECUTOGRAM', 'beleid#9'), fields: { ander: id(6) } };
    expect(plat(ketens([x, y])[0]!).map((r) => r.gram.id)).toEqual([id(6), id(7)]);
    // What a record received comes first, but only when it is at hand; else what it refers to.
    const kopie = { ...gram(id(8), 'EXECUTOGRAM', 'awb#8:79', id(5)), received: { from: 'rechtbank', id: id(9) } };
    expect(waarover(kopie, [a, b, kopie])?.id).toBe(id(5));
  });

  it('put an appeal to the court under the decision it contests, and the ruling under it', () => {
    const opBezwaar = gram('e', 'BESCHIKKING', 'awb#7:11', 'c');
    const beroep = { ...gram('f', 'BEROEP', 'awb#8:70'), recording_actor: 'rechtbank', received: { from: 'gemeente', id: 'e' } };
    const uitspraak = { ...gram('g', 'UITSPRAAK', 'awb#8:70', 'f'), recording_actor: 'rechtbank' };
    // The municipality's receipt of the ruling's copy refers to its own decision and received the ruling.
    const afschrift = { ...gram('h', 'EXECUTOGRAM', 'awb#8:79', 'e'), received: { from: 'rechtbank', id: 'g' } };
    // In the order of the chronicles, the municipality's first.
    const k = ketens([aanvraag, besluit, bezwaar, opBezwaar, afschrift, advies, beroep, uitspraak]);
    expect(k.map((x) => x.gram.id)).toEqual(['a', 'd']);
    expect(plat(k[0]!).map((r) => r.gram.id)).toEqual(['a', 'b', 'c', 'e', 'f', 'g', 'h']);
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

describe('besluitNaam', () => {
  it("calls the case cell's decision a Besluit and another cell's by its legal character", () => {
    expect(besluitNaam(besluit, casus)).toBe('Besluit');
    expect(besluitNaam({ ...gram('g', 'UITSPRAAK', 'awb#8:70', 'f'), recording_actor: 'rechtbank' }, casus)).toBe('Uitspraak');
  });
});

describe('termijn', () => {
  const metTermijn = { ...besluit, outputs: { verleend: false, termijn_einddatum: '2026-11-24' } };

  it('gives the last day of the field the form names, and whether it has passed, by the clock', () => {
    expect(termijn(metTermijn, '2026-11-24', 'termijn_einddatum')).toEqual({ einde: '2026-11-24', voorbij: false });
    expect(termijn(metTermijn, '2026-11-25', 'termijn_einddatum')).toEqual({ einde: '2026-11-24', voorbij: true });
  });

  it('gives nothing for a decision that names no deadline, or a form that names no field for it', () => {
    expect(termijn(besluit, '2026-11-25', 'termijn_einddatum')).toBeNull();
    expect(termijn(metTermijn, '2026-11-25', null)).toBeNull();
  });
});

describe('omschrijving of a decision of another cell', () => {
  it("names a court's ruling as on its own screen", async () => {
    const { omschrijving } = await import('./keten');
    expect(omschrijving({ ...gram('g', 'UITSPRAAK', 'awb#8:70', 'f'), recording_actor: 'rechtbank' }, casus, 'besluit')).toBe('Uitspraak');
    expect(omschrijving(besluit, casus, 'besluit')).toBe('Besluit (beschikking)');
    expect(omschrijving(bezwaar, casus, 'indiening')).toBe('Bezwaar');
  });

  it('hang a receipt under what it refers to when what it received is not in the list', () => {
    const afschrift = { ...gram('h', 'EXECUTOGRAM', 'awb#8:79', 'b'), received: { from: 'rechtbank', id: 'weg' } };
    expect(plat(ketens([aanvraag, besluit, afschrift])[0]!).map((r) => r.gram.id)).toEqual(['a', 'b', 'h']);
  });
});
