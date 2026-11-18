import { describe, expect, it } from 'vitest';
import { askedInputsFor, claimKeyFor, evaluationParamsFor, inputKind, missingFactsOf, nextQuestions, parseAnswer } from './askedInputs';
import type { AskedInput } from './askedInputs';

const law = {
  id: 'terras',
  doc: {
    articles: [
      {
        machine_readable: {
          execution: {
            parameters: [
              { name: 'kvk_nummer', type: 'string' },
              { name: 'terras_oppervlakte', type: 'number', required: false },
              { name: 'bereidt_voedsel', type: 'boolean' },
            ],
            input: [{ name: 'huurprijs', type: 'amount', source: {} }],
          },
        },
      },
    ],
  },
};
const corpus = { bindings: { terras: { huurprijs: { kind: 'claim', service: 'TOESLAGEN' }, adres: { kind: 'table', service: 'KVK' } } } };

const unknown = (...facts: Array<[name: string, kind?: string, lawId?: string]>) => ({ __unknown: true, missing: facts.map(([name, kind = 'no_data', lawId = 'terras']) => ({ law: lawId, name, kind })) });

describe('askedInputsFor', () => {
  it('lists claim inputs and non-identity parameters with their claims and whether they are required', () => {
    const claimFor = (lawId: string, name: string) => (name === 'huurprijs' ? { newValue: 65000 } : null);
    const asked = askedInputsFor(corpus, law, claimFor);
    expect(asked.map((a) => a.name)).toEqual(['huurprijs', 'terras_oppervlakte', 'bereidt_voedsel']);
    expect(asked[0]).toMatchObject({ isParameter: false, required: false, claim: { newValue: 65000 } });
    expect(asked[0]?.spec?.type).toBe('amount');
    expect(asked[1]).toMatchObject({ isParameter: true, required: false, claim: null });
    // `required` defaults to true when the law does not say.
    expect(asked[2]).toMatchObject({ isParameter: true, required: true });
  });

  it('knows which register inputs a parameter is the lookup key for', () => {
    const keyed = {
      bindings: {
        terras: {
          beschikbare_oppervlakte: { kind: 'table', service: 'GEMEENTE', table: 'bgt', select_on: [{ name: 'adres', value: '$adres' }, { name: 'oppervlakte', value: '$terras_oppervlakte' }] },
          huurprijs: { kind: 'claim', service: 'TOESLAGEN' },
        },
      },
    };
    const asked = askedInputsFor(keyed, law, () => null);
    expect(asked.find((a) => a.name === 'terras_oppervlakte')?.feeds).toEqual(['beschikbare_oppervlakte']);
    expect(asked.find((a) => a.name === 'bereidt_voedsel')?.feeds).toEqual([]);
  });

  it('builds evaluation parameters from the answered form parameters and leaves the rest out', () => {
    const asked = askedInputsFor(corpus, law, (lawId: string, name: string) => (name === 'terras_oppervlakte' ? { newValue: 18 } : null));
    expect(evaluationParamsFor({ bsn: '1', kvk_nummer: '2' }, asked)).toEqual({ bsn: '1', kvk_nummer: '2', terras_oppervlakte: 18 });
    const none = askedInputsFor(corpus, law, () => null);
    // Not `terras_oppervlakte: null`: an unanswered question is not an absence.
    expect(evaluationParamsFor({ kvk_nummer: '2' }, none)).toEqual({ kvk_nummer: '2' });
  });
});

describe('inputKind / parseAnswer', () => {
  it('derives the control from the spec', () => {
    expect(inputKind({ type: 'amount' })).toBe('amount');
    expect(inputKind({ type: 'number', type_spec: { unit: 'eurocent' } })).toBe('amount');
    expect(inputKind({ type: 'boolean' })).toBe('boolean');
    expect(inputKind({ type: 'date' })).toBe('date');
    expect(inputKind({ type: 'array' })).toBe('json');
    expect(inputKind(null, 'x')).toBe('text');
  });

  it('parses euros to eurocent, Dutch decimals, booleans and JSON', () => {
    expect(parseAnswer('amount', '650,50')).toBe(65050);
    expect(parseAnswer('amount', '1.250')).toBe(125000);
    expect(parseAnswer('number', '18,5')).toBe(18.5);
    expect(parseAnswer('boolean', 'true')).toBe(true);
    expect(parseAnswer('json', '["a"]')).toEqual(['a']);
    expect(parseAnswer('json', '{')).toBeUndefined();
    expect(parseAnswer('amount', '')).toBeUndefined();
    expect(parseAnswer('text', ' voor ')).toBe('voor');
  });
});

