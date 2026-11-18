import { describe, it, expect } from 'vitest';
import {
  valuesEqual,
  primitiveEqual,
  quotedValue,
  tableCellValue,
  tableToRecords,
  dispatch,
  type GherkinEngine,
} from './actions';
import { ExecutionContext } from './context';

// The tests drive dispatch with a context holding only the fields a step
// reads, and with an engine stub that has only the methods it calls.
const asCtx = (fields: Partial<ExecutionContext>) => Object.assign(new ExecutionContext(), fields);
const stubEngine = (methods: Partial<GherkinEngine> = {}) => methods as GherkinEngine;

const unknownFor = (...names: string[]) => ({
  __unknown: true,
  missing: names.map((name) => ({ law: 'test_wet', name, kind: 'no_data' })),
});

// Mirrors packages/engine/tests/bdd/helpers/value_conversion.rs
// (`values_equal_with_tolerance`) plus law-model's `PartialEq for Value`:
// the two runners must accept and reject the same pairs.
describe('valuesEqual', () => {
  it('compares scalars like before', () => {
    expect(valuesEqual(1, 1)).toBe(true);
    expect(valuesEqual('a', 'a')).toBe(true);
    expect(valuesEqual(true, true)).toBe(true);
    expect(valuesEqual(null, null)).toBe(true);
    expect(valuesEqual(true, false)).toBe(false);
    expect(valuesEqual('a', 'b')).toBe(false);
  });

  it('applies the 1e-9 numeric tolerance', () => {
    expect(valuesEqual(1.0, 1.0000000001)).toBe(true);
    expect(valuesEqual(100, 100.0)).toBe(true);
    expect(valuesEqual(100, 101)).toBe(false);
    expect(valuesEqual(0.1 + 0.2, 0.3)).toBe(true);
  });

  it('never equates a number with its numeric string', () => {
    expect(valuesEqual(1, '1')).toBe(false);
    expect(valuesEqual('1', 1)).toBe(false);
    expect(valuesEqual(true, 'true')).toBe(false);
    expect(valuesEqual(1, true)).toBe(false);
  });

  it('treats null as equal to null only', () => {
    expect(valuesEqual(null, [])).toBe(false);
    expect(valuesEqual([], null)).toBe(false);
    expect(valuesEqual(null, {})).toBe(false);
    expect(valuesEqual(null, 0)).toBe(false);
    expect(valuesEqual(null, '')).toBe(false);
    expect(valuesEqual(undefined, null)).toBe(false);
    expect(valuesEqual(undefined, [])).toBe(false);
  });

  it('compares arrays element-wise in order', () => {
    expect(valuesEqual([], [])).toBe(true);
    expect(valuesEqual([1, 'a', null, true], [1, 'a', null, true])).toBe(true);
    expect(valuesEqual([1, 2], [2, 1])).toBe(false);
    expect(valuesEqual([1, 2], [1, 2, 3])).toBe(false);
    expect(valuesEqual([1, 2, 3], [1, 2])).toBe(false);
    expect(valuesEqual([1], ['1'])).toBe(false);
  });

  it('applies the numeric tolerance inside arrays', () => {
    expect(valuesEqual([1.0], [1.0000000001])).toBe(true);
    expect(valuesEqual([[1.5, 2]], [[1.5000000001, 2]])).toBe(true);
  });

  it('compares nested objects by key set and values, regardless of key order', () => {
    const a = { postcode: '1234AB', huisnummer: 10, kinderen: [{ bsn: '1', leeftijd: 4 }] };
    const b = { kinderen: [{ leeftijd: 4, bsn: '1' }], huisnummer: 10, postcode: '1234AB' };
    expect(valuesEqual(a, b)).toBe(true);
    expect(valuesEqual({ a: 1 }, { a: 1, b: 2 })).toBe(false);
    expect(valuesEqual({ a: 1, b: 2 }, { a: 1 })).toBe(false);
    expect(valuesEqual({ a: 1 }, { b: 1 })).toBe(false);
    expect(valuesEqual({ a: { x: 1 } }, { a: { x: 2 } })).toBe(false);
    expect(valuesEqual({ a: null }, { a: [] })).toBe(false);
    expect(valuesEqual({ a: undefined }, { b: undefined })).toBe(false);
  });

  it('does not confuse arrays with objects', () => {
    expect(valuesEqual([], {})).toBe(false);
    expect(valuesEqual({}, [])).toBe(false);
    expect(valuesEqual({ 0: 'a', length: 1 }, ['a'])).toBe(false);
  });

  it('keeps the old name as an alias', () => {
    expect(primitiveEqual).toBe(valuesEqual);
  });

  // Mirrors law-model's PartialEq: two Unknowns are equal whatever they list
  // as missing, and an Unknown equals nothing else (RFC-036).
  it('equates two unknown values regardless of provenance', () => {
    expect(valuesEqual(unknownFor('huur'), unknownFor('partner_bsn'))).toBe(true);
    expect(valuesEqual(unknownFor('huur'), unknownFor('huur', 'partner_bsn'))).toBe(true);
    expect(valuesEqual({ __unknown: true, missing: [] }, unknownFor('huur'))).toBe(true);
  });

  it('never equates an unknown value with null or a plain object', () => {
    expect(valuesEqual(unknownFor('huur'), null)).toBe(false);
    expect(valuesEqual(null, unknownFor('huur'))).toBe(false);
    expect(valuesEqual(unknownFor('huur'), undefined)).toBe(false);
    expect(valuesEqual(unknownFor('huur'), { missing: [] })).toBe(false);
    expect(valuesEqual({ __unknown: false, missing: [] }, unknownFor('huur'))).toBe(false);
    expect(valuesEqual(unknownFor('huur'), 0)).toBe(false);
    expect(valuesEqual(unknownFor('huur'), [])).toBe(false);
  });

  it('applies the unknown rule inside arrays and objects too', () => {
    expect(valuesEqual([unknownFor('a')], [unknownFor('b')])).toBe(true);
    expect(valuesEqual({ x: unknownFor('a') }, { x: null })).toBe(false);
  });
});

