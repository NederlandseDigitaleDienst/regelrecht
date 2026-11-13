import { describe, expect, it } from 'vitest';
import { gekozen, inlogTekst, keuzeLabel } from '../src/inloggen.js';

const celNaam = (cel) => ({ voorbeeld_register: 'Voorbeeldregister' })[cel] ?? `cel ${cel}`;
const INLOGGEN = {
  veld: 'nummer',
  feit: 'r#1#ingelogd',
  keuzes: [
    {
      waarde: '1',
      naam: 'Vereniging Voorbeeld',
      cel: 'voorbeeld_register',
      vastgelegd_op: '2026-02-02',
      velden: { nummer: '1', naam: 'Vereniging Voorbeeld', niveau: 'substantieel', bevestigd: true },
    },
    { waarde: '2', naam: null, cel: 'voorbeeld_register', vastgelegd_op: '2026-02-03', velden: { nummer: '2' } },
  ],
};

describe('keuzeLabel', () => {
  it('names a record by its name and key, or by its key alone', () => {
    expect(keuzeLabel(INLOGGEN.keuzes[0])).toBe('Vereniging Voorbeeld (1)');
    expect(keuzeLabel(INLOGGEN.keuzes[1])).toBe('2');
  });
});

describe('gekozen', () => {
  it('finds the record whose key the field holds', () => {
    expect(gekozen(INLOGGEN, '1')).toBe(INLOGGEN.keuzes[0]);
    expect(gekozen(INLOGGEN, '9')).toBeUndefined();
    expect(gekozen(null, '1')).toBeUndefined();
  });
});

describe('inlogTekst', () => {
  it('says what the cell recorded at the login, besides the key and the name', () => {
    expect(inlogTekst(INLOGGEN.keuzes[0], celNaam)).toBe(
      'Volgens Voorbeeldregister (2026-02-02): niveau substantieel, bevestigd ja.',
    );
  });

  it('says nothing without a choice', () => {
    expect(inlogTekst(undefined, celNaam)).toBe('');
  });
});
