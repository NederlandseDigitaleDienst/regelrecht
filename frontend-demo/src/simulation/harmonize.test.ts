import { describe, expect, it } from 'vitest';
import { setLocale } from '../i18n/index.js';
import {
  CITIZEN_FEATURES,
  describeModel,
  etaSquared,
  evaluateModel,
  featuresFor,
  mean,
  predict,
  quantile,
  taxLawIds,
  trainBracketModel,
  trainingData,
} from './harmonize';
import type { BracketGroup, HarmonizeRun } from './harmonize';
import type { LawResult } from './stats';

/** A hand-built model: what `predict` and `describeModel` read, with the labels the tests set. */
interface TestModel {
  primary: { key: string; labelKey: string };
  groupKeys: { key: string; labelKey: string; kind: string }[];
  groups: BracketGroup[];
}

/** A law result with only the fields a test sets; `trainingData` reads ok, met and amount. */
function lawResult(fields: Partial<LawResult> & { ok: boolean }): LawResult {
  return fields as LawResult;
}

describe('quantile', () => {
  it('interpoleert lineair tussen waarnemingen, zoals numpy', () => {
    const xs = [0, 10, 20, 30, 40];
    expect(quantile(xs, 0)).toBe(0);
    expect(quantile(xs, 1)).toBe(40);
    expect(quantile(xs, 0.5)).toBe(20);
    expect(quantile(xs, 0.25)).toBe(10);
    // Tussen twee punten in: 0,375 ligt op driekwart tussen index 1 en 2.
    expect(quantile(xs, 0.375)).toBeCloseTo(15, 6);
  });

  it('gaat om met een lege en een eenpuntsreeks', () => {
    expect(quantile([], 0.5)).toBe(0);
    expect(quantile([7], 0.9)).toBe(7);
  });
});

describe('etaSquared', () => {
  it('is 1 als de groep het bedrag volledig bepaalt', () => {
    const groups = [[100, 100, 100], [200, 200, 200]];
    const all = groups.flat();
    const m = mean(all);
    const ssTotal = all.reduce((s, v) => s + (v - m) ** 2, 0);
    expect(etaSquared(groups, m, ssTotal)).toBeCloseTo(1, 6);
  });

  it('is 0 als de groepen niet verschillen', () => {
    const groups = [[100, 200], [100, 200]];
    const all = groups.flat();
    const m = mean(all);
    const ssTotal = all.reduce((s, v) => s + (v - m) ** 2, 0);
    expect(etaSquared(groups, m, ssTotal)).toBeCloseTo(0, 6);
  });

  it('is 0 als er geen spreiding te verklaren valt', () => {
    expect(etaSquared([[5, 5], [5, 5]], 5, 0)).toBe(0);
  });
});

// Een nagebootste simulatierun: bevolking met bekende eigenschappen, en een
// wet waarvan we de vorm zelf kiezen, zodat te controleren is of het model
// hem terugvindt.
interface FakeSubject {
  id: string;
  inkomen: number;
  leeftijd: number;
  huur: number;
  kinderen: number;
  partner: boolean;
  huurder: boolean;
  student: boolean;
}

function fakeRun(makeAmount: (subject: FakeSubject) => number, { count = 400, kind = 'burgers' }: { count?: number; kind?: string } = {}): HarmonizeRun<FakeSubject> {
  const results: HarmonizeRun<FakeSubject>['results'][number][] = [];
  for (let i = 0; i < count; i += 1) {
    const subject: FakeSubject = {
      id: `p${i}`,
      inkomen: 10000 + (i % 50) * 1000,
      leeftijd: 20 + (i % 45),
      huur: 500 + (i % 10) * 50,
      kinderen: i % 3,
      partner: i % 2 === 0,
      huurder: i % 4 !== 0,
      student: i % 11 === 0,
    };
    const amount = makeAmount(subject);
    results.push({
      subject,
      laws: { toeslag: { ok: true, met: amount > 0, amount, amountName: 'bedrag', error: null } },
    });
  }
  return { kind, results };
}

