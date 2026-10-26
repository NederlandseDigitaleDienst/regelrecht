// @vitest-environment node
import { readFileSync } from 'node:fs';
import { fileURLToPath } from 'node:url';
import { describe, expect, it } from 'vitest';
import { OPERATIONS, renderAction, renderFocus, makeVocabulary, normalise, type ArticleContext, type Operation, type Unit } from './index';

const schemaPath = fileURLToPath(new URL('../../../../schema/latest/schema.json', import.meta.url));
const schema = JSON.parse(readFileSync(schemaPath, 'utf8'));

// One complete example per operation. A new operation in the schema fails the
// first test until it has a form here and in the renderer.
const SAMPLES: Record<string, Operation> = {
  ADD: { operation: 'ADD', values: ['$a', '$b'] },
  SUBTRACT: { operation: 'SUBTRACT', values: ['$a', '$b'] },
  MULTIPLY: { operation: 'MULTIPLY', values: ['$a', 0.5] },
  DIVIDE: { operation: 'DIVIDE', values: ['$a', 12] },
  MIN: { operation: 'MIN', values: ['$a', '$b'] },
  MAX: { operation: 'MAX', values: ['$a', '$b'] },
  AND: { operation: 'AND', conditions: ['$a', '$b'] },
  OR: { operation: 'OR', conditions: ['$a', '$b'] },
  NOT: { operation: 'NOT', value: '$a' },
  EQUALS: { operation: 'EQUALS', subject: '$a', value: 'X' },
  GREATER_THAN: { operation: 'GREATER_THAN', subject: '$a', value: 5 },
  LESS_THAN: { operation: 'LESS_THAN', subject: '$a', value: 5 },
  GREATER_THAN_OR_EQUAL: { operation: 'GREATER_THAN_OR_EQUAL', subject: '$leeftijd', value: 18 },
  LESS_THAN_OR_EQUAL: { operation: 'LESS_THAN_OR_EQUAL', subject: '$a', value: 5 },
  IF: { operation: 'IF', cases: [{ when: '$a', then: 1 }], default: 0 },
  IN: { operation: 'IN', subject: '$a', values: [1, 2] },
  LIST: { operation: 'LIST', items: ['$a', '$b'] },
  FOREACH: { operation: 'FOREACH', collection: '$m', as: 'x', body: '$x.b', combine: 'ADD' },
  AGE: { operation: 'AGE', date_of_birth: '$g', reference_date: '$p' },
  DATE_ADD: { operation: 'DATE_ADD', date: '$d', months: 1, days: 3 },
  DATE: { operation: 'DATE', year: '$j', month: 1, day: 1 },
  DAY_OF_WEEK: { operation: 'DAY_OF_WEEK', date: '$d' },
  DATE_DIFF: { operation: 'DATE_DIFF', from: '$a', to: '$b', in: 'days' },
  DATE_PART: { operation: 'DATE_PART', date: '$d', in: 'year' },
  START_OF: { operation: 'START_OF', date: '$d', in: 'month' },
  ROUND: { operation: 'ROUND', value: { operation: 'DIVIDE', values: ['$a', 12] }, precision: 0 },
  CEIL: { operation: 'CEIL', value: '$a', precision: 2 },
  FLOOR: { operation: 'FLOOR', value: '$a', precision: -2 },
};

const EXPECTED: Record<string, string> = {
  ADD: '[a] plus [b]',
  SUBTRACT: '[a] min [b]',
  MULTIPLY: '[a] maal 0,5',
  DIVIDE: '[a] gedeeld door 12',
  MIN: 'de kleinste van ( [a]; [b] )',
  MAX: 'de grootste van ( [a]; [b] )',
  AND: '[a] én [b]',
  OR: '[a] óf [b]',
  NOT: 'niet [a]',
  EQUALS: '[a] is “X”',
  GREATER_THAN: '[a] is meer dan 5',
  LESS_THAN: '[a] is minder dan 5',
  GREATER_THAN_OR_EQUAL: '[leeftijd] is ten minste 18',
  LESS_THAN_OR_EQUAL: '[a] is ten hoogste 5',
  IF: '1 als [a], anders 0',
  IN: '[a] is een van ( 1; 2 )',
  LIST: 'de lijst ( [a]; [b] )',
  FOREACH: 'de som van [b] van [x] over elke [x] in [m]',
  AGE: 'het aantal volle jaren tussen [g] en [p]',
  DATE_ADD: '1 maand en 3 dagen na [d]',
  DATE: '1 januari van [j]',
  DAY_OF_WEEK: 'de weekdag van [d]',
  DATE_DIFF: 'het aantal dagen van [a] tot [b]',
  DATE_PART: 'het jaartal van [d]',
  START_OF: 'de eerste dag van de maand waarin [d] valt',
  ROUND: 'afgerond op hele eenheden ( [a] gedeeld door 12 )',
  CEIL: 'naar boven afgerond op 2 decimalen ( [a] )',
  FLOOR: 'naar beneden afgerond op veelvouden van 100 ( [a] )',
};

