import { describe, it, expect } from 'vitest';
import { tableToRecords } from './actions';

describe('tableToRecords', () => {
  it('builds one typed record per data row from the header row', () => {
    const records = tableToRecords([
      ['naam', 'leeftijd', 'verzekerd'],
      ['Jansen', '30', 'true'],
    ]);

    expect(records).toEqual([{ naam: 'Jansen', leeftijd: 30, verzekerd: true }]);
  });

  it('returns nothing for a table without data rows', () => {
    expect(tableToRecords([['naam', 'leeftijd']])).toEqual([]);
    expect(tableToRecords(null)).toEqual([]);
  });

  // Mirror of the Rust `rows_to_records` check: a short row must fail loudly
  // instead of yielding undefined for the missing column.
  it('rejects a row that is shorter than the header row', () => {
    expect(() =>
      tableToRecords([
        ['naam', 'leeftijd', 'verzekerd'],
        ['Jansen', '30'],
      ]),
    ).toThrow('data table row 1 has 2 cells, header row has 3');
  });

  it('rejects a row that is longer than the header row', () => {
    expect(() =>
      tableToRecords([
        ['naam', 'leeftijd', 'verzekerd'],
        ['Jansen', '30', 'true'],
        ['De Vries', '40', 'false', 'extra'],
      ]),
    ).toThrow('data table row 2 has 4 cells, header row has 3');
  });
});
