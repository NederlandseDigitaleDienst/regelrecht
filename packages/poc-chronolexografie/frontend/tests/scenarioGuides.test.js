import { readFileSync } from 'fs';
import { load } from 'js-yaml';
import { describe, expect, it } from 'vitest';
import { bouwGids, guideFor, invoerRegels } from '../src/scenarioGuides.js';
import { progress } from '../src/scenarioProgress.js';

// The guides are built from the scenarios of the case YAML; these tests
// check that every scenario of every case gives a guide the panel can show.
const casus = (id) => ({
  id,
  ...load(readFileSync(new URL(`../../corpus/casussen/${id}.yaml`, import.meta.url), 'utf8')),
});
const CASUSSEN = ['vakantieverlof', 'gehandicaptenparkeerkaart', 'bedrijfsparkeervergunning', 'zorgtoeslag'];
const TABS = ['aanvrager', 'bestuursorgaan', 'andere'];
// One sentence: no full stop, question or exclamation mark before the
// end, except a dot inside a number (€ 1.577).
const ONE_SENTENCE = /^(?:[^.!?]|\.(?=\d))+[.!?]$/;

describe.each(CASUSSEN)('the guides of %s', (id) => {
  const c = casus(id);
  const list = c.scenarios.map((s) => guideFor(c, s.naam));

  it('has a guide for every scenario, by its name', () => {
    expect(list.map((g) => g?.name)).toEqual(c.scenarios.map((s) => s.naam));
  });

  it('gives every action a step of its own', () => {
    c.scenarios.forEach((s, i) =>
      expect(list[i].steps, s.naam).toHaveLength(s.stappen.filter((x) => typeof x === 'object').length),
    );
  });

  it('sums up each scenario in one sentence', () => {
    for (const g of list) expect(g.summary, g.name).toMatch(ONE_SENTENCE);
  });

  it('ends each scenario with one sentence', () => {
    for (const s of c.scenarios) expect(s.eind, s.naam).toMatch(ONE_SENTENCE);
  });

  it('gives every step what the panel shows', () => {
    for (const g of list) {
      g.steps.forEach((s, i) => {
        const at = `${g.name}, step ${i + 1}`;
        expect(typeof s.title, at).toBe('string');
        expect(TABS, at).toContain(s.tab);
        expect(s.story, at).toMatch(ONE_SENTENCE);
        expect(s.todo, at).toMatch(ONE_SENTENCE);
        expect(typeof s.done, at).toBe('function');
        if (s.expect) expect(typeof s.expect.check, at).toBe('function');
        if (i < g.steps.length - 1) expect(s.seen, at).toMatch(ONE_SENTENCE);
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

  it("puts a record of the case's own cell on the authority's tab", () => {
    const feit = 'wet#4#leges_betaald';
    const casus = { cel: 'dienst', begin: { executogrammen: [{ feit }] }, andere_cellen: [] };
    const g = bouwGids(
      {
        naam: 'x',
        stappen: [
          { stap: 'vastleggen', feit, invoer: { bedrag: 5000 } },
          { stap: 'vastleggen', cel: 'dienst', feit },
        ],
      },
      casus,
    );
    expect(g.steps.map((s) => s.tab)).toEqual(['bestuursorgaan', 'bestuursorgaan']);
    expect(g.steps[0].seen).toContain('bestuursorgaan');
    // The begin of the case's own cell counts before the scenario.
    const k = (n) => ({ executogrammen: (f) => (f === 'leges_betaald' ? n : 0) });
    expect(g.steps[0].done(k(1))).toBe(false);
    expect(g.steps[0].done(k(2))).toBe(true);
    expect(g.steps.map((s) => [s.vooraf, s.keer])).toEqual([
      [1, 1],
      [1, 2],
    ]);
  });

  it('counts the records of a fact per cell and per article', () => {
    const casus = {
      cel: 'dienst',
      begin: { executogrammen: [{ feit: 'wet#4#melding' }] },
      andere_cellen: [{ cel: 'bank', begin: { executogrammen: [{ feit: 'wet#4#melding' }, { feit: 'wet#4#melding' }] } }],
    };
    const g = bouwGids(
      {
        naam: 'x',
        stappen: [
          { stap: 'vastleggen', cel: 'bank', feit: 'wet#4#melding' },
          { stap: 'vastleggen', feit: 'wet#4#melding' },
          { stap: 'vastleggen', feit: 'wet#5#melding' },
          { stap: 'vastleggen', feit: 'wet#4#melding' },
        ],
      },
      casus,
    );
    expect(g.steps.map((s) => [s.vooraf, s.keer])).toEqual([
      [2, 1],
      [1, 1],
      [0, 1],
      [1, 2],
    ]);
  });

  it('makes a check step done once its play date is reached', () => {
    const g = bouwGids(
      {
        naam: 'x',
        stappen: [{ stap: 'toetsen', datum: '2026-03-02', antwoorden: [{ bron: 'b#1#x', verwacht: 0 }] }],
      },
      { cel: 'dienst', andere_cellen: [] },
    );
    const [toets] = g.steps;
    expect(toets.tab).toBe('bestuursorgaan');
    expect(toets.done({ datum: '2026-03-01' })).toBe(false);
    expect(toets.done({ datum: '2026-03-02' })).toBe(true);
  });

  it('makes an ex officio step done once its article has as many decisions as the scenario took', () => {
    const g = bouwGids(
      {
        naam: 'x',
        stappen: [
          { stap: 'ambtshalve', artikel: 'w#3' },
          { stap: 'ambtshalve', artikel: 'w#4' },
          { stap: 'ambtshalve', artikel: 'w#3' },
        ],
      },
      { cel: 'dienst', andere_cellen: [] },
    );
    const tel = (per) => ({ ambtshalve: (a) => per[a] ?? 0 });
    expect(g.steps.map((s) => s.tab)).toEqual(['bestuursorgaan', 'bestuursorgaan', 'bestuursorgaan']);
    expect(g.steps.map((s) => s.done(tel({ 'w#3': 1, 'w#4': 1 })))).toEqual([true, true, false]);
    expect(g.steps.map((s) => s.done(tel({ 'w#3': 2 })))).toEqual([true, false, true]);
  });

  it('makes a refused ex officio step done once the cell refused it', () => {
    const g = bouwGids(
      { naam: 'x', stappen: [{ stap: 'ambtshalve', artikel: 'w#2', uitkomst: 'geweigerd' }] },
      { cel: 'dienst', andere_cellen: [] },
    );
    const [s] = g.steps;
    const k = (refusals, n = 0) => ({ refusals, ambtshalve: () => n });
    expect(s.done(k([]))).toBe(false);
    expect(s.done(k(["no cell of the case 'x' takes the decision of w#2 ex officio"]))).toBe(true);
    expect(s.expect.check(k([], 1))).toBe(false);
  });

  it('does not count a refused ex officio step towards the steps after it', () => {
    const g = bouwGids(
      {
        naam: 'x',
        stappen: [
          { stap: 'ambtshalve', artikel: 'w#3' },
          { stap: 'ambtshalve', artikel: 'w#3', uitkomst: 'geweigerd' },
          { stap: 'ambtshalve', artikel: 'w#3' },
        ],
      },
      { cel: 'dienst', andere_cellen: [] },
    );
    const k = { refusals: ['the decision of w#3 refers to x, which is no chronolexogram'], ambtshalve: () => 2 };
    expect(g.steps.map((s) => s.done(k))).toEqual([true, true, true]);
  });

  it('carries the answers a step expects, with the play date to ask them on', () => {
    const antwoorden = [{ bron: 'beleid#1#betaalde_leges', verwacht: 5000 }];
    const g = bouwGids(
      {
        naam: 'x',
        stappen: [
          { stap: 'aanvraag', datum: '2026-02-09' },
          { stap: 'besluit', antwoorden },
        ],
      },
      { cel: 'dienst', speeldatum: '2026-02-02', andere_cellen: [] },
    );
    expect(g.steps.map((s) => [s.antwoordDatum, s.antwoorden])).toEqual([
      ['2026-02-09', []],
      ['2026-02-09', antwoorden],
    ]);
  });
});

describe('guideFor', () => {
  const c = casus('vakantieverlof');
  const naam = c.scenarios[0].naam;

  it('builds a guide once per scenario', () => {
    expect(guideFor(c, naam)).toBe(guideFor(c, naam));
  });

  it('has none for a scenario of text alone, an unknown scenario or no case', () => {
    expect(guideFor({ id: 'x', scenarios: [{ naam, stappen: ['Klik op Aanvraag indienen.'] }] }, naam)).toBeNull();
    expect(guideFor(c, 'Bestaat niet')).toBeNull();
    expect(guideFor(null, naam)).toBeNull();
  });
});

// A case from a source (FICTIEF), the fixtures of the scenario test and of
// the prefill, as the app gets it: with the name of its source.
const bronCasus = (map, id) => ({
  id,
  bron: 'voorbeeld',
  ...load(readFileSync(new URL(`../../tests/${map}/casussen/${id}.yaml`, import.meta.url), 'utf8')),
});

describe('the guide of a case from a source', () => {
  const c = bronCasus('bron-scenarios', 'voorbeeld_subsidie');

  it('has a guide for every scenario, with a step per action', () => {
    for (const s of c.scenarios) {
      const g = guideFor(c, s.naam);
      expect(g?.name, s.naam).toBe(s.naam);
      expect(g.steps.map((x) => x.stap), s.naam).toEqual(s.stappen.filter((x) => typeof x === 'object'));
      for (const x of g.steps) {
        expect(TABS, s.naam).toContain(x.tab);
        expect(x.title, s.naam).toBeTruthy();
        expect(x.todo, s.naam).toBeTruthy();
      }
    }
  });

  it('checks what the scenario expects of its steps', () => {
    const g = guideFor(c, 'Bezwaar: gegrond');
    expect(g.steps.map((x) => x.expect?.text ?? null)).toEqual([
      null,
      'hier hoort het besluit de aanvraag af te wijzen',
      null,
      'hier hoort het bezwaar gegrond te zijn',
    ]);
    expect(g.steps[1].values).toEqual(['partij geregistreerd: Nee']);
  });

  it('keeps a step of text readable, as an instruction with the next action and no check of its own', () => {
    const g = guideFor(c, 'Bezwaar: gegrond');
    expect(g.steps).toHaveLength(4);
    expect(g.steps[0].notes).toEqual(['Een stap als tekst; de speler slaat haar over.']);
    expect(g.steps.slice(1).every((x) => x.notes.length === 0)).toBe(true);
    // The step of text does not hold the scenario up: one application is the first step done.
    expect(progress(g, { aanvragen: 1, decided: [] }).current).toBe(1);
  });

  it('gives a line of text after the last action to the end', () => {
    const g = bouwGids({ naam: 'x', stappen: [{ stap: 'aanvraag' }, 'Kijk in de kroniek.'] });
    expect(g.steps).toHaveLength(1);
    expect(g.after).toEqual(['Kijk in de kroniek.']);
  });

  it('refuses a step of a kind it does not know, rather than skip it', () => {
    expect(() => bouwGids({ naam: 'x', stappen: [{ stap: 'onbekend' }] })).toThrow(/unknown step 'onbekend'/);
  });
});

describe('the guide of a case with a login step', () => {
  const c = bronCasus('bron-inlogstap', 'voorbeeld_inlogstap');
  const feiten = (n) => ({ executogrammen: (f) => (f === 'authenticatie_verricht' ? n : 0), refusals: [] });

  it('has a step on the tab Aanvrager for every login', () => {
    const g = guideFor(c, 'Inloggen lukt en de vergunning wordt verleend');
    expect(g.steps[0]).toMatchObject({ title: 'Inloggen', tab: 'aanvrager', values: ['gebruikersnaam: bakker'] });
  });

  it('is done with a login once the service recorded one more authentication', () => {
    const g = guideFor(c, 'Inloggen lukt en de vergunning wordt verleend');
    expect(g.steps[0].done(feiten(0))).toBe(false);
    expect(g.steps[0].done(feiten(1))).toBe(true);
  });

  it('is done with a refused login once the screen kept the refusal, and warns when it got in', () => {
    const g = guideFor(c, 'Na intrekking komt de bloemist niet meer binnen');
    const weigering = g.steps[3];
    expect(weigering.title).toBe('Opnieuw inloggen');
    expect(weigering.done(feiten(1))).toBe(false);
    expect(weigering.done({ ...feiten(1), refusals: ['login refused: geen_machtiging'] })).toBe(true);
    // A second authentication is the login that should have been refused.
    expect(weigering.done(feiten(2))).toBe(true);
    expect(weigering.expect.check(feiten(2))).toBe(false);
  });
});
