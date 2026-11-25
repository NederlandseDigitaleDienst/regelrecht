// The citizen's applications are one list, a row per application, and every
// row opens the application on a page one level deeper, alone, whose back
// button returns to the list. What followed on it opens from there. The
// router keeps the component when the citizen goes from one to the other, and
// what they just submitted still shows up.
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
/** The status of a row of the list: the word beside its dot, which says nothing a reader does not hear in the word. */
const status = (rij: ReturnType<Awaited<ReturnType<typeof toon>>['w']['find']>) => {
  expect(rij.find('nldd-badge').attributes()).toHaveProperty('decorative');
  return rij.find('nldd-cell nldd-text').text();
};
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
    expect(w.find('nldd-banner').attributes('text')).toBe('Uw bezwaar is ingediend');
  }, 20_000);

  it('shows one row per application, with how it stands, that opens the application alone', async () => {
    const { w, router } = await toon('/burger');
    // The decision and the objection follow on the application: no rows of their own.
    const rijen = w.findAll('nldd-list[accessible-label="Mijn aanvragen"] nldd-list-item');
    expect(rijen.map((r) => r.find('nldd-text-cell').attributes('text'))).toEqual(['Aanvraag']);
    expect(rijen[0]!.find('nldd-text-cell').attributes('supporting-text')).toBe('13 oktober 2026 · gemeente Voorbeeld');
    expect(status(rijen[0]!)).toBe('Afgewezen');
    await rijen[0]!.trigger('click');
    await flushPromises();
    expect(router.currentRoute.value.path).toBe('/burger/dossier/a');
    expect(w.find('nldd-list[accessible-label="Mijn aanvragen"]').exists()).toBe(false);
    // The page takes the name of its row; when and where stand under it.
    expect(w.find('nldd-top-title-bar').attributes('text')).toBe('Aanvraag');
    expect(w.find('#stuk-titel').attributes('supporting-text')).toBe('13 oktober 2026 · gemeente Voorbeeld');
    // The article it rests on closes what was submitted, and the day is not said twice.
    const wat = w.find('nldd-list[accessible-label="Wat u indiende"]');
    expect(wat.findAll('nldd-list-item').at(-1)!.find('nldd-text-cell').attributes('text')).toBe('Op grond van');
    expect(w.text()).not.toContain('Ingediend op');
  });

  it('opens a waiting application with its status on top, and what was submitted right under it without a heading', async () => {
    dossier.mockReset().mockResolvedValue([aanvraag]);
    const { w } = await toon('/burger/dossier/a');
    const status = w.find('nldd-list[accessible-label="Samenvatting"] nldd-list-item').findAll('nldd-text-cell');
    expect(status.map((c) => c.attributes('text'))).toEqual(['Status', 'In behandeling']);
    expect(w.findAll('h4')).toHaveLength(0);
  });

  it('says an application waits while nothing answers it', async () => {
    dossier.mockReset().mockResolvedValue([aanvraag]);
    const { w } = await toon('/burger');
    expect(status(w.find('nldd-list[accessible-label="Mijn aanvragen"] nldd-list-item'))).toBe('In behandeling');
  });

  it('opens an objection on a page of its own with what was filed, what it contests and that it waits', async () => {
    const { w } = await toon('/burger/dossier/c');
    const t = teksten(w);
    // What it contests, with the way there.
    expect(t).toContain('Besluit (beschikking) van 13 oktober 2026');
    // What was filed, under the label of the UI spec.
    expect(t).toContain('Waarom u het er niet mee eens bent');
    const status = w.find('nldd-list[accessible-label="Samenvatting"] nldd-list-item').findAll('nldd-text-cell');
    expect(status.map((c) => c.attributes('text'))).toEqual(['Status', 'In behandeling']);
    // After what it contests, what was filed has its heading again.
    expect(w.findAll('h4').map((h) => h.text())).toContain('Wat u indiende');
  });

  it('returns from a record to the list with the back button', async () => {
    const { w, router } = await toon('/burger/dossier/b');
    expect(w.find('nldd-top-title-bar').attributes('back-text')).toBe('Mijn aanvragen');
    await w.find('nldd-top-title-bar').trigger('back');
    await flushPromises();
    expect(router.currentRoute.value.name).toBe('dossier');
    expect(w.find('nldd-list[accessible-label="Mijn aanvragen"]').exists()).toBe(true);
  });

  it('shows on an application the decision that answered it', async () => {
    const { w } = await toon('/burger/dossier/a');
    expect(teksten(w)).toContain('Besluit van 13 oktober 2026');
    expect(w.text()).not.toContain('In behandeling');
  });

  it('opens a decision on its own page, also at the address decisions had before', async () => {
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

  it('opens the letter about a record in a bottom sheet at its own address, over the page, and closes it there', async () => {
    const { w, router } = await toon('/burger/dossier/a#bericht/a');
    const sheet = () => w.find('nldd-sheet[accessible-label="Bericht"]');
    expect(sheet().attributes('placement')).toBe('bottom');
    // Its bar stays on top while the letter scrolls.
    expect(sheet().find('nldd-page').attributes('sticky-header')).toBeDefined();
    expect(sheet().attributes('open')).toBeDefined();
    // The page of the application stays under it.
    expect(w.find('#stuk-titel').exists()).toBe(true);
    await sheet().trigger('close');
    await flushPromises();
    expect(router.currentRoute.value.fullPath).toBe('/burger/dossier/a');
    expect(sheet().attributes('open')).toBeUndefined();
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
    const { w, router } = await toon('/burger');
    // On the row of the application, and on its page.
    const rij = w.find('nldd-list[accessible-label="Mijn aanvragen"] nldd-text-cell');
    expect(rij.attributes('supporting-text')).toBe('13 oktober 2026 · gemeente Voorbeeld · Uw vergunning is op 20 oktober 2026 gedrukt.');
    await router.push('/burger/dossier/a');
    await flushPromises();
    const na = w.find('nldd-list[accessible-label="Na de toekenning"]');
    expect(na.findAll('nldd-text-cell').map((c) => c.attributes('text'))).toEqual(['Uw vergunning is op 20 oktober 2026 gedrukt.']);
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
    // The way in stands on the page of the application, not in the list.
    expect(teksten((await toon('/burger')).w)).not.toContain('Vergunning kwijt?');
    const { w, router } = await toon('/burger/dossier/a');
    expect(teksten(w)).toContain('Vergunning kwijt?');
    await w.findAll('nldd-list-item').find((i) => i.find('nldd-text-cell').attributes('text') === 'Vergunning kwijt?')?.trigger('click');
    await vi.waitFor(() => expect(router.currentRoute.value.query).toEqual({ artikel: 'wet#2', op: 'b' }), { timeout: 10000 });

    // While the acts still run, nothing is offered.
    vervolg.mockReturnValue([{ ...klaar, stappen: [{ feit: 'beleid#5#vergunning_verzonden', gedaan: null }], volgende: 'beleid#5#vergunning_verzonden' }]);
    expect(teksten((await toon('/burger/dossier/a')).w)).not.toContain('Vergunning kwijt?');

    // Once the decision on such an application took the grant's place, the
    // grant says it no longer counts, and that application is a row of its own.
    const melding = gram('m', 'AANVRAAG', 'wet#2', 'b');
    const duplicaat = { ...gram('x', 'BESCHIKKING', 'wet#2', 'm'), outputs: { vervangen: true } };
    dossier.mockResolvedValue([aanvraag, verleend, melding, duplicaat]);
    vervolg.mockReturnValue([{ ...klaar, toekenning: 'x' }]);
    const lijst = (await toon('/burger')).w
      .findAll('nldd-list[accessible-label="Mijn aanvragen"] nldd-list-item')
      .map((r) => r.find('nldd-text-cell'));
    expect(lijst.map((c) => c.attributes('text'))).toEqual(['Aanvraag', 'Vergunning kwijt']);
    expect(lijst[0]!.attributes('supporting-text')).toContain('Deze vergunning geldt niet meer.');
    // The way in moves to the grant that counts now: on the page of the new application only.
    expect(teksten((await toon('/burger/dossier/a')).w)).not.toContain('Vergunning kwijt?');
    expect(teksten((await toon('/burger/dossier/m')).w)).toContain('Vergunning kwijt?');
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
      const { w, router } = await toon('/burger');
      // In the list: the application, with the outcome of the decision that holds now.
      expect(w.findAll('nldd-list[accessible-label="Mijn aanvragen"] nldd-list-item').map(status)).toEqual(['Toegewezen']);
      // On the application: the new decision holds, the refusal is revoked.
      await router.push('/burger/dossier/a');
      await flushPromises();
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
