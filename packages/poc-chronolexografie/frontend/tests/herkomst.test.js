import { beforeEach, describe, expect, it, vi } from 'vitest';

// The bundle: two versions of the policy, each with its own article 1. A
// source keys its files by their path in the source, and has its own text.
vi.mock('../src/api.js', async (importOriginal) => ({
  ...(await importOriginal()),
  opgehaaldeBronnen: async () => [
    { naam: 'kapot', fout: 'bron niet beschikbaar' },
    {
      naam: 'voorbeeld',
      bestanden: {
        'casussen/parkeren.yaml': 'cel: gemeente_wilgendaal\n',
        'regulation/nl/uitvoeringsbeleid/beleid_parkeren/2026-01-01.yaml': "articles:\n  - number: '1'\n    text: uit de bron\n",
      },
    },
  ],
  bestanden: async () => ({
    'pocs/x/corpus/casussen/parkeren.yaml': 'cel: gemeente_wilgendaal\n',
    'pocs/x/corpus/regulation/nl/uitvoeringsbeleid/beleid_parkeren/2026-01-01.yaml': "articles:\n  - number: '1'\n    text: oud\n",
    'pocs/x/corpus/regulation/nl/uitvoeringsbeleid/beleid_parkeren/2027-01-01.yaml': "articles:\n  - number: '1'\n    text: nieuw\n",
  }),
}));

import { artikelInhoud, gelezenGrammen, herkomstVan, herkomstVanBasis, klikbaar, laag, leden, machineReadableVan } from '../src/herkomst.js';
import { state } from '../src/state.js';

beforeEach(() => {
  state.casus = {
    id: 'parkeren',
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
      begrip: 'x',
      waarde: true,
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

describe('herkomstVanBasis', () => {
  it('points a chronolexogram at the article it rests on', () => {
    expect(herkomstVanBasis('beleid_parkeren#2')).toEqual({ soort: 'beleid', wet: 'beleid_parkeren', artikel: '2' });
    expect(herkomstVanBasis('algemene_wet_bestuursrecht#7:11')).toEqual({
      soort: 'wet',
      wet: 'algemene_wet_bestuursrecht',
      artikel: '7:11',
    });
  });

  it('gives null for a basis that names no article', () => {
    expect(herkomstVanBasis('beleid_parkeren')).toBeNull();
    expect(herkomstVanBasis('')).toBeNull();
    expect(herkomstVanBasis(undefined)).toBeNull();
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

const tekst = async (...args) => (await artikelInhoud(...args))?.tekst ?? null;

describe('artikelInhoud', () => {
  it('takes the version valid on the given day, or the play date', async () => {
    state.datum = '2027-03-01';
    expect(await tekst('beleid_parkeren', '1')).toBe('nieuw');
    expect(await tekst('beleid_parkeren', '1', '2026-06-01')).toBe('oud');
  });

  it('takes the first version before any is valid, and gives null for what it lacks', async () => {
    expect(await tekst('beleid_parkeren', '1', '2025-01-01')).toBe('oud');
    expect(await tekst('beleid_parkeren', '9')).toBeNull();
    expect(await tekst('onbekend', '1')).toBeNull();
  });

  it('reads a case from a source from the files of that source, not the bundle', async () => {
    state.casus.bron = 'voorbeeld';
    expect(await tekst('beleid_parkeren', '1', '2027-06-01')).toBe('uit de bron');
  });

  it('gives null for a case from a source that did not load', async () => {
    state.casus.bron = 'kapot';
    expect(await tekst('beleid_parkeren', '1')).toBeNull();
    state.casus.bron = 'onbekend';
    expect(await tekst('beleid_parkeren', '1')).toBeNull();
  });
});

describe('machineReadableVan', () => {
  const yaml = [
    'articles:',
    "  - number: '1'",
    '    text: Een.',
    "  - number: '7:11'",
    '    text: |',
    '      Bij ontvankelijkheid.',
    '    # Lid 1, zoals de wet het zegt.',
    '    machine_readable:',
    '      execution:',
    '        # De heroverweging.',
    '        output:',
    '          - name: herroepen',
    '',
    '    links: []',
    '  - number: 8',
    '    machine_readable:',
    '      definitions: {}',
    '',
  ].join('\n');

  it('cuts the block as written, with its comments, dedented to its key', () => {
    expect(machineReadableVan(yaml, '7:11')).toBe(
      [
        '# Lid 1, zoals de wet het zegt.',
        'machine_readable:',
        '  execution:',
        '    # De heroverweging.',
        '    output:',
        '      - name: herroepen',
      ].join('\n'),
    );
    expect(machineReadableVan(yaml, 8)).toBe('machine_readable:\n  definitions: {}');
  });

  it('gives null for an article without one, or one not in the text', () => {
    expect(machineReadableVan(yaml, '1')).toBeNull();
    expect(machineReadableVan(yaml, '2')).toBeNull();
  });

  it('comes with the article from the files of the case', async () => {
    state.casus.bron = undefined;
    expect((await artikelInhoud('beleid_parkeren', '1', '2026-06-01')).machineReadable).toBeNull();
  });
});
