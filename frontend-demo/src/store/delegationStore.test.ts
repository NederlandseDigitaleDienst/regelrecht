/**
 * What delegation does to the rest of the demo: whom the law computes about,
 * which schemes are shown, and what is and is not allowed. The store itself
 * pulls in a corpus and an engine, which a unit test does not have; the rules
 * it applies therefore live as pure functions in `data/delegation.ts` and
 * `data/featureFlags.ts`, and those are tested here.
 */
import { describe, expect, it } from 'vitest';
import {
  activeDelegationFor,
  caseOfSubject,
  chosenDelegationKey,
  delegationsFor,
  maySubmitClaims,
  portalSubject,
  subjectBsnFor,
  subjectParams,
} from '../data/delegation';
import { featureFlagEnabled } from '../data/featureFlags';
import type { Delegation } from '../data/delegation';
import type { DemoProfileConfig } from '../data/loadCorpus';

/** A delegation as the fixtures below write it: without the provider's service. */
type DelegationFixture = Omit<Delegation, 'service'>;

/** A profile as far as the context reads it. */
type ProfileFixture = Pick<DemoProfileConfig, 'bsn'> & Partial<DemoProfileConfig>;

/**
 * The store's state (chosen key, flag) with the functions demoStore.ts applies
 * to it. Only the storage is rebuilt here, not the rules.
 */
function makeContext({
  delegations = [],
  profile,
  delegationEnabledFlag = true,
}: {
  delegations?: DelegationFixture[];
  profile: ProfileFixture;
  delegationEnabledFlag?: boolean;
}) {
  // null: nog niets gekozen, dus de startmachtiging van het profiel; 'SELF' is
  // een gekozen Mezelf.
  const key: { value: string | null } = { value: null };
  const active = () =>
    activeDelegationFor({ profile, chosenKey: key.value, enabled: delegationEnabledFlag, delegations });
  return {
    key,
    list: () => (delegationEnabledFlag ? delegations : []),
    active,
    setDelegation: (d: DelegationFixture | null) => {
      key.value = chosenDelegationKey(d);
    },
    personaParams: () => subjectParams(active(), profile),
    subjectBsn: () => subjectBsnFor(active(), profile),
    portalSubject: () => portalSubject(active()),
    canSubmitClaims: () => maySubmitClaims(active()),
  };
}

const MERIJN: ProfileFixture = { bsn: '999100001', kvk: null, type: 'burger' };
const CLAUDIA: ProfileFixture = { bsn: '999999990', kvk: '85234567', type: 'ondernemer', start_namens: '85234567' };

const KIND: DelegationFixture = {
  subjectId: '999200001',
  subjectName: 'Kind 1 van der Meer',
  subjectType: 'CITIZEN',
  delegationType: 'OUDERLIJK_GEZAG',
  permissions: ['LEZEN', 'CLAIMS_INDIENEN', 'BESLUITEN_ONTVANGEN'],
  validFrom: null,
  validUntil: null,
  lawId: 'burgerlijk_wetboek_gezag',
  lawName: 'Gezag',
};

const EIGEN_ZAAK: DelegationFixture = {
  subjectId: '12345678',
  subjectName: 'Thuiszorg Van der Meer',
  subjectType: 'BUSINESS',
  delegationType: 'EIGENAAR',
  permissions: ['LEZEN', 'CLAIMS_INDIENEN', 'BESLUITEN_ONTVANGEN'],
  validFrom: null,
  validUntil: null,
  lawId: 'machtigingenwet',
  lawName: 'Machtigingenwet',
};

/** Vennoot met alleen leesrecht: de machtigingenwet geeft hier minder. */
const VENNOOTSCHAP: DelegationFixture = {
  ...EIGEN_ZAAK,
  subjectId: '99001122',
  subjectName: 'Zorggroep Nederland VOF',
  delegationType: 'VENNOOT',
  permissions: ['LEZEN'],
};

