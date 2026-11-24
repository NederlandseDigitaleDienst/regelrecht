// The admin moves the test clock; the trial band above the screen shows the
// new day at once, without waiting for the next route.
import { flushPromises, mount } from '@vue/test-utils';
import { describe, expect, it, vi } from 'vitest';
import { huidigeKlok } from '../gedeeld/klok';
import SimulatieView from './SimulatieView.vue';

const leegmaken = vi.fn(async () => ({ bestuurbaar: true, datum: '2026-10-13', modus: 'test' }));

vi.mock('./api', () => ({
  backofficeApi: {
    klok: async () => ({ bestuurbaar: true, datum: '2026-10-13', modus: 'test' }),
    klokZetten: async (datum: string) => ({ bestuurbaar: true, datum, modus: 'test' }),
    leegmaken: () => leegmaken(),
  },
}));

describe('the simulation screen', () => {
  it('moves the clock of the trial band along with the one it sets', async () => {
    huidigeKlok.value = { bestuurbaar: true, datum: '2026-10-13', modus: 'test' };
    const w = mount(SimulatieView);
    await flushPromises();
    await w.find('nldd-date-field').trigger('change', { detail: { value: '2026-12-01' } });
    await w.findAll('nldd-button').find((b) => b.attributes('text') === 'Klok zetten')!.trigger('click');
    await flushPromises();
    expect(huidigeKlok.value?.datum).toBe('2026-12-01');
  });

  it('empties the test setup only after the confirmation, and shows the clock back on the start', async () => {
    leegmaken.mockClear();
    huidigeKlok.value = { bestuurbaar: true, datum: '2026-12-01', modus: 'test' };
    const w = mount(SimulatieView);
    await flushPromises();
    const knop = (tekst: string) => w.findAll('nldd-button').find((b) => b.attributes('text') === tekst)!;
    await knop('Testomgeving leegmaken').trigger('click');
    const dialoog = w.find('nldd-modal-dialog');
    expect(dialoog.attributes('open')).toBeDefined();
    expect(dialoog.attributes('supporting-text')).toContain('niet terug te draaien');
    // Cancelling empties nothing.
    await knop('Annuleren').trigger('click');
    expect(leegmaken).not.toHaveBeenCalled();
    await knop('Testomgeving leegmaken').trigger('click');
    await knop('Leegmaken').trigger('click');
    await flushPromises();
    expect(leegmaken).toHaveBeenCalledTimes(1);
    expect(huidigeKlok.value?.datum).toBe('2026-10-13');
    expect(w.find('nldd-banner').attributes('text')).toContain('De testomgeving is leeg');
  });
});
