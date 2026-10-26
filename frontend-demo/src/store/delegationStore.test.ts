/**
 * Wat machtigen doet met de rest van de demo: over wie de wet rekent, welke
 * regelingen er staan, en wat er wel en niet mag. Dit is de integratie tussen
 * `data/delegation.ts` en de rest van de store, dus die logica staat hier los
 * getest — de store zelf trekt een corpus en een engine binnen, en die zijn er
 * in een unittest niet.
 */
import { describe, expect, it } from 'vitest';
import { computed, ref } from 'vue';
import { delegationKey, delegationsFor, maySubmitClaims, startDelegationKey } from '../data/delegation';
import type { Delegation } from '../data/delegation';
import type { DemoProfileConfig } from '../data/loadCorpus';

/** A delegation as the fixtures below write it: without the provider's service. */
type DelegationFixture = Omit<Delegation, 'service'>;

/** A profile as far as the context reads it. */
type ProfileFixture = Pick<DemoProfileConfig, 'bsn'> & Partial<DemoProfileConfig>;

/**
 * Dezelfde afleidingen als in demoStore.ts, met de invoer als losse refs. Wat
 * hier bewezen wordt is de regel, niet de bedrading.
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
  const key = ref<string | null>(null);
  const list = computed(() => (delegationEnabledFlag ? delegations : []));
  const active = computed(() => {
    const start = startDelegationKey(profile);
    // De startmachtiging volgt uit het profiel en de wet; de vlag gaat alleen
    // over wat er in de werkbalk te kiezen is.
    const k = (delegationEnabledFlag ? key.value : null) ?? start;
    if (!k || k === 'SELF') return null;
    const pool = k === start ? delegations : list.value;
    const found = pool.find((d) => delegationKey(d) === k) ?? null;
    return found && found.subjectType !== 'SELF' ? found : null;
  });
  const setDelegation = (d: DelegationFixture | null) => {
    key.value = !d || d.subjectType === 'SELF' ? 'SELF' : delegationKey(d);
  };
  const personaParams = () => {
    const d = active.value;
    if (d?.subjectType === 'BUSINESS') return { kvk_nummer: d.subjectId };
    if (d?.subjectType === 'CITIZEN') return { bsn: d.subjectId };
    return { bsn: profile.bsn };
  };
  const subjectBsn = () => (active.value?.subjectType === 'CITIZEN' ? active.value.subjectId : profile.bsn);
  const wantedDiscoverable = computed(() => {
    const d = active.value;
    return d?.subjectType === 'BUSINESS' ? 'BUSINESS' : 'CITIZEN';
  });
  return {
    key,
    list,
    active,
    setDelegation,
    personaParams,
    subjectBsn,
    wantedDiscoverable,
    canSubmitClaims: computed(() => maySubmitClaims(active.value)),
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
    expect(c.active.value).toBeNull();
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
    expect(c.wantedDiscoverable.value).toBe('CITIZEN');
    c.setDelegation(EIGEN_ZAAK);
    expect(c.wantedDiscoverable.value).toBe('BUSINESS');
    c.setDelegation(KIND);
    expect(c.wantedDiscoverable.value).toBe('CITIZEN');
  });

  it('behandelt "Mezelf" als het ontbreken van een machtiging', () => {
    const c = ctx();
    c.setDelegation(KIND);
    expect(c.active.value).not.toBeNull();
    c.setDelegation(ZELF);
    expect(c.active.value).toBeNull();
    expect(c.personaParams()).toEqual({ bsn: '999100001' });
  });

  it('staat corrigeren toe met het recht daartoe en verbiedt het zonder', () => {
    const c = ctx();
    expect(c.canSubmitClaims.value).toBe(true);
    c.setDelegation(EIGEN_ZAAK);
    expect(c.canSubmitClaims.value).toBe(true);
    c.setDelegation(VENNOOTSCHAP);
    expect(c.canSubmitClaims.value).toBe(false);
  });

  it('valt terug op de burger zelf als de bewaarde machtiging niet meer bestaat', () => {
    // Een keuze uit een vorige sessie die de wet nu niet meer geeft: bijvoorbeeld
    // een kind dat meerderjarig is geworden.
    const c = makeContext({ delegations: [ZELF], profile: MERIJN });
    c.setDelegation(KIND);
    expect(c.active.value).toBeNull();
    expect(c.personaParams()).toEqual({ bsn: '999100001' });
    expect(c.canSubmitClaims.value).toBe(true);
  });

  it('geeft geen machtigingen als de vlag voor dit profiel uit staat', () => {
    const c = makeContext({ delegations: [ZELF, KIND], profile: MERIJN, delegationEnabledFlag: false });
    expect(c.list.value).toEqual([]);
    c.setDelegation(KIND);
    expect(c.active.value).toBeNull();
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
    expect(c.active.value).toEqual(NOON);
    expect(c.personaParams()).toEqual({ kvk_nummer: '85234567' });
    expect(c.wantedDiscoverable.value).toBe('BUSINESS');
  });

  it('is als "Mezelf" een burger zoals ieder ander, zonder KvK-nummer', () => {
    const c = ctx();
    c.setDelegation(ZELF);
    expect(c.active.value).toBeNull();
    expect(c.personaParams()).toEqual({ bsn: '999999990' });
    expect(c.wantedDiscoverable.value).toBe('CITIZEN');
  });

  it('begint ook namens de onderneming met een opgeslagen staat van vóór start_namens', () => {
    // Zo'n staat heeft delegationKey: null; dat is geen gekozen Mezelf.
    const c = ctx();
    c.key.value = null;
    expect(c.active.value).toEqual(NOON);
  });

  it('begint ook namens de onderneming als DELEGATION uit staat; alleen de keuze verdwijnt', () => {
    const c = makeContext({ delegations: [ZELF, NOON], profile: CLAUDIA, delegationEnabledFlag: false });
    expect(c.active.value).toEqual(NOON);
    expect(c.personaParams()).toEqual({ kvk_nummer: '85234567' });
    expect(c.wantedDiscoverable.value).toBe('BUSINESS');
    expect(c.list.value).toEqual([]);
  });

  it('negeert een opgeslagen "Mezelf" zolang DELEGATION uit staat', () => {
    const c = makeContext({ delegations: [ZELF, NOON], profile: CLAUDIA, delegationEnabledFlag: false });
    c.key.value = 'SELF';
    expect(c.active.value).toEqual(NOON);
    expect(c.personaParams()).toEqual({ kvk_nummer: '85234567' });
  });

  it('begint als zichzelf als de wet de startmachtiging niet geeft', () => {
    const c = makeContext({ delegations: [ZELF], profile: CLAUDIA });
    expect(c.active.value).toBeNull();
    expect(c.personaParams()).toEqual({ bsn: '999999990' });
  });
});

/**
 * Dezelfde matchregel als `demoStore.findCase`. Welk veld telt volgt de wet:
 * een wet over een onderneming heeft haar zaak op het KvK-nummer, een wet over
 * een persoon op de BSN.
 */
