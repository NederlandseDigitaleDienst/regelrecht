// The dossier and a record in it are one screen: every row opens what it
// is beside the dossier, the router keeps the component when the citizen
// goes from one to the other, and what they just submitted still shows up.
import { flushPromises, mount } from '@vue/test-utils';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { createMemoryHistory, createRouter } from 'vue-router';
import type { Casus, Gram, Stand, Vervolgstand } from '../gedeeld/types';
import { burgerRoutes } from './routes';

const casus: Casus = {
  id: 'voorbeeld',
  cel: 'gemeente',
  cel_naam: 'gemeente Voorbeeld',
  artikel: 'wet#1',
  besluit: 'wet#1',
  uitkomst: 'verleend',
  andere_cellen: [],
  indieningen: [
    { artikel: 'wet#1', besluit: 'wet#1', cel: 'gemeente', cel_naam: 'gemeente Voorbeeld', soort: 'AANVRAAG' },
    { artikel: 'awb#7:11', besluit: 'awb#7:11', cel: 'gemeente', cel_naam: 'gemeente Voorbeeld', soort: 'BEZWAAR' },
  ],
  vandaag: '2026-10-13',
};
const gram = (id: string, kind: string, basis: string, refers_to?: string, fields?: Record<string, unknown>): Gram => ({
  id,
  kind,
  basis,
  refers_to,
  fields,
  recorded_on: '2026-10-13',
  recording_actor: 'gemeente',
});
const aanvraag = gram('a', 'AANVRAAG', 'wet#1', undefined, { naam_aanvrager: 'Anna Voorbeeld' });
const besluit = { ...gram('b', 'BESCHIKKING', 'wet#1', 'a'), outputs: { verleend: false } };
const bezwaar = gram('c', 'BEZWAAR', 'awb#7:11', 'b', { gronden_bezwaar_of_beroep: 'Ik voldoe wel.' });

/**
 * What the server would say of `grammen` in a plain chain, as a fixture: an
 * application or objection is a submission, an executogram a recorded fact,
 * the rest a decision, and each submission is answered by its last decision.
 * A test with revocations gives its own (`stand`).
 */
function standVan(grammen: Gram[]): Stand {
  const rollen: Stand['rollen'] = {};
  for (const g of grammen) {
    rollen[g.id] = ['AANVRAAG', 'BEZWAAR'].includes(g.kind) ? 'indiening' : g.kind === 'EXECUTOGRAM' ? 'vastlegging' : 'besluit';
  }
  const antwoorden: Stand['antwoorden'] = {};
  for (const g of grammen.filter((x) => rollen[x.id] === 'indiening')) {
    const laatste = grammen.filter((x) => x.refers_to === g.id && rollen[x.id] === 'besluit').at(-1);
    antwoorden[g.id] = laatste ? { soort: 'besloten', besluit: laatste.id } : { soort: 'open' };
  }
  return { rollen, herroepen: {}, antwoorden };
}

const dossier = vi.fn<() => Promise<Gram[]>>();
const stand = vi.fn<(grammen: Gram[]) => Stand>(standVan);
const vervolg = vi.fn<() => Vervolgstand[]>(() => []);
vi.mock('./api', () => ({
  burgerApi: {
    casus: async () => casus,
    formulier: async () => ({ indieningen: {}, besluiten: {}, teksten: { gronden_bezwaar_of_beroep: { label: 'Waarom u het er niet mee eens bent' } } }),
    ik: async () => null,
    dossier: async () => {
      const grammen = await dossier();
      return { grammen, stand: stand(grammen), vervolg: vervolg() };
    },
    waarom: async () => null,
  },
}));

async function toon(pad: string) {
  const router = createRouter({ history: createMemoryHistory(), routes: burgerRoutes });
  await router.push(pad);
  const w = mount({ template: '<RouterView />' }, { global: { plugins: [router], stubs: { teleport: true } } });
  await flushPromises();
  return { w, router };
}
const teksten = (w: Awaited<ReturnType<typeof toon>>['w']) => w.findAll('nldd-text-cell').map((c) => c.attributes('text'));

