import { describe, expect, it } from 'vitest';
import { NOT_LOGGED_IN, gekozen, loginHint, inlogTekst, keuzeLabel } from '../src/inloggen.js';

const celNaam = (cel) => ({ voorbeeld_register: 'Voorbeeldregister' })[cel] ?? `cel ${cel}`;
const INLOGGEN = {
  veld: 'nummer',
  feit: 'r#1#ingelogd',
  sleutel: 'nummer',
  naam: 'naam',
  keuzes: [
    {
      waarde: '1',
      naam: 'Vereniging Voorbeeld',
      cel: 'voorbeeld_register',
      vastgelegd_op: '2026-02-02',
      velden: { nummer: '1', naam: 'Vereniging Voorbeeld', niveau: 'substantieel', bevestigd: true, kopie: '1' },
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
    expect(gekozen(INLOGGEN, undefined)).toBeUndefined();
    // A key the form holds as a number still finds its record.
    expect(gekozen({ keuzes: [{ waarde: '7' }] }, 7)).toEqual({ waarde: '7' });
  });
});

describe('inlogTekst', () => {
  it('says what the cell recorded at the login, besides the key and the name', () => {
    // A field is left out by its name, not by its value: `kopie` holds the
    // key's value and still shows.
    expect(inlogTekst(INLOGGEN.keuzes[0], INLOGGEN, celNaam)).toBe(
      'Volgens Voorbeeldregister (2026-02-02): niveau substantieel, bevestigd ja, kopie 1.',
    );
  });

  it('says nothing without a choice', () => {
    expect(inlogTekst(undefined, INLOGGEN, celNaam)).toBe('');
  });
});

describe('loginHint', () => {
  it('names the record the values log in as, as the choice shows it', () => {
    expect(loginHint(INLOGGEN, { nummer: '1' })).toBe('Laat Ingelogd als op Vereniging Voorbeeld (1) staan.');
    expect(loginHint(INLOGGEN, { nummer: 2 })).toBe('Laat Ingelogd als op 2 staan.');
  });

  it('keeps an empty field on no login', () => {
    expect(loginHint(INLOGGEN, { nummer: null })).toBe(`Laat Ingelogd als op ${NOT_LOGGED_IN} staan.`);
  });

  it('says so when the cell offers no record with the key', () => {
    expect(loginHint(INLOGGEN, { nummer: '9' })).toMatch(/^Bij Ingelogd als staat niemand met 9;/);
  });

  it('says nothing without a login or without its field', () => {
    expect(loginHint(null, { nummer: '1' })).toBeNull();
    expect(loginHint(INLOGGEN, { naam: 'x' })).toBeNull();
  });
});
