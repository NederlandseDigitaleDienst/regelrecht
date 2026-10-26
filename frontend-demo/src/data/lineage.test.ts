import { describe, expect, it } from 'vitest';
import { leafValues, lineageFromTrace } from './lineage';
import type { LineageNode } from './lineage';

/** A node the test expects to be there: it fails here rather than on a property of undefined. */
function present(node: LineageNode | undefined): LineageNode {
  if (!node) throw new Error('expected a lineage node');
  return node;
}

// A traced data-source step as the engine emits it (RFC-039): the register is
// a field, and `message` is the line a person reads in a terminal. The message
// is kept in the fixture precisely because nothing may parse it any more.
const resolve = (name: string, result: unknown, source: string, extra: Record<string, unknown> = {}) => ({
  node_type: 'resolve',
  name,
  result,
  resolve_type: 'DATA_SOURCE',
  source: { kind: 'DATA_SOURCE', provider: source },
  message: `Resolving from SOURCE ${source}: ${JSON.stringify(result)}`,
  children: [],
  ...extra,
});

const TRACE = {
  node_type: 'article',
  name: 'zorgtoeslagwet',
  children: [
    resolve('is_verzekerde', true, 'RVZ'),
    resolve('is_verzekerde', true, 'RVZ'), // duplicate resolution, counted once
    resolve('huishoudtype', 'ALLEENSTAANDE', 'correcties'),
    {
      node_type: 'cross_law_reference',
      name: 'wet_inkomstenbelasting#toetsingsinkomen',
      result: 2500000,
      children: [
        { node_type: 'resolve', name: 'bsn', resolve_type: 'PARAMETER', result: '100000001', children: [] },
        {
          node_type: 'action',
          name: 'toetsingsinkomen',
          children: [{ node_type: 'operation', name: 'ADD', children: [resolve('loon', 2500000, 'BELASTINGDIENST')] }],
        },
      ],
    },
  ],
};

describe('lineageFromTrace', () => {
  const nodes = lineageFromTrace(TRACE, 'zorgtoeslagwet', { bsn: '100000001' });

  it('lists every register value once, with its owning law and key', () => {
    const values = nodes.filter((n) => n.kind === 'value');
    expect(values.map((v) => v.name)).toEqual(['is_verzekerde', 'huishoudtype']);
    expect(values[0]).toMatchObject({ law: 'zorgtoeslagwet', service: 'RVZ', corrected: false, keyField: 'bsn', keyValue: '100000001' });
  });

  it('marks a value from the corrections source as corrected, without a service', () => {
    const corrected = present(nodes.find((n) => n.name === 'huishoudtype'));
    expect(corrected.corrected).toBe(true);
    expect(corrected.service).toBeNull();
  });

  it('nests a cross-law call as a law node with the values it used', () => {
    const law = present(nodes.find((n) => n.kind === 'law'));
    expect(law).toMatchObject({ law: 'wet_inkomstenbelasting', name: 'toetsingsinkomen', value: 2500000 });
    expect(law.children).toHaveLength(1);
    expect(law.children?.[0]).toMatchObject({ name: 'loon', service: 'BELASTINGDIENST', law: 'wet_inkomstenbelasting' });
  });

  it('flattens to the register values for counting', () => {
    expect(leafValues(nodes).map((v) => v.name)).toEqual(['is_verzekerde', 'huishoudtype', 'loon']);
  });

  it('finds what an operation resolved at the point of use (RFC-043)', () => {
    // The shape a lazy resolution leaves: the operation reads `$inkomen`, and
    // the cross-law call and the register value hang under that read.
    const lazy = {
      node_type: 'article',
      name: 'zorgtoeslagwet',
      children: [
        {
          node_type: 'action',
          name: 'voldoet',
          children: [
            {
              node_type: 'operation',
              name: 'AND',
              children: [
                {
                  node_type: 'resolve',
                  name: 'leeftijd',
                  resolve_type: 'RESOLVED_INPUT',
                  children: [resolve('leeftijd', 17, 'RvIG')],
                },
                {
                  node_type: 'resolve',
                  name: 'inkomen',
                  resolve_type: 'RESOLVED_INPUT',
                  children: [
                    {
                      node_type: 'cross_law_reference',
                      name: 'wet_inkomstenbelasting#inkomen',
                      result: 100,
                      children: [
                        { node_type: 'resolve', name: 'bsn', resolve_type: 'PARAMETER', result: '100000001', children: [] },
                        resolve('loon', 100, 'BELASTINGDIENST'),
                      ],
                    },
                  ],
                },
              ],
            },
          ],
        },
      ],
    };
    const found = lineageFromTrace(lazy, 'zorgtoeslagwet', { bsn: '100000001' });
    expect(found.map((n) => [n.kind, n.name])).toEqual([
      ['value', 'leeftijd'],
      ['law', 'inkomen'],
    ]);
    expect(present(found[1]).children?.map((n) => n.name)).toEqual(['loon']);
  });

  it('handles a trace without children and a call keyed on kvk', () => {
    expect(lineageFromTrace({ children: [] }, 'x', { kvk_nummer: '85234567' })).toEqual([]);
    expect(lineageFromTrace(null, 'x', {})).toEqual([]);
  });
});
