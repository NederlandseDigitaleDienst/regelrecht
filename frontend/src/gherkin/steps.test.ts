import { describe, it, expect } from 'vitest';
import {
  parseValue,
  createStepDefinitions,
  SUPPORTED_TIERS,
  matchStep,
  typedArgs,
} from './steps';
import { GRAMMAR, VALUE_TYPING } from './grammar.generated';
import type { GherkinEngine } from './actions';
import type { ExecutionContext } from './context';
import type { Step } from './parser';

/** `value`, after asserting it is there: a missing value fails the test at this point. */
function defined<T>(value: T | null | undefined): T {
  expect(value).toBeDefined();
  expect(value).not.toBeNull();
  return value as T;
}

// The steps run here only set parameters; none of them reaches the engine.
const NO_ENGINE = null as unknown as GherkinEngine;

/** `line` matched against `pattern`, as the runner hands it to a step. */
function matchLine(line: string, pattern: RegExp): RegExpExecArray {
  return defined(line.match(pattern)) as RegExpExecArray;
}

/** A step carrying only a data table: the dispatch reads nothing else of it. */
function tableStep(dataTable: string[][]): Step {
  return { dataTable } as Step;
}

describe('parseValue', () => {
  it('parses booleans', () => {
    expect(parseValue('true')).toBe(true);
    expect(parseValue('false')).toBe(false);
  });

  it('parses null', () => {
    expect(parseValue('null')).toBe(null);
  });

  it('parses integers', () => {
    expect(parseValue('42')).toBe(42);
    expect(parseValue('-7')).toBe(-7);
    expect(parseValue('0')).toBe(0);
  });

  it('parses floats', () => {
    expect(parseValue('3.14')).toBeCloseTo(3.14);
    expect(parseValue('-0.5')).toBeCloseTo(-0.5);
  });

  it('returns strings for non-numeric values', () => {
    expect(parseValue('hello')).toBe('hello');
    expect(parseValue('Amsterdam')).toBe('Amsterdam');
  });

  it('parses large numeric strings as integers', () => {
    expect(parseValue('999993653')).toBe(999993653);
  });
});

describe('createStepDefinitions', () => {
  it('creates a step definition for every grammar entry', () => {
    const defs = createStepDefinitions({ loadDependency: async () => {} });
    expect(defs).toHaveLength(GRAMMAR.length);
    for (const def of defs) {
      expect(def.pattern).toBeInstanceOf(RegExp);
      expect(typeof def.execute).toBe('function');
      expect(typeof def.tier).toBe('string');
    }
  });

  it('declares the core tier as the editor-supported tier set', () => {
    expect(SUPPORTED_TIERS).toEqual(['core']);
  });
});

// `matchStep` is the one walk over the grammar, shared by every runner that is
// handed raw step text: the editor, the demo's Dutch renderer, and both panels
// on the docs site. It used to be hand-rolled identically in three of those, and
// the point of moving it here is that they can no longer disagree about what a
// step means.
describe('matchStep', () => {
  it('returns the entry and the raw captures', () => {
    const match = matchStep('parameter "bsn" is "999993653"');
    expect(match?.entry.id).toBe('set_parameter_string');
    expect(match?.args).toEqual(['bsn', '999993653']);
  });

  it('returns null for a phrasing the grammar does not have', () => {
    // The guarantee the language rests on: a step that is not in
    // bdd/grammar.yaml does not exist, and no runner may invent one.
    expect(matchStep('the vibes are good')).toBe(null);
    expect(matchStep('output "x" is roughly 42')).toBe(null);
  });

  it('tells the two parameter forms apart by their argument, not their words', () => {
    expect(matchStep('parameter "age" is 25')?.entry.id).toBe('set_parameter_number');
    expect(matchStep('parameter "age" is "25"')?.entry.id).toBe('set_parameter_string');
  });
});

// `typedArgs` applies `value_typing` from bdd/grammar.yaml to a step's captures.
// It deliberately stops there: the grammar literals are appended by `buildArgs`
// for the callers that dispatch, because a caller that only wants to *show* a
// step wants the captures alone.
describe('typedArgs', () => {
  const entryFor = (line: string) => defined(matchStep(line)).entry;

  it('reads a quoted capture by its content', () => {
    const line = 'parameter "bsn" is "999993653"';
    expect(typedArgs(entryFor(line), defined(matchStep(line)).args)).toEqual(['bsn', 999993653]);
  });

  it('reads a bare capture as a number', () => {
    const line = 'output "hoogte" equals 133084';
    expect(typedArgs(entryFor(line), defined(matchStep(line)).args)).toEqual(['hoogte', 133084]);
  });

  it('leaves the grammar literals out', () => {
    // `output "x" is true` carries the boolean as a literal, not a capture.
    // typedArgs yields one value; the dispatch path adds the literal after it.
    const line = 'output "x" is true';
    const entry = entryFor(line);
    expect(entry.literals).toEqual([true]);
    expect(typedArgs(entry, defined(matchStep(line)).args)).toEqual(['x']);
  });
});