beforeEach(() => dossier.mockReset().mockResolvedValue([aanvraag, besluit, bezwaar]));
// A test that gives the case acts after a grant takes them away again, also when it fails.
afterEach(() => {
  casus.na_toekenning = undefined;
  stand.mockReset().mockImplementation(standVan);
  vervolg.mockReset().mockReturnValue([]);
});

describe('the dossier', () => {
  // The first test mounts the routes, whose panes load on first use: on a
  // loaded machine that import alone can take longer than the default 5 s.
  it('loads again when the citizen comes back to it after submitting', async () => {
    dossier.mockReset().mockResolvedValueOnce([aanvraag]).mockResolvedValueOnce([aanvraag, besluit, bezwaar]);
    const { w, router } = await toon('/burger');
    expect(dossier).toHaveBeenCalledTimes(1);
    await router.push({ name: 'dossier', query: { ingediend: 'BEZWAAR' } });
    await flushPromises();
    expect(dossier).toHaveBeenCalledTimes(2);
    expect(teksten(w)).toContain('Bezwaar');
  }, 20_000);

  it('lets every row open, a submission as well as a decision', async () => {
    const { w } = await toon('/burger');
    const rijen = w.findAll('nldd-list-item');
    expect(rijen.length).toBe(3);
    expect(rijen.every((r) => r.attributes('button') !== undefined)).toBe(true);
  });

  it('opens an objection beside the dossier with what was filed, what it contests and that it waits', async () => {
    const { w, router } = await toon('/burger');
    await w.findAll('nldd-list-item')[2]!.trigger('click');
    await flushPromises();
    expect(router.currentRoute.value.path).toBe('/burger/dossier/c');
    const t = teksten(w);
    // What it contests, with the way there.
    expect(t).toContain('Besluit (beschikking) van 13 oktober 2026');
    // What was filed, under the label of the UI spec.
    expect(t).toContain('Waarom u het er niet mee eens bent');
    expect(w.text()).toContain('In behandeling bij gemeente Voorbeeld');
  });

  it('shows on an application the decision that answered it', async () => {
    const { w } = await toon('/burger/dossier/a');
    expect(teksten(w)).toContain('Besluit van 13 oktober 2026');
    expect(w.text()).not.toContain('In behandeling');
  });

  it('opens a decision in its own pane, also at the address decisions had before', async () => {
    const { w, router } = await toon('/burger/besluit/b');
    expect(router.currentRoute.value.name).toBe('stuk');
    expect(router.currentRoute.value.params.id).toBe('b');
    // The decision's pane, with what came out; not the pane of a submission.
    expect(w.text()).toContain('Wat eruit kwam');
    expect(w.text()).not.toContain('Wat u indiende');
  });

  it('opens a recorded fact with what it holds and what it is about', async () => {
    const afschrift = { ...gram('d', 'EXECUTOGRAM', 'awb#8:79', 'b', { opdracht_nieuw_besluit: true }), fact: 'afschrift_van_de_uitspraak' };
    dossier.mockReset().mockResolvedValue([aanvraag, besluit, bezwaar, afschrift]);
    const { w } = await toon('/burger/dossier/d');
    expect(w.text()).toContain('Wat er is vastgelegd');
    expect(teksten(w)).toContain('Besluit (beschikking) van 13 oktober 2026');
    expect(w.text()).not.toContain('In behandeling');
  });

  it('says so when a record is not in the dossier', async () => {
    const { w } = await toon('/burger/dossier/van-een-ander');
    expect(w.find('nldd-inline-dialog').attributes('text')).toBe('Dit stuk staat niet in uw dossier');
  });

  it('says under a grant how far the acts after it are, in the words of the case', async () => {
    const verleend = { ...besluit, outputs: { verleend: true } };
    const gedrukt = { ...gram('d', 'EXECUTOGRAM', 'beleid#5', 'b'), fact: 'vergunning_gedrukt', recorded_on: '2026-10-20' };
    dossier.mockResolvedValue([aanvraag, verleend, gedrukt]);
    casus.na_toekenning = {
      stand: 'Uw vergunning is verleend.',
      handelingen: [
        { feit: 'beleid#5#vergunning_gedrukt', knop: 'Drukken', stand: 'Uw vergunning is op {datum} gedrukt.' },
        { feit: 'beleid#5#vergunning_verzonden', knop: 'Verzenden', stand: 'Uw vergunning is verzonden.' },
      ],
    };
    vervolg.mockReturnValue([
      {
        besluit: 'b',
        toekenning: 'b',
        stappen: [
          { feit: 'beleid#5#vergunning_gedrukt', gedaan: '2026-10-20' },
          { feit: 'beleid#5#vergunning_verzonden', gedaan: null },
        ],
        volgende: 'beleid#5#vergunning_verzonden',
      },
    ]);
    const { w } = await toon('/burger');
    const regels = teksten(w);
    // Right under the decision, before what hangs under it.
    const onder = regels.indexOf('Besluit (beschikking)') + 1;
    expect(regels[onder]).toBe('Uw vergunning is op 20 oktober 2026 gedrukt.');
    expect(regels).toContain('Vergunning gedrukt');
  });

  it('offers an application about a grant once its acts are done, and marks a grant another took the place of', async () => {
    const verleend = { ...besluit, outputs: { verleend: true } };
    dossier.mockResolvedValue([aanvraag, verleend]);
    const na = {
      stand: 'Uw vergunning is verleend.',
      handelingen: [{ feit: 'beleid#5#vergunning_verzonden', knop: 'Verzenden', stand: 'Verzonden op {datum}.' }],
      vervallen: 'Deze vergunning geldt niet meer.',
      aanvragen: [{ artikel: 'wet#2', knop: 'Vergunning kwijt?', titel: 'Vergunning kwijt', uitleg: 'Meld het hier.', uitkomst: 'vervangen' }],
    };
    casus.na_toekenning = na;
    const klaar = { besluit: 'b', toekenning: 'b', stappen: [{ feit: 'beleid#5#vergunning_verzonden', gedaan: '2026-10-20' }], volgende: null };
    vervolg.mockReturnValue([klaar]);
    const { w, router } = await toon('/burger');
    expect(teksten(w)).toContain('Vergunning kwijt?');
    await w.findAll('nldd-list-item').find((i) => i.find('nldd-text-cell').attributes('text') === 'Vergunning kwijt?')?.trigger('click');
    await vi.waitFor(() => expect(router.currentRoute.value.query).toEqual({ artikel: 'wet#2', op: 'b' }), { timeout: 10000 });

    // While the acts still run, nothing is offered.
    vervolg.mockReturnValue([{ ...klaar, stappen: [{ feit: 'beleid#5#vergunning_verzonden', gedaan: null }], volgende: 'beleid#5#vergunning_verzonden' }]);
    expect(teksten((await toon('/burger')).w)).not.toContain('Vergunning kwijt?');

    // Once the decision on such an application took the grant's place, the
    // grant says it no longer counts.
    const melding = gram('m', 'AANVRAAG', 'wet#2', 'b');
    const duplicaat = { ...gram('x', 'BESCHIKKING', 'wet#2', 'm'), outputs: { vervangen: true } };
    dossier.mockResolvedValue([aanvraag, verleend, melding, duplicaat]);
    vervolg.mockReturnValue([{ ...klaar, toekenning: 'x' }]);
    const regels = teksten((await toon('/burger')).w);
    expect(regels).toContain('Deze vergunning geldt niet meer.');
    // The way in moves to the grant that counts now: one, under the new one.
    expect(regels.filter((t) => t === 'Vergunning kwijt?')).toHaveLength(1);
    expect(regels.indexOf('Vergunning kwijt?')).toBeGreaterThan(regels.indexOf('Deze vergunning geldt niet meer.'));
  });

  describe('after a court annulled the decision on objection', () => {
    // The refusal b, the objection c, its decision d, annulled by the receipt
    // r of the court's judgment, and the decision e taken again after it,
    // which revokes the refusal and holds a new decision in its place.
    const opBezwaar = { ...gram('d', 'BESCHIKKING', 'awb#7:11', 'c'), outputs: { bestreden_besluit_herroepen: false } };
    const ontvangst = {
      ...gram('r', 'EXECUTOGRAM', 'awb#8:79', 'd'),
      fact: 'afschrift_van_de_uitspraak',
      received: { from: 'rechtbank', id: 'u' },
      recorded_on: '2026-11-20',
    };
    const opnieuw = { ...gram('e', 'BESCHIKKING', 'awb#7:11', 'c'), after: 'r', recorded_on: '2026-12-01' };
    const grammen = [aanvraag, besluit, bezwaar, opBezwaar, ontvangst, opnieuw];
    const rollen = standVan(grammen).rollen;
    const nieuw = { basis: 'wet#1', outputs: { verleend: true }, recorded_on: '2026-12-01' };

    it('marks the revoked decisions and shows the new decision as the one that holds', async () => {
      dossier.mockResolvedValue(grammen);
      stand.mockReturnValue({
        rollen,
        herroepen: { d: 'r', b: 'e' },
        antwoorden: { a: { soort: 'in_de_plaats', besluit: 'b', door: 'e', uitvoer: 'nieuw_besluit', nieuw }, c: { soort: 'besloten', besluit: 'e' } },
      });
      const { w, router } = await toon('/burger/dossier/a');
      const tags = w.findAll('nldd-list[accessible-label="Aanvraag"] nldd-tag').map((t) => t.attributes('text'));
      // In the dossier: the refusal revoked, the annulled decision on objection, and the outcome of the new decision.
      expect(tags).toContain('Herroepen');
      expect(tags).toContain('Vernietigd');
      expect(tags).toContain('Toegewezen');
      // On the application: the new decision holds, the refusal is revoked.
      const lijst = w.find('nldd-list[accessible-label="Stand"]');
      expect(lijst.findAll('nldd-text-cell').map((c) => c.attributes('text'))).toEqual([
        'Nieuw besluit van 1 december 2026',
        'Besluit van 13 oktober 2026',
      ]);
      expect(lijst.findAll('nldd-badge').map((b) => b.attributes('text'))).toEqual(['Geldt', 'Herroepen']);
      // The refusal says it no longer holds; the decision taken again shows the new decision.
      await router.push('/burger/dossier/b');
      await flushPromises();
      expect(w.find('nldd-banner').attributes('text')).toBe('Dit besluit geldt niet meer');
      await router.push('/burger/dossier/e');
      await flushPromises();
      expect(w.text()).toContain('Het nieuwe besluit');
    });

    it('says a new decision follows while the objection waits after the receipt', async () => {
      dossier.mockResolvedValue(grammen.slice(0, 5));
      stand.mockReturnValue({
        rollen,
        herroepen: { d: 'r' },
        antwoorden: { a: { soort: 'besloten', besluit: 'b' }, c: { soort: 'vervallen', besluit: 'd', door: 'r', na: 'r' } },
      });
      const { w } = await toon('/burger/dossier/c');
      expect(w.text()).toContain('neemt een nieuw besluit');
      const rij = w.find('nldd-list[accessible-label="Stand"] nldd-list-item');
      expect(rij.find('nldd-badge').attributes('text')).toBe('Vernietigd');
    });
  });
});
