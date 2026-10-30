import { readFileSync } from 'fs';
import { load } from 'js-yaml';
import { describe, expect, it } from 'vitest';
import { bouwGids, guideFor, invoerRegels } from '../src/scenarioGuides.js';

// The guides are built from the scenarios of the case YAML; these tests
// check that every scenario of every case gives a guide the panel can show.
const casus = (id) => ({
  id,
  ...load(readFileSync(new URL(`../../corpus/casussen/${id}.yaml`, import.meta.url), 'utf8')),
});
const CASUSSEN = ['vakantieverlof', 'gehandicaptenparkeerkaart', 'bedrijfsparkeervergunning', 'zorgtoeslag'];
const TABS = ['aanvrager', 'bestuursorgaan', 'andere'];

describe.each(CASUSSEN)('the guides of %s', (id) => {
  const c = casus(id);
  const list = c.scenarios.map((s) => guideFor(c, s.naam));

  it('has a guide for every scenario, by its name', () => {
    expect(list.map((g) => g?.name)).toEqual(c.scenarios.map((s) => s.naam));
  });

  it('sums up each scenario in one sentence', () => {
    for (const g of list) expect(g.summary, g.name).toMatch(/^[^.!?]+[.!?]$/);
  });

  it('ends each scenario with one sentence', () => {
    for (const s of c.scenarios) expect(s.eind, s.naam).toMatch(/^[^.!?]+[.!?]$/);
  });

  it('gives every step what the panel shows', () => {
    for (const g of list) {
      g.steps.forEach((s, i) => {
        const at = `${g.name}, step ${i + 1}`;
        expect(typeof s.title, at).toBe('string');
        expect(TABS, at).toContain(s.tab);
        expect(s.story, at).toMatch(/^[^.!?]+[.!?]$/);
        expect(s.todo, at).toMatch(/^[^.!?]+[.!?]$/);
        expect(typeof s.done, at).toBe('function');
        if (s.expect) expect(typeof s.expect.check, at).toBe('function');
        if (i < g.steps.length - 1) expect(s.seen, at).toMatch(/^[^.!?]+[.!?]$/);
      });
    }
  });

  it('lists every value a step fills in', () => {
    for (const g of list) {
      for (const s of g.steps) {
        const namen = Object.keys(s.stap.invoer ?? {}).map((naam) => `${naam.replaceAll('_', ' ')}: `);
        expect(s.values.map((v) => v.slice(0, v.indexOf(': ') + 2)), `${g.name}: ${s.title}`).toEqual(namen);
      }
    }
  });

  it('only moves the play date forward, from the first day of the case', () => {
    const first = String(c.speeldatum);
    for (const g of list) {
      const dates = g.steps.map((s) => s.date).filter(Boolean).map(String);
      expect(dates, g.name).toEqual([...dates].sort());
      if (dates.length) expect(dates[0] >= first, g.name).toBe(true);
    }
  });

  it('shows the guide on screen as recorded', async () => {
    const shown = list.map((g) => ({
      name: g.name,
      summary: g.summary,
      steps: g.steps.map((s) => ({
        title: s.title,
        tab: s.tab,
        date: s.date ?? null,
        story: s.story,
        todo: s.todo,
        values: s.values,
        seen: s.seen,
        expect: s.expect?.text ?? null,
      })),
    }));
    await expect(`${JSON.stringify(shown, null, 1)}\n`).toMatchFileSnapshot(`./__snapshots__/gids-${id}.json`);
  });
});

describe('invoerRegels', () => {
  it('gives each value a line, with yes or no and a date as the page writes them', () => {
    expect(invoerRegels({ kvk_nummer: '99990022', kenteken: 'ZZ-002-Z' })).toEqual([
      'kvk nummer: 99990022',
      'kenteken: ZZ-002-Z',
    ]);
    expect(invoerRegels({ specifieke_aard_beroep_ouder: false })).toEqual(['specifieke aard beroep ouder: Nee']);
    expect(invoerRegels({ eerste_verlofdag: '2027-05-10' })).toEqual(['eerste verlofdag: 10 mei 2027']);
    expect(invoerRegels({})).toEqual([]);
  });
});

describe('bouwGids', () => {
  it('expects of a refused step the refusal of its kind, not a decision', () => {
    const g = bouwGids({
      naam: 'x',
      stappen: [{ stap: 'aanvraag' }, { stap: 'besluit', uitkomst: 'geweigerd' }],
    });
    const besluit = g.steps[1];
    const geweigerd = "refused: the decision of wet#1 asks 'x' of the deciding authority";
    expect(besluit.done({ refusals: [geweigerd], decided: [] })).toBe(true);
    // Another refusal, such as a play date that goes back, does not count.
    expect(besluit.done({ refusals: ['the day goes back'], decided: [] })).toBe(false);
    expect(besluit.expect.check({ decided: [true] })).toBe(false);
  });

  it('counts a record as done whatever the visitor changed in it', () => {
    const feit = 'beleid#1#vestiging_ingeschreven';
    const casus = { andere_cellen: [{ cel: 'kvk', begin: { executogrammen: [{ feit }, { feit }] } }] };
    const g = bouwGids(
      { naam: 'x', stappen: [{ stap: 'vastleggen', cel: 'kvk', feit, invoer: { postcode: '7713 AA' } }] },
      casus,
    );
    const k = (n) => ({ executogrammen: (f) => (f === 'vestiging_ingeschreven' ? n : 0) });
    expect(g.steps[0].done(k(2))).toBe(false);
    expect(g.steps[0].done(k(3))).toBe(true);
  });
});

describe('guideFor', () => {
  const c = casus('vakantieverlof');
  const naam = c.scenarios[0].naam;

  it('builds a guide once per scenario', () => {
    expect(guideFor(c, naam)).toBe(guideFor(c, naam));
  });

  it('has none for a case from a source, plain-text steps, an unknown scenario or no case', () => {
    expect(guideFor({ ...c, bron: 'napp' }, naam)).toBeNull();
    expect(guideFor({ id: 'x', scenarios: [{ naam, stappen: ['Klik op Aanvraag indienen.'] }] }, naam)).toBeNull();
    expect(guideFor(c, 'Bestaat niet')).toBeNull();
    expect(guideFor(null, naam)).toBeNull();
  });
});
