import { describe, expect, it } from 'vitest';
import { buildGraph, colourIndex, itemId, lawShape, lawSize, layout, neighbourhood, LAW_W } from './lawGraph';
import { caseRefsFor } from '../data/caseRefs';

interface LawSpec {
  sources?: string[];
  /** [input name, supplying law, its output] */
  inputs?: [string, string, string][];
  outputs?: string[];
}

/** The value, failing the test when it is missing. */
function must<T>(value: T | null | undefined): T {
  expect(value).toBeDefined();
  expect(value).not.toBeNull();
  return value as T;
}

const law = (id: string, { sources = [], inputs = [], outputs = [] }: LawSpec = {}) => ({
  id,
  name: id,
  service: 'X',
  doc: {
    articles: [
      {
        machine_readable: {
          execution: {
            input: [
              ...sources.map((name): { name: string; source: { regulation?: string; output?: string } } => ({ name, source: {} })),
              ...inputs.map(([name, regulation, output]) => ({ name, source: { regulation, output } })),
            ],
            output: outputs.map((name) => ({ name })),
          },
        },
      },
    ],
  },
});

describe('lawShape', () => {
  it('splits register sources from cross-law inputs and dedupes by name', () => {
    const l = law('a', { sources: ['polis'], inputs: [['leeftijd', 'brp', 'leeftijd']], outputs: ['toeslag'] });
    l.doc.articles.push({ machine_readable: { execution: { input: [{ name: 'polis', source: {} }], output: [{ name: 'toeslag' }] } } });
    const shape = lawShape(l);
    expect(shape.sources.map((s) => s.name)).toEqual(['polis']);
    expect(shape.inputs).toEqual([{ name: 'leeftijd', ref: { regulation: 'brp', output: 'leeftijd' } }]);
    expect(shape.outputs.map((o) => o.name)).toEqual(['toeslag']);
  });
});

describe('layout', () => {
  it('puts a law right of the laws it reads from', () => {
    const size = lawSize(lawShape(law('x', { outputs: ['o'] })));
    const positions = layout(
      new Map([
        ['consumer', { size, deps: ['supplier'] }],
        ['supplier', { size, deps: [] }],
      ]),
    );
    expect(must(positions.get('supplier')).x).toBe(0);
    expect(must(positions.get('consumer')).x).toBeGreaterThan(LAW_W);
  });
});

describe('buildGraph', () => {
  const brp = law('brp', { sources: ['geboortedatum'], outputs: ['leeftijd'] });
  const zt = law('zt', { inputs: [['leeftijd', 'brp', 'leeftijd'], ['inkomen', 'ib', 'inkomen']], outputs: ['hoogte'] });

  it('draws an edge from the supplying output to the input that reads it, and none to an absent law', () => {
    const { nodes, edges } = buildGraph([brp, zt]);
    expect(edges).toHaveLength(1);
    // Mee met de layout, die de leverancier links zet: van uitvoer naar invoer.
    expect(must(edges[0]).source).toBe(itemId('brp', 'out', 'leeftijd'));
    expect(must(edges[0]).target).toBe(itemId('zt', 'in', 'leeftijd'));
    expect(nodes.filter((n) => n.type === 'law')).toHaveLength(2);
    const item = must(nodes.find((n) => n.id === itemId('brp', 'src', 'geboortedatum')));
    expect(item.type).toBe('item');
    expect(item.type === 'item' && item.parentNode).toBe('brp::box::sources');
  });

  it('keeps the edges under the law frames, so no line runs across a title', () => {
    const { nodes, edges } = buildGraph([brp, zt]);
    // Een wetkader krijgt van vue-flow zelf z 0 (het heeft geen parent), en
    // `<Edges>` rendert vóór `<Nodes>`. Bij gelijke z wint de latere DOM-node,
    // dus de edge moet op 0 blijven staan en mag er niet bovenuit komen: de
    // `zIndex: 5` die hier stond tekende de lijnen over de wetnamen heen.
    expect(edges.every((e) => e.zIndex === 0)).toBe(true);
    expect(nodes.filter((n) => n.type === 'law').every((n) => n.zIndex === undefined)).toBe(true);
  });

  it('shows the persona values on the items and dims laws outside the focus', () => {
    const values = { outputs: { zt: { hoogte: '€ 1.654' } }, sources: { brp: { geboortedatum: '1989' } } };
    const ib = law('ib', { outputs: ['inkomen'] });
    const { nodes, edges } = buildGraph([brp, zt, ib], values, 'brp');
    const hoogte = must(nodes.find((n) => n.id === itemId('zt', 'out', 'hoogte')));
    expect(hoogte.type).toBe('item');
    expect(hoogte.type === 'item' && hoogte.data.value).toBe('€ 1.654');
    expect(must(nodes.find((n) => n.id === 'ib')).class).toMatch(/graph-dim/);
    expect(must(nodes.find((n) => n.id === 'zt')).class).not.toMatch(/graph-dim/);
    expect(must(edges.find((e) => e.data.to === 'ib')).style.opacity).toBeLessThan(0.5);
  });
});