const ZELF: DelegationFixture = {
  subjectId: MERIJN.bsn,
  subjectName: 'Mezelf',
  subjectType: 'SELF',
  delegationType: 'EIGEN_ZAKEN',
  permissions: ['LEZEN', 'CLAIMS_INDIENEN', 'BESLUITEN_ONTVANGEN'],
  validFrom: null,
  validUntil: null,
  lawId: null,
  lawName: null,
};

describe('handelen namens een ander', () => {
  const ctx = () => makeContext({ delegations: [ZELF, KIND, EIGEN_ZAAK, VENNOOTSCHAP], profile: MERIJN });

  it('rekent standaard over de ingelogde burger zelf', () => {
    const c = ctx();
    expect(c.active()).toBeNull();
    expect(c.personaParams()).toEqual({ bsn: '999100001' });
    expect(c.subjectBsn()).toBe('999100001');
  });

  it('bevraagt de wet op het kind zodra er namens het kind gehandeld wordt', () => {
    const c = ctx();
    c.setDelegation(KIND);
    expect(c.personaParams()).toEqual({ bsn: '999200001' });
    expect(c.subjectBsn()).toBe('999200001');
  });

  it('bevraagt de wet op het KvK-nummer namens een onderneming', () => {
    const c = ctx();
    c.setDelegation(EIGEN_ZAAK);
    expect(c.personaParams()).toEqual({ kvk_nummer: '12345678' });
    // Een onderneming heeft geen BSN; correcties blijven bij de gemachtigde.
    expect(c.subjectBsn()).toBe('999100001');
  });

  it('toont ondernemersregelingen namens een onderneming en burgerregelingen namens een kind', () => {
    const c = ctx();
    expect(c.portalSubject()).toBe('CITIZEN');
    c.setDelegation(EIGEN_ZAAK);
    expect(c.portalSubject()).toBe('BUSINESS');
    c.setDelegation(KIND);
    expect(c.portalSubject()).toBe('CITIZEN');
  });

  it('behandelt "Mezelf" als het ontbreken van een machtiging', () => {
    const c = ctx();
    c.setDelegation(KIND);
    expect(c.active()).not.toBeNull();
    c.setDelegation(ZELF);
    expect(c.active()).toBeNull();
    expect(c.personaParams()).toEqual({ bsn: '999100001' });
  });

  it('staat corrigeren toe met het recht daartoe en verbiedt het zonder', () => {
    const c = ctx();
    expect(c.canSubmitClaims()).toBe(true);
    c.setDelegation(EIGEN_ZAAK);
    expect(c.canSubmitClaims()).toBe(true);
    c.setDelegation(VENNOOTSCHAP);
    expect(c.canSubmitClaims()).toBe(false);
  });

  it('valt terug op de burger zelf als de bewaarde machtiging niet meer bestaat', () => {
    // Een keuze uit een vorige sessie die de wet nu niet meer geeft: bijvoorbeeld
    // een kind dat meerderjarig is geworden.
    const c = makeContext({ delegations: [ZELF], profile: MERIJN });
    c.setDelegation(KIND);
    expect(c.active()).toBeNull();
    expect(c.personaParams()).toEqual({ bsn: '999100001' });
    expect(c.canSubmitClaims()).toBe(true);
  });

  it('geeft geen machtigingen als de vlag voor dit profiel uit staat', () => {
    const c = makeContext({ delegations: [ZELF, KIND], profile: MERIJN, delegationEnabledFlag: false });
    expect(c.list()).toEqual([]);
    c.setDelegation(KIND);
    expect(c.active()).toBeNull();
  });
});