interface CaseFixture {
  id: string;
  lawId: string;
  status: string;
  bsn?: string;
  kvk: string | null;
}

function findCase(
  cases: CaseFixture[],
  law: { id: string; discoverable: string },
  params: { bsn?: string; kvk_nummer?: string },
): CaseFixture | null {
  const onBusiness = law.discoverable === 'BUSINESS' && params.kvk_nummer !== undefined;
  return (
    cases.find(
      (c) => c.lawId === law.id && c.status !== 'WITHDRAWN' && (onBusiness ? c.kvk === params.kvk_nummer : c.bsn === params.bsn),
    ) ?? null
  );
}

describe('de zaak van het juiste onderwerp', () => {
  const bedrijfswet = { id: 'accijns', discoverable: 'BUSINESS' };
  const burgerwet = { id: 'zorgtoeslag', discoverable: 'CITIZEN' };

  it('vindt de zaak van een onderneming ongeacht wie hem indiende', () => {
    // De eigenaar diende in; de zaak staat op háár BSN en op de KvK.
    const doorEigenaar = { id: 'z1', lawId: 'accijns', status: 'IN_REVIEW', bsn: '999999990', kvk: '85234567' };
    // Een gemachtigde ziet dezelfde zaak: het gaat om dezelfde onderneming.
    expect(findCase([doorEigenaar], bedrijfswet, { kvk_nummer: '85234567' })?.id).toBe('z1');
    // En de eigenaar zelf ook, die bsn én kvk bij zich draagt. Op de BSN
    // matchen liet juist deze zaak wegvallen zodra een ander hem indiende.
    expect(findCase([doorEigenaar], bedrijfswet, { bsn: '999999990', kvk_nummer: '85234567' })?.id).toBe('z1');
  });

  it('laat de eigenaar de zaak zien die een gemachtigde namens zijn bedrijf indiende', () => {
    const doorGemachtigde = { id: 'z2', lawId: 'accijns', status: 'IN_REVIEW', bsn: '999100001', kvk: '85234567' };
    expect(findCase([doorGemachtigde], bedrijfswet, { bsn: '999999990', kvk_nummer: '85234567' })?.id).toBe('z2');
  });

  it('houdt de zaak van een andere onderneming buiten beeld', () => {
    const andere = { id: 'z3', lawId: 'accijns', status: 'IN_REVIEW', bsn: '999999990', kvk: '99001122' };
    expect(findCase([andere], bedrijfswet, { kvk_nummer: '85234567' })).toBeNull();
  });

  it('houdt bij een burgerwet de zaak van een ander buiten beeld', () => {
    const vanHetKind = { id: 'z4', lawId: 'zorgtoeslag', status: 'IN_REVIEW', bsn: '999200001', kvk: null };
    expect(findCase([vanHetKind], burgerwet, { bsn: '999200001' })?.id).toBe('z4');
    // De ouder ziet de zaak van zijn kind niet zolang hij voor zichzelf handelt.
    expect(findCase([vanHetKind], burgerwet, { bsn: '999100001' })).toBeNull();
  });

  it('negeert een ingetrokken zaak', () => {
    const ingetrokken = { id: 'z5', lawId: 'accijns', status: 'WITHDRAWN', bsn: '999999990', kvk: '85234567' };
    expect(findCase([ingetrokken], bedrijfswet, { kvk_nummer: '85234567' })).toBeNull();
  });
});

