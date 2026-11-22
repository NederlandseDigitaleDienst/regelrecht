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

async function open(keuze: { cel: string; handeling: Handeling; verzoek?: string; vast?: Record<string, unknown> }) {
  const vastleggen = vi.fn(async () => ({}) as Gram);
  const w = mount(FeitVastleggen, { props: { keuze: null, vastleggen }, global: { stubs: { teleport: true } } });
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
    const velden = w.findComponent(SpecFormulier).props('velden') as { veld: { naam: string } }[];
    expect(velden.map((v) => v.veld.naam)).toEqual(['datum_onderzoek']);
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
});
