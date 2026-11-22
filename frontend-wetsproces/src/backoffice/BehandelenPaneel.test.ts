// A submission whose decision a later record revoked: with nothing in its
// place the decider decides again, after that record; with a new decision in
// its place that one holds and nothing waits. What answers the submission
// comes from the server's `stand`.
import { flushPromises, mount } from '@vue/test-utils';
import { describe, expect, it, vi } from 'vitest';
import type { Casus, Gram, Ik, Stand } from '../gedeeld/types';
import type { Beslissing, Kroniek } from './api';
import BehandelenPaneel from './BehandelenPaneel.vue';

const beslissen = vi.fn(async (b: Beslissing) => ({
  besluit: { ...gram('e', 'BESCHIKKING', 'awb#7:11', b.op), after: b.na, outputs: { verleend: true } },
  waarom: { ingevuld: [], antwoorden: [], bevindingen: [] },
}));
vi.mock('./api', () => ({
  backofficeApi: {
    beslissen: (b: Beslissing) => beslissen(b),
    voorvertoning: async () => ({ stil: [] }),
    waarom: async () => ({ ingevuld: [], antwoorden: [], bevindingen: [] }),
  },
  beheerApi: {},
}));

const casus: Casus = {
  id: 'voorbeeld',
  cel: 'gemeente',
  cel_naam: 'gemeente Voorbeeld',
  artikel: 'wet#1',
  besluit: 'wet#1',
  uitkomst: 'verleend',
  andere_cellen: [{ cel: 'rechtbank', cel_naam: 'Rechtbank Voorbeeld' }],
  indieningen: [],
  vandaag: '2026-12-01',
};
function gram(id: string, kind: string, basis: string, refers_to?: string): Gram {
  return { id, kind, basis, refers_to, recorded_on: '2026-11-15', recording_actor: 'gemeente' };
}
const bezwaar = gram('c', 'BEZWAAR', 'awb#7:11', 'b');
const opBezwaar = { ...gram('d', 'BESCHIKKING', 'awb#7:11', 'c'), outputs: { bestreden_besluit_herroepen: false } };
const ontvangst = { ...gram('r', 'EXECUTOGRAM', 'awb#8:79', 'd'), fact: 'afschrift_van_de_uitspraak', recorded_on: '2026-11-20' };
const rollen: Stand['rollen'] = { c: 'indiening', d: 'besluit', r: 'vastlegging' };
const beslisser: Ik = {
  sub: 'x',
  naam: null,
  burger: false,
  identiteit: null,
  niveau: 'beslisser',
  cellen: [{ cel: 'gemeente', cel_naam: 'gemeente Voorbeeld', lezen: true, behandelen: true, beslissen: true }],
  partijen_invoeren: false,
  klok_zetten: false,
};

function toon(stand: Stand, grammen: Gram[]) {
  const kroniek: Kroniek = { cel: 'gemeente', cel_naam: 'gemeente Voorbeeld', kop: grammen.length, grammen, stand };
  return mount(BehandelenPaneel, {
    props: { cel: 'gemeente', id: 'c', casus, vormen: null, wie: beslisser, kronieken: [kroniek] },
  });
}

describe('a submission whose decision was revoked', () => {
  it('waits for nothing when a decision revoked it without one in its place, such as a withdrawal', async () => {
    const intrekking = { ...gram('i', 'BESCHIKKING', 'wet#3', 'd'), recorded_on: '2026-11-20' };
    const w = toon(
      { rollen: { ...rollen, i: 'besluit' }, herroepen: { d: 'i' }, antwoorden: { c: { soort: 'vervallen', besluit: 'd', door: 'i' } } },
      [bezwaar, opBezwaar, intrekking],
    );
    await flushPromises();
    expect(w.find('nldd-badge').attributes('text')).toBe('Herroepen');
    expect(w.findAll('nldd-button').find((b) => b.attributes('text') === 'Besluit nemen')).toBeUndefined();
    expect(w.findAll('nldd-inline-dialog').map((d) => d.attributes('text'))).not.toContain('Een beslisser van deze cel neemt het besluit');
  });

  it('is decided again after the receipt that annulled its decision', async () => {
    const w = toon(
      { rollen, herroepen: { d: 'r' }, antwoorden: { c: { soort: 'vervallen', besluit: 'd', door: 'r', na: 'r' } } },
      [bezwaar, opBezwaar, ontvangst],
    );
    await flushPromises();
    // The earlier decision, marked as annulled, and the form to decide again.
    expect(w.find('nldd-badge').attributes('text')).toBe('Vernietigd');
    const banners = w.findAll('nldd-banner').map((b) => b.attributes('text'));
    expect(banners).toContain('Het besluit van 15 november 2026 geldt niet meer');
    expect(banners).toContain('Opnieuw beslissen');
    const knop = w.findAll('nldd-button').find((b) => b.attributes('text') === 'Besluit nemen');
    expect(knop).toBeDefined();
    await knop?.trigger('click');
    await flushPromises();
    expect(beslissen).toHaveBeenCalledWith(expect.objectContaining({ op: 'c', na: 'r' }));
    expect(w.findAll('nldd-banner').map((b) => b.attributes('text'))).toContain('Het besluit is genomen');
  });

  it('shows the new decision taken in its place as the one that holds, and offers no form', async () => {
    const opnieuw = { ...gram('e', 'BESCHIKKING', 'awb#7:11', 'c'), recorded_on: '2026-12-01' };
    const aanvraag = gram('a', 'AANVRAAG', 'wet#1');
    const geweigerd = { ...gram('b', 'BESCHIKKING', 'wet#1', 'a'), outputs: { verleend: false } };
    const w = mount(BehandelenPaneel, {
      props: {
        cel: 'gemeente',
        id: 'a',
        casus,
        vormen: null,
        wie: beslisser,
        kronieken: [
          {
            cel: 'gemeente',
            cel_naam: 'gemeente Voorbeeld',
            kop: 3,
            grammen: [aanvraag, geweigerd, opnieuw],
            stand: {
              rollen: { a: 'indiening', b: 'besluit', e: 'besluit' },
              herroepen: { b: 'e' },
              antwoorden: {
                a: {
                  soort: 'in_de_plaats',
                  besluit: 'b',
                  door: 'e',
                  uitvoer: 'nieuw_besluit',
                  nieuw: { outputs: { verleend: true }, recorded_on: '2026-12-01' },
                },
              },
            },
          },
        ],
      },
    });
    await flushPromises();
    const titels = w.findAll('nldd-title').map((t) => t.attributes('text'));
    expect(titels).toContain('Het eerdere besluit');
    expect(titels).toContain('Het geldende besluit');
    expect(w.find('nldd-badge').attributes('text')).toBe('Herroepen');
    expect(w.findAll('nldd-tag').map((t) => t.attributes('text'))).toContain('Toegewezen');
    expect(w.findAll('nldd-button').find((b) => b.attributes('text') === 'Besluit nemen')).toBeUndefined();
  });
});
