// The work queue shows what the account may do and nothing else: a decider
// gets the button to decide and the facts to record, a reader neither. The server still
// checks every call; this is what the screen hides. Its rows sit in lists, Te
// doen first, and what waits on someone else is kept out of Te doen.
import { flushPromises, mount } from '@vue/test-utils';
import { describe, expect, it, vi } from 'vitest';
import { createMemoryHistory, createRouter } from 'vue-router';
import type { EigenVerzoek } from './api';
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
    naam: 'A. de Vries',
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
    verzoeken: async () => ({ verzoeken: verzoeken.value }),
    kroniek: async (cel: string) => ({ cel, cel_naam: cel, kop: 1, grammen: cel === 'gemeente' ? kroniek.value : [], stand: { rollen: { a: 'indiening', r: 'vastlegging' }, herroepen: {}, antwoorden: {} } }),
    werkvoorraad: async () => ({ cel: 'gemeente', cel_naam: 'gemeente Voorbeeld', open: werk.value }),
  },
}));

/** The executor's chronicle and what the server says waits in it. */
const kroniek: { value: Gram[] } = { value: [aanvraag] };
const werk: { value: { stuk: Gram; na?: string }[] } = { value: [{ stuk: aanvraag }] };
/** The requests the executor's policy made. */
const verzoeken: { value: EigenVerzoek[] } = { value: [] };

async function toon(cellen: Ik['cellen'], lijst?: string) {
  ik.value = { ...ik.value, cellen };
  const router = createRouter({ history: createMemoryHistory(), routes: [{ path: '/:p(.*)*', component: { render: () => null } }] });
  await router.push({ path: '/backoffice', query: lijst ? { lijst } : {} });
  const w = mount(WerkvoorraadView, { global: { plugins: [router], stubs: { teleport: true } } });
  await flushPromises();
  return w;
}

type Scherm = Awaited<ReturnType<typeof toon>>;
const knoppen = (w: Scherm) => w.findAll('nldd-button').map((b) => b.attributes('text'));
/** The rows of the open list. */
const rijenVan = (w: Scherm, lijst = 'Te doen') => w.findAll(`nldd-list[accessible-label="${lijst}"] nldd-list-item`);
/** The rows of Te doen that open a submission beside it. */
const teOpenen = (w: Scherm) =>
  rijenVan(w).filter((r) => r.attributes('button') !== undefined && r.find('nldd-text-cell').attributes('supporting-text')?.startsWith('Ontvangen')).length;
/** The lists the sidebar offers, with how many rows each holds. */
const lijsten = (w: Scherm) =>
  w.findAll('nldd-list[accessible-label="Lijsten"] nldd-list-item').map((r) => r.findAll('nldd-text-cell').map((c) => c.attributes('text')).join(' '));
