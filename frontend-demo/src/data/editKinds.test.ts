import { describe, expect, it } from 'vitest';
import {
  columnKind,
  decimalsFor,
  editKind,
  emptyRow,
  isRecordArray,
  parseCell,
  parseDutchNumber,
  stepFor,
  tableColumns,
  unitLabel,
  valueKind,
} from './editKinds';
import type { EditKind } from './editKinds';
import type { FieldSpec } from './format';

// The seven types the corpus declares (boolean, string, amount, number, array,
// object, date) must each reach an editor that fits them, for a present value
// and for an absent one.
describe('editKind', () => {
  it('follows the declared type, also when the value is absent', () => {
    const cases: Array<[FieldSpec, EditKind]> = [
      [{ type: 'boolean' }, 'boolean'],
      [{ type: 'string' }, 'text'],
      [{ type: 'amount', type_spec: { unit: 'eurocent' } }, 'amount'],
      [{ type: 'number' }, 'number'],
      [{ type: 'date' }, 'date'],
      [{ type: 'array' }, 'list'],
      [{ type: 'object' }, 'record'],
    ];
    for (const [spec, expected] of cases) {
      expect(editKind(null, spec), `absent ${spec.type}`).toBe(expected);
    }
  });

  it('reads an array as a table when it holds records and as a list otherwise', () => {
    const spec = { type: 'array' };
    expect(editKind([{ bsn: '1', naam: 'Kind' }], spec)).toBe('rows');
    expect(editKind([5, 3], spec)).toBe('list');
    expect(editKind(['Ouderlijk gezag'], spec)).toBe('list');
    // An array-typed field with no rows yet is a list to fill in, not a blob.
    expect(editKind([], spec)).toBe('list');
  });

  it('falls back to the value when nothing is declared', () => {
    expect(editKind(true)).toBe('boolean');
    expect(editKind(42)).toBe('number');
    expect(editKind('2020-01-01')).toBe('date');
    expect(editKind('Amsterdam')).toBe('text');
    expect(editKind([{ a: 1 }])).toBe('rows');
    expect(editKind([1, 2])).toBe('list');
    expect(editKind({ straat: 'Meeuwenlaan' })).toBe('record');
    expect(editKind(null)).toBe('text');
  });

  it('prefers amount over number, because money is entered in euros', () => {
    expect(editKind(348000, { type: 'amount', type_spec: { unit: 'eurocent' } })).toBe('amount');
    expect(editKind(2, { type: 'number' })).toBe('number');
  });

  it('does not mistake a date-shaped string in a string field for a date', () => {
    // The declaration wins: a string field stays free text even when today's
    // value happens to look like a date.
    expect(editKind('2020-01-01', { type: 'string' })).toBe('text');
  });
});

describe('valueKind', () => {
  it('types one plain value inside a list or a cell', () => {
    expect(valueKind(true)).toBe('boolean');
    expect(valueKind(5)).toBe('number');
    expect(valueKind('2020-01-01')).toBe('date');
    expect(valueKind('Kind 1')).toBe('text');
    expect(valueKind(null)).toBe('text');
  });
});

describe('tables', () => {
  const rows = [
    { bsn: '999200001', geboortedatum: '2020-01-01', naam: 'Kind 1' },
    { bsn: '999200002', geboortedatum: '2022-01-01', naam: 'Kind 2' },
  ];

  it('recognises a table', () => {
    expect(isRecordArray(rows)).toBe(true);
    expect(isRecordArray([1, 2])).toBe(false);
    expect(isRecordArray([])).toBe(false);
    expect(isRecordArray(null)).toBe(false);
  });

  it('collects the columns in the order they appear', () => {
    expect(tableColumns(rows)).toEqual(['bsn', 'geboortedatum', 'naam']);
    // A row that carries an extra field widens the table rather than hiding it.
    expect(tableColumns([{ a: 1 }, { b: 2 }])).toEqual(['a', 'b']);
  });

  it('types a column on the values it holds', () => {
    expect(columnKind(rows, 'geboortedatum')).toBe('date');
    expect(columnKind(rows, 'naam')).toBe('text');
    expect(columnKind([{ n: 1 }, { n: 2 }], 'n')).toBe('number');
    expect(columnKind([{ b: true }, { b: null }], 'b')).toBe('boolean');
    // A column nobody filled in yet takes free text.
    expect(columnKind([{ x: null }], 'x')).toBe('text');
    // Mixed content is not a number column.
    expect(columnKind([{ x: 1 }, { x: 'twee' }], 'x')).toBe('text');
  });

  it('types an entered cell back to its column kind', () => {
    expect(parseCell('true', 'boolean')).toBe(true);
    expect(parseCell('false', 'boolean')).toBe(false);
    expect(parseCell('12', 'number')).toBe(12);
    expect(parseCell('1,5', 'number')).toBe(1.5);
    expect(parseCell('', 'number')).toBeNull();
    expect(parseCell('geen getal', 'number')).toBeNull();
    expect(parseCell('2020-01-01', 'date')).toBe('2020-01-01');
    expect(parseCell('', 'text')).toBeNull();
  });

  it('adds a row with the same columns, so the table keeps one shape', () => {
    expect(emptyRow(tableColumns(rows))).toEqual({ bsn: null, geboortedatum: null, naam: null });
    expect(emptyRow([])).toEqual({});
  });
});

