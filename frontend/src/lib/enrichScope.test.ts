import { describe, it, expect } from 'vitest';
import {
  articleOf,
  topArticles,
  describeScope,
  planChainRequests,
  isLargeRun,
  selectionSummary,
} from './enrichScope';
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

describe('planChainRequests', () => {
  const start = { lawId: 'zorgtoeslag', article: '2' };

  it('puts the start law in the article mode with its instruction, the start article first', () => {
    const selected = new Map([
      ['zorgtoeslag', ['1', '2']],
      ['awir', ['8', '9']],
    ]);
    const requests = planChainRequests(start, selected, { provider: 'claude', feedbackRounds: 1 }, ' let op ');
    expect(requests).toEqual([
      {
        lawId: 'zorgtoeslag',
        options: {
          provider: 'claude',
          feedbackRounds: 1,
          mode: 'article',
          articles: ['2', '1'],
          instruction: 'let op',
          context: true,
        },
      },
      {
        lawId: 'awir',
        options: { provider: 'claude', feedbackRounds: 1, mode: 'law', articles: ['8', '9'], context: true },
      },
    ]);
  });

  it('always sends the start article, and skips a law with nothing ticked', () => {
    const requests = planChainRequests(start, new Map([['awir', []]]), {});
    expect(requests).toHaveLength(1);
    expect(requests[0]!.options.articles).toEqual(['2']);
    expect(requests[0]!.options.instruction).toBeUndefined();
  });

  it('turns into a request body the server accepts', () => {
    const [, law] = planChainRequests(start, new Map([['awir', ['8']]]), {}, 'alleen hier');
    expect(enrichBody(law!.options)).toEqual({ mode: 'law', articles: ['8'], context: true });
  });
});

describe('isLargeRun', () => {
  it('warns above five laws or forty articles', () => {
    const law = (n: number) => ({ lawId: `w${n}`, options: { articles: ['1'] } });
    expect(isLargeRun([1, 2, 3, 4, 5].map(law))).toBe(false);
    expect(isLargeRun([1, 2, 3, 4, 5, 6].map(law))).toBe(true);
    expect(isLargeRun([{ lawId: 'w', options: { articles: Array.from({ length: 41 }, (_, i) => String(i)) } }])).toBe(true);
  });
});

describe('selectionSummary', () => {
  it('counts articles and laws in Dutch', () => {
    expect(selectionSummary(1, 1)).toBe('1 artikel in 1 wet');
    expect(selectionSummary(3, 2)).toBe('3 artikelen in 2 wetten');
  });
});
