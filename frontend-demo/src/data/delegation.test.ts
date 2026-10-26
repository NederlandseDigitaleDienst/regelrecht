import { describe, expect, it } from 'vitest';
import { delegationKey, startDelegationKey, delegationProviders, delegationsFor, maySubmitClaims } from './delegation';
import type { DelegationEngine, ProviderLaw } from './delegation';

/** Een provider-wet met de standaarduitvoer van de machtigingsinterface. */
function providerLaw(
  id: string,
  { name = id, service = 'RvIG', outputs = null }: { name?: string; service?: string; outputs?: string[] | null } = {},
): ProviderLaw {
  const names = outputs ?? [
    'heeft_delegaties',
    'subject_ids',
    'subject_names',
    'subject_types',
    'delegation_types',
    'permissions',
    'valid_from_dates',
    'valid_until_dates',
  ];
  return {
    id,
    name,
    service,
    doc: {
      articles: [{ machine_readable: { execution: { output: names.map((n) => ({ name: n })) } } }],
    },
  };
}

function corpusOf(...laws: (Partial<ProviderLaw> & { id: string })[]): { latestById: Map<string, ProviderLaw> } {
  // A law in a fixture may leave out the name and service it does not need.
  return { latestById: new Map(laws.map((l) => [l.id, l as ProviderLaw])) };
}

/** Een engine die per wet een vast antwoord teruggeeft, of gooit. */
function engineOf(byLaw: Record<string, Record<string, unknown> | Error>): DelegationEngine {
  return {
    executeMultipleWithTrace(lawId) {
      const entry = byLaw[lawId];
      if (entry === undefined) return { outputs: { heeft_delegaties: false } };
      if (entry instanceof Error) throw entry.message;
      return { outputs: entry };
    },
  };
}

const GEZAG = {
  heeft_delegaties: true,
  subject_ids: ['999200001', '999200002'],
  subject_names: ['Kind 1', 'Kind 2'],
  subject_types: ['CITIZEN', 'CITIZEN'],
  delegation_types: ['OUDERLIJK_GEZAG', 'OUDERLIJK_GEZAG'],
  permissions: [
    ['LEZEN', 'CLAIMS_INDIENEN', 'BESLUITEN_ONTVANGEN'],
    ['LEZEN', 'CLAIMS_INDIENEN', 'BESLUITEN_ONTVANGEN'],
  ],
  valid_from_dates: ['2020-01-01', '2022-01-01'],
  valid_until_dates: [null, null],
};

const MACHTIGINGEN = {
  heeft_delegaties: true,
  subject_ids: ['12345678', '99001122'],
  subject_names: ['Thuiszorg Van der Meer', 'Zorggroep Nederland VOF'],
  subject_types: ['BUSINESS', 'BUSINESS'],
  delegation_types: ['EIGENAAR', 'VENNOOT'],
  permissions: [['LEZEN', 'CLAIMS_INDIENEN', 'BESLUITEN_ONTVANGEN'], ['LEZEN']],
  valid_from_dates: ['2015-01-01', '2019-01-01'],
  valid_until_dates: [null, null],
};

const zelf = (permissions: string[]) => ({
  heeft_delegaties: true,
  subject_ids: ['999100001'],
  subject_names: ['Mezelf'],
  subject_types: ['SELF'],
  delegation_types: ['EIGEN_ZAKEN'],
  permissions: [permissions],
  valid_from_dates: [null],
  valid_until_dates: [null],
});

describe('delegationProviders', () => {
  it('kiest alleen wetten die machtigingen leveren', () => {
    const corpus = corpusOf(
      providerLaw('burgerlijk_wetboek_gezag'),
      { id: 'zorgtoeslagwet', doc: { articles: [{ machine_readable: { execution: { output: [{ name: 'hoogte_toeslag' }] } } }] } },
    );
    expect(delegationProviders(corpus).map((l) => l.id)).toEqual(['burgerlijk_wetboek_gezag']);
  });

  it('geeft een lege lijst zonder corpus', () => {
    expect(delegationProviders(null)).toEqual([]);
  });
});

