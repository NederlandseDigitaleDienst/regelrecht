// A submission whose decision a later record revoked: with nothing in its
// place the decider decides again, after that record; with a new decision in
// its place that one holds and nothing waits. What answers the submission
// comes from the server's `stand`.
import { flushPromises, mount } from '@vue/test-utils';
import { describe, expect, it, vi } from 'vitest';
import type { Casus, Gram, Ik, Stand, Voorvertoning } from '../gedeeld/types';
import type { Beslissing, Kroniek } from './api';
import BehandelenPaneel from './BehandelenPaneel.vue';

const beslissen = vi.fn(async (b: Beslissing) => ({
  besluit: { ...gram('e', 'BESCHIKKING', 'awb#7:11', b.op), after: b.na, outputs: { verleend: true } },
  waarom: { ingevuld: [], antwoorden: [], bevindingen: [] },
}));
const voorvertoning = vi.fn(async (_b: Beslissing): Promise<Voorvertoning> => ({ stil: [] }));
vi.mock('./api', () => ({
  backofficeApi: {
    beslissen: (b: Beslissing) => beslissen(b),
    voorvertoning: (b: Beslissing) => voorvertoning(b),
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

describe('the decision as it would be taken', () => {
  const wacht: Stand = { rollen, herroepen: {}, antwoorden: { c: { soort: 'open' } } };
  const keten = [
    {
      soort: 'artikel',
      wet: 'awb',
      artikel: '7:11',
      stappen: [{ soort: 'uitkomst', naam: 'verleend', waarde: true, ontbreekt: [], stappen: [] }],
    },
  ];

  it('shows without a click, as a preview, with its outcome, why and the full chain beside the button', async () => {
    voorvertoning.mockClear().mockResolvedValueOnce({
      stil: [],
      voorlopig: {
        besluit: { ...gram('p', 'BESCHIKKING', 'awb#7:11', 'c'), outputs: { verleend: true } },
        waarom: { ingevuld: [], antwoorden: [], bevindingen: [], keten },
      },
    });
    const w = toon(wacht, [bezwaar]);
    await flushPromises();
    expect(voorvertoning).toHaveBeenCalledWith(expect.objectContaining({ op: 'c' }));
    const titels = w.findAll('nldd-title').map((t) => t.attributes('text'));
    expect(titels).toContain('Voorvertoning van het besluit');
    expect(w.findAll('nldd-tag').map((t) => t.attributes('text'))).toContain('Zou toegewezen worden');
    const meer = w.find('nldd-list[accessible-label="Volledige keten van het besluit"]');
    expect(meer.exists()).toBe(true);
    // Folded away until asked for.
    expect(meer.find('nldd-list-item').attributes('expanded')).toBe('false');
    expect(w.findAll('nldd-button').map((b) => b.attributes('text'))).toEqual(['Besluit nemen']);
  });

  it('says what is missing instead of an outcome when no decision can be computed yet', async () => {
    // As kern::voorvertoning words it: a policy leaves the finding to the authority.
    voorvertoning.mockClear().mockResolvedValueOnce({
      stil: [],
      geweigerd:
        "the policy leaves 'langdurige_loopbeperking_volgens_college' to the authority for this case: Required open term " +
        "'langdurige_loopbeperking_volgens_college' on regeling#3 is decided per case by the authority, but no judgement was given",
    });
    const w = toon(wacht, [bezwaar]);
    await flushPromises();
    const dialoog = w.findAll('nldd-inline-dialog').find((d) => d.attributes('text') === 'Er is nog geen besluit te berekenen');
    expect(dialoog?.attributes('supporting-text')?.toLowerCase()).toContain('"langdurige loopbeperking volgens college"');
    expect(dialoog?.attributes('supporting-text')).toContain('Geef die bevinding.');
    expect(w.findAll('nldd-title').map((t) => t.attributes('text'))).not.toContain('Voorvertoning van het besluit');
  });

  it('asks again when the chronicle changes, such as when an advice comes in', async () => {
    voorvertoning.mockClear();
    const w = toon(wacht, [bezwaar]);
    await flushPromises();
    expect(voorvertoning).toHaveBeenCalledTimes(1);
    const advies = { ...gram('a', 'EXECUTOGRAM', 'beleid#5', undefined), fact: 'advies_ontvangen' };
    const kroniek: Kroniek = { cel: 'gemeente', cel_naam: 'gemeente Voorbeeld', kop: 2, grammen: [bezwaar, advies], stand: wacht };
    await w.setProps({ kronieken: [kroniek] });
    await new Promise((r) => setTimeout(r, 350));
    await flushPromises();
    expect(voorvertoning).toHaveBeenCalledTimes(2);
  });
});
