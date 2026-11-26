// The sheet that records a fact. Opened on a request, it does not ask what
// the request already holds, shows it instead, and sends the request along.
import { flushPromises, mount } from '@vue/test-utils';
import { describe, expect, it, vi } from 'vitest';
import SpecFormulier from '../gedeeld/SpecFormulier.vue';
import type { Gram, Handeling } from '../gedeeld/types';
import { createMemoryHistory, createRouter } from 'vue-router';
import FeitVastleggen from './FeitVastleggen.vue';

const handeling: Handeling = {
  feit: 'beleid#5#advies_uitgebracht',
  velden: [
    { naam: 'bsn_aanvrager', type: 'string', basis: 'beleid#5' },
    { naam: 'datum_onderzoek', type: 'date', basis: 'beleid#5' },
  ],
};

async function open(
  keuze: { cel: string; handeling: Handeling; verzoek?: string; vast?: Record<string, unknown> },
  openVerzoeken: { vast: Record<string, unknown> }[] = [],
) {
  const vastleggen = vi.fn(async () => ({}) as Gram);
  const router = createRouter({ history: createMemoryHistory(), routes: [{ path: '/:p(.*)*', component: { render: () => null } }] });
  await router.push('/backoffice');
  const w = mount(FeitVastleggen, {
    props: { keuze: null, vastleggen, open: openVerzoeken },
    global: { plugins: [router], stubs: { teleport: true } },
  });
  await w.setProps({ keuze });
  await flushPromises();
  return { w, vastleggen, router };
}

describe('FeitVastleggen', () => {
  it('asks every field of the fact without a request', async () => {
    const { w } = await open({ cel: 'adviseur', handeling });
    const velden = w.findComponent(SpecFormulier).props('velden') as { veld: { naam: string } }[];
    expect(velden.map((v) => v.veld.naam)).toEqual(['bsn_aanvrager', 'datum_onderzoek']);
  });

  it('takes from the request what it holds, and sends the request along', async () => {
    const { w, vastleggen } = await open({ cel: 'adviseur', handeling, verzoek: 'v1', vast: { bsn_aanvrager: '999990020' } });
    // Every field goes to the form; the one the request holds is fixed there.
    const velden = w.findComponent(SpecFormulier).props('velden') as { veld: { naam: string }; systeem?: unknown }[];
    expect(velden.filter((v) => !v.systeem).map((v) => v.veld.naam)).toEqual(['datum_onderzoek']);
    // Shown as a row of a list, not as an input, with the request as its origin.
    const [rij, waarde] = w.findAll('nldd-list nldd-text-cell');
    expect(rij!.attributes('text')).toBe('Bsn aanvrager');
    expect(rij!.attributes('supporting-text')).toContain('Uit het verzoek');
    expect(waarde!.attributes('text')).toBe('999990020');
    await w.find('nldd-button[text="Leg vast"]').trigger('click');
    await flushPromises();
    expect(vastleggen).toHaveBeenCalledWith(
      expect.objectContaining({ cel: 'adviseur', feit: handeling.feit, verzoek: 'v1' }),
      expect.any(String),
    );
  });

  it('stops an answer without the request while one is open for the same person, in words', async () => {
    const { w, vastleggen } = await open({ cel: 'adviseur', handeling }, [{ vast: { bsn_aanvrager: '999990020' } }]);
    w.findComponent(SpecFormulier).vm.$emit('zet', 'bsn_aanvrager', '999990020');
    await w.find('nldd-button[text="Leg vast"]').trigger('click');
    await flushPromises();
    expect(vastleggen).not.toHaveBeenCalled();
    expect(w.find('nldd-inline-dialog').attributes('supporting-text')).toContain('vanuit dat verzoek');
    // Another person has no request open: recorded.
    w.findComponent(SpecFormulier).vm.$emit('zet', 'bsn_aanvrager', '999990032');
    await w.find('nldd-button[text="Leg vast"]').trigger('click');
    await flushPromises();
    expect(vastleggen).toHaveBeenCalledOnce();
  });

  it('stops it as well when the field the request holds is left empty, as the server would refuse it', async () => {
    const { w, vastleggen } = await open({ cel: 'adviseur', handeling }, [{ vast: { bsn_aanvrager: '999990020' } }]);
    await w.find('nldd-button[text="Leg vast"]').trigger('click');
    await flushPromises();
    expect(vastleggen).not.toHaveBeenCalled();
    expect(w.find('nldd-inline-dialog').attributes('supporting-text')).toContain('vanuit dat verzoek');
  });

  it('anchors the bar to a title in the page, and records with an imperative in a vertical group', async () => {
    const { w } = await open({ cel: 'adviseur', handeling });
    const titel = w.find('nldd-title[heading-level="2"]');
    expect(titel.attributes('text')).toBe(w.find('nldd-top-title-bar').attributes('text'));
    expect(w.find('nldd-top-title-bar').attributes('collapse-anchor')).toBe(titel.attributes('id'));
    expect(w.find('nldd-button-group').attributes('orientation')).toBe('vertical');
    expect(w.find('nldd-button-group nldd-button').attributes('text')).toBe('Leg vast');
    expect(w.findComponent(SpecFormulier).props('artikelen')).toBe(false);
  });

  it('asks the fields in one flat list, without the group headings of the applicant\'s form', async () => {
    const { w } = await open({ cel: 'adviseur', handeling });
    await w.setProps({ teksten: { bsn_aanvrager: { group: 'Uw persoonsgegevens' } } });
    const velden = w.findComponent(SpecFormulier).props('velden') as { groep: string }[];
    expect(velden.map((v) => v.groep)).toEqual(['', '']);
    expect(w.text()).not.toContain('Uw persoonsgegevens');
  });

  it('lists per article what the fact asks and who fills it, on a page of its own', async () => {
    const { w, router } = await open({ cel: 'adviseur', handeling, verzoek: 'v1', vast: { bsn_aanvrager: '999990020' } });
    const link = w.find('nldd-link a');
    // The page has an address of its own, beside the sheet's.
    expect(link.attributes('href')).toBe('#vastleggen-herkomst/adviseur/advies_uitgebracht/v1');
    await link.trigger('click');
    await flushPromises();
    expect(router.currentRoute.value.hash).toBe('#vastleggen-herkomst/adviseur/advies_uitgebracht/v1');
    const balk = w.find('nldd-top-title-bar');
    expect(balk.attributes('text')).toBe('Waar deze gegevens vandaan komen');
    expect(balk.attributes('back-text')).toBe('Advies uitgebracht vastleggen');
    expect(w.findAll('nldd-title[heading-level="3"]').map((t) => t.attributes('text'))).toHaveLength(1);
    expect(w.findAll('nldd-rich-text li').map((li) => li.text())).toEqual([
      expect.stringMatching(/: uit het verzoek\.$/),
      expect.stringMatching(/: u vult dit in\.$/),
    ]);
    await balk.trigger('back');
    await flushPromises();
    expect(router.currentRoute.value.hash).toBe('#vastleggen/adviseur/advies_uitgebracht/v1');
    expect(w.find('nldd-top-title-bar').attributes('text')).toBe('Advies uitgebracht vastleggen');
  });
});
