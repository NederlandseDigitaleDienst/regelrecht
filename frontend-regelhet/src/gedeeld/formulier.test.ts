// @vitest-environment node
import { describe, expect, it } from 'vitest';
import { aanvraagformulier, aanvraagVelden, beginwaarden, herkomst, indieningTitel, inGroepen, kaalSchermveld, naamVeld, opVolgorde, schermveld, tegenBesluitVelden } from './formulier';
import type { Casus, Gram, Indieningsvorm } from './types';

// A made-up application, not of any case: the form follows from what the
// server derives from the law and what the UI spec says of each field.
const vorm: Indieningsvorm = {
  soort: 'AANVRAAG',
  basis: 'voorbeeldwet#3',
  besluit: 'voorbeeldwet#3',
  cel: 'voorbeeldcel',
  velden: [
    { naam: 'nummer_aanvrager', type: 'string', mag_leeg: false, basis: 'voorbeeldwet#3' },
    { naam: 'wil_een_vergunning', type: 'boolean', mag_leeg: false, basis: 'voorbeeldwet#3' },
    { naam: 'ondertekend', type: 'boolean', mag_leeg: true, basis: 'algemene_wet#4:2' },
    { naam: 'kanaal', type: 'string', mag_leeg: true, basis: 'voorbeeldwet#3' },
    { naam: 'dagtekening', type: 'date', mag_leeg: true, basis: 'algemene_wet#4:2' },
  ],
  systeem: { nummer_aanvrager: {}, kanaal: { waarde: 'portaal' } },
  verborgen: ['kanaal'],
};

describe('aanvraagformulier', () => {
  it('asks what the law asks, but not what the system fills or the UI spec hides', () => {
    const f = aanvraagformulier(vorm, {}, '999990019');
    expect(f.invullen.map((s) => s.veld.naam)).toEqual(['wil_een_vergunning', 'ondertekend', 'dagtekening']);
    // The login fills the number and the form shows it; the channel is hidden.
    const getoond = f.groepen.flatMap((g) => g.velden);
    expect(getoond.filter((v) => v.systeem)).toEqual([
      expect.objectContaining({
        label: 'Nummer aanvrager',
        systeem: { waarde: '999990019', herkomst: 'Uit de inlog van de aanvrager.' },
      }),
    ]);
  });

  it('names a field after the regelwerk, unless the UI spec gives it a label and help', () => {
    const f = aanvraagformulier(vorm, { wil_een_vergunning: { label: 'Ik vraag een vergunning aan', help: 'Zet dit aan.' } }, null);
    expect(f.invullen.map((s) => [s.label, s.help])).toEqual([
      ['Ik vraag een vergunning aan', 'Zet dit aan.'],
      ['Ondertekend', ''],
      ['Dagtekening', ''],
    ]);
  });

  it('shows every field of the law in its order when the case has no UI spec', () => {
    const kaal = { ...vorm, systeem: undefined, verborgen: undefined };
    const f = aanvraagformulier(kaal, undefined, null);
    expect(f.invullen.map((s) => s.veld.naam)).toEqual(vorm.velden.map((v) => v.naam));
    expect(f.groepen).toHaveLength(1);
    expect(f.groepen[0]?.titel).toBe('');
  });

  it('orders and groups the fields as the UI spec says, and shows what the system fills with its origin', () => {
    const metSysteem: Indieningsvorm = {
      ...vorm,
      systeem: {
        nummer_aanvrager: { soort: 'inloggen', waarde: '999990019' },
        ondertekend: { soort: 'bron', bron: 'beleid_voorbeeld#8#geldt_als_ondertekend', waarde: true },
        dagtekening: { soort: 'datum', waarde: '2026-10-13' },
        kanaal: { soort: 'waarde', waarde: 'portaal' },
      },
    };
    const teksten = {
      nummer_aanvrager: { group: 'Uw gegevens' },
      wil_een_vergunning: { group: 'Uw aanvraag' },
      dagtekening: { group: 'Over deze aanvraag' },
      ondertekend: { group: 'Over deze aanvraag' },
    };
    const volgorde = ['nummer_aanvrager', 'wil_een_vergunning', 'dagtekening', 'ondertekend'];
    const f = aanvraagformulier(metSysteem, teksten, null, volgorde);
    expect(f.groepen.map((g) => [g.titel, g.velden.map((v) => v.veld.naam)])).toEqual([
      ['Uw gegevens', ['nummer_aanvrager']],
      ['Uw aanvraag', ['wil_een_vergunning']],
      ['Over deze aanvraag', ['dagtekening', 'ondertekend']],
    ]);
    const veld = (naam: string) => f.groepen.flatMap((g) => g.velden).find((v) => v.veld.naam === naam);
    expect(veld('ondertekend')?.systeem).toEqual({ waarde: true, herkomst: 'Volgt uit Beleid voorbeeld artikel 8.' });
    expect(veld('dagtekening')?.systeem?.herkomst).toBe('De dag van indiening, volgens de klok van het systeem.');
    expect(f.invullen.map((s) => s.veld.naam)).toEqual(['wil_een_vergunning']);
  });
});

