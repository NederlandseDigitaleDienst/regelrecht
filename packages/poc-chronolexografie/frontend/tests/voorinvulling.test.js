import { describe, expect, it } from 'vitest';
import { beginWaarden, bronInvoer, herkomstTekst, voorgevuldeWaarden } from '../src/voorinvulling.js';
import { BESTUUR, RIJEN } from './fixtureTabel.js';

const celNaam = (cel) => ({ voorbeeld_register: 'Voorbeeldregister' })[cel] ?? `cel ${cel}`;

describe('bronInvoer', () => {
  it('gives a source every field with a value, a prefilled one too', () => {
    // The seats read the designation, which is prefilled itself.
    const invoer = bronInvoer({ nummer: '1', naam_opgegeven: 'X', leeg: undefined });
    expect(invoer).toEqual({ nummer: '1', naam_opgegeven: 'X' });
  });
});

describe('beginWaarden', () => {
  const voorgevuld = { naam: { waarde: 'Uit de bron', herkomst: [] } };

  it('takes the source over the example, and the step over the source', () => {
    const scenario = { naam: 'Uit het voorbeeld', jaar: 2028 };
    expect(beginWaarden(scenario, {}, voorgevuld)).toEqual({ naam: 'Uit de bron', jaar: 2028 });
    const stap = { ...scenario, naam: 'Uit de stap' };
    expect(beginWaarden(stap, { naam: 'Uit de stap' }, voorgevuld)).toEqual({ naam: 'Uit de stap', jaar: 2028 });
  });

  it('starts from the sources alone in free play', () => {
    expect(beginWaarden(null, null, voorgevuld)).toEqual({ naam: 'Uit de bron' });
  });
});

describe('voorgevuldeWaarden', () => {
  it('takes the value of each prefilled field the sources gave', () => {
    const voorgevuld = {
      naam_opgegeven: { bron: 'r#2#naam', waarde: 'Vereniging Voorbeeld', herkomst: [] },
      zetels: { bron: 'r#3#zetels', waarde: null, herkomst: [], fout: 'nummer ontbreekt' },
    };
    expect(voorgevuldeWaarden(voorgevuld)).toEqual({ naam_opgegeven: 'Vereniging Voorbeeld' });
    expect(voorgevuldeWaarden(undefined)).toEqual({});
    // A table from a chronicle holds a decimal cell as its number.
    const tabel = { bestuur: { waarde: [{ aantal: { __decimal: '1.5' } }], herkomst: [] } };
    expect(voorgevuldeWaarden(tabel)).toEqual({ bestuur: [{ aantal: 1.5 }] });
  });
});

describe('herkomstTekst', () => {
  const voorgevuld = {
    bron: 'r#2#naam_volgens_register',
    waarde: 'Vereniging Voorbeeld',
    herkomst: [{ door: 'voorbeeld_register', antwoord: 'naam_bij_nummer' }],
  };

  it('names the cell the value comes from', () => {
    expect(herkomstTekst(voorgevuld, 'Vereniging Voorbeeld', celNaam)).toBe(
      'Ingevuld uit Voorbeeldregister (naam bij nummer).',
    );
  });

  it('says the applicant changed it, next to what the source gives', () => {
    expect(herkomstTekst(voorgevuld, 'Iets anders', celNaam)).toBe(
      'Door u gewijzigd. Voorbeeldregister geeft: Vereniging Voorbeeld.',
    );
    expect(herkomstTekst(voorgevuld, undefined, celNaam)).toBe(
      'Door u leeggemaakt. Voorbeeldregister geeft: Vereniging Voorbeeld.',
    );
  });

  it('says why a source gave nothing', () => {
    expect(herkomstTekst({ ...voorgevuld, waarde: null, herkomst: [], fout: 'refused: no output' }, undefined, celNaam)).toBe(
      'Niet vooringevuld: refused: no output',
    );
  });

  it('shows the value the source gives as the app shows values', () => {
    expect(herkomstTekst({ ...voorgevuld, waarde: true }, false, celNaam)).toBe(
      'Door u gewijzigd. Voorbeeldregister geeft: ja.',
    );
  });

  it('says nothing when no cell gave a value', () => {
    expect(herkomstTekst({ ...voorgevuld, waarde: null, herkomst: [] }, undefined, celNaam)).toBe('');
    expect(herkomstTekst(undefined, 'x', celNaam)).toBe('');
  });
});

describe('herkomstTekst of a table', () => {
  const voorgevuld = {
    bron: 'voorbeeld_inschrijving#3#bestuur_volgens_register',
    waarde: RIJEN,
    herkomst: [{ door: 'voorbeeld_register', antwoord: 'bestuur_bij_nummer' }],
  };
  const tekst = (waarde) => herkomstTekst(voorgevuld, waarde, celNaam, BESTUUR.kolommen);
  const [een, twee] = RIJEN;

  it('names the cell when the table holds what the source gives, in any order', () => {
    const ingevuld = 'Ingevuld uit Voorbeeldregister (bestuur bij nummer).';
    expect(tekst(RIJEN.map((r) => ({ ...r })))).toBe(ingevuld);
    expect(tekst([twee, een])).toBe(ingevuld);
  });

  it('says what the source gives where the applicant changed the table', () => {
    expect(tekst([{ ...een, vergoeding: 15000 }, twee])).toBe(
      'Door u gewijzigd. Voorbeeldregister geeft bij Lid Een, voorzitter, 2026-01-01: vergoeding 12500.',
    );
    const nieuw = { naam: 'Lid Drie', functie: 'secretaris', sinds: '2026-03-01', vergoeding: null };
    expect(tekst([een, nieuw])).toBe(
      'Door u gewijzigd. Voorbeeldregister geeft ook Lid Twee, penningmeester, 2026-01-15; ' +
        'niet Lid Drie, secretaris, 2026-03-01.',
    );
  });

  it('names a changed row by its other cells when its key cells are empty', () => {
    const k = [
      { naam: 'code', type: 'string' },
      { naam: 'aantal', type: 'number' },
    ];
    const bron = { ...voorgevuld, waarde: [{ code: null, aantal: 1 }] };
    expect(herkomstTekst(bron, [{ aantal: 2 }], celNaam, k)).toBe(
      'Door u gewijzigd. Voorbeeldregister geeft bij 1: aantal 1.',
    );
  });

  it('names an added row by its other cells when its key cells are empty', () => {
    const k = [
      { naam: 'code', type: 'string' },
      { naam: 'aantal', type: 'number' },
    ];
    const bron = { ...voorgevuld, waarde: [{ code: 'A', aantal: 1 }] };
    expect(herkomstTekst(bron, [{ code: 'A', aantal: 1 }, { code: null, aantal: 3 }], celNaam, k)).toBe(
      'Door u gewijzigd. Voorbeeldregister geeft niet 3.',
    );
  });

  it('lists the rows of the source when the applicant emptied the table', () => {
    expect(tekst(undefined)).toBe(
      'Door u leeggemaakt. Voorbeeldregister geeft: Lid Een, voorzitter, 2026-01-01, 12500; ' +
        'Lid Twee, penningmeester, 2026-01-15.',
    );
  });
});
