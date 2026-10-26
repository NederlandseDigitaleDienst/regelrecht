import { readFileSync } from 'fs';
import { load } from 'js-yaml';
import { describe, expect, it } from 'vitest';
import { guideFor, guides } from '../src/scenarioGuides.js';

// The guides follow the scenarios of the case YAML by name; these tests keep
// the two in step.
const casus = (id) =>
  load(readFileSync(new URL(`../../casussen/${id}.yaml`, import.meta.url), 'utf8'));
const TABS = ['aanvrager', 'bestuursorgaan', 'andere'];

describe.each(Object.keys(guides))('the guides of %s', (id) => {
  const yaml = casus(id);
  const list = guides[id];

  it('has a guide for every scenario of the YAML, in its order', () => {
    expect(list.map((g) => g.name)).toEqual(yaml.scenarios.map((s) => s.naam));
  });

  it('sums up each scenario in one sentence', () => {
    for (const g of list) expect(g.summary, g.name).toMatch(/^[^.!?]+[.!?]$/);
  });

  it('gives every step what the panel shows', () => {
    for (const g of list) {
      g.steps.forEach((s, i) => {
        const at = `${g.name}, step ${i + 1}`;
        expect(typeof s.title, at).toBe('string');
        expect(TABS, at).toContain(s.tab);
        expect(s.todo, at).toBeTruthy();
        expect(typeof s.done, at).toBe('function');
        if (s.expect) expect(typeof s.expect.check, at).toBe('function');
        // The last step shows the `eind` of the YAML instead.
        if (i < g.steps.length - 1) expect(s.seen, at).toBeTruthy();
      });
    }
  });

  it('only moves the play date forward, from the first day of the case', () => {
    const first = new Date(yaml.speeldatum).toISOString().slice(0, 10);
    for (const g of list) {
      const dates = g.steps.map((s) => s.date).filter(Boolean);
      expect(dates, g.name).toEqual([...dates].sort());
      if (dates.length) expect(dates[0] >= first, g.name).toBe(true);
    }
  });
});

describe('guideFor', () => {
  it('finds a guide by case and scenario name', () => {
    const naam = guides.vakantieverlof[0].name;
    expect(guideFor({ id: 'vakantieverlof' }, naam)).toBe(guides.vakantieverlof[0]);
  });

  it('has none for a case from a source, an unknown scenario or no case', () => {
    const naam = guides.vakantieverlof[0].name;
    expect(guideFor({ id: 'vakantieverlof', bron: 'napp' }, naam)).toBeNull();
    expect(guideFor({ id: 'vakantieverlof' }, 'Bestaat niet')).toBeNull();
    expect(guideFor(null, naam)).toBeNull();
  });
});
