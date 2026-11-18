/**
 * Wat een portaal toont, afgeleid uit de wet (RFC-038).
 *
 * De grens is die van de Awb: een beschikking vraag je aan en er volgt een
 * besluit; een rechtspositie ontstaat van rechtswege; al het andere is een
 * tussenstap waar een andere wet op rekent. Wat hier vastligt is dat die drie
 * uit elkaar blijven, want ze liepen eerder door elkaar in één veld.
 */
import { describe, expect, it } from 'vitest';
import type { Produces } from '@regelrecht/frontend-shared/produces';
import { isDelegationProvider, isEntrypointFor, producesBeschikking, subjectOf } from './entrypoints';

const law = (produces: Produces, params: string[] = ['bsn'], outputs: string[] = ['bedrag']) => ({
  articles: [
    {
      machine_readable: {
        execution: {
          ...(produces ? { produces } : {}),
          parameters: params.map((name) => ({ name })),
          output: outputs.map((name) => ({ name })),
        },
      },
    },
  ],
});

const DELEGATION_OUT = ['heeft_delegaties', 'subject_ids', 'delegation_types', 'permissions'];

describe('producesBeschikking', () => {
  it('herkent een beschikking', () => {
    // De zorgtoeslag: een aanspraak die je inroept, afgedaan met een besluit.
    expect(producesBeschikking(law({ legal_character: 'BESCHIKKING' }))).toBe(true);
  });

  it('herkent een rechtspositie', () => {
    // Kiesgerechtigdheid ontstaat van rechtswege, maar gaat de burger wel aan.
    expect(producesBeschikking(law({ legal_character: 'RECHTSPOSITIE' }))).toBe(true);
  });

  it('laat tussenstappen buiten beschouwing', () => {
    for (const lc of ['TOETS', 'WAARDEBEPALING', 'INFORMATIEF', 'BESLUIT_VAN_ALGEMENE_STREKKING']) {
      expect(producesBeschikking(law({ legal_character: lc }))).toBe(false);
    }
  });

  it('zegt nee als een wet niets produceert', () => {
    // De penitentiaire beginselenwet stelt een feit vast waar anderen op rekenen.
    expect(producesBeschikking(law(null))).toBe(false);
    expect(producesBeschikking(null)).toBe(false);
  });

  it('herkent de beschikking in een lijst van wat een artikel voortbrengt', () => {
    // Leerplichtwet art. 13a: de aanvraag en het besluit erop (schema v0.8.1).
    const produces = [{ type: 'AANVRAAG', requests: 'leerplichtwet_1969#13a' }, { type: 'BESCHIKKING' }];
    expect(producesBeschikking(law(produces))).toBe(true);
    expect(producesBeschikking(law([{ type: 'AANVRAAG' }]))).toBe(false);
  });

  it('kijkt naar alle artikelen, niet alleen het eerste', () => {
    // De Wet op de zorgtoeslag heeft het recht (art. 2) én een vermogenstoets
    // (art. 3); één artikel dat een beschikking geeft, maakt de wet een ingang.
    const doc = {
      articles: [
        { machine_readable: { execution: { produces: { legal_character: 'TOETS' } } } },
        { machine_readable: { execution: { produces: { legal_character: 'BESCHIKKING' } } } },
      ],
    };
    expect(producesBeschikking(doc)).toBe(true);
  });
});

describe('subjectOf', () => {
  it('leidt het onderwerp af uit de sleutel waarop de wet rekent', () => {
    expect(subjectOf(law(null, ['bsn']))).toBe('CITIZEN');
    expect(subjectOf(law(null, ['kvk_nummer']))).toBe('BUSINESS');
    // De werkgeversbijdrage rekent op een loonheffingennummer; ook een onderneming.
    expect(subjectOf(law(null, ['loonheffingennummer', 'bruto_loon']))).toBe('BUSINESS');
  });

  it('laat de onderneming winnen als een wet om allebei vraagt', () => {
    // Een ondernemer draagt beide bij zich; de wet gaat dan over het bedrijf
    // en de BSN is alleen wie er namens dat bedrijf handelt.
    expect(subjectOf(law(null, ['bsn', 'kvk_nummer']))).toBe('BUSINESS');
  });

  it('geeft niets terug zonder sleutel', () => {
    expect(subjectOf(law(null, []))).toBeNull();
  });
});

describe('isDelegationProvider', () => {
  it('herkent een machtigingswet aan wat zij levert', () => {
    // Niet aan een label op de wet: het uitvoercontract maakt haar wat ze is.
    expect(isDelegationProvider(law({ legal_character: 'RECHTSPOSITIE' }, ['bsn'], DELEGATION_OUT))).toBe(true);
  });

  it('houdt een gewone regeling erbuiten', () => {
    expect(isDelegationProvider(law({ legal_character: 'BESCHIKKING' }))).toBe(false);
  });

  it('vraagt het hele contract, niet een deel ervan', () => {
    expect(isDelegationProvider(law(null, ['bsn'], ['heeft_delegaties']))).toBe(false);
  });
});

describe('isEntrypointFor', () => {
  it('zet een beschikking op het portaal van haar onderwerp', () => {
    expect(isEntrypointFor(law({ legal_character: 'BESCHIKKING' }, ['bsn']), 'CITIZEN')).toBe(true);
    expect(isEntrypointFor(law({ legal_character: 'BESCHIKKING' }, ['bsn']), 'BUSINESS')).toBe(false);
  });

  it('houdt een achtergrondwet van het portaal af', () => {
    expect(isEntrypointFor(law({ legal_character: 'INFORMATIEF' }, ['bsn']), 'CITIZEN')).toBe(false);
  });

  it('houdt een machtigingswet van het portaal af', () => {
    // Gezag en curatele bepalen wie namens wie mag handelen; je vraagt ze niet
    // aan en er valt niets te ontvangen.
    const provider = law({ legal_character: 'RECHTSPOSITIE' }, ['bsn'], DELEGATION_OUT);
    expect(producesBeschikking(provider)).toBe(true);
    expect(isEntrypointFor(provider, 'CITIZEN')).toBe(false);
  });
});
