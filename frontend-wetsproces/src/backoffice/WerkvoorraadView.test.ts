// The work queue shows what the account may do and nothing else: a decider
// gets the button to decide, a handler the facts to record. The server still
// checks every call; this is what the screen hides.
import { flushPromises, mount } from '@vue/test-utils';
import { describe, expect, it, vi } from 'vitest';
import { createMemoryHistory, createRouter } from 'vue-router';
import type { Casus, Gram, Ik, Vormen } from '../gedeeld/types';
import WerkvoorraadView from './WerkvoorraadView.vue';

const casus: Casus = {
  id: 'voorbeeld',
  cel: 'gemeente',
  cel_naam: 'gemeente Voorbeeld',
  artikel: 'wet#1',
  besluit: 'wet#1',
  andere_cellen: [{ cel: 'adviseur', cel_naam: 'Adviseur Voorbeeld' }],
  indieningen: [{ artikel: 'wet#1', besluit: 'wet#1', cel: 'gemeente', cel_naam: 'gemeente Voorbeeld', soort: 'AANVRAAG' }],
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
  ],
};
const aanvraag: Gram = {
  id: 'a',
  kind: 'AANVRAAG',
  basis: 'wet#1',
  recorded_on: '2026-10-13',
  recording_actor: 'gemeente',
};

const ik: { value: Ik } = { value: { sub: 'x', naam: null, burger: false, identiteit: null, niveau: null, cellen: [], klok_zetten: false } };

vi.mock('./api', () => ({
  backofficeApi: {
    casus: async () => casus,
    formulier: async () => vormen,
    ik: async () => ik.value,
    kroniek: async (cel: string) => ({ cel, cel_naam: cel, kop: 1, grammen: cel === 'gemeente' ? [aanvraag] : [] }),
  },
}));

async function toon(cellen: Ik['cellen']) {
  ik.value = { ...ik.value, cellen };
  const router = createRouter({ history: createMemoryHistory(), routes: [{ path: '/:p(.*)*', component: { render: () => null } }] });
  const w = mount(WerkvoorraadView, { global: { plugins: [router], stubs: { teleport: true } } });
  await flushPromises();
  return w;
}

const knoppen = (w: Awaited<ReturnType<typeof toon>>) => w.findAll('nldd-button').map((b) => b.attributes('text'));
/** The rows of the queue that open a submission beside it. */
const teOpenen = (w: Awaited<ReturnType<typeof toon>>) =>
  w.findAll('nldd-list-item').filter((r) => r.attributes('button') !== undefined).length;

describe('the work queue', () => {
  it('lets a decider of the cell open a submission to decide on it', async () => {
    const w = await toon([{ cel: 'gemeente', cel_naam: 'gemeente Voorbeeld', lezen: true, behandelen: true, beslissen: true }]);
    expect(teOpenen(w)).toBe(1);
    expect(knoppen(w).some((t) => t?.endsWith('vastleggen'))).toBe(false);
  });

  it('lets the decider flag alone open a submission, without the facts a handler records', async () => {
    const w = await toon([{ cel: 'gemeente', cel_naam: 'gemeente Voorbeeld', lezen: true, behandelen: false, beslissen: true }]);
    expect(teOpenen(w)).toBe(1);
    expect(knoppen(w)).toEqual([]);
  });

  it('shows a handler the submissions without the button to decide', async () => {
    const w = await toon([{ cel: 'gemeente', cel_naam: 'gemeente Voorbeeld', lezen: true, behandelen: true, beslissen: false }]);
    expect(teOpenen(w)).toBe(0);
    expect(w.text()).toContain('Een beslisser van deze cel neemt het besluit');
  });

  it('offers a handler of a cell the facts that cell records, and a reader nothing', async () => {
    const recht = { cel: 'adviseur', cel_naam: 'Adviseur Voorbeeld', lezen: true, beslissen: false };
    expect(knoppen(await toon([{ ...recht, behandelen: true }]))).toContain('Advies uitgebracht vastleggen');
    expect(knoppen(await toon([{ ...recht, behandelen: false }]))).toEqual([]);
  });
});