// Mirrors packages/engine/tests/bdd/helpers/table.rs (`rows_to_records`).
describe('tableToRecords', () => {
  it('omits the key for an empty cell and keeps a literal null (RFC-036)', () => {
    const records = tableToRecords([
      ['bsn', 'huur', 'partner_bsn'],
      ['1', '', 'null'],
      ['2', '500', '  '],
    ]);
    expect(records).toEqual([
      { bsn: 1, partner_bsn: null },
      { bsn: 2, huur: 500 },
    ]);
    expect(Object.hasOwn(records[0], 'huur')).toBe(false);
    expect(Object.hasOwn(records[1], 'partner_bsn')).toBe(false);
  });

  it('keeps every cell with content, typed by content', () => {
    expect(tableToRecords([['a', 'b'], ['x', 'true']])).toEqual([{ a: 'x', b: true }]);
  });

  it('returns no records for a header-only or missing table', () => {
    expect(tableToRecords([['a']])).toEqual([]);
    expect(tableToRecords(null)).toEqual([]);
  });

  it('rejects a row whose width differs from the header', () => {
    expect(() => tableToRecords([['a', 'b'], ['1']])).toThrow(
      'data table row 1 has 1 cells, header row has 2',
    );
  });
});

describe('quoted and table-cell values', () => {
  it('parses a quoted JSON array or object like the Rust convert_gherkin_value', () => {
    expect(quotedValue('[]')).toEqual([]);
    expect(quotedValue('[5, 7]')).toEqual([5, 7]);
    expect(quotedValue('{"postcode": "1234AB", "huisnummer": "10"}')).toEqual({
      postcode: '1234AB',
      huisnummer: '10',
    });
    expect(tableCellValue('[]')).toEqual([]);
  });

  it('keeps a cell that only starts like JSON as a string', () => {
    expect(quotedValue('[not json')).toBe('[not json');
  });

  it('keeps the scalar rules', () => {
    expect(quotedValue('42')).toBe(42);
    expect(quotedValue('true')).toBe(true);
    expect(quotedValue('null')).toBe(null);
    expect(quotedValue('GM0384')).toBe('GM0384');
  });

  // RFC-036: the Rust runner reads `parameter "x" is ""` as Null
  // (`convert_gherkin_value("")`), so the quoted empty string is an absence
  // here too, not the string ''. The empty *cell* never gets this far.
  it('reads the quoted empty string as null, like the Rust runner', () => {
    expect(quotedValue('')).toBe(null);
    expect(quotedValue('   ')).toBe(null);
    expect(tableCellValue('')).toBe(null);
  });
});

describe('set_parameters_table through dispatch', () => {
  it('leaves a parameter with an empty cell out and passes null for the word null (RFC-036)', async () => {
    const ctx = asCtx({ parameters: {} });
    await dispatch(ctx, stubEngine(), 'set_parameters_table', [], [
      ['bsn', '999993653'],
      ['aanvraag_bedrag', ''],
      ['partner_bsn', 'null'],
      ['huur', '   '],
    ], { loadDependency: async () => {} });
    expect(ctx.parameters).toEqual({ bsn: 999993653, partner_bsn: null });
    expect(Object.hasOwn(ctx.parameters, 'aanvraag_bedrag')).toBe(false);
  });
});

describe('assert_equals through dispatch', () => {
  const ctxWith = (outputs: Record<string, unknown>) => asCtx({ result: { outputs }, executed: true, error: null });
  const run = (ctx: ExecutionContext, expected: unknown) =>
    dispatch(ctx, stubEngine(), 'assert_equals', ['kinderen', expected], null, { loadDependency: async () => {} });

  it('accepts an empty array output against a quoted "[]"', async () => {
    await expect(run(ctxWith({ kinderen: [] }), quotedValue('[]'))).resolves.toBeUndefined();
  });

  it('accepts an object output against a quoted object literal', async () => {
    await expect(
      run(ctxWith({ kinderen: { a: 1, b: [1, 2] } }), quotedValue('{"b":[1,2],"a":1}')),
    ).resolves.toBeUndefined();
  });

  it('rejects a null output against "[]" and renders both sides as JSON', async () => {
    await expect(run(ctxWith({ kinderen: null }), quotedValue('[]'))).rejects.toThrow(
      'Expected output "kinderen" to equal [], got: null',
    );
  });

  it('renders unequal arrays with JSON.stringify in the message', async () => {
    await expect(run(ctxWith({ kinderen: [1, 2] }), quotedValue('[2,1]'))).rejects.toThrow(
      'Expected output "kinderen" to equal [2,1], got: [1,2]',
    );
  });

  it('rejects a numeric string against a number', async () => {
    await expect(run(ctxWith({ kinderen: '3' }), 3)).rejects.toThrow(
      'Expected output "kinderen" to equal 3, got: "3"',
    );
  });
});

