// The work queue shows what the account may do and nothing else: a decider
// gets the button to decide, a handler the facts to record. The server still
// checks every call; this is what the screen hides.
import { flushPromises, mount } from '@vue/test-utils';
import { describe, expect, it, vi } from 'vitest';
import { createMemoryHistory, createRouter } from 'vue-router';
import type { Casus, Gram, Ik, Vervolgstand, Vormen } from '../gedeeld/types';
import WerkvoorraadView from './WerkvoorraadView.vue';

const casus: Casus = {
  id: 'voorbeeld',
  cel: 'gemeente',
  cel_naam: 'gemeente Voorbeeld',
  artikel: 'wet#1',
  besluit: 'wet#1',
  andere_cellen: [{ cel: 'adviseur', cel_naam: 'Adviseur Voorbeeld' }],
  indieningen: [{ artikel: 'wet#1', besluit: 'wet#1', cel: 'gemeente', cel_naam: 'gemeente Voorbeeld', soort: 'AANVRAAG' }],
  titel: 'Vergunning',
  // What follows a grant: two acts, each with the button that records it.
  na_toekenning: {
    stand: 'Uw vergunning is verleend.',
    handelingen: [
      { feit: 'beleid#5#vergunning_gedrukt', knop: 'Vergunning drukken', stand: 'Uw vergunning wordt gedrukt.' },
      { feit: 'beleid#5#vergunning_verzonden', knop: 'Vergunning verzenden', stand: 'Verzonden op {datum}.' },
    ],
  },
  vandaag: '2026-10-13',
};
const vormen: Vormen = {
  indieningen: {},
  besluiten: {},
  eigen: {
    cel: 'gemeente',
    cel_naam: 'gemeente Voorbeeld',
    handelingen: [
      { feit: 'wet#9#betaling_gedaan', velden: [{ naam: 'bedrag', type: 'amount' }] },
      { feit: 'beleid#5#vergunning_gedrukt', velden: [{ naam: 'datum_druk', type: 'date' }], systeem: { datum_druk: { soort: 'datum', waarde: '2026-10-13' } } },
      { feit: 'beleid#5#vergunning_verzonden', velden: [{ naam: 'datum_verzending', type: 'date' }] },
    ],
  },
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

// One grant, of which the first act is done.
const vervolg: Vervolgstand[] = [
  {
    besluit: 'b',
    toekenning: 'b',
    stappen: [
      { feit: 'beleid#5#vergunning_gedrukt', gedaan: '2026-10-14' },
      { feit: 'beleid#5#vergunning_verzonden', gedaan: null },
    ],
    volgende: 'beleid#5#vergunning_verzonden',
    toegekend_op: '2026-10-13',
    aanvrager: '999990032',
  },
  // A grant whose acts are all done no longer waits.
  { besluit: 'c', toekenning: 'c', stappen: [], volgende: null, toegekend_op: '2026-10-13', aanvrager: '999990020' },
];

const ik: { value: Ik } = {
  value: { sub: 'x', naam: null, burger: false, identiteit: null, niveau: null, cellen: [], partijen_invoeren: false, klok_zetten: false },
};

vi.mock('./api', () => ({
  backofficeApi: {
    casus: async () => casus,
    formulier: async () => vormen,
    ik: async () => ik.value,
    vervolg: async () => vervolg,
    kroniek: async (cel: string) => ({ cel, cel_naam: cel, kop: 1, grammen: cel === 'gemeente' ? kroniek.value : [], stand: { rollen: { a: 'indiening', r: 'vastlegging' }, herroepen: {}, antwoorden: {} } }),
    werkvoorraad: async () => ({ cel: 'gemeente', cel_naam: 'gemeente Voorbeeld', open: werk.value }),
  },
}));

/** The executor's chronicle and what the server says waits in it. */
const kroniek: { value: Gram[] } = { value: [aanvraag] };
const werk: { value: { stuk: Gram; na?: string }[] } = { value: [{ stuk: aanvraag }] };

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
  w.findAll('nldd-list[accessible-label^="Werkvoorraad"] nldd-list-item').filter((r) => r.attributes('button') !== undefined).length;

describe('the work queue', () => {
  it('shows a submission whose decision was annulled as one to decide again, after the receipt', async () => {
    const ontvangst: Gram = {
      id: 'r',
      kind: 'EXECUTOGRAM',
      basis: 'awb#8:79',
      fact: 'afschrift_van_de_uitspraak',
      refers_to: 'b',
      recorded_on: '2026-11-20',
      recording_actor: 'gemeente',
    };
    kroniek.value = [aanvraag, ontvangst];
    werk.value = [{ stuk: aanvraag, na: 'r' }];
    try {
      const w = await toon([{ cel: 'gemeente', cel_naam: 'gemeente Voorbeeld', lezen: true, behandelen: false, beslissen: true }]);
      const rij = w.find('nldd-list[accessible-label^="Werkvoorraad"] nldd-list-item');
      expect(rij.find('nldd-badge').attributes('text')).toBe('Opnieuw beslissen');
      expect(rij.find('nldd-text-cell').attributes('supporting-text')).toContain('Opnieuw beslissen na afschrift van de uitspraak');
      expect(rij.find('nldd-text-cell').attributes('supporting-text')).toContain('20 november 2026');
    } finally {
      kroniek.value = [aanvraag];
      werk.value = [{ stuk: aanvraag }];
    }
  });

  it('lets a decider of the cell open a submission to decide on it', async () => {
    const w = await toon([{ cel: 'gemeente', cel_naam: 'gemeente Voorbeeld', lezen: true, behandelen: true, beslissen: true }]);
    expect(teOpenen(w)).toBe(1);
    expect(knoppen(w)).not.toContain('Advies uitgebracht vastleggen');
  });

  it('lets the decider flag alone open a submission, without the facts a handler records', async () => {
    const w = await toon([{ cel: 'gemeente', cel_naam: 'gemeente Voorbeeld', lezen: true, behandelen: false, beslissen: true }]);
    expect(teOpenen(w)).toBe(1);
    expect(knoppen(w)).toEqual([]);
  });

  it('lets a handler open a submission to read it, and says a decider decides', async () => {
    const w = await toon([{ cel: 'gemeente', cel_naam: 'gemeente Voorbeeld', lezen: true, behandelen: true, beslissen: false }]);
    expect(teOpenen(w)).toBe(1);
    expect(w.text()).toContain('Een beslisser neemt het besluit');
  });

  it('offers a handler the facts the executor records, a reader nothing, and nobody an external party\'s', async () => {
    const recht = { cel: 'gemeente', cel_naam: 'gemeente Voorbeeld', lezen: true, beslissen: false };
    const handler = knoppen(await toon([{ ...recht, behandelen: true }]));
    expect(handler).toContain('Betaling gedaan vastleggen');
    // The advisor's advice goes through the admin panel, not the work queue.
    expect(handler).not.toContain('Advies uitgebracht vastleggen');
    expect(knoppen(await toon([{ ...recht, behandelen: false }]))).toEqual([]);
  });

  it('shows each grant that still waits and how far it is, and lets a handler open its next act', async () => {
    const recht = { cel: 'gemeente', cel_naam: 'gemeente Voorbeeld', lezen: true, beslissen: false };
    const handler = await toon([{ ...recht, behandelen: true }]);
    const rijen = handler.findAll('nldd-list[accessible-label="Na de toekenning"] nldd-list-item');
    expect(rijen).toHaveLength(1);
    // The row opens the sheet of the next act.
    expect(rijen[0]?.attributes('button')).toBeDefined();
    const cel = rijen[0]?.find('nldd-text-cell');
    expect(cel?.attributes('text')).toBe('Vergunning voor 999990032');
    expect(cel?.attributes('supporting-text')).toBe('Vergunning gedrukt op 14 oktober 2026 · Volgende stap: Vergunning verzenden');
    const knoppenVanHandler = knoppen(handler);
    // An act after a grant is never offered on its own.
    expect(knoppenVanHandler).not.toContain('Vergunning gedrukt vastleggen');
    expect(knoppenVanHandler).not.toContain('Vergunning verzonden vastleggen');
    // A reader sees how far it is, without the button.
    const lezer = await toon([{ ...recht, behandelen: false }]);
    const rij = lezer.find('nldd-list[accessible-label="Na de toekenning"] nldd-list-item');
    expect(rij.find('nldd-text-cell').attributes('supporting-text')).toBe('Vergunning gedrukt op 14 oktober 2026');
    expect(rij.attributes('button')).toBeUndefined();
    expect(knoppen(lezer)).toEqual([]);
  });
});