describe('delegationsFor', () => {
  const corpus = corpusOf(
    providerLaw('burgerlijk_wetboek_gezag', { name: 'Gezag' }),
    providerLaw('machtigingenwet', { name: 'Machtigingenwet', service: 'KVK' }),
    providerLaw('burgerlijk_wetboek_minderjarigheid', { name: 'Minderjarigheid' }),
  );

  it('verzamelt de machtigingen van alle wetten, met de wet erbij', () => {
    const engine = engineOf({ burgerlijk_wetboek_gezag: GEZAG, machtigingenwet: MACHTIGINGEN });
    const { delegations, errors } = delegationsFor(engine, corpus, '999100001', '2025-01-01');
    expect(errors).toEqual([]);
    expect(delegations).toHaveLength(4);
    const kind = delegations.find((d) => d.subjectId === '999200001');
    expect(kind).toMatchObject({
      subjectName: 'Kind 1',
      subjectType: 'CITIZEN',
      delegationType: 'OUDERLIJK_GEZAG',
      lawId: 'burgerlijk_wetboek_gezag',
      lawName: 'Gezag',
    });
  });

  it('houdt de rechten per machtiging apart', () => {
    const engine = engineOf({ machtigingenwet: MACHTIGINGEN });
    const { delegations } = delegationsFor(engine, corpus, '999100001', '2025-01-01');
    const eigenaar = delegations.find((d) => d.subjectId === '12345678');
    const vennoot = delegations.find((d) => d.subjectId === '99001122');
    expect(eigenaar?.permissions).toEqual(['LEZEN', 'CLAIMS_INDIENEN', 'BESLUITEN_ONTVANGEN']);
    expect(vennoot?.permissions).toEqual(['LEZEN']);
  });

  it('zet mezelf vooraan en de rest op naam', () => {
    const engine = engineOf({
      burgerlijk_wetboek_gezag: GEZAG,
      machtigingenwet: MACHTIGINGEN,
      burgerlijk_wetboek_minderjarigheid: zelf(['LEZEN', 'CLAIMS_INDIENEN', 'BESLUITEN_ONTVANGEN']),
    });
    const { delegations } = delegationsFor(engine, corpus, '999100001', '2025-01-01');
    expect(delegations[0]?.subjectType).toBe('SELF');
    expect(delegations.slice(1).map((d) => d.subjectName)).toEqual([
      'Kind 1',
      'Kind 2',
      'Thuiszorg Van der Meer',
      'Zorggroep Nederland VOF',
    ]);
  });

  it('voegt meerdere mezelf-machtigingen samen op de doorsnede van de rechten', () => {
    const engine = engineOf({
      burgerlijk_wetboek_minderjarigheid: zelf(['LEZEN', 'CLAIMS_INDIENEN', 'BESLUITEN_ONTVANGEN']),
      // Een tweede wet beperkt: onder curatele mag alleen gelezen worden.
      machtigingenwet: zelf(['LEZEN']),
    });
    const { delegations } = delegationsFor(engine, corpus, '999100001', '2025-01-01');
    const self = delegations.filter((d) => d.subjectType === 'SELF');
    expect(self).toHaveLength(1);
    expect(self[0]?.permissions).toEqual(['LEZEN']);
    expect(self[0]?.sourceLaws?.map((l) => l.id)).toEqual([
      'burgerlijk_wetboek_minderjarigheid',
      'machtigingenwet',
    ]);
  });

  it('laat ten minste lezen over als de doorsnede leeg is', () => {
    const engine = engineOf({
      burgerlijk_wetboek_minderjarigheid: zelf(['CLAIMS_INDIENEN']),
      machtigingenwet: zelf(['BESLUITEN_ONTVANGEN']),
    });
    const { delegations } = delegationsFor(engine, corpus, '999100001', '2025-01-01');
    expect(delegations[0]?.permissions).toEqual(['LEZEN']);
  });

  it('laat een machtiging die pas later ingaat nog niet zien', () => {
    // Claudia's café staat sinds 2025-01-15 in het handelsregister. Op
    // een peildatum daarvóór bestaat die machtiging nog niet, en dan hoort de
    // keuze uit de werkbalk te verdwijnen. Dat is de wet, geen fout.
    const engine = engineOf({
      machtigingenwet: {
        heeft_delegaties: true,
        subject_ids: ['85234567'],
        subject_names: ['Café Noon B.V.'],
        subject_types: ['BUSINESS'],
        delegation_types: ['EIGENAAR'],
        permissions: [['LEZEN']],
        valid_from_dates: ['2025-01-15'],
        valid_until_dates: [null],
      },
    });
    expect(delegationsFor(engine, corpus, '999999990', '2025-01-01').delegations).toEqual([]);
    // Op de dag van inschrijving gaat zij in, en later geldt zij nog steeds.
    expect(delegationsFor(engine, corpus, '999999990', '2025-01-15').delegations).toHaveLength(1);
    expect(delegationsFor(engine, corpus, '999999990', '2025-06-01').delegations).toHaveLength(1);
  });

  it('laat een machtiging weg die op de peildatum nog niet of niet meer geldt', () => {
    const engine = engineOf({
      burgerlijk_wetboek_gezag: {
        ...GEZAG,
        valid_from_dates: ['2020-01-01', '2030-01-01'],
        valid_until_dates: ['2024-12-31', null],
      },
    });
    const { delegations } = delegationsFor(engine, corpus, '999100001', '2025-01-01');
    expect(delegations).toEqual([]);
  });

  it('meldt een wet die de engine niet kan uitvoeren en gaat door met de rest', () => {
    const engine = engineOf({
      burgerlijk_wetboek_gezag: new Error('geen gegevens'),
      machtigingenwet: MACHTIGINGEN,
    });
    const { delegations, errors } = delegationsFor(engine, corpus, '999100001', '2025-01-01');
    expect(errors).toEqual([{ lawId: 'burgerlijk_wetboek_gezag', message: 'geen gegevens' }]);
    expect(delegations).toHaveLength(2);
  });

  it('slaat een wet over die de interface niet spreekt', () => {
    const partial = corpusOf(providerLaw('halve_wet', { outputs: ['subject_ids'] }));
    const engine = engineOf({ halve_wet: GEZAG });
    expect(delegationsFor(engine, partial, '999100001', '2025-01-01').delegations).toEqual([]);
  });

  it('geeft niets terug zonder engine, corpus of bsn', () => {
    const engine = engineOf({ burgerlijk_wetboek_gezag: GEZAG });
    expect(delegationsFor(null, corpus, '999100001', '2025-01-01').delegations).toEqual([]);
    expect(delegationsFor(engine, null, '999100001', '2025-01-01').delegations).toEqual([]);
    expect(delegationsFor(engine, corpus, null, '2025-01-01').delegations).toEqual([]);
  });

  it('leest een wet die één machtiging niet als lijst teruggeeft', () => {
    const engine = engineOf({
      machtigingenwet: {
        heeft_delegaties: true,
        subject_ids: '85234567',
        subject_names: 'Café Noon B.V.',
        subject_types: 'BUSINESS',
        delegation_types: 'EIGENAAR',
        permissions: ['LEZEN'],
        valid_from_dates: null,
        valid_until_dates: null,
      },
    });
    const { delegations } = delegationsFor(engine, corpus, '999999990', '2025-01-01');
    expect(delegations).toHaveLength(1);
    expect(delegations[0]).toMatchObject({ subjectId: '85234567', subjectName: 'Café Noon B.V.' });
  });
});