describe('een ondernemer die namens haar zaak begint', () => {
  const NOON = {
    ...EIGEN_ZAAK,
    subjectId: '85234567',
    subjectName: 'Café Noon B.V.',
  };
  const ctx = () => makeContext({ delegations: [ZELF, NOON], profile: CLAUDIA });

  it('handelt bij binnenkomst namens de onderneming uit start_namens', () => {
    const c = ctx();
    expect(c.active()).toEqual(NOON);
    expect(c.personaParams()).toEqual({ kvk_nummer: '85234567' });
    expect(c.portalSubject()).toBe('BUSINESS');
  });

  it('is als "Mezelf" een burger zoals ieder ander, zonder KvK-nummer', () => {
    const c = ctx();
    c.setDelegation(ZELF);
    expect(c.active()).toBeNull();
    expect(c.personaParams()).toEqual({ bsn: '999999990' });
    expect(c.portalSubject()).toBe('CITIZEN');
  });

  it('begint ook namens de onderneming met een opgeslagen staat van vóór start_namens', () => {
    // Zo'n staat heeft delegationKey: null; dat is geen gekozen Mezelf.
    const c = ctx();
    c.key.value = null;
    expect(c.active()).toEqual(NOON);
  });

  it('begint ook namens de onderneming als DELEGATION uit staat; alleen de keuze verdwijnt', () => {
    const c = makeContext({ delegations: [ZELF, NOON], profile: CLAUDIA, delegationEnabledFlag: false });
    expect(c.active()).toEqual(NOON);
    expect(c.personaParams()).toEqual({ kvk_nummer: '85234567' });
    expect(c.portalSubject()).toBe('BUSINESS');
    expect(c.list()).toEqual([]);
  });

  it('negeert een opgeslagen "Mezelf" zolang DELEGATION uit staat', () => {
    const c = makeContext({ delegations: [ZELF, NOON], profile: CLAUDIA, delegationEnabledFlag: false });
    c.key.value = 'SELF';
    expect(c.active()).toEqual(NOON);
    expect(c.personaParams()).toEqual({ kvk_nummer: '85234567' });
  });

  it('begint als zichzelf als de wet de startmachtiging niet geeft', () => {
    const c = makeContext({ delegations: [ZELF], profile: CLAUDIA });
    expect(c.active()).toBeNull();
    expect(c.personaParams()).toEqual({ bsn: '999999990' });
  });
});

describe('de zaak van het juiste onderwerp', () => {
  // Het onderwerp volgt uit de parameters van de wet (`subjectOf`).
  const lawAsking = (id: string, ...params: string[]) => ({
    id,
    doc: { articles: [{ machine_readable: { execution: { parameters: params.map((name) => ({ name })) } } }] },
  });
  const bedrijfswet = lawAsking('accijns', 'bsn', 'kvk_nummer');
  const burgerwet = lawAsking('zorgtoeslag', 'bsn');

  it('vindt de zaak van een onderneming ongeacht wie hem indiende', () => {
    // De eigenaar diende in; de zaak staat op háár BSN en op de KvK.
    const doorEigenaar = { id: 'z1', lawId: 'accijns', status: 'IN_REVIEW', bsn: '999999990', kvk: '85234567' };
    // Een gemachtigde ziet dezelfde zaak: het gaat om dezelfde onderneming.
    expect(caseOfSubject([doorEigenaar], bedrijfswet, { kvk_nummer: '85234567' })?.id).toBe('z1');
    // En de eigenaar zelf ook, die bsn én kvk bij zich draagt. Op de BSN
    // matchen liet juist deze zaak wegvallen zodra een ander hem indiende.
    expect(caseOfSubject([doorEigenaar], bedrijfswet, { bsn: '999999990', kvk_nummer: '85234567' })?.id).toBe('z1');
  });

  it('laat de eigenaar de zaak zien die een gemachtigde namens zijn bedrijf indiende', () => {
    const doorGemachtigde = { id: 'z2', lawId: 'accijns', status: 'IN_REVIEW', bsn: '999100001', kvk: '85234567' };
    expect(caseOfSubject([doorGemachtigde], bedrijfswet, { bsn: '999999990', kvk_nummer: '85234567' })?.id).toBe('z2');
  });

  it('houdt de zaak van een andere onderneming buiten beeld', () => {
    const andere = { id: 'z3', lawId: 'accijns', status: 'IN_REVIEW', bsn: '999999990', kvk: '99001122' };
    expect(caseOfSubject([andere], bedrijfswet, { kvk_nummer: '85234567' })).toBeNull();
  });

  it('houdt bij een burgerwet de zaak van een ander buiten beeld', () => {
    const vanHetKind = { id: 'z4', lawId: 'zorgtoeslag', status: 'IN_REVIEW', bsn: '999200001', kvk: null };
    expect(caseOfSubject([vanHetKind], burgerwet, { bsn: '999200001' })?.id).toBe('z4');
    // De ouder ziet de zaak van zijn kind niet zolang hij voor zichzelf handelt.
    expect(caseOfSubject([vanHetKind], burgerwet, { bsn: '999100001' })).toBeNull();
  });

  it('negeert een ingetrokken zaak', () => {
    const ingetrokken = { id: 'z5', lawId: 'accijns', status: 'WITHDRAWN', bsn: '999999990', kvk: '85234567' };
    expect(caseOfSubject([ingetrokken], bedrijfswet, { kvk_nummer: '85234567' })).toBeNull();
  });
});

