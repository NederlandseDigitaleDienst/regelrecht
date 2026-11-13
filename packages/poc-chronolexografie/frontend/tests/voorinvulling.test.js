import { describe, expect, it } from 'vitest';
import { beginWaarden, bronInvoer, herkomstTekst, voorgevuldeWaarden } from '../src/voorinvulling.js';

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
