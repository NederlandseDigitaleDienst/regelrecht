import { describe, it, expect } from 'vitest';
import { articleOf, topArticles, describeScope } from './enrichScope';
import { enrichBody } from '../composables/useTasks';

describe('articleOf', () => {
  it('groups an entry under the shortest existing entry it extends', () => {
    const entries = new Set(['69', '69.1', '69.1.a', '2.62', '2.63']);
    expect(articleOf('69.1', entries)).toBe('69');
    expect(articleOf('69.1.a', entries)).toBe('69');
    expect(articleOf('2.62', entries)).toBe('2.62');
    expect(articleOf('8', entries)).toBe('8');
  });
});

describe('topArticles', () => {
  it('folds entries into their article and remembers machine_readable', () => {
    const law = {
      articles: [
        { number: '1' },
        { number: '2' },
        { number: '2.a', machine_readable: { execution: {} } },
        { number: 3 },
      ],
    };
    expect(topArticles(law)).toEqual([
      { number: '1', hasMachineReadable: false },
      { number: '2', hasMachineReadable: true },
      { number: '3', hasMachineReadable: false },
    ]);
    expect(topArticles(null)).toEqual([]);
  });
});

describe('describeScope', () => {
  it('names one article, a count, or the whole law', () => {
    expect(describeScope({ mode: 'article', articles: ['3'] })).toBe('artikel 3');
    expect(describeScope({ mode: 'law', articles: ['1', '2'] })).toBe('2 artikelen');
    expect(describeScope(null)).toBe('dit regelwerk');
  });
});

describe('enrichBody', () => {
  it('sends only what was chosen, in the API field names', () => {
    expect(enrichBody(undefined)).toEqual({});
    expect(enrichBody({})).toEqual({});
    expect(
      enrichBody({
        mode: 'article',
        articles: ['3'],
        instruction: '  let op lid 2 ',
        context: true,
        feedbackRounds: 2,
        model: 'claude-sonnet-5-5',
      }),
    ).toEqual({
      mode: 'article',
      articles: ['3'],
      instruction: 'let op lid 2',
      context: true,
      feedback_rounds: 2,
      model: 'claude-sonnet-5-5',
    });
    expect(enrichBody({ mode: 'law', onlyMissing: true, instruction: '   ' })).toEqual({
      mode: 'law',
      only_missing: true,
    });
  });
});
