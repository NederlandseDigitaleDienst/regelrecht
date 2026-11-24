import { describe, it, expect } from 'vitest';
import {
  chainCounts,
  chainUrl,
  gapTags,
  groupChain,
  hotspotsUrl,
  pathText,
  preselection,
  reasonText,
  refText,
} from './enrichChain';
import type { Chain, ChainNode } from './enrichChain';

const lawName = (id: string) => ({ awir: 'Awir', regeling: 'Regeling standaardpremie' })[id] ?? id;

function node(overrides: Partial<ChainNode>): ChainNode {
  return {
    law_id: 'zorgtoeslag',
    bwb_id: 'BWBZ',
    article: '2',
    depth: 0,
    has_mr: true,
    gaps: { missing_mr: false, dangling: 0, unresolved: 0, open_terms: 0 },
    via: null,
    hotspot: null,
    ...overrides,
  };
}

const chain: Chain = {
  root: { law_id: 'zorgtoeslag', article: '2' },
  nodes: [
    node({}),
    node({ article: '3', via: { law_id: 'zorgtoeslag', article: '2', kind: 'cites' } }),
    node({
      law_id: 'awir',
      bwb_id: 'BWBA',
      article: '8',
      depth: 1,
      has_mr: false,
      gaps: { missing_mr: true, dangling: 0, unresolved: 0, open_terms: 0 },
      via: { law_id: 'zorgtoeslag', article: '3', kind: 'binds' },
      hotspot: { score: 6, reasons: [{ signal: 'M', points: 3, detail: 'Geen machine_readable', refs: [] }] },
    }),
  ],
  gaps: [],
  partial: { unread_laws: 0, depth_capped: false, node_capped: false },
};

describe('pathText', () => {
  it('walks back to the root and names a law only where the path leaves one', () => {
    expect(pathText(chain.nodes[2]!, chain, lawName)).toBe('via art. 2 → art. 3 → Awir art. 8');
    expect(pathText(chain.nodes[1]!, chain, lawName)).toBe('via art. 2 → art. 3');
    expect(pathText(chain.nodes[0]!, chain, lawName)).toBeNull();
  });
});

describe('groupChain and counts', () => {
  it('groups per depth and law, in the order given', () => {
    const groups = groupChain(chain);
    expect(groups.map((g) => [g.depth, g.lawId, g.nodes.length])).toEqual([
      [0, 'zorgtoeslag', 2],
      [1, 'awir', 1],
    ]);
    expect(chainCounts(chain)).toEqual({ articles: 3, laws: 2, missing: 1, hotspots: 1 });
  });

  it('preselects the root and the hotspots', () => {
    expect([...preselection(chain)]).toEqual(['zorgtoeslag#2', 'awir#8']);
  });
});

describe('words', () => {
  it('names gaps and reasons in plain Dutch', () => {
    expect(gapTags({ missing_mr: true, dangling: 2, unresolved: 1, open_terms: 0 })).toEqual([
      'Geen machine_readable',
      '2 losse bindingen',
      '1 binding komt niet aan',
    ]);
    expect(
      reasonText(
        { signal: 'B', points: 8, detail: '4 artikelen elders lezen dit', refs: ['awir#8', 'awir#9', 'regeling#1', 'x#2'] },
        lawName,
      ),
    ).toBe('4 artikelen elders lezen dit: Awir art. 8, Awir art. 9, Regeling standaardpremie art. 1, … (+8)');
    expect(refText('zorgtoeslag#4', lawName, 'zorgtoeslag')).toBe('art. 4');
  });

  it('builds the URLs with everything encoded', () => {
    expect(chainUrl('t-abcd1234', 'wet a', '2:3', 6)).toBe(
      '/api/trajects/t-abcd1234/corpus/laws/wet%20a/articles/2%3A3/chain?depth=6',
    );
    expect(hotspotsUrl('t-abcd1234', 5)).toBe('/api/trajects/t-abcd1234/hotspots?limit=5');
    expect(hotspotsUrl('t-abcd1234', 50, 'awir')).toBe('/api/trajects/t-abcd1234/hotspots?limit=50&law_id=awir');
  });
});