describe('trainingData', () => {
  it('telt de bedragen van de gekozen wetten bij elkaar op', () => {
    const run = {
      kind: 'burgers',
      results: [
        {
          subject: { inkomen: 20000, leeftijd: 30, huur: 600, kinderen: 1, partner: true, huurder: true, student: false },
          laws: {
            zorg: { ok: true, met: true, amount: 100 },
            huur: { ok: true, met: true, amount: 250 },
          },
        },
      ],
    };
    const { rows } = trainingData(run, ['zorg', 'huur']);
    expect(rows).toHaveLength(1);
    expect(rows[0]?.amount).toBe(350);
    expect(rows[0]?.eligible).toBe(true);
    expect(rows[0]?.values.heeft_partner).toBe(1);
    expect(rows[0]?.values.heeft_kinderen).toBe(1);
  });

  it('slaat een subject over waarvoor geen enkele gekozen wet kon rekenen', () => {
    const run = {
      kind: 'burgers',
      results: [
        { subject: { inkomen: 1 }, laws: { zorg: lawResult({ ok: false, error: 'kapot' }) } },
        { subject: { inkomen: 2 }, laws: { zorg: { ok: true, met: true, amount: 10 } } },
      ],
    };
    expect(trainingData(run, ['zorg']).rows).toHaveLength(1);
  });

  it('telt een onbekende uitkomst niet als recht', () => {
    const run: HarmonizeRun = {
      kind: 'burgers',
      results: [{ subject: { inkomen: 1 }, laws: { zorg: { ok: true, met: 'unknown', amount: null } } }],
    };
    expect(trainingData(run, ['zorg']).rows[0]?.eligible).toBe(false);
  });

  it('kiest de ondernemerskenmerken bij een ondernemersrun', () => {
    expect(featuresFor('ondernemers').map((f) => f.key)).toContain('oppervlakte');
    expect(featuresFor('burgers')).toEqual(CITIZEN_FEATURES);
  });

  it('trekt een belasting af in plaats van hem op te tellen', () => {
    const run = {
      kind: 'burgers',
      results: [
        {
          subject: { inkomen: 40000, leeftijd: 40, huur: 0, kinderen: 0, partner: false, huurder: false, student: false },
          laws: {
            wet_inkomstenbelasting: { ok: true, met: null, amount: 4000 },
            zorgtoeslagwet: { ok: true, met: true, amount: 1600 },
          },
        },
      ],
    };
    const zonder = trainingData(run, ['wet_inkomstenbelasting', 'zorgtoeslagwet']);
    expect(zonder.rows[0]?.amount).toBe(5600);
    // Met de belasting als belasting: € 1.600 toeslag min € 4.000 belasting.
    const met = trainingData(run, ['wet_inkomstenbelasting', 'zorgtoeslagwet'], new Set(['wet_inkomstenbelasting']));
    expect(met.rows[0]?.amount).toBe(-2400);
  });
});

describe('taxLawIds', () => {
  it('leest uit de configuratie welke wetten geld kosten', () => {
    const corpus = {
      config: {
        simulation: {
          disposable_income: [
            { law: 'wet_inkomstenbelasting', kind: 'tax' },
            { law: 'zorgverzekeringswet/bijdrage', kind: 'tax' },
            { law: 'zorgtoeslagwet', kind: 'benefit' },
          ],
        },
      },
    };
    expect([...taxLawIds(corpus)].sort()).toEqual(['wet_inkomstenbelasting', 'zorgverzekeringswet/bijdrage']);
  });

  it('geeft een lege verzameling zonder configuratie', () => {
    expect(taxLawIds(null).size).toBe(0);
    expect(taxLawIds({ config: {} }).size).toBe(0);
  });
});