describe('vlaggen die de presentator omzet', () => {
  const profile = { feature_flags: { DELEGATION: true, CHANGE_WIZARD: true, HARMONIZE: false } };

  it('volgt het profiel zolang er niets is omgezet', () => {
    expect(featureFlagEnabled(profile, {}, 'DELEGATION')).toBe(true);
    expect(featureFlagEnabled(profile, {}, 'HARMONIZE')).toBe(false);
    // Een vlag die het profiel niet noemt staat uit.
    expect(featureFlagEnabled(profile, {}, 'AUTO_APPROVE_CLAIMS')).toBe(false);
  });

  it('laat een omgezette vlag winnen van het profiel, allebei de kanten op', () => {
    expect(featureFlagEnabled(profile, { DELEGATION: false }, 'DELEGATION')).toBe(false);
    expect(featureFlagEnabled(profile, { HARMONIZE: true }, 'HARMONIZE')).toBe(true);
  });

  it('raakt de andere vlaggen niet aan', () => {
    const overrides = { DELEGATION: false };
    expect(featureFlagEnabled(profile, overrides, 'CHANGE_WIZARD')).toBe(true);
  });

  it('valt terug op het profiel als de overrides leeg zijn', () => {
    // "Features terug naar het profiel" leegt de lijst; false is dan weer
    // false omdat het profiel dat zegt, niet omdat het is uitgezet.
    expect(featureFlagEnabled(profile, {}, 'DELEGATION')).toBe(true);
  });
});

describe('delegationsFor tegen de echte interface', () => {
  it('leest de parallelle lijsten zoals de wetten ze opleveren', () => {
    const law = {
      id: 'burgerlijk_wetboek_gezag',
      name: 'Gezag',
      service: 'RvIG',
      doc: {
        discoverable: 'DELEGATION_PROVIDER',
        articles: [
          {
            machine_readable: {
              execution: {
                output: [
                  'heeft_delegaties',
                  'subject_ids',
                  'subject_names',
                  'subject_types',
                  'delegation_types',
                  'permissions',
                  'valid_from_dates',
                  'valid_until_dates',
                ].map((name) => ({ name })),
              },
            },
          },
        ],
      },
    };
    const engine = {
      executeMultipleWithTrace: () => ({
        outputs: {
          heeft_delegaties: true,
          subject_ids: ['999200001'],
          subject_names: ['Kind 1 van der Meer'],
          subject_types: ['CITIZEN'],
          delegation_types: ['OUDERLIJK_GEZAG'],
          permissions: [['LEZEN', 'CLAIMS_INDIENEN', 'BESLUITEN_ONTVANGEN']],
          valid_from_dates: ['2020-01-01'],
          valid_until_dates: [null],
        },
      }),
    };
    const corpus = { latestById: new Map([[law.id, law]]) };
    const { delegations } = delegationsFor(engine, corpus, '999100001', '2025-01-01');
    expect(delegations).toHaveLength(1);
    expect(delegations[0]).toMatchObject({
      subjectId: '999200001',
      subjectType: 'CITIZEN',
      delegationType: 'OUDERLIJK_GEZAG',
      lawName: 'Gezag',
    });
  });
});
