import { describe, expect, it, vi } from 'vitest';
import { klopt, toetsAntwoorden } from '../src/antwoorden.js';

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
    const uit = await toetsAntwoorden(step, vraag);
    expect(vraag.mock.calls).toEqual([
      [null, 'beleid#1#betaalde_leges', {}, '2026-02-16'],
      ['register', 'beleid#1#openstaand', { x: 1 }, '2026-02-16'],
    ]);
    expect(uit.map((a) => [a.kreeg, a.klopt])).toEqual([
      [5000, true],
      [250, false],
    ]);
  });

  it('counts a refusal of the cell as no answer, with the reason', async () => {
    const vraag = async () => {
      throw new Error('refused: reductie_x is a reduction (RFC-056)');
    };
    const [a] = await toetsAntwoorden({ ...step, antwoorden: [step.antwoorden[0]] }, vraag);
    expect(a).toMatchObject({ kreeg: null, klopt: false, fout: expect.stringContaining('RFC-056') });
  });

  it('has nothing to ask of a step without answers', async () => {
    expect(await toetsAntwoorden({}, vi.fn())).toEqual([]);
  });
});

describe('klopt', () => {
  it('compares values, objects included', () => {
    expect(klopt(0, 0)).toBe(true);
    expect(klopt(null, 0)).toBe(false);
    expect(klopt({ a: 1 }, { a: 1 })).toBe(true);
  });
});
