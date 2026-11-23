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
    expect(stappen.map((s) => s.attributes('status'))).toEqual(['past', 'past', 'past', 'current', 'current']);
    expect(stappen.map((s) => s.attributes('position'))).toEqual(['first', 'between', 'between', 'between', 'last']);
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

  it('opens a record where the side has a screen for it, and marks the one on screen', async () => {
    const naar = (g: Gram) => (g.kind === 'AANVRAAG' ? undefined : { name: 'stuk', params: { id: g.id } });
    const { w, router } = await toon({ naar, huidig: 'b' });
    const rijen = w.findAll('nldd-list-item');
    expect(rijen[0]?.attributes('button')).toBeUndefined();
    expect(rijen[2]?.attributes('current')).toBe('true');
    await rijen[2]?.trigger('click');
    await flushPromises();
    expect(router.currentRoute.value.fullPath).toBe('/stuk/b');
  });

  it('says when nothing is open, and when it did not load', async () => {
    const { w } = await toon({ tijdlijn: { ...tijdlijn, volgende: [] } });
    expect(w.text()).toContain('Er staat nu niets open bij deze aanvraag.');
    const { w: fout } = await toon({ tijdlijn: null, fout: 'stuk' });
    expect(fout.find('nldd-inline-dialog').attributes('text')).toBe('De tijdlijn laadt niet');
  });
});
