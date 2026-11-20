// The dossier and a decision in it are one screen, so the router keeps the
// component when the citizen goes from one to the other. What they just
// submitted must still show up.
import { flushPromises, mount } from '@vue/test-utils';
import { describe, expect, it, vi } from 'vitest';
import { createMemoryHistory, createRouter } from 'vue-router';
import type { Casus, Gram } from '../gedeeld/types';
import DossierView from './DossierView.vue';

const casus: Casus = {
  id: 'voorbeeld',
  cel: 'gemeente',
  cel_naam: 'gemeente Voorbeeld',
  artikel: 'wet#1',
  besluit: 'wet#2',
  andere_cellen: [],
  indieningen: [{ artikel: 'wet#1', besluit: 'wet#2', cel: 'gemeente', cel_naam: 'gemeente Voorbeeld', soort: 'BEZWAAR' }],
  vandaag: '2026-10-13',
};
const gram = (id: string, kind: string): Gram => ({ id, kind, basis: 'wet#1', recorded_on: '2026-10-13', recording_actor: 'gemeente' });

const dossier = vi.fn<() => Promise<Gram[]>>();
vi.mock('./api', () => ({
  burgerApi: {
    casus: async () => casus,
    formulier: async () => null,
    ik: async () => null,
    dossier: () => dossier(),
    waarom: async () => null,
  },
}));

describe('the dossier', () => {
  it('loads again when the citizen comes back to it after submitting', async () => {
    dossier.mockResolvedValueOnce([gram('a', 'AANVRAAG')]).mockResolvedValueOnce([gram('a', 'AANVRAAG'), gram('b', 'BEZWAAR')]);
    const router = createRouter({
      history: createMemoryHistory(),
      routes: [
        { path: '/burger', name: 'dossier', component: DossierView },
        { path: '/burger/besluit/:id', name: 'besluit', component: DossierView, props: true },
      ],
    });
    await router.push('/burger');
    const w = mount({ template: '<RouterView />' }, { global: { plugins: [router], stubs: { teleport: true } } });
    await flushPromises();
    expect(dossier).toHaveBeenCalledTimes(1);

    await router.push({ name: 'dossier', query: { ingediend: 'BEZWAAR' } });
    await flushPromises();
    expect(dossier).toHaveBeenCalledTimes(2);
    expect(w.findAll('nldd-text-cell').map((c) => c.attributes('text'))).toContain('Bezwaar');
  });
});