// Each canonical example line is parsed by exactly one core grammar pattern,
// and that pattern carries the expected action. This is the proof that the
// generated patterns match their canonical phrasings.
describe('core grammar patterns match their canonical example lines', () => {
  const examples = [
    { line: 'the calculation date is "2025-01-01"', action: 'set_calculation_date' },
    { line: 'law "my_law" is loaded', action: 'load_law' },
    { line: 'parameter "bsn" is "999993653"', action: 'set_parameter' },
    { line: 'parameter "age" is 25', action: 'set_parameter' },
    { line: 'the following parameters:', action: 'set_parameters_table' },
    { line: 'the following "personal_data" data with key "bsn":', action: 'set_data_source' },
    { line: 'I evaluate "result" of "my_law"', action: 'evaluate' },
    { line: 'the execution succeeds', action: 'assert_succeeds' },
    { line: 'the execution fails', action: 'assert_fails' },
    { line: 'the execution fails with "some error"', action: 'assert_fails_with' },
    { line: 'output "x" is true', action: 'assert_boolean' },
    { line: 'output "x" is false', action: 'assert_boolean' },
    { line: 'output "x" equals 42', action: 'assert_equals' },
    { line: 'output "x" equals "hello"', action: 'assert_equals' },
    { line: 'output "x" is absent', action: 'assert_null' },
    { line: 'output "x" contains "sub"', action: 'assert_contains' },
  ];

  const coreEntries = GRAMMAR.filter((e) => e.tier === 'core');

  for (const { line, action } of examples) {
    it(`matches: ${line}`, () => {
      const matching = coreEntries.filter((e) => e.pattern.test(line));
      expect(matching).toHaveLength(1);
      expect(defined(matching[0]).action).toBe(action);
    });
  }
});

// Mirror of Rust `rows_to_params` in packages/engine/tests/bdd/dispatch.rs:
// a parameter table carries no header row.
describe('set_parameters_table dispatch', () => {
  async function runTable(dataTable: string[][]) {
    const ctx = { calculationDate: null, parameters: {}, result: null, error: null, executed: false };
    const def = defined(createStepDefinitions({ loadDependency: async () => {} }).find(
      (d) => d.pattern.test('the following parameters:'),
    ));
    await def.execute(ctx, NO_ENGINE, matchLine('the following parameters:', def.pattern), tableStep(dataTable));
    return ctx.parameters;
  }

  it('reads every row of a corpus parameter table', async () => {
    expect(
      await runTable([
        ['gemeente_code', 'GM0363'],
        ['type_beplanting', 'boom'],
        ['postcode', '1012'],
      ]),
    ).toEqual({ gemeente_code: 'GM0363', type_beplanting: 'boom', postcode: 1012 });
  });

  it('reads a single-row parameter table', async () => {
    expect(await runTable([['indieningsdatum', '2025-01-01']])).toEqual({
      indieningsdatum: '2025-01-01',
    });
  });
});

// The canonical typing rule lives in bdd/grammar.yaml (`value_typing`) and is
// carried into both dispatchers by codegen. These assertions are the editor
// half of bdd/conformance/value_typing.feature: the same three lines, the same
// three types. Drift here is the failure mode of issue #1160 — a scenario that
// runs green in the editor and red in CI.
describe('value typing follows bdd/grammar.yaml', () => {
  const defs = createStepDefinitions({ loadDependency: async () => {} });

  async function runStep(line: string, table: string[][] | null = null) {
    // Only `parameters` is read or written by the steps run here.
    const ctx = { parameters: {} } as ExecutionContext;
    const found = defs.find((d) => d.pattern.test(line));
    expect(found, `no step matches "${line}"`).toBeTruthy();
    const def = defined(found);
    await def.execute(ctx, NO_ENGINE, matchLine(line, def.pattern), table ? tableStep(table) : null);
    return ctx.parameters;
  }

  it('declares the three rules', () => {
    expect(VALUE_TYPING).toEqual({ quoted: 'inferred', bare: 'number', table_cell: 'inferred' });
  });

  it('reads a quoted value by its content, not by its quotes', async () => {
    expect(await runStep('parameter "waarde" is "42"')).toEqual({ waarde: 42 });
    expect(await runStep('parameter "verzekerd" is "true"')).toEqual({ verzekerd: true });
    expect(await runStep('parameter "gemeente" is "GM0384"')).toEqual({ gemeente: 'GM0384' });
  });

  it('reads a bare value as a number', async () => {
    expect(await runStep('parameter "waarde" is 42')).toEqual({ waarde: 42 });
  });

  it('lets the content decide in a data table cell', async () => {
    expect(await runStep('the following parameters:', [['waarde', '42']])).toEqual({ waarde: 42 });
  });
});
