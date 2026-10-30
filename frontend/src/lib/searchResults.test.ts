import { describe, it, expect } from 'vitest';
import { groupBySource, otherSourceLabel, pickSuggestions, officialTextUrl } from './searchResults';

const law = (law_id: string, source_id: string, source_priority: number, source_name = source_id) => ({
  law_id,
  source_id,
  source_name,
  source_priority,
});

describe('groupBySource', () => {
  it('orders the sources by priority and keeps the order within each', () => {
    const groups = groupBySource([
      law('central_a', 'central', 2, 'Centrale Regelrecht Corpus'),
      law('own_a', 'traject-own', 0, 'example-org/regelrecht-corpus-example'),
      law('central_b', 'central', 2, 'Centrale Regelrecht Corpus'),
      law('own_b', 'traject-own', 0, 'example-org/regelrecht-corpus-example'),
    ]);
    expect(groups.map((g) => g.sourceName)).toEqual([
      'example-org/regelrecht-corpus-example',
      'Centrale Regelrecht Corpus',
    ]);
    expect(groups[0]?.laws.map((l) => l.law_id)).toEqual(['own_a', 'own_b']);
    expect(groups[1]?.laws.map((l) => l.law_id)).toEqual(['central_a', 'central_b']);
  });

  it('is empty without matches', () => {
    expect(groupBySource([])).toEqual([]);
  });
});

describe('otherSourceLabel', () => {
  const group = (n: number) => ({
    sourceId: 'central',
    sourceName: 'Centraal',
    laws: Array.from({ length: n }, (_, i) => law(`l${i}`, 'central', 2)),
  });

  it('counts the matches, singular for one', () => {
    expect(otherSourceLabel(group(1))).toBe('Ook 1 resultaat in Centraal');
    expect(otherSourceLabel(group(3))).toBe('Ook 3 resultaten in Centraal');
  });

  it('marks a count the result cap may have cut short', () => {
    expect(otherSourceLabel(group(1), true)).toBe('Ook 1+ resultaten in Centraal');
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
