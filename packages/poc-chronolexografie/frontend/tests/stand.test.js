import { describe, expect, it } from 'vitest';
import { invoerVoor, soortVan, toonWaarde, vraagStand } from '../src/stand.js';

const saldo = {
  veld: 'saldo',
  bron: 'b#2#saldo',
  besluit: 'besluit',
  type: 'amount',
  eenheid: 'eurocent',
  parameters: ['besluit', 'percentage'],
};

describe('a view beside a decision', () => {
  it('gives the decision under its parameter, and only what the article takes besides', () => {
    expect(invoerVoor(saldo, 'id-1', { percentage: 6, ander: 1, leeg: '' })).toEqual({
      besluit: 'id-1',
      percentage: 6,
    });
    expect(invoerVoor({ ...saldo, parameters: ['besluit'] }, 'id-1', { percentage: 6 })).toEqual({ besluit: 'id-1' });
  });

  it('writes an answer by its type and unit', () => {
    expect(toonWaarde(1913600, saldo)).toBe(toonWaarde(19136, { type: 'amount', eenheid: 'euro' }));
    expect(toonWaarde(8493, saldo)).toMatch(/84,93/);
    expect(toonWaarde('2028-06-12')).toBe('12 juni 2028');
    expect(toonWaarde(true)).toBe('Ja');
    expect(toonWaarde(null, saldo)).toBe('geen');
    expect(toonWaarde(27, { type: 'number' })).toBe('27');
  });

  it('asks the cell per answer, and keeps a refusal or an answer the law lacks apart', async () => {
    const vraag = async (cel, bron, invoer) => {
      if (bron === 'b#2#weigert') throw new Error('refused');
      return { waarde: invoer.besluit === 'id-1' ? 100 : null };
    };
    const uit = await vraagStand(
      {
        antwoorden: [
          saldo,
          { ...saldo, veld: 'w', bron: 'b#2#weigert' },
          { veld: 'x', bron: 'b#9#x', besluit: 'besluit', fout: 'no output' },
        ],
      },
      'id-1',
      '2028-05-08',
      {},
      vraag,
    );
    expect(uit.map((a) => [a.kreeg, a.fout ?? null])).toEqual([
      [100, null],
      [null, 'refused'],
      [null, 'no output'],
    ]);
  });

  it('finds the type and unit of a source a view shows', () => {
    expect(soortVan('b#2#saldo', [{ antwoorden: [saldo] }]).eenheid).toBe('eurocent');
    expect(soortVan('b#2#ander', [{ antwoorden: [saldo] }])).toEqual({});
    expect(soortVan('b#2#saldo')).toEqual({});
  });
});
