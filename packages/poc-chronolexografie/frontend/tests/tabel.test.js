import { describe, expect, it } from 'vitest';
import { alsInvoer, celTekst, kolomVeld, sleutelKolommen, tabelVerschil, zelfdeTabel } from '../src/tabel.js';
import { BESTUUR, RIJEN } from './fixtureTabel.js';

const { kolommen } = BESTUUR;
const [een, twee] = RIJEN;

describe('the fixture', () => {
  it('is the table of the test source', () => {
    expect(kolommen.map((k) => k.type)).toEqual(['string', 'string', 'date', 'amount']);
    expect(RIJEN).toHaveLength(2);
  });
});

describe('sleutelKolommen', () => {
  it('takes every column that does not count something as what names a row', () => {
    expect(sleutelKolommen(kolommen)).toEqual(['naam', 'functie', 'sinds']);
    const alles = [
      { naam: 'a', type: 'string' },
      { naam: 'b', type: 'date' },
      { naam: 'c', type: 'boolean' },
      { naam: 'd', type: 'number' },
      { naam: 'e', type: 'amount' },
    ];
    expect(sleutelKolommen(alles)).toEqual(['a', 'b', 'c']);
  });
});

describe('zelfdeTabel', () => {
  it('compares on content, whatever the order of the rows', () => {
    expect(zelfdeTabel(RIJEN, [twee, een], kolommen)).toBe(true);
    expect(zelfdeTabel(RIJEN, RIJEN.map((r) => ({ ...r })), kolommen)).toBe(true);
  });

  it('reads a column a row leaves out as empty, and a decimal as its number', () => {
    const zonder = { naam: twee.naam, functie: twee.functie, sinds: twee.sinds };
    expect(zelfdeTabel([twee], [zonder], kolommen)).toBe(true);
    const k = [{ naam: 'aantal', type: 'number' }];
    expect(zelfdeTabel([{ aantal: { __decimal: '2.5' } }], [{ aantal: 2.5 }], k)).toBe(true);
  });

  it('sees a changed value, a row more or less, and a row twice', () => {
    expect(zelfdeTabel(RIJEN, [een, { ...twee, vergoeding: 100 }], kolommen)).toBe(false);
    expect(zelfdeTabel(RIJEN, [een], kolommen)).toBe(false);
    expect(zelfdeTabel(RIJEN, [een, twee, een], kolommen)).toBe(false);
    expect(zelfdeTabel(RIJEN, [een, een], kolommen)).toBe(false);
  });
});

describe('tabelVerschil', () => {
  it('pairs a changed row with the source by its key columns', () => {
    const anders = { ...een, vergoeding: 15000 };
    expect(tabelVerschil(RIJEN, [twee, anders], kolommen)).toEqual({
      gewijzigd: [{ bron: een, eigen: anders }],
      ontbreekt: [],
      extra: [],
    });
  });

  it('names a row the source gives that is gone, and a row the applicant added', () => {
    const nieuw = { naam: 'Lid Drie', functie: 'secretaris', sinds: '2026-03-01', vergoeding: null };
    expect(tabelVerschil(RIJEN, [een, nieuw], kolommen)).toEqual({
      gewijzigd: [],
      ontbreekt: [twee],
      extra: [nieuw],
    });
  });

  it('pairs nothing when every column counts something', () => {
    const k = [{ naam: 'aantal', type: 'number' }];
    expect(tabelVerschil([{ aantal: 1 }], [{ aantal: 2 }], k)).toEqual({
      gewijzigd: [],
      ontbreekt: [{ aantal: 1 }],
      extra: [{ aantal: 2 }],
    });
  });
});

describe('celTekst', () => {
  it('shows a cell as the app shows values, an empty one as nothing', () => {
    expect(celTekst('Lid Een')).toBe('Lid Een');
    expect(celTekst(true)).toBe('ja');
    expect(celTekst(12500)).toBe('12500');
    expect(celTekst({ __decimal: '2.5' })).toBe('2.5');
    expect(celTekst(null)).toBe('');
    expect(celTekst(undefined)).toBe('');
  });
});

describe('kolomVeld', () => {
  it('makes a column a field the form can show, which may stay empty', () => {
    expect(kolomVeld({ naam: 'sinds', type: 'date' })).toEqual({
      naam: 'sinds',
      type: 'date',
      mag_leeg: true,
      basis: '',
    });
  });
});

describe('the edge cases of the comparison', () => {
  const getallen = [
    { naam: 'a', type: 'number' },
    { naam: 'b', type: 'amount' },
  ];

  it('compares a table without key columns as a multiset of whole rows', () => {
    const rijen = [{ a: 1, b: 2 }, { a: 3, b: 4 }, { a: 1, b: 2 }];
    expect(zelfdeTabel(rijen, [{ a: 3, b: 4 }, { a: 1, b: 2 }, { a: 1, b: 2 }], getallen)).toBe(true);
    expect(zelfdeTabel(rijen, [{ a: 3, b: 4 }, { a: 1, b: 2 }, { a: 3, b: 4 }], getallen)).toBe(false);
  });

  it('compares rows with the same key as whole rows, in any order', () => {
    const k = [
      { naam: 'naam', type: 'string' },
      { naam: 'aantal', type: 'number' },
    ];
    const bron = [{ naam: 'X', aantal: 1 }, { naam: 'X', aantal: 2 }];
    expect(zelfdeTabel(bron, [{ naam: 'X', aantal: 2 }, { naam: 'X', aantal: 1 }], k)).toBe(true);
    // One of the two changed: paired with the first row left with that key.
    expect(tabelVerschil(bron, [{ naam: 'X', aantal: 2 }, { naam: 'X', aantal: 5 }], k)).toEqual({
      gewijzigd: [{ bron: { naam: 'X', aantal: 1 }, eigen: { naam: 'X', aantal: 5 } }],
      ontbreekt: [],
      extra: [],
    });
  });

  it('takes an empty key cell for an empty one, null or left out', () => {
    const k = [
      { naam: 'code', type: 'string' },
      { naam: 'aantal', type: 'number' },
    ];
    expect(zelfdeTabel([{ code: null, aantal: 1 }], [{ aantal: 1 }], k)).toBe(true);
    expect(tabelVerschil([{ code: null, aantal: 1 }], [{ aantal: 2 }], k).gewijzigd).toHaveLength(1);
  });

  it('holds no table equal to a value that is none', () => {
    expect(zelfdeTabel(RIJEN, undefined, kolommen)).toBe(false);
    expect(zelfdeTabel(RIJEN, 'tekst', kolommen)).toBe(false);
  });
});

describe('alsInvoer', () => {
  it('holds a decimal cell as its number, and leaves a value that is no table', () => {
    expect(alsInvoer([{ aantal: { __decimal: '2.5' }, naam: 'A' }])).toEqual([{ aantal: 2.5, naam: 'A' }]);
    expect(alsInvoer('tekst')).toBe('tekst');
    expect(alsInvoer(undefined)).toBeUndefined();
  });
});