const ctx: ArticleContext = { declared: new Map(), vocab: makeVocabulary() };

describe('RegelRijm covers the schema', () => {
  it('has a form for exactly the operations of the latest schema', () => {
    const fromSchema = [...schema.definitions.operationType.enum].sort();
    expect([...OPERATIONS].sort()).toEqual(fromSchema);
    expect(Object.keys(SAMPLES).sort()).toEqual(fromSchema);
  });

  it.each(OPERATIONS)('reads %s without a fallback', (op) => {
    const r = renderAction({ output: 'u', value: SAMPLES[op] }, { ctx });
    const problems = r.segments.filter((s) => typeof s !== 'string' && (s.tk === 'error' || s.tk === 'hole'));
    expect(problems).toEqual([]);
    expect(r.text).toBe(`[u] wordt: ${EXPECTED[op]}`);
  });

  it('shows an unknown operation as an error, not as something else', () => {
    const r = renderAction({ output: 'u', value: { operation: 'SWITCH', cases: [] } }, { ctx });
    expect(r.text).toContain('⟨onbekende operatie SWITCH⟩');
  });

  it('shows a missing operand in a half-edited tree as a hole', () => {
    const r = renderAction({ output: 'u', value: { operation: 'EQUALS', subject: '$a' } }, { ctx });
    expect(r.text).toBe('[u] wordt: [a] is ⟨ontbreekt: value⟩');
  });

  it.each([
    [{ operation: 'IF', default: 1 }, '[u] wordt: als ⟨ontbreekt: cases⟩'],
    [{ operation: 'LIST' }, '[u] wordt: de lijst ⟨ontbreekt: items⟩'],
    [{ operation: 'ROUND', value: '$a' }, '[u] wordt: afgerond op ⟨ontbreekt: precision⟩ ( [a] )'],
    [{ operation: 'DATE_DIFF', from: '$a', to: '$b', in: 'weeks' }, '[u] wordt: het verschil van [a] tot [b] in ⟨onbekende eenheid "weeks"⟩'],
  ])('keeps a broken tree visible: %j', (value, text) => {
    expect(renderAction({ output: 'u', value }, { ctx }).text).toBe(text);
  });

  it.each([
    ['foo', '[u] wordt: als ⟨ontbreekt: cases⟩'],
    [{ when: true }, '[u] wordt: als ⟨ontbreekt: cases⟩'],
    [['foo'], '[u] wordt: ⟨ontbreekt: then⟩ als ⟨ontbreekt: when⟩, anders 2'],
    [[null, { when: '$a', then: 1 }], '[u] wordt: ⟨ontbreekt: then⟩ als ⟨ontbreekt: when⟩, 1 als [a], anders 2'],
  ])('reads wrong-typed cases mid-edit as holes, not as a crash: %j', (cases, text) => {
    const action = { output: 'u', value: { operation: 'IF', cases, default: 2 } };
    expect(renderAction(action, { ctx }).text).toBe(text);
    expect(() => renderFocus(action, ['value'], { ctx, depth: 1 })).not.toThrow();
  });

  it('reads the one row a filter leaves as its only value', () => {
    const value = { operation: 'FOREACH', collection: '$m', as: 'x', filter: { operation: 'EQUALS', subject: '$x.b', value: '$c' }, body: '$x', combine: 'ONLY' };
    expect(renderAction({ output: 'u', value }, { ctx }).text).toBe('[u] wordt: de enige waarde van [x] over elke [x] in [m] waarvoor ( [b] van [x] [c] is )');
  });

  it('reads an action without its output as a hole', () => {
    expect(renderAction({ value: '$a' }, { ctx }).text).toBe('⟨ontbreekt: output⟩ wordt: [a]');
  });

  it('reads an article whose declarations are mid-edit', () => {
    const article = { machine_readable: { definitions: [1], execution: { parameters: {}, input: 'x', output: null } } };
    expect(renderAction({ output: 'u', value: '$a' }, { article }).text).toBe('[u] wordt: [a]');
  });

  it('does not flatten away an empty inner sum, which the engine rejects', () => {
    const valuesOf = (v: unknown) => (v as Operation).values;
    expect(valuesOf(normalise({ operation: 'ADD', values: ['$x', { operation: 'ADD', values: [] }] }))).toHaveLength(2);
    expect(valuesOf(normalise({ operation: 'ADD', values: ['$x', { operation: 'ADD', values: ['$y'] }] }))).toEqual(['$x', '$y']);
  });

  it('reads three or more conditions as an enumeration', () => {
    const and = { operation: 'AND', conditions: ['$a', '$b', { operation: 'NOT', value: { operation: 'EQUALS', subject: '$c', value: null } }] };
    expect(renderAction({ output: 'u', value: and }, { ctx }).text).toBe([
      '[u] wordt:',
      '\taan alle volgende voorwaarden is voldaan:',
      '\t\t• [a]',
      '\t\t• [b]',
      '\t\t• [c] is bekend',
    ].join('\n'));
    const inner = { operation: 'AND', conditions: ['$d', '$e', '$f'] };
    const or = { operation: 'IF', cases: [{ when: { operation: 'OR', conditions: ['$a', { operation: 'AND', conditions: ['$b', '$c'] }, inner] }, then: 1 }], default: 0 };
    expect(renderAction({ output: 'u', value: or }, { ctx }).text).toBe([
      '[u] wordt:',
      '\t1 als aan ten minste één van de volgende voorwaarden is voldaan:',
      '\t\t• [a]',
      '\t\t• [b] én [c]',
      '\t\t• aan alle volgende voorwaarden is voldaan:',
      '\t\t\t•• [d]',
      '\t\t\t•• [e]',
      '\t\t\t•• [f]',
      '\t, anders 0',
    ].join('\n'));
  });

  it('names a repeated long part with a letter in a legend', () => {
    const choice = { operation: 'IF', cases: [{ when: '$heeft_partner', then: '$drempel_met_partner' }], default: '$drempel_alleen' };
    const value = { operation: 'SUBTRACT', values: ['$inkomen', { operation: 'MULTIPLY', values: [choice, 2] }, choice] };
    expect(renderAction({ output: 'u', value }, { ctx }).text).toBe([
      '[u] wordt: [inkomen] min ( A maal 2 ) min A',
      'waarbij:',
      '\tA = [drempel met partner] als [heeft partner], anders [drempel alleen]',
    ].join('\n'));
  });

  it('rounds money to whole euros or cents by the unit of the value', () => {
    const declared = new Map<string, Unit>([['e', 'euro'], ['c', 'eurocent']]);
    const c: ArticleContext = { declared, vocab: makeVocabulary() };
    const read = (value: unknown, precision: number) => renderAction({ output: 'u', value: { operation: 'ROUND', value, precision } }, { ctx: c }).text;
    expect(read('$e', 0)).toBe("[u] wordt: afgerond op hele euro's ( [e] )");
    expect(read('$c', -2)).toBe("[u] wordt: afgerond op hele euro's ( [c] )");
    expect(read('$c', 0)).toBe('[u] wordt: afgerond op hele centen ( [c] )');
    expect(read('$e', 2)).toBe('[u] wordt: afgerond op 2 decimalen ( [e] )');
  });

  it('folds a part into its signature: what it gives and what it reads', () => {
    const c: ArticleContext = { declared: new Map([['premie', 'eurocent'], ['inkomen', 'eurocent']]), vocab: makeVocabulary(), constants: new Set(['drempel']) };
    const premie = { operation: 'IF', cases: [{ when: { operation: 'EQUALS', subject: '$heeft_partner', value: true }, then: { operation: 'MULTIPLY', values: [2, '$premie'] } }], default: '$premie' };
    const bijdrage = { operation: 'MULTIPLY', values: [{ operation: 'SUBTRACT', values: ['$inkomen', '$drempel'] }, '$percentage_inkomen'] };
    const action = { output: 'hoogte', value: { operation: 'MAX', values: [0, { operation: 'SUBTRACT', values: [premie, bijdrage] }] } };
    const f = renderFocus(action, ['value'], { ctx: c, depth: 1 });
    // The article's constant (drempel) is how, not what: it is no input.
    expect(f.text).toBe('[hoogte] wordt: het deel van ⟨bedrag uit [heeft partner] en [premie]⟩ boven ⟨bedrag uit [inkomen] en [percentage inkomen]⟩');
    expect(f.folds.map((x) => x.number)).toEqual(['1.1', '1.2']);
    expect(f.signature).toBe('bedrag uit [heeft partner], [premie], [inkomen] en nog 1');
  });

  it('names only free inputs in a signature, and counts what it does not show', () => {
    const c: ArticleContext = { declared: new Map(), vocab: makeVocabulary(), constants: new Set() };
    const foreach = {
      operation: 'FOREACH', collection: '$kinderen', as: 'kind',
      filter: { operation: 'GREATER_THAN', subject: '$kind.leeftijd', value: '$grens' },
      body: { operation: 'MULTIPLY', values: ['$kind.bedrag', '$a', '$b', '$c'] }, combine: 'ADD',
    };
    // `kind` is bound inside the part, so `$kind.…` is no input of it.
    expect(renderFocus({ output: 'u', value: foreach }, ['value'], { ctx: c }).signature).toBe('getal uit [kinderen], [grens], [a] en nog 2');
    const cond = { operation: 'AND', conditions: [{ operation: 'GREATER_THAN', subject: '$x', value: 1 }, { operation: 'LESS_THAN', subject: '$y', value: 2 }] };
    expect(renderFocus({ output: 'u', value: cond }, ['value'], { ctx: c }).signature).toBe('voorwaarde over [x] en [y]');
    expect(renderFocus({ output: 'u', value: { operation: 'ADD', values: [1, 2] } }, ['value'], { ctx: c }).signature).toBe('getal uit vaste waarden');
  });

  it('opens a folded part one level at a time', () => {
    const c: ArticleContext = { declared: new Map([['inkomen', 'eurocent'], ['hoogte', 'eurocent']]), vocab: makeVocabulary(), constants: new Set(['drempel']) };
    const bijdrage = { operation: 'MULTIPLY', values: [{ operation: 'SUBTRACT', values: ['$inkomen', '$drempel', '$korting'] }, '$percentage_inkomen'] };
    const action = { output: 'hoogte', value: { operation: 'MAX', values: [0, bijdrage] } };
    const read = (...open: string[]) => renderFocus(action, ['value'], { ctx: c, depth: 1, open: new Set(open) });
    expect(read().text).toBe('[hoogte] wordt: de grootste van ( € 0,00; ⟨bedrag uit [inkomen], [korting] en [percentage inkomen]⟩ )');
    // Opened, the part shows its own parts folded, numbered as in the full reading.
    const once = read('value/values/1');
    expect(once.text).toBe('[hoogte] wordt: de grootste van ( € 0,00; ⟨bedrag uit [inkomen] en [korting]⟩ maal [percentage inkomen] )');
    expect(once.folds.map((x) => x.number)).toEqual(['1.1.1']);
    // A bracket inside a bracket breaks, one operand per line.
    expect(read('value/values/1', 'value/values/1/values/0').text).toBe([
      '[hoogte] wordt:',
      '\tde grootste van (',
      '\t\t€ 0,00;',
      '\t\t( [inkomen] min [drempel] min [korting] ) maal [percentage inkomen]',
      '\t)',
    ].join('\n'));
    // A part opened under one that is still folded stays folded.
    expect(read('value/values/1/values/0').text).toBe(read().text);
  });

  it('keeps a part in full when that reads shorter than its signature, numbering in reading order', () => {
    const sum = (a: unknown) => ({ operation: 'ADD', values: [a, 1] });
    const foreach = { operation: 'FOREACH', collection: sum('$m'), as: 'x', filter: { operation: 'EQUALS', subject: '$x', value: 1 }, body: sum('$x'), combine: 'ADD' };
    const f = renderFocus({ output: 'u', value: foreach }, ['value'], { ctx, depth: 1 });
    expect(f.text).toBe('[u] wordt: de som van ( [x] plus 1 ) over elke [x] in ( [m] plus 1 ) waarvoor ( [x] 1 is )');
    expect(f.folds).toEqual([]);
    const long = (a: unknown) => ({ operation: 'ADD', values: [a, '$b', '$c', '$e', '$f'] });
    const g = renderFocus({ output: 'u', value: { operation: 'DATE_ADD', date: long('$d'), years: long('$n') } }, ['value'], { ctx, depth: 1 });
    expect(g.folds.map((x) => x.number)).toEqual(['1.1', '1.2']);
  });
});
