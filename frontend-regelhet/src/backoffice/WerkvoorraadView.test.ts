// The work queue shows what the account may do and nothing else: a decider
// gets the button to decide and the facts to record, a reader neither. The server still
// checks every call; this is what the screen hides. Its rows sit in lists, Te
// doen first, and what waits on someone else is kept out of Te doen.
import { flushPromises, mount } from '@vue/test-utils';
import { describe, expect, it, vi } from 'vitest';
import { createMemoryHistory, createRouter } from 'vue-router';
import type { EigenVerzoek, Kroniek } from './api';
import { menuactie } from '../gedeeld/schil';
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
      { feit: 'wet#9#betaling_ontvangen', velden: [{ naam: 'bedrag', type: 'amount' }] },
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
    kroniek: async (cel: string) => ({ cel, cel_naam: cel, kop: 1, grammen: cel === 'gemeente' ? kroniek.value : [], stand }),
    werkvoorraad: async () => ({ cel: 'gemeente', cel_naam: 'gemeente Voorbeeld', open: werk.value }),
  },
}));

/** What the kern says of the executor's chronicle: two decided applications, each with its decision, beside `a`. */
const stand: Kroniek['stand'] = {
  rollen: { a: 'indiening', r: 'vastlegging', t: 'indiening', bt: 'besluit', w: 'indiening', bw: 'besluit' },
  herroepen: {},
  antwoorden: { t: { soort: 'besloten', besluit: 'bt' }, w: { soort: 'besloten', besluit: 'bw' } },
};
/** The executor's chronicle and what the server says waits in it. */
const kroniek: { value: Gram[] } = { value: [aanvraag] };
const werk: { value: { stuk: Gram; na?: string }[] } = { value: [{ stuk: aanvraag }] };
/** The requests the executor's policy made. */
const verzoeken: { value: EigenVerzoek[] } = { value: [] };

async function toon(cellen: Ik['cellen'], lijst?: string, open?: string) {
  ik.value = { ...ik.value, cellen };
  const router = createRouter({ history: createMemoryHistory(), routes: [
      { path: '/backoffice/behandelen/:cel/:id', name: 'behandelen', component: { render: () => null } },
      { path: '/:p(.*)*', component: { render: () => null } },
    ],
  });
  await router.push({ path: open ? `/backoffice/behandelen/gemeente/${open}` : '/backoffice', query: lijst ? { lijst } : {} });
  const w = mount(WerkvoorraadView, {
    props: open ? { cel: 'gemeente', id: open } : {},
    global: { plugins: [router], stubs: { teleport: true, BehandelenPaneel: true } },
  });
  await flushPromises();
  return Object.assign(w, { router });
}