describe('assert_unknown and assert_unknown_for through dispatch', () => {
  const ctxWith = (outputs: Record<string, unknown>) => asCtx({ result: { outputs }, executed: true, error: null });
  const run = (ctx: ExecutionContext, action: string, args: unknown[]) =>
    dispatch(ctx, stubEngine(), action, args, null, { loadDependency: async () => {} });

  it('assert_unknown passes on an unknown output whatever is missing', async () => {
    await expect(run(ctxWith({ x: unknownFor('huur') }), 'assert_unknown', ['x'])).resolves.toBeUndefined();
    await expect(run(ctxWith({ x: { __unknown: true, missing: [] } }), 'assert_unknown', ['x'])).resolves.toBeUndefined();
  });

  it('assert_unknown fails on null, a value or a missing output, showing the actual value', async () => {
    await expect(run(ctxWith({ x: null }), 'assert_unknown', ['x'])).rejects.toThrow(
      'Expected output "x" to be unknown, got: null',
    );
    await expect(run(ctxWith({ x: 500 }), 'assert_unknown', ['x'])).rejects.toThrow(
      'Expected output "x" to be unknown, got: 500',
    );
    await expect(run(ctxWith({ x: { missing: [] } }), 'assert_unknown', ['x'])).rejects.toThrow(
      'Expected output "x" to be unknown, got: {"missing":[]}',
    );
    await expect(run(ctxWith({}), 'assert_unknown', ['x'])).rejects.toThrow(
      'Expected output "x" to be unknown, got: undefined',
    );
  });

  it('assert_unknown_for passes when one of the missing facts has the name', async () => {
    await expect(
      run(ctxWith({ x: unknownFor('huur', 'partner_bsn') }), 'assert_unknown_for', ['x', 'partner_bsn']),
    ).resolves.toBeUndefined();
  });

  it('assert_unknown_for names the facts that are missing when the wanted one is not', async () => {
    await expect(
      run(ctxWith({ x: unknownFor('huur', 'partner_bsn') }), 'assert_unknown_for', ['x', 'inkomen']),
    ).rejects.toThrow(
      'Expected output "x" to be unknown for lack of "inkomen", but it is unknown for lack of: huur (test_wet), partner_bsn (test_wet)',
    );
  });

  it('assert_unknown_for fails on a value that is not unknown at all', async () => {
    await expect(run(ctxWith({ x: null }), 'assert_unknown_for', ['x', 'huur'])).rejects.toThrow(
      'Expected output "x" to be unknown, got: null',
    );
  });

  it('assert_null and assert_equals reject an unknown output', async () => {
    await expect(run(ctxWith({ x: unknownFor('huur') }), 'assert_null', ['x'])).rejects.toThrow(
      'Expected output "x" to equal null, got: {"__unknown":true,"missing":[{"law":"test_wet","name":"huur","kind":"no_data"}]}',
    );
    await expect(run(ctxWith({ x: unknownFor('huur') }), 'assert_equals', ['x', 500])).rejects.toThrow(
      'Expected output "x" to equal 500',
    );
  });

  it('assert_unknown reports a failed or missing execution like the other asserts', async () => {
    await expect(
      run(asCtx({ result: null, executed: true, error: new Error('boom') }), 'assert_unknown', ['x']),
    ).rejects.toThrow('No outputs available (execution failed)');
  });
});

describe('set_data_source through dispatch', () => {
  it('registers records without a key for an empty cell', async () => {
    const calls: unknown[][] = [];
    const engine = stubEngine({ registerDataSource: (...a) => void calls.push(a) });
    await dispatch(asCtx({}), engine, 'set_data_source', ['huurgegevens', 'bsn'], [
      ['bsn', 'huur'],
      ['1', ''],
      ['2', 'null'],
    ], { loadDependency: async () => {} });
    expect(calls).toEqual([['huurgegevens', 'bsn', [{ bsn: 1 }, { bsn: 2, huur: null }]]]);
  });
});

describe('text arguments through dispatch', () => {
  // A quoted capture is typed by inference, so "404" arrives as a number.
  // The arms that compare text must still treat it as text.
  it('assert_fails_with matches a numeric quoted capture against the error', async () => {
    const ctx = asCtx({ executed: true, error: new Error('HTTP 404 from source') });
    await expect(
      dispatch(ctx, stubEngine(), 'assert_fails_with', [quotedValue('404')], null, { loadDependency: async () => {} }),
    ).resolves.toBeUndefined();
  });
});
