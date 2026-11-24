// The timeline of an application: its records in the server's order, each
// opening its own screen where the side has one, and under them the next
// steps with who is to act, in the words of the records, the case and the
// UI spec.
import { flushPromises, mount } from '@vue/test-utils';
import { describe, expect, it } from 'vitest';
import { createMemoryHistory, createRouter } from 'vue-router';
import { artikelNaam } from './keten';
import TijdlijnLijst from './TijdlijnLijst.vue';
import type { Casus, Gram, Tijdlijn } from './types';

const casus: Casus = {
  id: 'voorbeeld',
  cel: 'gemeente',
  cel_naam: 'gemeente Voorbeeld',
  artikel: 'wet#1',
  besluit: 'wet#1',
  uitkomst: 'verleend',
  andere_cellen: [{ cel: 'rechtbank', cel_naam: 'Rechtbank Voorbeeld' }],
  indieningen: [],
  na_toekenning: {
    stand: '',
    handelingen: [{ feit: 'beleid#5#kaart_ontvangen', knop: 'Ontvangst vastleggen', stand: '' }],
  },
  vandaag: '2026-10-20',
};
function gram(id: string, kind: string, extra: Partial<Gram> = {}): Gram {
  return { id, kind, basis: 'wet#1', recorded_on: '2026-10-13', recording_actor: 'gemeente', ...extra };
}
const aanvraag = gram('a', 'AANVRAAG');
const verzoek = gram('v', 'EXECUTOGRAM', { basis: 'beleid#9', fact: 'onderzoek_aangevraagd' });
const besluit = gram('b', 'BESCHIKKING', { refers_to: 'a', outputs: { verleend: true, termijn_einddatum: '2026-12-01' } });
const tijdlijn: Tijdlijn = {
  stuk: 'a',
  grammen: [aanvraag, verzoek, besluit],
  stand: {
    rollen: { a: 'indiening', v: 'vastlegging', b: 'besluit' },
    herroepen: {},
    antwoorden: { a: { soort: 'besloten', besluit: 'b' } },
  },
  door_het_systeem: { v: 'beleid#9' },
  volgende: [
    { soort: 'handeling', wie: 'uitvoerder', cel: 'gemeente', op: 'b', feit: 'beleid#5#kaart_ontvangen' },
    { soort: 'termijn', wie: 'indiener', op: 'b', veld: 'termijn_einddatum', tot: '2026-12-01' },
  ],
};
const metGgd: Casus = { ...casus, andere_cellen: [...casus.andere_cellen, { cel: 'ggd', cel_naam: 'GGD Voorbeeld' }] };
const teksten = { termijn_einddatum: { label: 'Laatste dag voor bezwaar of beroep' } };

async function toon(props: Partial<InstanceType<typeof TijdlijnLijst>['$props']> = {}) {
  const router = createRouter({
    history: createMemoryHistory(),
    routes: [
      { path: '/', component: { template: '<div />' } },
      { path: '/stuk/:id', name: 'stuk', component: { template: '<div />' } },
    ],
  });
  await router.push('/');
  const w = mount(TijdlijnLijst, {
    props: { tijdlijn, casus, kant: 'burger', teksten, ...props },
    global: { plugins: [router] },
  });
  await flushPromises();
  return { w, router };
}

const cellen = (w: Awaited<ReturnType<typeof toon>>['w']) =>
  w.findAll('nldd-text-cell').map((c) => [c.attributes('overline') ?? '', c.attributes('text'), c.attributes('supporting-text')]);

