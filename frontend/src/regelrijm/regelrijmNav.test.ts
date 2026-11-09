import { describe, expect, it } from 'vitest';
import { renderAction } from '@regelrecht/frontend-shared/regelrijm/index';
import { findOutput, isNavigable, rawAt, resolveReference } from './regelrijmNav';

const law = {
  $id: 'voorbeeldwet',
  articles: [
    {
      number: '1',
      machine_readable: {
        definitions: { drempel: { value: 1000, type: 'amount', type_spec: { unit: 'eurocent' } }, factor: 2 },
        execution: {
          parameters: [{ name: 'bsn', type: 'string' }],
          input: [
            { name: 'inkomen', type: 'amount', source: { regulation: 'andere_wet', output: 'toetsingsinkomen' } },
            { name: 'leeftijd', type: 'number', source: { output: 'leeftijd' } },
            { name: 'woonplaats', type: 'string', source: { description: 'BRP' } },
          ],
          actions: [{ output: 'recht', value: { operation: 'AND', conditions: ['$a', '$b'] } }],
        },
      },
    },
    { number: '2', machine_readable: { execution: { actions: [{ output: 'leeftijd', value: 18 }] } } },
  ],
};
const art1 = law.articles[0];

describe('resolveReference', () => {
  it.each([
    ['$recht', { kind: 'action', articleNumber: '1', output: 'recht' }],
    ['$inkomen', { kind: 'external', lawId: 'andere_wet', output: 'toetsingsinkomen' }],
    ['$leeftijd', { kind: 'action', articleNumber: '2', output: 'leeftijd' }],
    ['$woonplaats', { kind: 'data', description: 'BRP' }],
    ['$bsn', { kind: 'parameter' }],
    ['$drempel', { kind: 'definition', value: 1000 }],
    ['$factor', { kind: 'definition', value: 2 }],
    ['$referencedate', { kind: 'context' }],
    ['$medebewoner.inkomen', { kind: 'local', name: 'medebewoner' }],
  ])('%s', (ref, want) => {
    expect(resolveReference(ref, law, art1)).toMatchObject(want);
  });
});

describe('findOutput', () => {
  it('finds the article that computes an output', () => {
    expect(findOutput(law, 'leeftijd')?.article.number).toBe('2');
    expect(findOutput(law, 'bestaat_niet')).toBeNull();
  });
});

describe('isNavigable', () => {
  it('leads to another rule only for a computed or external value', () => {
    expect(isNavigable({ kind: 'action' })).toBe(true);
    expect(isNavigable({ kind: 'external' })).toBe(true);
    for (const kind of ['data', 'parameter', 'definition', 'context', 'local']) expect(isNavigable({ kind })).toBe(false);
  });
});

describe('rawAt', () => {
  // ADD(ADD(a, b), MULTIPLY(c, 2), SUBTRACT(d, e)) reads as one sum: the inner
  // ADD is flattened away, so the reading's paths shift against the YAML.
  const inner = { operation: 'ADD', values: ['$a', '$b'] };
  const times = { operation: 'MULTIPLY', values: ['$c', 2] };
  const minus = { operation: 'SUBTRACT', values: ['$d', '$e'] };
  const sum = { operation: 'ADD', values: [inner, times, minus] };

  it('maps every numbered part of a flattened reading to the operation as written', () => {
    const action = { output: 'totaal', value: sum };
    const { numbers } = renderAction(action);
    const written = new Map([...numbers.keys()].map((k) => [k, rawAt(action, k.split('/'))]));
    // The flattened sum is a, b, times, minus: the two operands moved up.
    expect(written.get('value')).toBe(sum);
    expect(written.get('value/values/2')).toBe(times);
    expect(written.get('value/values/3')).toBe(minus);
    expect([...written.values()]).not.toContain(undefined);
  });

  it('gives the action itself for an inline operation', () => {
    const action = { output: 'totaal', ...sum };
    expect(rawAt(action, ['value'])).toBe(action);
    expect(rawAt(action, ['value', 'values', '2'])).toBe(times);
  });

  it('is undefined where no operation stands', () => {
    const action = { output: 'totaal', value: sum };
    expect(rawAt(action, ['value', 'values', '0'])).toBeUndefined();
    expect(rawAt(action, ['value', 'values', '9'])).toBeUndefined();
    expect(rawAt(action, ['output'])).toBeUndefined();
  });
});
