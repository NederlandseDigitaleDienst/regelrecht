// The admin panel offers per external party what the process asks of it now:
// the facts it records, a decision on what waits for it, and a copy of a
// decision not sent yet. A party of which nothing is asked says so.
import { flushPromises, mount } from '@vue/test-utils';
import { describe, expect, it, vi } from 'vitest';
import { createMemoryHistory, createRouter } from 'vue-router';
import type { Casus, Gram, Ik, Vormen } from '../gedeeld/types';
import type { Partijen } from './api';
import BeheerView from './BeheerView.vue';

const casus: Casus = {
  id: 'voorbeeld',
  cel: 'gemeente',
  cel_naam: 'gemeente Voorbeeld',
  artikel: 'wet#1',
  besluit: 'wet#1',
  andere_cellen: [
    { cel: 'adviseur', cel_naam: 'Adviseur Voorbeeld' },
    { cel: 'rechter', cel_naam: 'Rechtbank Voorbeeld' },
  ],
  indieningen: [{ artikel: 'awb#8:70', besluit: 'awb#8:70', cel: 'rechter', cel_naam: 'Rechtbank Voorbeeld', soort: 'BEROEP' }],
  vandaag: '2026-10-13',
};
const vormen: Vormen = {
  indieningen: {},
  besluiten: {},
  eigen: { cel: 'gemeente', cel_naam: 'gemeente Voorbeeld', handelingen: [] },
  andere_cellen: [
    {
      cel: 'adviseur',
      cel_naam: 'Adviseur Voorbeeld',
      handelingen: [{ feit: 'beleid#5#advies_uitgebracht', velden: [{ naam: 'advies', type: 'boolean' }] }],
    },
    { cel: 'rechter', cel_naam: 'Rechtbank Voorbeeld', handelingen: [] },
  ],
};
const gram = (id: string, kind: string): Gram => ({ id, kind, basis: 'awb#8:70', recorded_on: '2026-10-13', recording_actor: 'rechter' });
const ik: Ik = {
  sub: 'beheerder',
  naam: null,
  burger: false,
  identiteit: null,
  niveau: 'admin',
  cellen: [],
  partijen_invoeren: true,
  klok_zetten: false,
};
const panel: { value: Partijen } = { value: { uitvoerder: { cel: 'gemeente', cel_naam: 'gemeente Voorbeeld' }, partijen: [] } };

vi.mock('./api', () => ({
  backofficeApi: { casus: async () => casus, formulier: async () => vormen, ik: async () => ik, kroniek: async () => null },
  beheerApi: { partijen: async () => panel.value },
}));

async function toon(partijen: Partijen['partijen']) {
  panel.value = { ...panel.value, partijen };
  const router = createRouter({ history: createMemoryHistory(), routes: [{ path: '/:p(.*)*', component: { render: () => null } }] });
  const w = mount(BeheerView, { global: { plugins: [router], stubs: { teleport: true } } });
  await flushPromises();
  return w;
}

const knoppen = (w: Awaited<ReturnType<typeof toon>>) => w.findAll('nldd-button').map((b) => b.attributes('text'));

describe('the admin panel', () => {
  it('offers the facts a party records, a decision on what waits, and a copy to send', async () => {
    const w = await toon([
      { cel: 'adviseur', cel_naam: 'Adviseur Voorbeeld', open: [], toe_te_zenden: [] },
      { cel: 'rechter', cel_naam: 'Rechtbank Voorbeeld', open: [gram('b1', 'BEROEP')], toe_te_zenden: [gram('u1', 'UITSPRAAK')] },
    ]);
    expect(knoppen(w)).toEqual(['Advies uitgebracht vastleggen', 'Besluit invoeren', 'Afschrift toezenden']);
  });

  it('says so when the process asks nothing of a party', async () => {
    const w = await toon([{ cel: 'rechter', cel_naam: 'Rechtbank Voorbeeld', open: [], toe_te_zenden: [] }]);
    expect(knoppen(w)).toEqual([]);
    expect(w.text()).toContain('Op dit moment vraagt het proces niets van Rechtbank Voorbeeld');
  });
});