describe('opVolgorde', () => {
  it('puts the fields the UI spec names first, in its order, and the rest after in the order of the law', () => {
    const velden = ['a', 'b', 'c', 'd'].map((naam) => ({ naam }));
    expect(opVolgorde(velden, ['c', 'a']).map((v) => v.naam)).toEqual(['c', 'a', 'b', 'd']);
    expect(opVolgorde(velden).map((v) => v.naam)).toEqual(['a', 'b', 'c', 'd']);
  });
});

describe('inGroepen', () => {
  it('puts a field under the group where that group first stands, so no heading shows twice', () => {
    const velden = [
      { naam: 'a', groep: 'Een' },
      { naam: 'b', groep: 'Twee' },
      { naam: 'c', groep: 'Een' },
      { naam: 'd', groep: '' },
    ];
    expect(inGroepen(velden).map((g) => [g.titel, g.velden.map((v) => v.naam)])).toEqual([
      ['Een', ['a', 'c']],
      ['Twee', ['b']],
      ['', ['d']],
    ]);
  });

  it('leaves a field without a group where it stands, not with an earlier one', () => {
    const velden = [
      { naam: 'x', groep: '' },
      { naam: 'y', groep: 'G' },
      { naam: 'z', groep: '' },
    ];
    expect(inGroepen(velden).map((g) => [g.titel, g.velden.map((v) => v.naam)])).toEqual([
      ['', ['x']],
      ['G', ['y']],
      ['', ['z']],
    ]);
  });
});

describe('herkomst', () => {
  it('says for each kind of fill where the value comes from', () => {
    expect(herkomst({ soort: 'inloggen' })).toBe('Uit de inlog van de aanvrager.');
    expect(herkomst({ soort: 'inloggen', inlog: 'naam', waarde: 'A' })).toBe('Uit de inlog van de aanvrager.');
    expect(herkomst({ soort: 'waarde', waarde: 'x' })).toBe('Hetzelfde bij elke aanvraag.');
    expect(herkomst({ soort: 'gevraagd_besluit', waarde: 'w#1' })).toBe('Het besluit dat de wet bij deze aanvraag noemt.');
    expect(herkomst({ soort: 'inlogmiddel', waarde: 'DigiD' })).toBe('Het middel waarmee de aanvrager inlogde.');
    expect(herkomst({ soort: 'bron', bron: 'algemene_wet_bestuursrecht#4:2#x' })).toBe('Volgt uit Awb artikel 4:2.');
  });
});

describe('aanvraagVelden', () => {
  it('sends what the applicant filled in, and nothing the system fills', () => {
    const velden = aanvraagVelden(vorm, { wil_een_vergunning: true, ondertekend: undefined, nummer_aanvrager: 'x', kanaal: 'y' });
    // The server fills those itself and refuses them from the request.
    expect(velden).toEqual({ wil_een_vergunning: true });
  });
});

describe('een veld dat het scherm in delen vraagt', () => {
  const delen = {
    adres_aanvrager: [
      { naam: 'straat_huisnummer', label: 'Straat en huisnummer', autocomplete: 'street-address' },
      { naam: 'postcode', label: 'Postcode', autocomplete: 'postal-code', width: '12ch' },
      { naam: 'woonplaats', label: 'Woonplaats' },
    ],
  };
  const metAdres: Indieningsvorm = {
    ...vorm,
    velden: [{ naam: 'adres_aanvrager', type: 'string', mag_leeg: true, basis: 'algemene_wet#4:2' }],
    systeem: {},
    verborgen: [],
  };

  it('asks one input per part, each by its own label', () => {
    const f = aanvraagformulier(metAdres, {}, null, [], delen);
    expect(f.invullen.map((v) => v.veld.naam)).toEqual([
      'adres_aanvrager.straat_huisnummer',
      'adres_aanvrager.postcode',
      'adres_aanvrager.woonplaats',
    ]);
    expect(f.invullen.map((v) => v.label)).toEqual(['Straat en huisnummer', 'Postcode', 'Woonplaats']);
    expect(f.invullen[1]?.deel).toEqual({
      veld: 'adres_aanvrager',
      naam: 'postcode',
      autocomplete: 'postal-code',
      width: '12ch',
    });
  });

  it('requires every part of a field the UI spec requires, though the law lets it be empty', () => {
    const f = aanvraagformulier(metAdres, {}, null, [], delen, ['adres_aanvrager']);
    expect(f.invullen.map((v) => v.verplicht)).toEqual([true, true, true]);
    expect(aanvraagformulier(metAdres, {}, null, [], delen).invullen.some((v) => v.verplicht)).toBe(false);
  });

  it('joins the parts into the one value the law asks, in the order of the UI spec', () => {
    const velden = aanvraagVelden(
      metAdres,
      {
        'adres_aanvrager.postcode': '1234 AB',
        'adres_aanvrager.straat_huisnummer': 'Wilgenlaan 12',
        'adres_aanvrager.woonplaats': 'Wilgendaal',
      },
      delen,
    );
    expect(velden).toEqual({ adres_aanvrager: 'Wilgenlaan 12, 1234 AB, Wilgendaal' });
  });

  it('leaves a part the applicant skipped out of the value, and the field out when all are empty', () => {
    expect(
      aanvraagVelden(metAdres, { 'adres_aanvrager.straat_huisnummer': 'Wilgenlaan 12', 'adres_aanvrager.postcode': '  ' }, delen),
    ).toEqual({ adres_aanvrager: 'Wilgenlaan 12' });
    expect(aanvraagVelden(metAdres, { 'adres_aanvrager.woonplaats': '' }, delen)).toEqual({});
  });
});

