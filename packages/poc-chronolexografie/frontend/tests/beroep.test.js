import { beforeEach, describe, expect, it } from 'vitest';
import { binnenTermijn, herkomst, openOntvangsten, ontvangstVan, uitspraakOp } from '../src/beroep.js';
import { dossier, geldendBesluit, isBesluit, isBesluitwaarde, rol, state, waarde } from '../src/state.js';

// Ids verzonnen, op de laatste acht tekens na herkenbaar; de vormen zoals de
// cel ze vastlegt (RFC-053).
const id = (staart) => `019a0000-0000-7000-8000-0000${staart}`;
const aanvraag = { id: id('0000aaaa'), kind: 'AANVRAAG', basis: 'wet#1' };
const weigering = { id: id('0000bbbb'), kind: 'BESCHIKKING', basis: 'wet#1', refers_to: aanvraag.id, outputs: { toegekend: false } };
const bezwaar = { id: id('0000cccc'), kind: 'BEZWAAR', basis: 'awb#7:11', refers_to: weigering.id };
const ongegrond = { id: id('0000dddd'), kind: 'BESCHIKKING', basis: 'awb#7:11', refers_to: bezwaar.id, outputs: { bestreden_besluit_herroepen: false } };
const beroep = { id: id('0000eeee'), kind: 'BEROEP', basis: 'awb#8:70', received: { from: 'school', id: ongegrond.id } };
const uitspraak = { id: id('0000ffff'), kind: 'UITSPRAAK', basis: 'awb#8:70', refers_to: beroep.id };
const ontvangst = {
  id: id('00001111'),
  kind: 'EXECUTOGRAM',
  basis: 'awb#8:79',
  fact: 'afschrift_van_de_uitspraak',
  refers_to: ongegrond.id,
  received: { from: 'rechtbank', id: uitspraak.id },
  fields: { opdracht_nieuw_besluit: true, termijn_nieuw_besluit_einddatum: '2027-01-25' },
};
const gegrond = { id: id('00002222'), kind: 'BESCHIKKING', basis: 'awb#7:11', refers_to: bezwaar.id, after: ontvangst.id, outputs: { bestreden_besluit_herroepen: true, nieuw_besluit: id('00003333') } };
const nieuw = { id: id('00003333'), kind: 'BESCHIKKING', basis: 'wet#1', refers_to: bezwaar.id, outputs: { toegekend: true } };

beforeEach(() => {
  state.casus = {
    cel: 'school',
    besluit: 'wet#1',
    beslissing_op_bezwaar: 'awb#7:11',
    beroep: 'awb#8:70',
    ontvangst: 'awb#8:79#afschrift_van_de_uitspraak',
    uitkomst: 'toegekend',
    rechter: { cel: 'rechtbank', cel_naam: 'Rechtbank' },
  };
  state.grammen = [aanvraag, weigering, bezwaar, ongegrond];
  state.rechter = [];
});

