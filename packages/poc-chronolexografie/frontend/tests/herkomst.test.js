import { beforeEach, describe, expect, it, vi } from 'vitest';

// The bundle: two versions of the policy, each with its own article 1.
vi.mock('../src/api.js', async (importOriginal) => ({
  ...(await importOriginal()),
  bestanden: async () => ({
    'corpus/regulation/nl/uitvoeringsbeleid/beleid_parkeren/2026-01-01.yaml': "articles:\n  - number: '1'\n    text: oud\n",
    'corpus/regulation/nl/uitvoeringsbeleid/beleid_parkeren/2027-01-01.yaml': "articles:\n  - number: '1'\n    text: nieuw\n",
  }),
}));

import { artikelTekst, gelezenGrammen, herkomstVan, klikbaar, laag, leden } from '../src/herkomst.js';
import { state } from '../src/state.js';

beforeEach(() => {
  state.casus = {
    cel: 'gemeente_wilgendaal',
    regelwerken: [
      { id: 'parkeerverordening', naam: null, map: 'regulation/nl/gemeentelijke_verordening/wilgendaal/parkeerverordening' },
      { id: 'beleid_parkeren', naam: null, map: 'regulation/nl/uitvoeringsbeleid/beleid_parkeren' },
      { id: 'reductie_vestiging', naam: null, map: 'cells/kamer_van_koophandel/reductions/reductie_vestiging' },
    ],
  };
  state.grammen = [];
  state.andere = [
    {
      cel: 'kamer_van_koophandel',
      grammen: [{ id: 'k1' }, { id: 'k2' }, { id: 'k3' }],
    },
  ];
});

describe('laag', () => {
  it('reads law, policy and a reduction from the directory of the regelwerk', () => {
    expect(laag('parkeerverordening')).toBe('wet');
    expect(laag('beleid_parkeren')).toBe('beleid');
    expect(laag('reductie_vestiging')).toBe('reductie');
    expect(laag('onbekend')).toBe('wet');
  });
});

describe('herkomstVan', () => {
  const begrip = (ingevuld_door) => ({ soort: 'begrip', naam: 'x', waarde: true, ingevuld_door });

  it('points an article and a hook at their article', () => {
    expect(herkomstVan({ soort: 'artikel', wet: 'beleid_parkeren', artikel: '1' })).toEqual({
      soort: 'beleid',
      wet: 'beleid_parkeren',
      artikel: '1',
    });
    expect(herkomstVan({ soort: 'hook', wet: 'parkeerverordening', artikel: '2' }).soort).toBe('wet');
    expect(herkomstVan({ soort: 'artikel', wet: 'parkeerverordening', artikel: '' })).toBeNull();
  });

  it('names who filled in a term', () => {
    expect(herkomstVan(begrip('finding'))).toEqual({ soort: 'bevinding' });
    expect(herkomstVan(begrip({ regulation: { law_id: 'beleid_parkeren', article: '3' } }))).toEqual({
      soort: 'beleid',
      wet: 'beleid_parkeren',
      artikel: '3',
    });
    const antwoord = { answerer: 'kamer_van_koophandel', answer: 'postcode', last_record: 'k2' };
    expect(herkomstVan(begrip({ answer: { law_id: 'beleid_parkeren', article: '1', answers: [antwoord] } }))).toEqual({
      soort: 'reductie',
      antwoorden: [antwoord],
    });
    expect(herkomstVan(begrip(null))).toBeNull();
    expect(herkomstVan({ soort: 'uitkomst', naam: 'y', ontbreekt: [] })).toBeNull();
  });

  it('opens nothing for a finding', () => {
    expect(klikbaar({ soort: 'bevinding' })).toBe(false);
    expect(klikbaar({ soort: 'wet', wet: 'a', artikel: '1' })).toBe(true);
    expect(klikbaar(null)).toBe(false);
  });
});

describe('gelezenGrammen', () => {
  it('takes the chronicle of the answering cell up to the last gram it read', () => {
    expect(gelezenGrammen({ answerer: 'kamer_van_koophandel', last_record: 'k2' }).map((g) => g.id)).toEqual(['k1', 'k2']);
    expect(gelezenGrammen({ answerer: 'kamer_van_koophandel', last_record: null })).toEqual([]);
  });
});

describe('leden', () => {
  it('splits a literal text on blank lines and joins its wrapped lines', () => {
    expect(leden('1 Aan een\ngehandicapte kan.\n\n2 Aan de\nzorgaanbieder.\n')).toEqual([
      '1 Aan een gehandicapte kan.',
      '2 Aan de zorgaanbieder.',
    ]);
  });

  it('splits a folded text on single newlines', () => {
    expect(leden('FICTIEF.\n1 Het college oordeelt.\n2 Zelf.')).toEqual(['FICTIEF.', '1 Het college oordeelt.', '2 Zelf.']);
  });
});

describe('artikelTekst', () => {
  it('takes the version valid on the given day, or the play date', async () => {
    state.datum = '2027-03-01';
    expect(await artikelTekst('beleid_parkeren', '1')).toBe('nieuw');
    expect(await artikelTekst('beleid_parkeren', '1', '2026-06-01')).toBe('oud');
  });

  it('takes the first version before any is valid, and gives null for what it lacks', async () => {
    expect(await artikelTekst('beleid_parkeren', '1', '2025-01-01')).toBe('oud');
    expect(await artikelTekst('beleid_parkeren', '9')).toBeNull();
    expect(await artikelTekst('onbekend', '1')).toBeNull();
  });

  it('gives null for a case from a source, whose regelwerken are not in the bundle', async () => {
    state.casus.bron = 'napp';
    expect(await artikelTekst('beleid_parkeren', '1', '2026-06-01')).toBeNull();
  });
});