describe('maySubmitClaims', () => {
  it('mag corrigeren met het recht daartoe', () => {
    expect(maySubmitClaims({ permissions: ['LEZEN', 'CLAIMS_INDIENEN'] })).toBe(true);
  });

  it('mag niet corrigeren met alleen leesrecht', () => {
    expect(maySubmitClaims({ permissions: ['LEZEN'] })).toBe(false);
  });

  it('mag corrigeren zonder machtiging: dat is de burger zelf', () => {
    expect(maySubmitClaims(null)).toBe(true);
  });
});

describe('delegationKey', () => {
  it('onderscheidt een onderneming van een burger met hetzelfde nummer', () => {
    expect(delegationKey({ subjectType: 'BUSINESS', subjectId: '123' })).toBe('BUSINESS:123');
    expect(delegationKey({ subjectType: 'CITIZEN', subjectId: '123' })).toBe('CITIZEN:123');
  });

  it('geeft niets terug zonder machtiging', () => {
    expect(delegationKey(null)).toBeNull();
  });
});

describe('startDelegationKey', () => {
  it('wijst de onderneming uit start_namens aan', () => {
    expect(startDelegationKey({ start_namens: '85234567' })).toBe('BUSINESS:85234567');
  });

  it('geeft niets voor een profiel zonder start_namens', () => {
    expect(startDelegationKey({ bsn: '999100001' })).toBeNull();
    expect(startDelegationKey(null)).toBeNull();
  });
});
