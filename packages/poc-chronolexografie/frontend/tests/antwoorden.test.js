import { describe, expect, it, vi } from 'vitest';
import { klopt, metRecords, toetsAntwoorden } from '../src/antwoorden.js';

vi.mock('../src/api.js', () => ({ api: {} }));

const step = {
  antwoordDatum: '2026-02-16',
  antwoorden: [
    { bron: 'beleid#1#betaalde_leges', verwacht: 5000 },
    { bron: 'beleid#1#openstaand', cel: 'register', invoer: { x: 1 }, verwacht: 0 },
  ],
};

describe('toetsAntwoorden', () => {
  it('asks each answer of its cell on the play date of the step and compares it', async () => {
    const vraag = vi.fn(async (cel, bron) => ({ waarde: bron.endsWith('leges') ? 5000 : 250 }));
    const uit = await toetsAntwoorden(step, null, vraag);
    expect(vraag.mock.calls).toEqual([
      [null, 'beleid#1#betaalde_leges', {}, '2026-02-16'],
      ['register', 'beleid#1#openstaand', { x: 1 }, '2026-02-16'],
    ]);
    expect(uit.map((a) => [a.kreeg, a.klopt])).toEqual([
      [5000, true],
      [250, false],
    ]);
  });

  it('asks on the later of the play date of the step and the date given', async () => {
    const vraag = vi.fn(async () => ({ waarde: 5000 }));
    await toetsAntwoorden({ ...step, antwoorden: [step.antwoorden[0]] }, '2026-03-01', vraag);
    await toetsAntwoorden({ ...step, antwoorden: [step.antwoorden[0]] }, '2026-01-01', vraag);
    expect(vraag.mock.calls.map((c) => c[3])).toEqual(['2026-03-01', '2026-02-16']);
  });

  it('counts a refusal of the cell as no answer, with the reason', async () => {
    const vraag = async () => {
      throw new Error('refused: reductie_x is a reduction (RFC-056)');
    };
    const [a] = await toetsAntwoorden({ ...step, antwoorden: [step.antwoorden[0]] }, null, vraag);
    expect(a).toMatchObject({ kreeg: null, klopt: false, fout: expect.stringContaining('RFC-056') });
  });

  it('has nothing to ask of a step without answers', async () => {
    expect(await toetsAntwoorden({}, null, vi.fn())).toEqual([]);
  });
});

describe('klopt', () => {
  it('compares values, objects included', () => {
    expect(klopt(0, 0)).toBe(true);
    expect(klopt(null, 0)).toBe(false);
    expect(klopt({ a: 1, b: 2 }, { b: 2, a: 1 })).toBe(true);
    expect(klopt({ __decimal: '12.50' }, { __decimal: '12.5' })).toBe(true);
    expect(klopt({ __decimal: '5' }, 5)).toBe(false);
  });
});

describe('metRecords', () => {
  it('replaces a kenmerk by the id of the record of its step', () => {
    const record = (k) => ({ voorschot: 'b1' })[k] ?? null;
    expect(metRecords({ voorschotbesluit: { kenmerk: 'voorschot' }, jaar: 2025 }, record)).toEqual({
      voorschotbesluit: 'b1',
      jaar: 2025,
    });
  });

  it('refuses a kenmerk whose record is not there', () => {
    expect(() => metRecords({ besluit: { kenmerk: 'later' } }, () => null)).toThrow("stap 'later'");
  });

  it('asks nothing when a kenmerk has no record, and says why', async () => {
    const vraag = vi.fn();
    const [a] = await toetsAntwoorden(
      { antwoordDatum: '2026-02-16', antwoorden: [{ bron: 'b#1#x', invoer: { besluit: { kenmerk: 'k' } }, verwacht: 0 }] },
      null,
      vraag,
      () => null,
    );
    expect(vraag).not.toHaveBeenCalled();
    expect(a).toMatchObject({ klopt: false, fout: expect.stringContaining("stap 'k'") });
  });
});