describe('trainBracketModel', () => {
  it('vindt een rechtlijnige afbouw op inkomen vrijwel exact terug', () => {
    // Een simpele regeling: € 2000 minus 5% van het inkomen, nooit onder nul.
    const run = fakeRun((s) => Math.max(0, 2000 - 0.05 * s.inkomen));
    const model = trainBracketModel(trainingData(run, ['toeslag']), { primary: 'inkomen' });
    expect(model.primary.key).toBe('inkomen');
    // Een rechte lijn is precies wat een staffel kan; de fout hoort klein te zijn.
    expect(model.metrics.r2).toBeGreaterThan(0.98);
    expect(model.metrics.mae).toBeLessThan(25);
  });

  it('geeft groepen met een eigen bedrag een eigen staffel', () => {
    // Wie een partner heeft krijgt structureel € 500 meer: dat hoort het
    // model als aparte staffel terug te vinden.
    const run = fakeRun((s) => Math.max(0, 2000 - 0.05 * s.inkomen) + (s.partner ? 500 : 0));
    const model = trainBracketModel(trainingData(run, ['toeslag']), { primary: 'inkomen' });
    expect(model.groupKeys.map((f) => f.key)).toContain('heeft_partner');
    const withPartner = predict(model, { inkomen: 20000, heeft_partner: 1, heeft_kinderen: 0, huurder: 1, student: 0 });
    const without = predict(model, { inkomen: 20000, heeft_partner: 0, heeft_kinderen: 0, huurder: 1, student: 0 });
    expect(withPartner - without).toBeGreaterThan(300);
  });

  it('houdt de staffel doorlopend over een trederand heen', () => {
    // Meten moet óver de grens: `predict` neemt de eerste trede waar x in past
    // en de randen zijn aan beide kanten inclusief, dus `upper - 1` en `upper`
    // liggen allebei op dezelfde lijn. Zo bleef een sprong van € 512 als 0,24
    // uit de test komen.
    const run = fakeRun((s) => Math.max(0, 3000 - 0.00004 * s.inkomen ** 1.6));
    const model = trainBracketModel(trainingData(run, ['toeslag']), { primary: 'inkomen' });
    const group = model.groups[0];
    if (!group) throw new Error('the model has no group');
    expect(group.steps.length).toBeGreaterThan(1);
    for (const step of group.steps) {
      const atEdge = predict(model, { ...zeroes(model), inkomen: step.upper });
      const justAbove = predict(model, { ...zeroes(model), inkomen: step.upper + 1 });
      // Elke trede wordt apart gefit, dus de twee kanten van een grens hoeven
      // niet tot op de cent gelijk te zijn. Wat telt is dat het verschil klein
      // blijft: één euro meer inkomen mag geen zichtbare sprong geven.
      expect(Math.abs(justAbove - atEdge)).toBeLessThanOrEqual(5);
    }
  });

  it('rondt de trederanden af op leesbare getallen', () => {
    const run = fakeRun((s) => Math.max(0, 2000 - 0.05 * s.inkomen));
    const model = trainBracketModel(trainingData(run, ['toeslag']), { primary: 'inkomen' });
    for (const b of model.boundaries) expect(b % 1000).toBe(0);
  });

  it('geeft wie geen eigen staffel kreeg een reststaffel, niet die van een andere groep', () => {
    // Twee groepskenmerken waarvan maar twee van de vier cellen groot genoeg
    // waren. Zonder reststaffel viel wie erbuiten viel terug op de eerste
    // groep in de lijst — een wíllekeurige andere combinatie.
    const stap = (a: number) => ({ lower: 0, upper: 100000, amountAtLower: a, amountAtUpper: a, count: 100 });
    const model: TestModel = {
      primary: { key: 'inkomen', labelKey: 'harm.feature.income' },
      groupKeys: [
        { key: 'heeft_partner', labelKey: 'harm.feature.has_partner', kind: 'boolean' },
        { key: 'huurder', labelKey: 'harm.feature.rents_a_home', kind: 'boolean' },
      ],
      groups: [
        { filter: { heeft_partner: 0, huurder: 0 }, keys: ['heeft_partner', 'huurder'], steps: [stap(100)], count: 200 },
        { filter: { heeft_partner: 1, huurder: 1 }, keys: ['heeft_partner', 'huurder'], steps: [stap(900)], count: 200 },
        { filter: {}, keys: [], steps: [stap(500)], count: 20 },
      ],
    };
    // Een cel met een eigen staffel houdt die.
    expect(predict(model, { inkomen: 30000, heeft_partner: 1, huurder: 1 })).toBe(900);
    // Een cel zonder eigen staffel krijgt de rest, niet de 100 van de eerste groep.
    expect(predict(model, { inkomen: 30000, heeft_partner: 1, huurder: 0 })).toBe(500);
  });

  it('noemt de reststaffel apart in de tabel', () => {
    const stap = (a: number) => ({ lower: 0, upper: 100000, amountAtLower: a, amountAtUpper: a, count: 10 });
    const metRest: TestModel = {
      primary: { key: 'inkomen', labelKey: 'harm.feature.income' },
      groupKeys: [{ key: 'heeft_partner', labelKey: 'harm.feature.has_partner', kind: 'boolean' }],
      groups: [
        { filter: { heeft_partner: 1 }, keys: ['heeft_partner'], steps: [stap(900)], count: 200 },
        { filter: {}, keys: [], steps: [stap(500)], count: 20 },
      ],
    };
    expect(describeModel(metRest).map((r) => r.group)).toEqual(['Heeft partner: ja', 'Overige combinaties']);
    // Eén groep zonder kenmerken is gewoon iedereen.
    const alleen = { ...metRest, groups: [{ filter: {}, keys: [], steps: [stap(500)], count: 220 }] };
    expect(describeModel(alleen).map((r) => r.group)).toEqual(['Iedereen']);
  });

  it('names the groups in the active language', () => {
    const step = (a: number) => ({ lower: 0, upper: 100000, amountAtLower: a, amountAtUpper: a, count: 10 });
    const model: TestModel = {
      primary: { key: 'inkomen', labelKey: 'harm.feature.income' },
      groupKeys: [{ key: 'heeft_partner', labelKey: 'harm.feature.has_partner', kind: 'boolean' }],
      groups: [
        { filter: { heeft_partner: 0 }, keys: ['heeft_partner'], steps: [step(900)], count: 200 },
        { filter: {}, keys: [], steps: [step(500)], count: 20 },
      ],
    };
    setLocale('en');
    try {
      expect(describeModel(model).map((r) => r.group)).toEqual(['Has a partner: no', 'Other combinations']);
      expect(describeModel({ ...model, groups: [model.groups[1]!] }).map((r) => r.group)).toEqual(['Everyone']);
    } finally {
      setLocale('nl');
    }
  });

  it('weigert te leren van te weinig gegevens', () => {
    const run = fakeRun(() => 100, { count: 5 });
    expect(() => trainBracketModel(trainingData(run, ['toeslag']))).toThrow(/te weinig gegevens/i);
  });

  it('levert altijd een staffel, ook als geen enkel kenmerk groepen rechtvaardigt', () => {
    // Een vast bedrag voor iedereen: niets om op te groeperen.
    const run = fakeRun(() => 500);
    const model = trainBracketModel(trainingData(run, ['toeslag']), { primary: 'inkomen' });
    expect(model.groups.length).toBeGreaterThanOrEqual(1);
    expect(predict(model, { ...zeroes(model), inkomen: 25000 })).toBeCloseTo(500, 0);
  });
});