describe('beginwaarden', () => {
  it('starts a yes-or-no the law requires at no, and leaves the rest empty', () => {
    const velden = vorm.velden.map((v) => schermveld(v));
    expect(beginwaarden(velden)).toEqual({ wil_een_vergunning: false });
  });
});

describe('tegenBesluitVelden', () => {
  it('leaves out what the decision already holds', () => {
    const bezwaar: Indieningsvorm = {
      ...vorm,
      velden: [
        { naam: 'gronden', type: 'string', mag_leeg: true },
        { naam: 'termijn_einddatum', type: 'date', mag_leeg: false },
      ],
    };
    const velden = tegenBesluitVelden(bezwaar, { outputs: { termijn_einddatum: '2026-11-24' } }, {});
    expect(velden.map((s) => s.veld.naam)).toEqual(['gronden']);
  });
});

describe('schermveld', () => {
  it('names the article a field comes from in words, never by its key', () => {
    const veld = vorm.velden.find((v) => v.naam === 'dagtekening')!;
    expect(schermveld(veld).veld.basis).toBe('Algemene wet artikel 4:2');
    expect(schermveld({ ...veld, basis: 'algemene_wet_bestuursrecht#4:2' }).veld.basis).toBe('Awb artikel 4:2');
    // The field itself is left as it is: the value still goes under its own name.
    expect(veld.basis).toBe('algemene_wet#4:2');
  });

  it('takes the record texts on a screen that shows a recorded submission, and the form texts where it gives none', () => {
    const teksten = {
      kaart: { group: 'Vraag de kaart aan als', label: 'Bestuurder', help: 'Zet dit aan als u rijdt.', record: { group: 'Gevraagde kaart', help: '' } },
    };
    const veld = { naam: 'kaart', type: 'boolean' as const };
    expect(schermveld(veld, teksten)).toMatchObject({ groep: 'Vraag de kaart aan als', label: 'Bestuurder', help: 'Zet dit aan als u rijdt.' });
    // The label `record` does not give comes from the form; an empty help puts nothing under it.
    expect(schermveld(veld, teksten, true)).toMatchObject({ groep: 'Gevraagde kaart', label: 'Bestuurder', help: '' });
    expect(kaalSchermveld('kaart', teksten).groep).toBe('Gevraagde kaart');
  });
});

describe('indieningTitel', () => {
  const metNaam: Indieningsvorm = { ...vorm, systeem: { ...vorm.systeem, naam_aanvrager: { soort: 'inloggen', inlog: 'naam' } } };
  // Only the cell is read, to tell an own decision from another's; a submission does not get that far.
  const casus = { cel: 'voorbeeldcel' } as Casus;
  const stuk = (fields: Record<string, unknown>): Gram => ({ id: 's', kind: 'AANVRAAG', basis: 'voorbeeldwet#3', fields, recorded_on: '2026-10-13', recording_actor: 'voorbeeldcel' });

  it('finds the field the login fills with the name, not the one it fills with the identifier', () => {
    expect(naamVeld(metNaam)).toBe('naam_aanvrager');
    expect(naamVeld(vorm)).toBeUndefined();
  });

  it('names a submission by its kind and the applicant', () => {
    expect(indieningTitel(stuk({ naam_aanvrager: 'P. Voorbeeld' }), casus, metNaam)).toBe('Aanvraag P. Voorbeeld');
  });

  it('keeps to the kind when the form takes no name or the submission holds none', () => {
    expect(indieningTitel(stuk({ naam_aanvrager: 'P. Voorbeeld' }), casus, vorm)).toBe('Aanvraag');
    expect(indieningTitel(stuk({ naam_aanvrager: ' ' }), casus, metNaam)).toBe('Aanvraag');
    expect(indieningTitel(stuk({}), casus, undefined)).toBe('Aanvraag');
  });
});
