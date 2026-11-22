import { describe, expect, it } from 'vitest';
import { genummerd, herkomstVan, klikbaar, laagVanMap, stapTekst } from './keten';
import type { Stap } from './keten';

const namen = {
  wet: (wet: string) => (wet === 'babw' ? 'BABW' : wet),
  artikel: (wet: string, artikel: string) => `${wet === 'babw' ? 'BABW' : wet} artikel ${artikel}`,
};

describe('the chain of a decision', () => {
  const keten: Stap[] = [
    {
      soort: 'artikel',
      wet: 'babw',
      artikel: '49',
      stappen: [
        { soort: 'hook', wet: 'awb', artikel: '6:7', stappen: [] },
        { soort: 'uitkomst', naam: 'kaart_verstrekt', waarde: true, ontbreekt: [], stappen: [] },
      ],
    },
  ];

  it('numbers each step as its key, down the tree', () => {
    const [art] = genummerd(keten);
    expect(art?.nummer).toBe('1');
    expect(art?.kinderen.map((k) => k.nummer)).toEqual(['1.1', '1.2']);
  });

  it('names a step by the names the app gives, a field by its label', () => {
    expect(stapTekst(keten[0]!, namen).tekst).toBe('BABW artikel 49 wordt uitgevoerd');
    expect(stapTekst(keten[0]!.stappen![0]!, namen).tekst).toBe('Ook van toepassing: awb artikel 6:7');
    const label = { ...namen, veld: (n: string) => (n === 'kaart_verstrekt' ? 'Kaart verstrekt' : n) };
    expect(stapTekst(keten[0]!.stappen![1]!, label).tekst).toBe('Kaart verstrekt: ja');
    expect(stapTekst({ soort: 'uitkomst', naam: 'x_y', waarde: null, ontbreekt: ['a_b'] }, namen)).toEqual({
      tekst: 'x y: niet bepaald',
      toelichting: 'ontbreekt: a b',
    });
  });

  it('tells law, policy and a reduction apart by where the regelwerk sits', () => {
    expect(laagVanMap('regulation/nl/uitvoeringsbeleid/beleid')).toBe('beleid');
    expect(laagVanMap('cells/gemeente/reductie')).toBe('reductie');
    expect(laagVanMap('regulation/nl/amvb/babw')).toBe('wet');
  });

  it('gives the origin of a term: a finding, an article, or the answers of cells', () => {
    const laag = (wet: string) => (wet === 'beleid' ? 'beleid' : 'wet') as const;
    expect(herkomstVan({ soort: 'begrip', naam: 'x', ingevuld_door: 'finding' }, laag)).toEqual({ soort: 'bevinding' });
    expect(
      herkomstVan({ soort: 'begrip', naam: 'x', ingevuld_door: { regulation: { law_id: 'beleid', article: '3' } } }, laag),
    ).toEqual({ soort: 'beleid', wet: 'beleid', artikel: '3' });
    const antwoord = { answerer: 'ggd' };
    expect(
      herkomstVan(
        { soort: 'begrip', naam: 'x', waarde: true, ingevuld_door: { answer: { law_id: 'b', article: '1', answers: [antwoord] } } },
        laag,
      ),
    ).toEqual({ soort: 'reductie', begrip: 'x', waarde: true, antwoorden: [antwoord] });
    expect(herkomstVan({ soort: 'uitkomst', naam: 'y' }, laag)).toBeNull();
    expect(klikbaar({ soort: 'bevinding' })).toBe(false);
    expect(klikbaar({ soort: 'wet', wet: 'babw', artikel: '49' })).toBe(true);
  });
});
