// The admin moves the test clock; the trial band above the screen shows the
// new day at once, without waiting for the next route.
import { flushPromises, mount } from '@vue/test-utils';
import { describe, expect, it, vi } from 'vitest';
import { huidigeKlok } from '../gedeeld/klok';
import KlokView from './KlokView.vue';

vi.mock('./api', () => ({
  backofficeApi: {
    klok: async () => ({ bestuurbaar: true, datum: '2026-10-13', modus: 'test' }),
    klokZetten: async (datum: string) => ({ bestuurbaar: true, datum, modus: 'test' }),
  },
}));

describe('the clock screen', () => {
  it('moves the clock of the trial band along with the one it sets', async () => {
    huidigeKlok.value = { bestuurbaar: true, datum: '2026-10-13', modus: 'test' };
    const w = mount(KlokView);
    await flushPromises();
    await w.find('nldd-date-field').trigger('change', { detail: { value: '2026-12-01' } });
    await w.findAll('nldd-button').find((b) => b.attributes('text') === 'Klok zetten')!.trigger('click');
    await flushPromises();
    expect(huidigeKlok.value?.datum).toBe('2026-12-01');
  });
});
