import { describe, expect, it } from 'vitest';
import * as yaml from 'js-yaml';
import { applyOverrides, definitionKind, effectiveOverrides, overridableDefinitions } from './lawParameters';

const DOC = {
  $id: 'test',
  articles: [
    { number: '1', machine_readable: { definitions: { drempelinkomen: 3700000, percentage_alleenstaand: 0.01896, naam: 'x', lijst: [1, 2] } } },
    { number: '2', machine_readable: { definitions: { drempelinkomen: 3700000, maximum_vermogen: 14000000 } } },
    { number: '3', text: 'geen machine_readable' },
  ],
};

describe('overridableDefinitions', () => {
  it('lists numeric definitions once, with their article', () => {
    const defs = overridableDefinitions(DOC);
    expect(defs.map((d) => d.key)).toEqual(['drempelinkomen', 'percentage_alleenstaand', 'maximum_vermogen']);
    expect(defs[0]).toEqual({ key: 'drempelinkomen', value: 3700000, article: '1' });
  });

  it('copes with a document without articles', () => {
    expect(overridableDefinitions({})).toEqual([]);
    expect(overridableDefinitions(null)).toEqual([]);
  });
});

describe('applyOverrides', () => {
  it('rewrites the definition in every article that has it and leaves the original alone', () => {
    const text = applyOverrides(DOC, { drempelinkomen: 4000000, onbekend: 1, naam: 5, percentage_alleenstaand: 'x' });
    // The dumped copy of DOC: its first two articles carry definitions.
    const out = yaml.load(text) as { articles: Array<{ machine_readable: { definitions: Record<string, unknown> } }> };
    const defsAt = (i: number): Record<string, unknown> => {
      const article = out.articles[i];
      if (!article) throw new Error(`no article ${i}`);
      return article.machine_readable.definitions;
    };
    expect(defsAt(0).drempelinkomen).toBe(4000000);
    expect(defsAt(1).drempelinkomen).toBe(4000000);
    expect(defsAt(0).naam).toBe('x');
    expect(defsAt(0).percentage_alleenstaand).toBe(0.01896);
    expect(defsAt(0).onbekend).toBeUndefined();
    expect(DOC.articles[0]?.machine_readable?.definitions.drempelinkomen).toBe(3700000);
  });
});

describe('effectiveOverrides', () => {
  it('keeps only known keys whose value differs from the law', () => {
    expect(effectiveOverrides(DOC, { drempelinkomen: 3700000, maximum_vermogen: 1, onbekend: 2, naam: 3 })).toEqual({ maximum_vermogen: 1 });
    expect(effectiveOverrides(DOC, undefined)).toEqual({});
  });
});

describe('definitionKind', () => {
  it('guesses percentages, amounts and plain numbers', () => {
    expect(definitionKind('percentage_alleenstaand', 0.018)).toBe('percentage');
    expect(definitionKind('drempelinkomen', 3700000)).toBe('eurocent');
    expect(definitionKind('vrijstellingsdrempel_m2', 50)).toBe('number');
    expect(definitionKind('minimale_leeftijd', 18)).toBe('number');
  });
});
