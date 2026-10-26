import { describe, it, expect } from 'vitest';
import { firstSourceGroup, pickSuggestions, officialTextUrl } from './searchResults';

const law = (law_id: string, source_id: string, source_priority: number, source_name = source_id) => ({
  law_id,
  source_id,
  source_name,
  source_priority,
});

describe('firstSourceGroup', () => {
  it('returns only the matches of the highest-priority source, named', () => {
    const group = firstSourceGroup([
      law('central_a', 'central', 2, 'Centrale Regelrecht Corpus'),
      law('own_a', 'traject-own', 0, 'example-org/regelrecht-corpus-example'),
      law('own_b', 'traject-own', 0, 'example-org/regelrecht-corpus-example'),
    ]);
    expect(group?.sourceName).toBe('example-org/regelrecht-corpus-example');
    expect(group?.laws.map((l) => l.law_id)).toEqual(['own_a', 'own_b']);
  });

  it('falls through to the next source when the first has no matches', () => {
    const group = firstSourceGroup([law('central_a', 'central', 2, 'Centraal')]);
    expect(group?.sourceName).toBe('Centraal');
  });

  it('is null without matches', () => {
    expect(firstSourceGroup([])).toBeNull();
  });
});

describe('pickSuggestions', () => {
  it('picks the requested number of distinct ids', () => {
    const ids = ['a', 'b', 'c', 'd', 'e', 'f', 'g'];
    const picked = pickSuggestions(ids, 5);
    expect(picked).toHaveLength(5);
    expect(new Set(picked).size).toBe(5);
    expect(picked.every((id) => ids.includes(id))).toBe(true);
  });

  it('returns all ids when there are fewer than asked', () => {
    expect(pickSuggestions(['a', 'b'], 5).sort()).toEqual(['a', 'b']);
  });

  it('follows the random source, so the pick changes between openings', () => {
    const ids = ['a', 'b', 'c', 'd'];
    expect(pickSuggestions(ids, 2, () => 0)).toEqual(['a', 'b']);
    expect(pickSuggestions(ids, 2, () => 0.99)).toEqual(['d', 'a']);
  });
});

describe('officialTextUrl', () => {
  it('links to the regulation on wetten.overheid.nl', () => {
    expect(officialTextUrl('BWBR0018451')).toBe('https://wetten.overheid.nl/BWBR0018451');
  });
});
