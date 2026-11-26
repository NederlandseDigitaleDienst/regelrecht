// Every record of a chronicle opens with what it holds; a submission, or a
// decision on one, leads on to its panel in the work queue.
import { flushPromises, mount } from '@vue/test-utils';
import { describe, expect, it, vi } from 'vitest';
import { createMemoryHistory, createRouter } from 'vue-router';
import type { Casus, Gram } from '../gedeeld/types';
import KroniekView from './KroniekView.vue';

const casus: Casus = {
  id: 'voorbeeld',
  cel: 'gemeente',
  cel_naam: 'gemeente Voorbeeld',
  artikel: 'wet#1',
  besluit: 'wet#1',
  andere_cellen: [],
  indieningen: [{ artikel: 'wet#1', besluit: 'wet#1', cel: 'gemeente', cel_naam: 'gemeente Voorbeeld', soort: 'AANVRAAG' }],
  vandaag: '2026-10-13',
};
const g = (id: string, kind: string, refers_to?: string, extra: Partial<Gram> = {}): Gram => ({
  id,
  kind,
  basis: 'wet#1',
  refers_to,
  recorded_on: '2026-10-13',
  recording_actor: 'gemeente',
  ...extra,
});
const grammen = [
  g('a', 'AANVRAAG', undefined, { fields: { naam_aanvrager: 'Anna Voorbeeld' } }),
  g('b', 'BESCHIKKING', 'a', { outputs: { verleend: true } }),
  g('c', 'EXECUTOGRAM', undefined, { fact: 'betaling_gedaan', fields: { bedrag: 10 } }),
];

vi.mock('./api', () => ({
  backofficeApi: {
    casus: async () => casus,
    formulier: async () => ({ teksten: {} }),
    ik: async () => ({ cellen: [{ cel: 'gemeente', cel_naam: 'gemeente Voorbeeld', lezen: true, beslissen: true }] }),
    kroniek: async () => ({
      cel: 'gemeente',
      cel_naam: 'gemeente Voorbeeld',
      kop: 3,
      grammen,
      stand: { rollen: { a: 'indiening', b: 'besluit', c: 'vastlegging' }, herroepen: {}, antwoorden: {} },
    }),
  },
}));

async function toon() {
  const leeg = { render: () => null };
  const router = createRouter({
    history: createMemoryHistory(),
    routes: [
      { path: '/backoffice/kroniek/:cel?', name: 'kroniek', component: KroniekView, props: true },
      { path: '/backoffice/behandelen/:cel/:id', name: 'behandelen', component: leeg },
    ],
  });
  await router.push('/backoffice/kroniek/gemeente');
  const w = mount({ template: '<RouterView />' }, { global: { plugins: [router], stubs: { teleport: true } } });
  await flushPromises();
  return { w, router };
}
const rijen = (w: Awaited<ReturnType<typeof toon>>['w']) =>
  w.findAll('nldd-list[accessible-label="Kroniek van gemeente Voorbeeld"] nldd-list-item');
const knop = (w: Awaited<ReturnType<typeof toon>>['w'], tekst: string) =>
  w.findAll('nldd-button').find((b) => b.attributes('text') === tekst);

describe('the chronicle', () => {
  it('lists the newest record first, each with its number in the chronicle, and lets every one open', async () => {
    const { w } = await toon();
    const teksten = rijen(w).map((r) => r.find('nldd-text-cell').attributes('text'));
    expect(teksten.map((t) => t?.split(':')[0])).toEqual(['3', '2', '1']);
    expect(rijen(w)[1]!.find('nldd-text-cell').attributes('supporting-text')).toContain('verwijst naar nr. 1');
    expect(rijen(w).every((r) => r.attributes('button') !== undefined)).toBe(true);
  });

  it('shows a decision with what came out, and leads to the submission it answers', async () => {
    const { w, router } = await toon();
    await rijen(w)[1]!.trigger('click');
    expect(w.find('nldd-sheet').attributes('open')).toBeDefined();
    expect(w.findAll('nldd-list').some((l) => l.attributes('accessible-label') === 'Uitkomst van het besluit')).toBe(true);
    await knop(w, 'Open in Aanvragen')!.trigger('click');
    await flushPromises();
    expect(router.currentRoute.value.path).toBe('/backoffice/behandelen/gemeente/a');
  });

  it('leads from a submission straight to its panel', async () => {
    const { w, router } = await toon();
    await rijen(w)[2]!.trigger('click');
    await knop(w, 'Open in Aanvragen')!.trigger('click');
    await flushPromises();
    expect(router.currentRoute.value.path).toBe('/backoffice/behandelen/gemeente/a');
  });

  it('shows a recorded fact with what it holds, and no way to the work queue', async () => {
    const { w } = await toon();
    await rijen(w)[0]!.trigger('click');
    expect(w.find('nldd-sheet').attributes('open')).toBeDefined();
    expect(knop(w, 'Open in Aanvragen')).toBeUndefined();
  });
});
