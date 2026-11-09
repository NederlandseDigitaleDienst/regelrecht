// Fragments of the article text tied to a rule by a linking note, on a small
// fictional article.
import { describe, it, expect } from 'vitest';
import { linkedOutput, linksByOutput } from './regelrijmLinks';

const TEXT = 'De aanvrager heeft recht op een toeslag. De toeslag bedraagt het verschil.';
const link = (output: string, law = 'wet_a') => ({ motivation: 'linking', body: { type: 'SpecificResource', source: `regelrecht://${law}/${output}`, purpose: 'linking' } });
const span = (quote: string, from = 0) => {
  const start = TEXT.indexOf(quote, from);
  return { start, end: start + quote.length };
};

describe('linkedOutput', () => {
  it('reads the output a linking note points at in this law', () => {
    expect(linkedOutput(link('hoogte'), 'wet_a')).toBe('hoogte');
    expect(linkedOutput({ ...link('hoogte'), body: [link('hoogte').body] }, 'wet_a')).toBe('hoogte');
  });

  it('reads the form the note editor writes, also next to a comment', () => {
    const body = { type: 'SpecificResource', source: 'regelrecht://wet_a/hoogte#hoogte', purpose: 'linking' };
    expect(linkedOutput({ motivation: 'linking', body }, 'wet_a')).toBe('hoogte');
    const comment = { type: 'TextualBody', value: 'x', purpose: 'commenting' };
    expect(linkedOutput({ motivation: 'commenting', body: [comment, body] }, 'wet_a')).toBe('hoogte');
    expect(linkedOutput({ motivation: 'linking', body: { source: 'regelrecht://doc/traject/mvt.md', purpose: 'linking' } }, 'wet_a')).toBeNull();
  });

  it('ignores comments, links to another law and other URIs', () => {
    expect(linkedOutput({ motivation: 'commenting', body: { type: 'TextualBody', value: 'x' } }, 'wet_a')).toBeNull();
    expect(linkedOutput(link('hoogte', 'wet_b'), 'wet_a')).toBeNull();
    expect(linkedOutput({ motivation: 'linking', body: { source: 'https://example.test/x' } }, 'wet_a')).toBeNull();
    expect(linkedOutput({ motivation: 'linking', body: { source: 'regelrecht://wet_a' } }, 'wet_a')).toBeNull();
    expect(linkedOutput(null, 'wet_a')).toBeNull();
    expect(linkedOutput({ motivation: 'linking' }, 'wet_a')).toBeNull();
    expect(linkedOutput({ motivation: 'linking', body: [null] }, 'wet_a')).toBeNull();
  });

  it('accepts any of the ids the law goes by', () => {
    // The note form writes the route id, the corpus the $id.
    expect(linkedOutput(link('hoogte', 'route_id'), ['wet_a', 'route_id'])).toBe('hoogte');
    expect(linkedOutput(link('hoogte', 'wet_a'), [undefined, 'wet_a'])).toBe('hoogte');
  });
});

describe('linksByOutput', () => {
  it('gives each output its fragments in text order, once each', () => {
    const entries = [
      { note: link('hoogte'), spans: [span('het verschil')] },
      { note: link('recht'), spans: [span('recht op een toeslag')] },
      { note: link('hoogte'), spans: [span('toeslag', 41)] },
      { note: link('hoogte'), spans: [span('het verschil')] },
      { note: { motivation: 'commenting', body: { value: 'x' } }, spans: [span('aanvrager')] },
    ];
    const links = linksByOutput(entries, { lawId: 'wet_a', text: TEXT });
    expect([...links.keys()]).toEqual(['hoogte', 'recht']);
    expect(links.get('hoogte')?.map((f) => f.quote)).toEqual(['toeslag', 'het verschil']);
    expect(links.get('recht')?.[0]?.quote).toBe('recht op een toeslag');
  });

  it('is empty without notes', () => {
    expect(linksByOutput([], { lawId: 'wet_a', text: TEXT }).size).toBe(0);
    expect(linksByOutput(undefined, { lawId: 'wet_a', text: TEXT }).size).toBe(0);
  });
});