/**
 * Dezelfde afleiding als `demoStore.featureEnabled`: wat de presentator omzette
 * wint van het profiel, de rest volgt demo-config.yaml.
 */
function featureEnabled(
  profile: { feature_flags?: Record<string, boolean> } | null | undefined,
  overrides: Record<string, boolean> | null | undefined,
  key: string,
): boolean {
  const override = overrides?.[key];
  if (override !== undefined) return override;
  return !!profile?.feature_flags?.[key];
}

describe('vlaggen die de presentator omzet', () => {
  const profile = { feature_flags: { DELEGATION: true, CHANGE_WIZARD: true, HARMONIZE: false } };

  it('volgt het profiel zolang er niets is omgezet', () => {
    expect(featureEnabled(profile, {}, 'DELEGATION')).toBe(true);
    expect(featureEnabled(profile, {}, 'HARMONIZE')).toBe(false);
    // Een vlag die het profiel niet noemt staat uit.
    expect(featureEnabled(profile, {}, 'AUTO_APPROVE_CLAIMS')).toBe(false);
  });

  it('laat een omgezette vlag winnen van het profiel, allebei de kanten op', () => {
    expect(featureEnabled(profile, { DELEGATION: false }, 'DELEGATION')).toBe(false);
    expect(featureEnabled(profile, { HARMONIZE: true }, 'HARMONIZE')).toBe(true);
  });

  it('raakt de andere vlaggen niet aan', () => {
    const overrides = { DELEGATION: false };
    expect(featureEnabled(profile, overrides, 'CHANGE_WIZARD')).toBe(true);
  });

  it('valt terug op het profiel als de overrides leeg zijn', () => {
    // "Features terug naar het profiel" leegt de lijst; false is dan weer
    // false omdat het profiel dat zegt, niet omdat het is uitgezet.
    expect(featureEnabled(profile, {}, 'DELEGATION')).toBe(true);
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
