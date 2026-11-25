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
const panel: { value: Partijen } = {
  value: { uitvoerder: { cel: 'gemeente', cel_naam: 'gemeente Voorbeeld' }, partijen: [], antwoord_op_verzoek: [] },
};

vi.mock('./api', () => ({
  backofficeApi: { casus: async () => casus, formulier: async () => vormen, ik: async () => ik, kroniek: async () => null },
  beheerApi: { partijen: async () => panel.value },
}));

async function toon(partijen: Partijen['partijen'], antwoord_op_verzoek: string[] = [], gekoppeld: Partijen['gekoppeld'] = []) {
  panel.value = { ...panel.value, partijen, antwoord_op_verzoek, gekoppeld };
  const router = createRouter({ history: createMemoryHistory(), routes: [{ path: '/:p(.*)*', component: { render: () => null } }] });
  const w = mount(BeheerView, { global: { plugins: [router], stubs: { teleport: true } } });
  await flushPromises();
  return w;
}

const knoppen = (w: Awaited<ReturnType<typeof toon>>) => w.findAll('nldd-button').map((b) => b.attributes('text'));

describe('the admin panel', () => {
  it('offers the facts a party records, a decision on what waits, and a copy to send', async () => {
    const w = await toon([
      { cel: 'adviseur', cel_naam: 'Adviseur Voorbeeld', open: [], toe_te_zenden: [], verzoeken: [] },
      { cel: 'rechter', cel_naam: 'Rechtbank Voorbeeld', open: [{ stuk: gram('b1', 'BEROEP') }], toe_te_zenden: [gram('u1', 'UITSPRAAK')], verzoeken: [] },
    ]);
    expect(knoppen(w)).toEqual(['Advies uitgebracht vastleggen', 'Afschrift toezenden']);
    // The appeal opens beside the panel, to decide on the court's behalf.
    expect(w.findAll('nldd-list-item').filter((r) => r.attributes('button') !== undefined)).toHaveLength(1);
  });

  it('records an answer from an open request only, which says for whom it is', async () => {
    const verzoek: Gram = {
      id: 'v1',
      kind: 'EXECUTOGRAM',
      basis: 'beleid#9',
      fact: 'verzoek_om_onderzoek_ontvangen',
      recorded_on: '2026-10-13',
      recording_actor: 'adviseur',
      fields: { advies: true, aanvraag: 'a1' },
    };
    const w = await toon(
      [
        {
          cel: 'adviseur',
          cel_naam: 'Adviseur Voorbeeld',
          open: [],
          toe_te_zenden: [],
          verzoeken: [{ verzoek, antwoordfeit: 'beleid#5#advies_uitgebracht' }],
        },
      ],
      ['beleid#5#advies_uitgebracht'],
    );
    // The answer starts at the request; on its own it is only kept in
    // reserve, for an applicant without an open request.
    expect(knoppen(w)).toEqual(['Advies uitgebracht vastleggen zonder verzoek']);
    const cel = w.find('nldd-text-cell');
    expect(cel.attributes('text')).toBe('Verzoek om onderzoek ontvangen');
    expect(cel.attributes('supporting-text')).toContain('13 oktober 2026');
    await w.find('nldd-list-item[button]').trigger('click');
    // The sheet opens on the answer, with what the request already holds.
    const sheet = w.findComponent({ name: 'FeitVastleggen' });
    expect(sheet.props('keuze')).toMatchObject({ cel: 'adviseur', verzoek: 'v1', vast: { advies: true } });
  });

  it('says so when the process asks nothing of a party', async () => {
    const w = await toon([{ cel: 'rechter', cel_naam: 'Rechtbank Voorbeeld', open: [], toe_te_zenden: [], verzoeken: [] }]);
    expect(knoppen(w)).toEqual([]);
    expect(w.text()).toContain('Op dit moment vraagt het proces niets van Rechtbank Voorbeeld');
  });

  it('points to the system of a linked party, and enters for it only after the admin asks to', async () => {
    const w = await toon(
      [{ cel: 'adviseur', cel_naam: 'Adviseur Voorbeeld', nooduitweg: true, open: [], toe_te_zenden: [], verzoeken: [] }],
      [],
      [{ cel: 'adviseur', cel_naam: 'Adviseur Voorbeeld', voorkant: 'https://adviseur.example.org' }],
    );
    expect(knoppen(w)).toEqual(['Naar het systeem van Adviseur Voorbeeld', 'Toch hier invoeren']);
    const link = w.find('nldd-button[href]');
    expect(link.attributes('href')).toBe('https://adviseur.example.org');
    expect(link.attributes('target')).toBe('_blank');
    expect(w.find('nldd-banner[variant="warning"]').exists()).toBe(false);

    await w.find('nldd-button[text="Toch hier invoeren"]').trigger('click');
    expect(knoppen(w)).toEqual(['Naar het systeem van Adviseur Voorbeeld', 'Invoer hier verbergen', 'Advies uitgebracht vastleggen']);
    expect(w.find('nldd-banner[variant="warning"]').attributes('text')).toBe('Adviseur Voorbeeld werkt in een eigen systeem');

    await w.find('nldd-button[text="Invoer hier verbergen"]').trigger('click');
    expect(knoppen(w)).toEqual(['Naar het systeem van Adviseur Voorbeeld', 'Toch hier invoeren']);
  });

  it('offers no way to enter for a linked party when the server does not', async () => {
    const w = await toon([], [], [{ cel: 'adviseur', cel_naam: 'Adviseur Voorbeeld', voorkant: null }]);
    expect(knoppen(w)).toEqual([]);
    expect(w.text()).toContain('Adviseur Voorbeeld werkt in een eigen systeem');
    expect(w.text()).not.toContain('Deze casus kent geen externe partijen');
  });
});