const verzoek: EigenVerzoek = {
  verzoek: { id: 'v', kind: 'EXECUTOGRAM', basis: 'beleid#5', fact: 'onderzoek_gevraagd', fields: { aanvraag: 'a' }, recorded_on: '2026-10-13', recording_actor: 'gemeente' },
  aan: 'adviseur',
  aan_naam: 'Adviseur Voorbeeld',
  open: true,
  antwoord_ontvangen: null,
};

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
      const w = await toon([{ cel: 'gemeente', cel_naam: 'gemeente Voorbeeld', lezen: true, beslissen: true }]);
      const rij = w.find('nldd-list[accessible-label="Te doen"] nldd-list-item:has(nldd-badge)');
      expect(rij.find('nldd-badge').attributes('text')).toBe('Opnieuw beslissen');
      expect(rij.find('nldd-text-cell').attributes('supporting-text')).toContain('Opnieuw beslissen na afschrift van de uitspraak');
      expect(rij.find('nldd-text-cell').attributes('supporting-text')).toContain('20 november 2026');
    } finally {
      kroniek.value = [aanvraag];
      werk.value = [{ stuk: aanvraag }];
    }
  });

  it('lets a decider of the cell open a submission to decide on it', async () => {
    const w = await toon([{ cel: 'gemeente', cel_naam: 'gemeente Voorbeeld', lezen: true, beslissen: true }]);
    expect(teOpenen(w)).toBe(1);
    expect(knoppen(w)).not.toContain('Advies uitgebracht vastleggen');
  });

  it('lets a reader open a submission to read it, and says a decider decides', async () => {
    const w = await toon([{ cel: 'gemeente', cel_naam: 'gemeente Voorbeeld', lezen: true, beslissen: false }]);
    expect(teOpenen(w)).toBe(1);
    expect(w.text()).toContain('Een beslisser neemt het besluit');
  });

  it('offers a decider the facts the executor records, a reader nothing, and nobody an external party\'s', async () => {
    const recht = { cel: 'gemeente', cel_naam: 'gemeente Voorbeeld', lezen: true };
    const decider = knoppen(await toon([{ ...recht, beslissen: true }]));
    expect(decider).toContain('Betaling gedaan vastleggen');
    // The advisor's advice goes through the admin panel, not the work queue.
    expect(decider).not.toContain('Advies uitgebracht vastleggen');
    expect(knoppen(await toon([{ ...recht, beslissen: false }]))).toEqual([]);
  });

  it('shows each grant that still waits and how far it is, and lets a decider open its next act', async () => {
    const recht = { cel: 'gemeente', cel_naam: 'gemeente Voorbeeld', lezen: true };
    const decider = await toon([{ ...recht, beslissen: true }]);
    const rijen = rijenVan(decider, 'Na toekenning');
    expect(rijen).toHaveLength(0);
    const naToekenning = await toon([{ ...recht, beslissen: true }], 'na-toekenning');
    expect(rijenVan(naToekenning, 'Na toekenning')).toHaveLength(1);
    rijen.push(...rijenVan(naToekenning, 'Na toekenning'));
    // The row opens the sheet of the next act.
    expect(rijen[0]?.attributes('button')).toBeDefined();
    const cel = rijen[0]?.find('nldd-text-cell');
    // By name, as the application recorded it; the number only where there is no name.
    expect(cel?.attributes('text')).toBe('Vergunning voor A. de Vries');
    expect(cel?.attributes('supporting-text')).toBe('Vergunning gedrukt op 14 oktober 2026 · Volgende stap: Vergunning verzenden');
    const knoppenVanDecider = knoppen(decider);
    // An act after a grant is never offered on its own.
    expect(knoppenVanDecider).not.toContain('Vergunning gedrukt vastleggen');
    expect(knoppenVanDecider).not.toContain('Vergunning verzonden vastleggen');
    // A reader sees how far it is, without the button.
    const lezer = await toon([{ ...recht, beslissen: false }], 'na-toekenning');
    const rij = lezer.find('nldd-list[accessible-label="Na toekenning"] nldd-list-item');
    expect(rij.find('nldd-text-cell').attributes('supporting-text')).toBe('Vergunning gedrukt op 14 oktober 2026');
    expect(rij.attributes('button')).toBeUndefined();
    expect(knoppen(lezer)).toEqual([]);
  });

  it('puts what waits on a step here in Te doen, what waited longest first', async () => {
    const w = await toon([{ cel: 'gemeente', cel_naam: 'gemeente Voorbeeld', lezen: true, beslissen: true }]);
    // The grant waits since its last act, 14 October; the submission came in on the 13th.
    const rijen = rijenVan(w).map((r) => r.find('nldd-text-cell'));
    expect(rijen).toHaveLength(2);
    expect(rijen[0]?.attributes('supporting-text')).toContain('Ontvangen 13 oktober 2026');
    expect(rijen[1]?.attributes('text')).toBe('Vergunning voor A. de Vries');
    expect(lijsten(w)).toEqual(['Te doen 2', 'Na toekenning 1']);
  });

  it('moves a submission that waits on the answer to a request out of Te doen', async () => {
    verzoeken.value = [verzoek];
    try {
      const w = await toon([{ cel: 'gemeente', cel_naam: 'gemeente Voorbeeld', lezen: true, beslissen: true }]);
      expect(lijsten(w)).toEqual(['Te doen 1', 'Na toekenning 1', 'Wacht op anderen 1']);
      const wacht = await toon([{ cel: 'gemeente', cel_naam: 'gemeente Voorbeeld', lezen: true, beslissen: true }], 'wacht');
      const rij = rijenVan(wacht, 'Wacht op anderen');
      expect(rij).toHaveLength(1);
      expect(rij[0]?.find('nldd-text-cell').attributes('supporting-text')).toContain('Wacht op Adviseur Voorbeeld');
      // Once the answer is in, it is to do again.
      verzoeken.value = [{ ...verzoek, open: false }];
      expect(lijsten(await toon([{ cel: 'gemeente', cel_naam: 'gemeente Voorbeeld', lezen: true, beslissen: true }]))).toEqual([
        'Te doen 2',
        'Na toekenning 1',
      ]);
    } finally {
      verzoeken.value = [];
    }
  });
});