// Numbers, floats and money are the three shapes a citizen types by hand, and
// each has a way of going wrong: a thousands separator, a decimal the law does
// not admit, and cents that drift when rounded in euros.
describe('numbers', () => {
  it('reads a number the way a Dutch person writes it', () => {
    expect(parseDutchNumber('1.234,56')).toBe(1234.56);
    expect(parseDutchNumber('1,5')).toBe(1.5);
    expect(parseDutchNumber('1234')).toBe(1234);
    // A value copied straight from a register keeps the dot as decimal mark.
    expect(parseDutchNumber('1234.56')).toBe(1234.56);
    expect(parseDutchNumber('-2,5')).toBe(-2.5);
    expect(parseDutchNumber('')).toBeNull();
    expect(parseDutchNumber('geen getal')).toBeNull();
    expect(parseDutchNumber(null)).toBeNull();
  });

  it('parses a thousands separator inside a table cell too', () => {
    expect(parseCell('1.234,56', 'number')).toBe(1234.56);
    expect(parseCell('1,5', 'number')).toBe(1.5);
    expect(parseCell('', 'number')).toBeNull();
  });

  it('takes the number of decimals from the law', () => {
    expect(decimalsFor({ type: 'number', type_spec: { precision: 2 } })).toBe(2);
    expect(decimalsFor({ type: 'number', type_spec: { precision: 0 } })).toBe(0);
    expect(decimalsFor({ type: 'number' })).toBeNull();
    // Money is entered in euros, so it always admits cents.
    expect(decimalsFor({ type: 'amount', type_spec: { unit: 'eurocent' } })).toBe(2);
  });

  it('gives a float field a step it can actually reach', () => {
    // A whole-number field steps by one; a float field must not be integer-only.
    expect(stepFor({ type: 'number', type_spec: { precision: 0 } })).toBe('1');
    expect(stepFor({ type: 'number', type_spec: { precision: 2 } })).toBe('0.01');
    expect(stepFor({ type: 'number', type_spec: { precision: 1 } })).toBe('0.1');
    expect(stepFor({ type: 'number' })).toBe('any');
    expect(stepFor({ type: 'amount', type_spec: { unit: 'eurocent' } })).toBe('0.01');
  });

  it('names the unit the law counts in', () => {
    expect(unitLabel({ type: 'amount', type_spec: { unit: 'eurocent' } })).toBe('in euro');
    expect(unitLabel({ type: 'number', type_spec: { unit: 'years' } })).toBe('in jaren');
    expect(unitLabel({ type: 'number', type_spec: { unit: 'weeks' } })).toBe('in weken');
    expect(unitLabel({ type: 'number', type_spec: { unit: 'percentage' } })).toBe('in procent');
    expect(unitLabel({ type: 'number' })).toBeNull();
  });

  it('treats a number field carrying money as money', () => {
    // Some laws declare `number` with a eurocent unit; it is still money and
    // must be entered in euros, not in cents.
    expect(editKind(348000, { type: 'number', type_spec: { unit: 'eurocent' } })).toBe('amount');
  });

  it('keeps a float field a number, not a whole-number field', () => {
    expect(editKind(67.25, { type: 'number', type_spec: { unit: 'years', precision: 2 } })).toBe('number');
  });
});

// RFC-036: an unknown value is a marker object, not a value. The edit box used
// to seed its field from it, so a citizen opening "Woonlandfactor" was shown
// the literal text "[object Object]" to correct.
describe('an unknown value', () => {
  const unknown = { __unknown: true, missing: [{ law: 'wkb', name: 'kinderen_woonlanden', kind: 'no_data' }] };

  it('follows the declared type, like an absent value does', () => {
    expect(editKind(unknown, { type: 'number' })).toBe('number');
    expect(editKind(unknown, { type: 'date' })).toBe('date');
    expect(editKind(unknown, { type: 'boolean' })).toBe('boolean');
    expect(editKind(unknown, { type: 'amount', type_spec: { unit: 'eurocent' } })).toBe('amount');
  });

  it('is never read as a record, whatever its shape', () => {
    expect(editKind(unknown)).toBe('text');
    expect(editKind(unknown, { type: 'object' })).toBe('record');
    expect(valueKind(unknown)).toBe('text');
  });
});