type Scherm = Awaited<ReturnType<typeof toon>>;
const knoppen = (w: Scherm) => w.findAll('nldd-button').map((b) => b.attributes('text'));
/** What the Vastleggen menu offers in the shell's bar, as the screen last mounted set it. */
const vastleggen = (_w: Scherm) => menuactie.value?.opties.map((o) => o.tekst) ?? [];
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
  it('keeps every submission in Alle aanvragen, the newest first, with the day each was decided', async () => {
    const besloten = (id: string, op: string): Gram[] => [
      { ...aanvraag, id, recorded_on: op },
      { id: `b${id}`, kind: 'BESLUIT', basis: 'wet#1', refers_to: id, recorded_on: op, recording_actor: 'gemeente' },
    ];
    kroniek.value = [...besloten('t', '2026-10-01'), ...besloten('w', '2026-10-05'), aanvraag];
    try {
      const w = await toon([{ cel: 'gemeente', cel_naam: 'gemeente Voorbeeld', lezen: true, beslissen: true }], 'alle');
      expect(lijsten(w)).toContain('Alle aanvragen 3');
      // Te doen holds only what waits: decided submissions are found here.
      expect(lijsten(w)).toContain('Te doen 2');
      const rijen = rijenVan(w, 'Alle aanvragen');
      const regels = rijen.map((r) => r.find('nldd-text-cell').attributes('supporting-text'));
      expect(regels[0]).not.toContain('Besloten');
      expect(regels[1]).toContain('Besloten 5 oktober 2026');
      expect(regels[2]).toContain('Besloten 1 oktober 2026');
      // Each opens beside the list, as a row of Te doen does.
      await rijen[2]?.trigger('click');
      await flushPromises();
      expect(w.router.currentRoute.value.path).toBe('/backoffice/behandelen/gemeente/t');
      expect(w.router.currentRoute.value.query.lijst).toBe('alle');
    } finally {
      kroniek.value = [aanvraag];
    }
  });

  it('names a submission by what it asks for and the applicant, as its form has the login fill the name', async () => {
    vormen.indieningen['wet#1'] = { soort: 'AANVRAAG', basis: 'wet#1', besluit: 'wet#1', cel: 'gemeente', velden: [], systeem: { naam_aanvrager: { soort: 'inloggen', inlog: 'naam' } } };
    werk.value = [{ stuk: { ...aanvraag, fields: { naam_aanvrager: 'P. Voorbeeld' } } }];
    try {
      const w = await toon([{ cel: 'gemeente', cel_naam: 'gemeente Voorbeeld', lezen: true, beslissen: true }]);
      expect(rijenVan(w).map((r) => r.find('nldd-text-cell').attributes('text'))).toContain('Vergunning · P. Voorbeeld');
    } finally {
      delete vormen.indieningen['wet#1'];
      werk.value = [{ stuk: aanvraag }];
    }
  });

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
    expect(vastleggen(w)).not.toContain('Uitgebracht advies');
  });

  it('opens nothing for an address that names no submission of the cell, and asks for a choice', async () => {
    const recht = [{ cel: 'gemeente', cel_naam: 'gemeente Voorbeeld', lezen: true, beslissen: true }];
    const open = await toon(recht, 'alle', 'a');
    expect(open.findComponent({ name: 'BehandelenPaneel' }).props('id')).toBe('a');
    // Such as an address from before the test store was emptied.
    const weg = await toon(recht, 'alle', 'weg');
    expect(weg.findComponent({ name: 'BehandelenPaneel' }).exists()).toBe(false);
    expect(weg.find('[slot="main"]').attributes('empty')).toBe('true');
    expect(weg.findAll('nldd-inline-dialog').map((d) => d.attributes('text'))).toContain('Kies een aanvraag');
  });

  it('lets a reader open a submission to read it, and says a decider decides', async () => {
    const w = await toon([{ cel: 'gemeente', cel_naam: 'gemeente Voorbeeld', lezen: true, beslissen: false }]);
    expect(teOpenen(w)).toBe(1);
    expect(w.text()).toContain('Een beslisser neemt het besluit');
  });

  it('offers a decider the facts the executor records, a reader nothing, and nobody an external party\'s', async () => {
    const recht = { cel: 'gemeente', cel_naam: 'gemeente Voorbeeld', lezen: true };
    const w = await toon([{ ...recht, beslissen: true }]);
    expect(menuactie.value?.tekst).toBe('Vastleggen');
    expect(vastleggen(w)).toEqual(['Ontvangen betaling']);
    // The advisor's advice goes through the admin panel, not the work queue.
    expect(vastleggen(w)).not.toContain('Uitgebracht advies');
    // Choosing it opens the sheet of that fact.
    menuactie.value?.opties[0]?.kies();
    await flushPromises();
    expect(w.findComponent({ name: 'FeitVastleggen' }).props('keuze')?.handeling.feit).toBe('wet#9#betaling_ontvangen');
    // The sheet has an address on the screen it opened over, so a link to it opens it there.
    expect(w.router.currentRoute.value.hash).toBe('#vastleggen/gemeente/betaling_ontvangen');
    const opAanvraag = await toon([{ ...recht, beslissen: true }], 'te-doen', 'a');
    await opAanvraag.router.push({ path: '/backoffice/behandelen/gemeente/a', hash: '#vastleggen/gemeente/betaling_ontvangen' });
    await flushPromises();
    expect(opAanvraag.findComponent({ name: 'FeitVastleggen' }).props('keuze')?.handeling.feit).toBe('wet#9#betaling_ontvangen');
    // An address for a fact this screen does not offer opens nothing.
    await opAanvraag.router.push({ path: '/backoffice', hash: '#vastleggen/gemeente/advies_uitgebracht' });
    await flushPromises();
    expect(opAanvraag.findComponent({ name: 'FeitVastleggen' }).props('keuze')).toBeNull();
    opAanvraag.unmount();
    const lezer = await toon([{ ...recht, beslissen: false }]);
    expect(menuactie.value).toBeNull();
    expect(knoppen(lezer)).toEqual([]);
    lezer.unmount();
    expect(menuactie.value).toBeNull();
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
    // An act after a grant is never offered on its own.
    expect(vastleggen(decider)).not.toContain('Gedrukt vergunning');
    expect(vastleggen(decider)).not.toContain('Verzonden vergunning');
    // A reader sees how far it is, without the button.
    const lezer = await toon([{ ...recht, beslissen: false }], 'na-toekenning');
    const rij = lezer.find('nldd-list[accessible-label="Na toekenning"] nldd-list-item');
    expect(rij.find('nldd-text-cell').attributes('supporting-text')).toBe('Vergunning gedrukt op 14 oktober 2026');
    expect(rij.attributes('button')).toBeUndefined();
    expect(knoppen(lezer)).toEqual([]);
  });

  it('puts what waits on a step here in Te doen, what waited longest first', async () => {
    const w = await toon([{ cel: 'gemeente', cel_naam: 'gemeente Voorbeeld', lezen: true, beslissen: true }]);
    // What the list holds is behind the button beside its title.
    const uitleg = w.find('#lijst-titel nldd-icon-button[slot="end"] nldd-popover[slot="popup"]');
    expect(uitleg.text()).toBe('Alles waarvan de volgende stap hier ligt, wat het langst wacht bovenaan.');
    expect(w.findAll('nldd-inline-dialog').map((d) => d.attributes('text'))).toContain('Kies een aanvraag');
    // The grant waits since its last act, 14 October; the submission came in on the 13th.
    const rijen = rijenVan(w).map((r) => r.find('nldd-text-cell'));
    expect(rijen).toHaveLength(2);
    expect(rijen[0]?.attributes('supporting-text')).toContain('Ontvangen 13 oktober 2026');
    expect(rijen[1]?.attributes('text')).toBe('Vergunning voor A. de Vries');
    expect(lijsten(w)).toEqual(['Te doen 2', 'Na toekenning 1', 'Alle aanvragen 1']);
  });

  it('moves a submission that waits on the answer to a request out of Te doen', async () => {
    verzoeken.value = [verzoek];
    try {
      const w = await toon([{ cel: 'gemeente', cel_naam: 'gemeente Voorbeeld', lezen: true, beslissen: true }]);
      expect(lijsten(w)).toEqual(['Te doen 1', 'Na toekenning 1', 'Wacht op anderen 1', 'Alle aanvragen 1']);
      const wacht = await toon([{ cel: 'gemeente', cel_naam: 'gemeente Voorbeeld', lezen: true, beslissen: true }], 'wacht');
      const rij = rijenVan(wacht, 'Wacht op anderen');
      expect(rij).toHaveLength(1);
      expect(rij[0]?.find('nldd-text-cell').attributes('supporting-text')).toContain('Wacht op Adviseur Voorbeeld');
      // Once the answer is in, it is to do again.
      verzoeken.value = [{ ...verzoek, open: false }];
      expect(lijsten(await toon([{ cel: 'gemeente', cel_naam: 'gemeente Voorbeeld', lezen: true, beslissen: true }]))).toEqual([
        'Te doen 2',
        'Na toekenning 1',
        'Alle aanvragen 1',
      ]);
    } finally {
      verzoeken.value = [];
    }
  });

  it('opens the list of the open submission when the address names none', async () => {
    verzoeken.value = [verzoek];
    try {
      const w = await toon([{ cel: 'gemeente', cel_naam: 'gemeente Voorbeeld', lezen: true, beslissen: true }], undefined, 'a');
      expect(rijenVan(w, 'Wacht op anderen')).toHaveLength(1);
    } finally {
      verzoeken.value = [];
    }
  });

  it('closes the open submission when another list is chosen, and keeps it in its own', async () => {
    const recht = [{ cel: 'gemeente', cel_naam: 'gemeente Voorbeeld', lezen: true, beslissen: true }];
    const kies = async (w: Scherm, lijst: string) => {
      const rij = w.findAll('nldd-list[accessible-label="Lijsten"] nldd-list-item').find((r) => r.find('nldd-text-cell').attributes('text') === lijst);
      expect(rij).toBeDefined();
      await rij?.trigger('click');
      await flushPromises();
      return w.router.currentRoute.value;
    };
    const elders = await kies(await toon(recht, 'te-doen', 'a'), 'Na toekenning');
    expect(elders.path).toBe('/backoffice');
    expect(elders.query.lijst).toBe('na-toekenning');
    const zelf = await kies(await toon(recht, 'na-toekenning', 'a'), 'Te doen');
    expect(zelf.path).toBe('/backoffice/behandelen/gemeente/a');
    expect(zelf.query.lijst).toBe('te-doen');
  });

  it('says the work is done when Te doen is empty, and what still waits on someone else', async () => {
    const bewaard = vervolg.splice(0);
    verzoeken.value = [verzoek];
    try {
      const w = await toon([{ cel: 'gemeente', cel_naam: 'gemeente Voorbeeld', lezen: true, beslissen: true }]);
      const dialoog = w.find('nldd-inline-dialog');
      expect(dialoog.attributes('text')).toBe('U bent helemaal klaar');
      expect(dialoog.attributes('supporting-text')).toContain('Eén aanvraag wacht op een andere partij');
      // An empty state carries no icon.
      expect(dialoog.attributes('icon')).toBeUndefined();
      expect(dialoog.attributes('heading-level')).toBe('3');
      // No title and no description above an empty list: the dialog says what matters.
      expect(w.find('#lijst-titel').exists()).toBe(false);
      // Nothing to choose: no list column, and the main pane says it instead of asking for a choice.
      expect(w.find('[slot="secondary-sidebar"]').exists()).toBe(false);
      expect(dialoog.element.closest('[slot="main"]')).not.toBeNull();
      expect(w.findAll('nldd-inline-dialog').map((d) => d.attributes('text'))).not.toContain('Kies een aanvraag');
      // Stacked, the lists show until one is opened.
      expect(w.find('[slot="main"]').attributes('empty')).toBe('true');
      const open = await toon([{ cel: 'gemeente', cel_naam: 'gemeente Voorbeeld', lezen: true, beslissen: true }], 'te-doen');
      expect(open.find('[slot="main"]').attributes('empty')).toBeUndefined();
      expect(w.findAll('nldd-text').map((t) => t.text())).not.toContain('Alles waarvan de volgende stap hier ligt, wat het langst wacht bovenaan.');
    } finally {
      vervolg.push(...bewaard);
      verzoeken.value = [];
    }
  });
});
