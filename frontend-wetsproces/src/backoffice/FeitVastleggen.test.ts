// The sheet that records a fact. Opened on a request, it does not ask what
// the request already holds, shows it instead, and sends the request along.
import { flushPromises, mount } from '@vue/test-utils';
import { describe, expect, it, vi } from 'vitest';
import SpecFormulier from '../gedeeld/SpecFormulier.vue';
import type { Gram, Handeling } from '../gedeeld/types';
import FeitVastleggen from './FeitVastleggen.vue';

const handeling: Handeling = {
  feit: 'beleid#5#advies_uitgebracht',
  velden: [
    { naam: 'bsn_aanvrager', type: 'string' },
    { naam: 'datum_onderzoek', type: 'date' },
  ],
};

async function open(
  keuze: { cel: string; handeling: Handeling; verzoek?: string; vast?: Record<string, unknown> },
  openVerzoeken: { vast: Record<string, unknown> }[] = [],
) {
  const vastleggen = vi.fn(async () => ({}) as Gram);
  const w = mount(FeitVastleggen, { props: { keuze: null, vastleggen, open: openVerzoeken }, global: { stubs: { teleport: true } } });
  await w.setProps({ keuze });
  await flushPromises();
  return { w, vastleggen };
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
    // Shown read-only, from the request.
    expect(w.findAll('nldd-text-field[readonly]')).toHaveLength(1);
    expect(w.findAll('nldd-form-field').map((f) => f.attributes('label'))).toContain('Bsn aanvrager');
    expect(w.find('nldd-form-field-help-text').text()).toContain('Uit het verzoek');
    await w.find('nldd-button[text="Vastleggen"]').trigger('click');
    await flushPromises();
    expect(vastleggen).toHaveBeenCalledWith(
      expect.objectContaining({ cel: 'adviseur', feit: handeling.feit, verzoek: 'v1' }),
      expect.any(String),
    );
  });

  it('stops an answer without the request while one is open for the same person, in words', async () => {
    const { w, vastleggen } = await open({ cel: 'adviseur', handeling }, [{ vast: { bsn_aanvrager: '999990020' } }]);
    w.findComponent(SpecFormulier).vm.$emit('zet', 'bsn_aanvrager', '999990020');
    await w.find('nldd-button[text="Vastleggen"]').trigger('click');
    await flushPromises();
    expect(vastleggen).not.toHaveBeenCalled();
    expect(w.find('nldd-inline-dialog').attributes('supporting-text')).toContain('vanuit dat verzoek');
    // Another person has no request open: recorded.
    w.findComponent(SpecFormulier).vm.$emit('zet', 'bsn_aanvrager', '999990032');
    await w.find('nldd-button[text="Vastleggen"]').trigger('click');
    await flushPromises();
    expect(vastleggen).toHaveBeenCalledOnce();
  });

  it('stops it as well when the field the request holds is left empty, as the server would refuse it', async () => {
    const { w, vastleggen } = await open({ cel: 'adviseur', handeling }, [{ vast: { bsn_aanvrager: '999990020' } }]);
    await w.find('nldd-button[text="Vastleggen"]').trigger('click');
    await flushPromises();
    expect(vastleggen).not.toHaveBeenCalled();
    expect(w.find('nldd-inline-dialog').attributes('supporting-text')).toContain('vanuit dat verzoek');
  });
});