describe('the timeline of an application', () => {
  it('shows the records in order, then the next steps, on one track', async () => {
    const { w } = await toon();
    expect(cellen(w)).toEqual([
      ['', 'Aanvraag', '13 oktober 2026 · gemeente Voorbeeld'],
      ['', 'Onderzoek aangevraagd', `13 oktober 2026 · gemeente Voorbeeld · Automatisch vastgelegd volgens ${artikelNaam('beleid#9')}`],
      ['', 'Besluit (beschikking)', '13 oktober 2026 · gemeente Voorbeeld'],
      ['Volgende stap', 'Kaart ontvangen', 'Aan zet: gemeente Voorbeeld'],
      ['Volgende stap', 'Laatste dag voor bezwaar of beroep: 1 december 2026', 'Aan zet: u · Bij besluit (beschikking) van 13 oktober 2026'],
    ]);
    const stappen = w.findAll('nldd-step-cell');
    expect(stappen.map((s) => s.attributes('status'))).toEqual(['past', 'past', 'past', 'current', 'future']);
    expect(stappen.map((s) => s.attributes('position'))).toEqual(['first', 'between', 'between', 'between', 'last']);
    // What is done carries a check mark, in a lane wide enough for it.
    expect(stappen.map((s) => s.attributes('icon') ?? '')).toEqual(['check-mark', 'check-mark', 'check-mark', '', '']);
    expect(new Set(stappen.map((s) => s.attributes('size')))).toEqual(new Set(['md']));
    expect(w.findAll('nldd-tag').map((t) => t.attributes('text'))).toEqual(['Toegewezen']);
  });

  it('names who acts as the employee reads it, and the act as the case words its button', async () => {
    const { w } = await toon({ kant: 'medewerker' });
    const volgende = cellen(w).filter(([o]) => o === 'Volgende stap');
    expect(volgende).toEqual([
      ['Volgende stap', 'Ontvangst vastleggen', 'Aan zet: gemeente Voorbeeld'],
      ['Volgende stap', 'Laatste dag voor bezwaar of beroep: 1 december 2026', 'Aan zet: de indiener · Bij besluit (beschikking) van 13 oktober 2026'],
    ]);
  });

  it('opens a record where the side has a screen for it, but not the one on screen', async () => {
    const naar = (g: Gram) => (g.kind === 'AANVRAAG' ? undefined : { name: 'stuk', params: { id: g.id } });
    const { w, router } = await toon({ naar, huidig: 'b' });
    const rijen = w.findAll('nldd-list-item');
    expect(rijen[0]?.attributes('button')).toBeUndefined();
    // The decision is on screen: its row neither opens nor stands out.
    expect(rijen[2]?.attributes('button')).toBeUndefined();
    expect(rijen.some((r) => r.attributes('current'))).toBe(false);
    await rijen[1]?.trigger('click');
    await flushPromises();
    expect(router.currentRoute.value.fullPath).toBe('/stuk/v');
  });

  it('puts the stages of a request of the policy under it, the receipts as minor steps, and opens what it adds in a sheet', async () => {
    const ontvangen = gram('o', 'EXECUTOGRAM', { basis: 'beleid#9', fact: 'verzoek_ontvangen', recording_actor: 'ggd', fields: { aanvraag: 'a' } });
    const antwoord = gram('n', 'EXECUTOGRAM', {
      basis: 'beleid#5',
      fact: 'advies_uitgebracht',
      recording_actor: 'ggd',
      recorded_on: '2026-10-15',
      fields: { aanvraag: 'a', loopbeperking: true },
    });
    const metVerzoek: Tijdlijn = {
      ...tijdlijn,
      grammen: [aanvraag, { ...verzoek, fields: { aanvraag: 'a' } }, ontvangen, antwoord, besluit],
      stand: { ...tijdlijn.stand, rollen: { ...tijdlijn.stand.rollen, o: 'vastlegging', n: 'vastlegging' } },
      volgende: [],
    };
    const verzoeken = [{ verzoek: 'v', ontvangen: 'o', antwoord: 'n', antwoord_ontvangen: null }];
    const { w } = await toon({ tijdlijn: metVerzoek, casus: metGgd, verzoeken, naar: (g: Gram) => ({ name: 'stuk', params: { id: g.id } }) });
    expect(cellen(w)).toEqual([
      ['', 'Aanvraag', '13 oktober 2026 · gemeente Voorbeeld'],
      ['', 'Onderzoek aangevraagd', '13 oktober 2026 · gemeente Voorbeeld aan GGD Voorbeeld'],
      ['', 'Verzoek ontvangen', '13 oktober 2026 · GGD Voorbeeld'],
      ['', 'Advies uitgebracht', '15 oktober 2026 · GGD Voorbeeld'],
      ['', 'Besluit (beschikking)', '13 oktober 2026 · gemeente Voorbeeld'],
    ]);
    // The answer is a step of its own; the receipt stands under it as a minor one.
    expect(w.findAll('nldd-step-cell').map((c) => [c.attributes('level'), c.attributes('icon') ?? ''])).toEqual([
      ['major', 'check-mark'],
      ['major', 'check-mark'],
      ['minor', ''],
      ['major', 'check-mark'],
      ['major', 'check-mark'],
    ]);
    // A stage is no record of its own to open; the request is no link either,
    // but has a button that opens its sheet.
    const rijen = w.findAll('nldd-list-item');
    expect(rijen.map((r) => r.attributes('button') !== undefined)).toEqual([true, false, false, false, true]);
    const info = w.findAll('nldd-icon-button');
    expect(info.map((b) => [b.attributes('icon'), b.attributes('size'), b.attributes('text')])).toEqual([['info', 'sm', 'Over dit verzoek']]);
    const sheet = () => document.body.querySelector('nldd-sheet')!;
    expect(sheet().hasAttribute('open')).toBe(false);
    await info[0]!.trigger('click');
    expect(sheet().getAttribute('open')).toBe('true');
    expect(sheet().querySelector('nldd-top-title-bar')?.getAttribute('text')).toBe('Onderzoek aangevraagd');
    expect(sheet().textContent).toContain(
      `Een verzoek van gemeente Voorbeeld aan GGD Voorbeeld, volgens ${artikelNaam('beleid#9')}. Het systeem legde het automatisch vast.`,
    );
    // Of the answer only what the request does not carry.
    const antwoordRijen = [...sheet().querySelectorAll('nldd-list[accessible-label="Antwoord op het verzoek"] nldd-list-item')];
    expect(antwoordRijen.map((r) => [...r.querySelectorAll('nldd-text-cell')].map((c) => c.getAttribute('text')))).toEqual([['Loopbeperking', 'Ja']]);
    w.unmount();
  });

  it('puts the step that waits on an answer under the request, as the step that is current', async () => {
    const open: Tijdlijn = {
      ...tijdlijn,
      grammen: [aanvraag, verzoek],
      volgende: [
        { soort: 'besluit', wie: 'uitvoerder', cel: 'gemeente', op: 'a' },
        { soort: 'antwoord', wie: 'externe_partij', cel: 'ggd', op: 'v', feit: 'beleid#5#advies_uitgebracht', taak: 'Advies uitbrengen' },
      ],
    };
    const verzoeken = [{ verzoek: 'v', ontvangen: null, antwoord: null, antwoord_ontvangen: null }];
    const { w } = await toon({ tijdlijn: open, casus: metGgd, verzoeken });
    expect(cellen(w)).toEqual([
      ['', 'Aanvraag', '13 oktober 2026 · gemeente Voorbeeld'],
      ['', 'Onderzoek aangevraagd', '13 oktober 2026 · gemeente Voorbeeld aan GGD Voorbeeld'],
      // The step goes by the task the case names, not by the fact it records.
      ['Volgende stap', 'Advies uitbrengen', 'Aan zet: GGD Voorbeeld'],
      ['Volgende stap', 'Beslissen op aanvraag van 13 oktober 2026', 'Aan zet: gemeente Voorbeeld'],
    ]);
    const stappen = w.findAll('nldd-step-cell');
    expect(stappen.map((c) => [c.attributes('level'), c.attributes('status')])).toEqual([
      ['major', 'past'],
      ['major', 'past'],
      ['major', 'current'],
      ['major', 'future'],
    ]);
    w.unmount();
  });

  it('keeps every record as its own row where no requests are given, as the back office shows them', async () => {
    const { w } = await toon();
    expect(w.findAll('nldd-list-item[button]')).toHaveLength(0);
    expect(document.body.querySelector('nldd-sheet')).toBeNull();
    w.unmount();
  });

  it('says when nothing is open, and when it did not load', async () => {
    const { w } = await toon({ tijdlijn: { ...tijdlijn, volgende: [] } });
    expect(w.text()).toContain('Er staat nu niets open bij deze aanvraag.');
    const { w: fout } = await toon({ tijdlijn: null, fout: 'stuk' });
    expect(fout.find('nldd-inline-dialog').attributes('text')).toBe('De tijdlijn laadt niet');
  });
});