describe('missingFactsOf', () => {
  it('collects the missing facts of every unknown output once, in order', () => {
    const evaluation = { ok: true, outputs: { a: unknown(['servicekosten'], ['huurprijs']), b: unknown(['huurprijs'], ['spaargeld', 'no_data', 'ib']), c: 5 } };
    expect(missingFactsOf(evaluation).map((f) => `${f.law}.${f.name}`)).toEqual(['terras.servicekosten', 'terras.huurprijs', 'ib.spaargeld']);
    expect(missingFactsOf(null)).toEqual([]);
  });
});

describe('nextQuestions / claimKeyFor', () => {
  const asked: Omit<AskedInput, 'spec'>[] = [
    { name: 'huurprijs', claim: null, isParameter: false, required: false },
    { name: 'servicekosten', claim: null, isParameter: false, required: false },
    { name: 'subsidiabele_servicekosten', claim: null, isParameter: false, required: false },
    { name: 'terras_locatie', claim: null, isParameter: true, required: false },
  ];

  it('asks the claim inputs the outcome misses, in the order the engine reached them', () => {
    const evaluation = { ok: true, outputs: { voldoet: unknown(['servicekosten'], ['huurprijs'], ['inkomen']) } };
    expect(nextQuestions(asked, evaluation, 'terras').map((a) => a.name)).toEqual(['servicekosten', 'huurprijs']);
  });

  it('asks a form parameter only when the outcome misses it as not passed', () => {
    const notReached = { ok: true, outputs: { voldoet: unknown(['huurprijs']) } };
    expect(nextQuestions(asked, notReached, 'terras').map((a) => a.name)).toEqual(['huurprijs']);
    const reached = { ok: true, outputs: { voldoet: unknown(['terras_locatie', 'not_passed']) } };
    expect(nextQuestions(asked, reached, 'terras').map((a) => a.name)).toEqual(['terras_locatie']);
    // A parameter missing as no_data is a register input somewhere, not this question.
    const wrongKind = { ok: true, outputs: { voldoet: unknown(['terras_locatie', 'no_data']) } };
    expect(nextQuestions(asked, wrongKind, 'terras')).toEqual([]);
  });

  it('asks a form parameter when a register input it is the lookup key for is missing', () => {
    const keyed = asked.map((a) => (a.name === 'terras_locatie' ? { ...a, feeds: ['beschikbare_oppervlakte'] } : a));
    const evaluation = { ok: true, outputs: { voldoet: unknown(['beschikbare_oppervlakte', 'no_data']) } };
    expect(nextQuestions(keyed, evaluation, 'terras').map((a) => a.name)).toEqual(['terras_locatie']);
  });

  it('ignores facts another law misses', () => {
    const evaluation = { ok: true, outputs: { voldoet: unknown(['huurprijs', 'no_data', 'andere_wet']) } };
    expect(nextQuestions(asked, evaluation, 'terras')).toEqual([]);
  });

  it('asks a required parameter first, whatever the outcome says', () => {
    const withRequired = [...asked, { name: 'bereidt_voedsel', claim: null, isParameter: true, required: true }];
    const failed = { ok: false, error: 'Variable not found: bereidt_voedsel' };
    expect(nextQuestions(withRequired, failed, 'terras').map((a) => a.name)).toEqual(['bereidt_voedsel']);
    const evaluation = { ok: true, outputs: { voldoet: unknown(['huurprijs']) } };
    expect(nextQuestions(withRequired, evaluation, 'terras').map((a) => a.name)).toEqual(['bereidt_voedsel', 'huurprijs']);
  });

  it('skips answered inputs and asks nothing without unanswered ones or without an outcome', () => {
    const answered = asked.map((a) => ({ ...a, claim: { newValue: 1 } }));
    expect(nextQuestions(answered, null, 'terras')).toEqual([]);
    expect(nextQuestions(asked, null, 'terras')).toEqual([]);
  });

  it('keys answers on the KVK number for a business law', () => {
    expect(claimKeyFor(law, { bsn: '1', kvk_nummer: '2' })).toEqual({ keyField: 'kvk_nummer', keyValue: '2' });
    const citizenLaw = { doc: { articles: [{ machine_readable: { execution: { parameters: [{ name: 'bsn' }] } } }] } };
    expect(claimKeyFor(citizenLaw, { bsn: '1', kvk_nummer: '2' })).toEqual({ keyField: 'bsn', keyValue: '1' });
  });
});