describe('het beroep in twee kronieken', () => {
  it('een ontvangst van een uitspraak verwijst naar een besluit en is er geen', () => {
    expect(isBesluit(ontvangst)).toBe(false);
    expect(isBesluit(ongegrond)).toBe(true);
    expect(rol(ontvangst)).toBe('Ontvangst uitspraak');
    expect(rol(beroep)).toBe('Beroep');
    expect(rol(uitspraak)).toBe('Uitspraak');
  });

  it('het dossier volgt de stukken over de celgrens', () => {
    state.rechter = [beroep, uitspraak];
    state.grammen.push(ontvangst, gegrond, nieuw);
    const stappen = dossier(aanvraag).map((s) => [s.rol, s.rechter]);
    expect(stappen).toEqual([
      ['Aanvraag', false],
      ['Besluit', false],
      ['Bezwaar', false],
      ['Beslissing op bezwaar', false],
      ['Beroep', true],
      ['Uitspraak', true],
      ['Ontvangst uitspraak', false],
      ['Beslissing op bezwaar', false],
      ['Nieuw besluit', false],
    ]);
    // De tweede beslissing herroept de weigering; het nieuwe besluit geldt.
    expect(geldendBesluit(aanvraag)).toEqual(nieuw);
  });

  it('een verlening die een vernietigde beslissing nam, geldt niet meer', () => {
    // Gegrond met verlening, door de rechter vernietigd, daarna ongegrond. De
    // latere beslissing geldt; dat de vernietigde zonder haar al niet geldt,
    // toetst de test hieronder.
    const verleend = { ...nieuw, id: id('00004444') };
    const eerst = { ...gegrond, id: id('00005555'), after: undefined, outputs: { bestreden_besluit_herroepen: true, nieuw_besluit: verleend.id } };
    const daarna = { ...ongegrond, id: id('00006666'), after: ontvangst.id };
    const vernietigd = { ...ontvangst, refers_to: eerst.id, fields: { ...ontvangst.fields, rechtsgevolgen_vernietigd: true } };
    state.grammen = [aanvraag, weigering, bezwaar, eerst, verleend, vernietigd, daarna];
    expect(dossier(aanvraag).map((s) => s.gram.id)).not.toContain(verleend.id);
    expect(geldendBesluit(aanvraag)).toEqual(weigering);
  });

  it('tussen de ontvangst en het opnieuw beslissen geldt de vernietigde verlening al niet', () => {
    const verleend = { ...nieuw, id: id('00004444') };
    const eerst = { ...gegrond, id: id('00005555'), after: undefined, outputs: { bestreden_besluit_herroepen: true, nieuw_besluit: verleend.id } };
    const vernietigd = { ...ontvangst, refers_to: eerst.id, fields: { ...ontvangst.fields, rechtsgevolgen_vernietigd: true } };
    state.grammen = [aanvraag, weigering, bezwaar, eerst, verleend, vernietigd];
    expect(geldendBesluit(aanvraag)).toEqual(weigering);
  });

  it('een ontvangst blijft open tot er na haar opnieuw is beslist', () => {
    state.rechter = [beroep, uitspraak];
    expect(uitspraakOp(beroep.id)).toEqual(uitspraak);
    expect(ontvangstVan(uitspraak.id)).toBeNull();
    state.grammen.push(ontvangst);
    expect(ontvangstVan(uitspraak.id)).toEqual(ontvangst);
    expect(openOntvangsten()).toEqual([ontvangst]);
    state.grammen.push(gegrond);
    expect(openOntvangsten()).toEqual([]);
    // Zonder opdracht is er niets om opnieuw op te beslissen.
    state.grammen = [{ ...ontvangst, fields: { opdracht_nieuw_besluit: false } }];
    expect(openOntvangsten()).toEqual([]);
  });

  it('de termijn van de rechter en de herkomst in één regel', () => {
    expect(binnenTermijn(ontvangst, '2027-01-18')).toBe(true);
    expect(binnenTermijn(ontvangst, '2027-01-26')).toBe(false);
    expect(binnenTermijn({ fields: {} }, '2027-01-26')).toBeNull();
    expect(herkomst(ontvangst)).toBe('verwijst naar 0000dddd · ontvangen van rechtbank 0000ffff');
    expect(herkomst(gegrond)).toBe('verwijst naar 0000cccc · na 00001111');
    expect(herkomst(aanvraag)).toBe('');
  });
});

describe('waarde van een besluit in de uitspraak', () => {
  // Voorziet de rechter zelf in de zaak (Awb 8:72, derde lid, onder b), dan
  // houdt de uitspraak de beslissing op bezwaar als besluitwaarde, met het
  // nieuwe besluit daarin.
  const verlening = { basis: 'wet#1', fields: {}, findings: {}, outputs: { toegekend: true } };
  const inDePlaats = {
    basis: 'awb#7:11',
    fields: { bestreden_besluit: verlening },
    findings: {},
    outputs: { bestreden_besluit_herroepen: true, nieuw_besluit: verlening },
  };

  it('toont de basis en de uitvoer, een besluit daarin tussen haakjes', () => {
    expect(waarde(inDePlaats)).toBe(
      'besluit onder awb#7:11; bestreden besluit herroepen: true; nieuw besluit: (besluit onder wet#1; toegekend: true)',
    );
  });

  it('laat een gewone waarde JSON en een decimaal het getal', () => {
    expect(waarde({ basis: 'wet#1', outputs: {} })).toBe('{"basis":"wet#1","outputs":{}}');
    expect(waarde({ __decimal: '1.5' })).toBe('1.5');
    expect(waarde(null)).toBe('null');
  });

  it('kent een besluitwaarde aan haar vier delen', () => {
    expect(isBesluitwaarde(inDePlaats)).toBe(true);
    expect(isBesluitwaarde({ ...inDePlaats, outputs: null })).toBe(false);
    expect(isBesluitwaarde(null)).toBe(false);
  });
});
