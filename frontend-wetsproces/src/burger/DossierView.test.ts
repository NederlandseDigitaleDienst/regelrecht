// The dossier and a record in it are one screen: every row opens what it
// is beside the dossier, the router keeps the component when the citizen
// goes from one to the other, and what they just submitted still shows up.
import { flushPromises, mount } from '@vue/test-utils';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { createMemoryHistory, createRouter } from 'vue-router';
import type { Casus, Gram, Vervolgstand } from '../gedeeld/types';
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

const dossier = vi.fn<() => Promise<Gram[]>>();
const vervolg = vi.fn<() => Vervolgstand[]>(() => []);
vi.mock('./api', () => ({
  burgerApi: {
    casus: async () => casus,
    formulier: async () => ({ indieningen: {}, besluiten: {}, teksten: { gronden_bezwaar_of_beroep: { label: 'Waarom u het er niet mee eens bent' } } }),
    ik: async () => null,
    dossier: async () => ({ grammen: await dossier(), vervolg: vervolg() }),
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
  vervolg.mockReset().mockReturnValue([]);
});

describe('the dossier', () => {
  it('loads again when the citizen comes back to it after submitting', async () => {
    dossier.mockReset().mockResolvedValueOnce([aanvraag]).mockResolvedValueOnce([aanvraag, besluit, bezwaar]);
    const { w, router } = await toon('/burger');
    expect(dossier).toHaveBeenCalledTimes(1);
    await router.push({ name: 'dossier', query: { ingediend: 'BEZWAAR' } });
    await flushPromises();
    expect(dossier).toHaveBeenCalledTimes(2);
    expect(teksten(w)).toContain('Bezwaar');
  });

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
});