function zeroes(model: { groupKeys: readonly { key: string }[] }): Record<string, number> {
  const out: Record<string, number> = {};
  for (const f of model.groupKeys) out[f.key] = 0;
  return out;
}

describe('evaluateModel', () => {
  it('vangt een uitzondering die op een kenmerk hangt in een eigen staffel', () => {
    // Een vaste bonus voor studenten, los van het inkomen. Dat is geen
    // uitzondering die de staffel niet kan volgen: het hangt aan een kenmerk,
    // dus het model hoort er een aparte staffel voor te maken en daarna
    // vrijwel geen fout meer te hebben.
    const run = fakeRun((s) => Math.max(0, 2000 - 0.05 * s.inkomen) + (s.student ? 3000 : 0));
    const model = trainBracketModel(trainingData(run, ['toeslag']), { primary: 'inkomen' });
    expect(model.groupKeys.map((f) => f.key)).toContain('student');
    expect(model.metrics.r2).toBeGreaterThan(0.98);
  });

  it('houdt een uitzondering over die aan geen enkel kenmerk hangt', () => {
    // Een bonus die aan niets hangt wat het model kan zien (hier: een
    // willekeurige groep). Zo'n regel kan een staffel niet volgen, en dan
    // hoort hij bij de grootste afwijkingen terug te komen — dat is precies
    // waar harmonisatie iets over de wet zegt.
    const run = fakeRun((s) => Math.max(0, 2000 - 0.05 * s.inkomen) + (Number(s.id.slice(1)) % 97 === 0 ? 4000 : 0));
    const model = trainBracketModel(trainingData(run, ['toeslag']), { primary: 'inkomen' });
    expect(model.metrics.worst).toHaveLength(5);
    // De onverklaarbare gevallen zitten er ver naast, niet een paar euro.
    // Math.abs(undefined) is NaN, which is not greater than 1000.
    expect(Math.abs(model.metrics.worst[0]?.error as number)).toBeGreaterThan(1000);
  });

  it('geeft nul terug zonder gegevens', () => {
    // De assertie op r2 en mae alleen is niets waard: zonder rijen komt er
    // sowieso 0 uit, ook als de bewaking weg is. Wat de bewaking wél doet is
    // `worst` als lege lijst opleveren in plaats van te struikelen, en dat is
    // waar het scherm op rekent.
    const uitkomst = evaluateModel({ groups: [], primary: { key: 'inkomen' } }, []);
    expect(uitkomst).toEqual({ r2: 0, mae: 0, meanAmount: 0, worst: [] });
    expect(Array.isArray(uitkomst.worst)).toBe(true);
  });

  it('telt de afwijking absoluut, zodat te veel en te weinig elkaar niet opheffen', () => {
    // Een staffel die iedereen € 100 toekent, naast twee mensen die € 0 en
    // € 200 hadden moeten krijgen. Met tekens erbij is het gemiddelde 0, en
    // dan zou het scherm beweren dat het model perfect is terwijl het er bij
    // allebei € 100 naast zit.
    const model = {
      primary: { key: 'inkomen', labelKey: 'harm.feature.income' },
      groupKeys: [],
      groups: [{ filter: {}, keys: [], count: 2, steps: [{ lower: 0, upper: 100000, amountAtLower: 100, amountAtUpper: 100 }] }],
    };
    const rows = [
      { values: { inkomen: 10000 }, amount: 0 },
      { values: { inkomen: 20000 }, amount: 200 },
    ];
    expect(evaluateModel(model, rows).mae).toBe(100);
  });
});

describe('describeModel', () => {
  it('schrijft de staffel op als leesbare tabel', () => {
    const run = fakeRun((s) => Math.max(0, 2000 - 0.05 * s.inkomen) + (s.partner ? 500 : 0));
    const model = trainBracketModel(trainingData(run, ['toeslag']), { primary: 'inkomen' });
    const described = describeModel(model);
    expect(described.length).toBeGreaterThan(0);
    const [first] = described;
    const [firstGroup] = model.groups;
    if (!first || !firstGroup) throw new Error('nothing described');
    expect(first.steps.length).toBe(firstGroup.steps.length);
    expect(first.steps[0]?.range).toMatch(/\d/);
    expect(first.steps[0]?.from).toMatch(/€/);
  });
});
