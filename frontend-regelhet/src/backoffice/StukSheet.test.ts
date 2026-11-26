// One record of a chronicle in a sheet: for a decision of the executor it
// asks the server why it came out so, and says so when that does not load.
// The nldd-* elements are not defined in this test, so it reads the
// attributes Vue sets on them.
import { flushPromises, mount } from '@vue/test-utils';
import { describe, expect, it, vi } from 'vitest';
import { createMemoryHistory, createRouter } from 'vue-router';
import type { Casus, Gram } from '../gedeeld/types';
import type { Kroniek } from './api';
import StukSheet from './StukSheet.vue';

const waarom = vi.fn();
vi.mock('./api', () => ({ backofficeApi: { waarom: (id: string) => waarom(id) } }));

const casus: Casus = {
  id: 'voorbeeld',
  cel: 'gemeente',
  cel_naam: 'gemeente Voorbeeld',
  artikel: 'wet#1',
  besluit: 'wet#1',
  uitkomst: 'verleend',
  andere_cellen: [],
  indieningen: [],
  vandaag: '2026-12-01',
};
const aanvraag: Gram = { id: 'a', kind: 'AANVRAAG', basis: 'wet#1', recorded_on: '2026-11-15', recording_actor: 'gemeente' };
const besluit: Gram = { ...aanvraag, id: 'b', kind: 'BESCHIKKING', refers_to: 'a', outputs: { verleend: true } };
const kroniek: Kroniek = {
  cel: 'gemeente',
  cel_naam: 'gemeente Voorbeeld',
  kop: 2,
  grammen: [aanvraag, besluit],
  stand: { rollen: { a: 'indiening', b: 'besluit' }, herroepen: {}, antwoorden: {} },
};

function toon(stuk: Gram) {
  const router = createRouter({ history: createMemoryHistory(), routes: [{ path: '/', component: { template: '<div />' } }] });
  return mount(StukSheet, {
    props: { stuk, kroniek, casus, vormen: null },
    global: { stubs: { teleport: true }, plugins: [router] },
  });
}

describe('StukSheet', () => {
  it('titles the record in the page, with the bar taking it over, and lists its article and what it refers to', async () => {
    waarom.mockResolvedValueOnce({ ingevuld: [], antwoorden: [], bevindingen: [] });
    const w = toon(besluit);
    await flushPromises();
    const bar = w.find('nldd-top-title-bar');
    const titel = w.find(`nldd-title[id="${bar.attributes('collapse-anchor')}"]`);
    expect([titel.attributes('heading-level'), titel.attributes('text')]).toEqual(['2', bar.attributes('text')]);
    const rijen = w
      .findAll('nldd-list[accessible-label="Gegevens van het stuk"] nldd-list-item')
      .map((r) => r.findAll('nldd-text-cell').map((c) => c.attributes('text')));
    expect(rijen).toEqual([
      ['Op grond van', expect.any(String)],
      ['Verwijst naar', expect.stringMatching(/^Nr\. 1/)],
    ]);
    // Every heading under the title is an h3 of one size.
    const koppen = w.findAll('nldd-title').filter((t) => t.attributes('heading-level') !== '2');
    expect(new Set(koppen.map((t) => `${t.attributes('heading-level')} ${t.attributes('size')}`))).toEqual(new Set(['3 display-5']));
  });

  it('leads to the panel of the submission with a secondary button in a vertical group', () => {
    const w = toon(aanvraag);
    const groep = w.find('nldd-button-group');
    expect(groep.attributes('orientation')).toBe('vertical');
    expect(groep.find('nldd-button').attributes()).toMatchObject({ appearance: 'secondary', text: 'Open in Aanvragen' });
  });

  it('says when the chain of a decision does not load', async () => {
    waarom.mockRejectedValueOnce(new Error('De server antwoordt niet'));
    const w = toon(besluit);
    await flushPromises();
    expect(waarom).toHaveBeenCalledWith('b');
    const melding = w.find('nldd-inline-dialog[text="De keten van dit besluit laadt niet"]');
    expect(melding.attributes('supporting-text')).toContain('De server antwoordt niet');
  });

  it('asks no chain for a record that is no decision', async () => {
    waarom.mockClear();
    toon(aanvraag);
    await flushPromises();
    expect(waarom).not.toHaveBeenCalled();
  });
});