describe('neighbourhood', () => {
  it('adds what a selected law reads from and what reads from it, but not further', () => {
    const brp = law('brp', { outputs: ['leeftijd'] });
    const zvw = law('zvw', { inputs: [['detentie', 'pbw', 'detentie']], outputs: ['verzekerd'] });
    const pbw = law('pbw', { outputs: ['detentie'] });
    const zt = law('zt', { inputs: [['leeftijd', 'brp', 'leeftijd'], ['verzekerd', 'zvw', 'verzekerd']], outputs: ['hoogte'] });
    const ww = law('ww', { inputs: [['leeftijd', 'brp', 'leeftijd']], outputs: ['uitkering'] });
    const shown = neighbourhood(new Set(['zt']), [brp, zvw, pbw, zt, ww]);
    expect([...shown].sort()).toEqual(['brp', 'zt', 'zvw']);
  });
});

describe('colourIndex', () => {
  it('gives a service a stable colour and wraps after seven', () => {
    const idx = colourIndex(['A', 'B', 'A', 'C', 'D', 'E', 'F', 'G', 'H']);
    expect(idx('A')).toBe(0);
    expect(idx('B')).toBe(1);
    expect(idx('H')).toBe(0);
  });
});

describe('buildGraph with a visible set', () => {
  it('keeps every law in place and only hides what is outside the set', () => {
    const brp = law('brp', { outputs: ['leeftijd'] });
    const zt = law('zt', { inputs: [['leeftijd', 'brp', 'leeftijd']], outputs: ['hoogte'] });
    const ww = law('ww', { inputs: [['leeftijd', 'brp', 'leeftijd']], outputs: ['uitkering'] });
    const all = buildGraph([brp, zt, ww]);
    const some = buildGraph([brp, zt, ww], {}, null, new Set(['brp', 'zt']));
    expect(must(some.nodes.find((n) => n.id === 'zt')).position).toEqual(must(all.nodes.find((n) => n.id === 'zt')).position);
    expect(must(some.nodes.find((n) => n.id === 'ww')).hidden).toBe(true);
    expect(must(some.nodes.find((n) => n.id === 'zt')).hidden).toBe(false);
    expect(must(some.edges.find((e) => e.data.from === 'ww')).hidden).toBe(true);
    expect(must(some.edges.find((e) => e.data.from === 'zt')).hidden).toBe(false);
  });
});

describe('edge direction classes', () => {
  it('marks what the focused law reads as outgoing and who reads it as incoming', () => {
    const brp = law('brp', { outputs: ['leeftijd'] });
    const zvw = law('zvw', { inputs: [['leeftijd', 'brp', 'leeftijd']], outputs: ['verzekerd'] });
    const zt = law('zt', { inputs: [['verzekerd', 'zvw', 'verzekerd']], outputs: ['hoogte'] });
    const { edges } = buildGraph([brp, zvw, zt], {}, 'zvw');
    expect(must(edges.find((e) => e.data.from === 'zvw')).class).toBe('graph-edge--out');
    expect(must(edges.find((e) => e.data.to === 'zvw')).class).toBe('graph-edge--in');
  });
});

describe('verwijzingen via besloten zaken', () => {
  const terras = { id: 'apv_terrassen', law_path: 'algemene_plaatselijke_verordening/terrassen', doc: { articles: [{ machine_readable: { execution: { output: [{ name: 'vergunde_oppervlakte' }] } } }] } };
  const precario = {
    id: 'precario',
    law_path: 'verordening_precariobelasting/gemeenten',
    doc: { articles: [{ machine_readable: { execution: { input: [{ name: 'vergunde_oppervlakte', source: {} }, { name: 'heeft_actieve_vergunning', source: {} }], output: [{ name: 'belasting_per_jaar' }] } } }] },
  };
  const bindings = {
    precario: {
      vergunde_oppervlakte: { kind: 'cases', field: 'vergunde_oppervlakte', select_on: [{ name: 'law', value: 'algemene_plaatselijke_verordening/terrassen' }] },
      heeft_actieve_vergunning: { kind: 'cases', field: 'approved', select_on: [{ name: 'law', value: 'algemene_plaatselijke_verordening/terrassen' }] },
    },
  };
  const byPath = (p: string) => [terras, precario].find((l) => l.law_path === p) ?? null;

  it('ziet een veld uit de besloten zaak van een andere wet als verwijzing naar die wet', () => {
    const law = { ...precario, caseRefs: caseRefsFor(precario, bindings, byPath) };
    expect(lawShape(law).inputs).toContainEqual({ name: 'vergunde_oppervlakte', ref: { regulation: 'apv_terrassen', output: 'vergunde_oppervlakte' } });
    expect(neighbourhood(new Set(['precario']), [terras, law]).has('apv_terrassen')).toBe(true);
  });

  it('laat een feit van de zaak zelf (approved) een gewone invoer', () => {
    expect(caseRefsFor(precario, bindings, byPath).map((r) => r.name)).toEqual(['vergunde_oppervlakte']);
  });
});
