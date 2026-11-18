import { describe, expect, it } from 'vitest';
import {
  accountTekst,
  claimRegel,
  ingelogdAls,
  ingevuldDoorInloggen,
  metSessie,
  nietIngelogdTekst,
  weigerTekst,
  weigeringTelt,
} from '../src/inlogstap.js';

// FICTIEF: the login of the test source tests/bron-inlogstap.
const INLOGGEN = {
  veld: 'kvk_nummer',
  sleutel: 'kvk_nummer',
  feit: 'wet_digitale_overheid#15#authenticatie_verricht',
  inlogstap: {
    cel: 'voorbeeld_erkende_dienst',
    cel_naam: 'Erkende dienst voor bedrijfs- en organisatiemiddelen',
    dienst: 'urn:etoegang:DV:00000009999999997000:services:1',
    wachtwoord: 'welkom',
    gebruikersnamen: ['bakker', 'kapper'],
  },
};
const SESSIE = {
  gebruikersnaam: 'bakker',
  claims: { pseudoniem: 'ps-b4k001', kvk_nummer: '99970011', betrouwbaarheidsniveau: 'substantieel' },
};
const VELDEN = ['kvk_nummer', 'kenteken', 'betrouwbaarheidsniveau'];

describe('metSessie', () => {
  it('takes the login field and the claims named after a field from the session', () => {
    const waarden = { kvk_nummer: '11111111', kenteken: 'ZZ-001-Z' };
    expect(metSessie(INLOGGEN, SESSIE, waarden, VELDEN)).toEqual({
      kvk_nummer: '99970011',
      kenteken: 'ZZ-001-Z',
      betrouwbaarheidsniveau: 'substantieel',
    });
  });

  it('leaves the login field empty without a session, whatever the form held', () => {
    expect(metSessie(INLOGGEN, null, { kvk_nummer: '11111111', kenteken: 'X' }, VELDEN)).toEqual({ kenteken: 'X' });
  });

  it('takes the login field from the step itself without a session, as on paper', () => {
    expect(metSessie(INLOGGEN, null, { kvk_nummer: '11111111' }, VELDEN, '99970033')).toEqual({ kvk_nummer: '99970033' });
  });

  it('changes nothing for a case without a login step', () => {
    const waarden = { kvk_nummer: '11111111' };
    const zonder = { ...INLOGGEN, inlogstap: undefined };
    expect(metSessie(zonder, SESSIE, waarden, VELDEN)).toBe(waarden);
  });

  it('puts the claim sleutel in the login field when their names differ', () => {
    const inloggen = { ...INLOGGEN, veld: 'kvk_nummer_opgegeven' };
    expect(metSessie(inloggen, SESSIE, {}, ['kvk_nummer_opgegeven'])).toEqual({ kvk_nummer_opgegeven: '99970011' });
  });
});

describe('ingevuldDoorInloggen', () => {
  it('names the login field and the fields a claim fills', () => {
    expect([...ingevuldDoorInloggen(INLOGGEN, SESSIE, VELDEN)].sort()).toEqual([
      'betrouwbaarheidsniveau',
      'kvk_nummer',
    ]);
    expect([...ingevuldDoorInloggen(INLOGGEN, null, VELDEN)]).toEqual(['kvk_nummer']);
    expect(ingevuldDoorInloggen({ veld: 'kvk_nummer' }, SESSIE, VELDEN).size).toBe(0);
  });
});

describe('weigeringTelt', () => {
  it('counts a refusal only at a login step that expects one', () => {
    expect(weigeringTelt({ stap: 'inloggen', uitkomst: 'geweigerd' })).toBe(true);
    // A mistyped name at a login that should get in.
    expect(weigeringTelt({ stap: 'inloggen' })).toBe(false);
    expect(weigeringTelt({ stap: 'aanvraag', uitkomst: 'geweigerd' })).toBe(false);
    expect(weigeringTelt(undefined)).toBe(false);
  });
});

describe('the texts of the login step', () => {
  it('says who is logged in, the key first', () => {
    expect(ingelogdAls(SESSIE)).toBe('Ingelogd als bakker');
    expect(claimRegel(INLOGGEN, SESSIE)).toBe(
      'kvk nummer 99970011 · pseudoniem ps-b4k001 · betrouwbaarheidsniveau substantieel',
    );
  });

  it('names, before the login, both the new application and the ones before', () => {
    expect(nietIngelogdTekst(INLOGGEN.inlogstap)).toBe(
      'Log in bij Erkende dienst voor bedrijfs- en organisatiemiddelen voor een nieuwe aanvraag en voor de aanvragen die je al indiende. Het inloggen is gesimuleerd.',
    );
  });

  it('names the accounts and the password', () => {
    expect(accountTekst(INLOGGEN.inlogstap)).toBe(
      'Testaccounts: bakker, kapper. Het wachtwoord is voor elk account welkom.',
    );
    expect(accountTekst({ wachtwoord: 'welkom', gebruikersnamen: [] })).toBe(
      'Het wachtwoord is voor elk account welkom.',
    );
  });

  it('says why the service refused', () => {
    expect(weigerTekst('wachtwoord')).toMatch(/^Het wachtwoord klopt niet/);
    expect(weigerTekst('geen_machtiging')).toMatch(/geen geldige machtiging/);
  });
});
