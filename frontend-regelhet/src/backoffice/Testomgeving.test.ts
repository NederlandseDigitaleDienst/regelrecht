// The tools of the test setup: the clock in a sheet, emptying in a dialog,
// each opened by its hash over whatever screen is open. The trial band above
// shows the new day at once.
import { flushPromises, mount } from '@vue/test-utils';
import { describe, expect, it, vi } from 'vitest';
import { createMemoryHistory, createRouter } from 'vue-router';
import { huidigeKlok } from '../gedeeld/klok';
import Testomgeving from './Testomgeving.vue';

const leegmaken = vi.fn(async () => ({ bestuurbaar: true, datum: '2026-10-13', modus: 'test' }));

vi.mock('./api', () => ({
  backofficeApi: {
    klok: async () => ({ bestuurbaar: true, datum: '2026-10-13', modus: 'test' }),
    klokZetten: async (datum: string) => ({ bestuurbaar: true, datum, modus: 'test' }),
    leegmaken: () => leegmaken(),
  },
}));

async function toon(hash: string) {
  const router = createRouter({ history: createMemoryHistory(), routes: [{ path: '/:p(.*)*', component: { render: () => null } }] });
  await router.push({ path: '/backoffice/kroniek', hash });
  const w = mount(Testomgeving, { global: { plugins: [router], stubs: { teleport: true } } });
  await flushPromises();
  return Object.assign(w, { router });
}

const knop = (w: Awaited<ReturnType<typeof toon>>, tekst: string) => w.findAll('nldd-button').find((b) => b.attributes('text') === tekst)!;

describe('the tools of the test setup', () => {
  it('sets the clock in a sheet, and moves the clock of the trial band along', async () => {
    huidigeKlok.value = { bestuurbaar: true, datum: '2026-10-13', modus: 'test' };
    const w = await toon('#klok');
    expect(w.find('nldd-sheet').attributes('open')).toBeDefined();
    expect(w.find('nldd-modal-dialog').attributes('open')).toBeUndefined();
    await w.find('nldd-date-field').trigger('change', { detail: { value: '2026-12-01' } });
    await knop(w, 'Zet klok').trigger('click');
    await flushPromises();
    expect(huidigeKlok.value?.datum).toBe('2026-12-01');
    // Closing it leaves the screen it was over.
    w.find('nldd-sheet').trigger('close');
    await flushPromises();
    expect(w.router.currentRoute.value.fullPath).toBe('/backoffice/kroniek');
  });

  it('empties the test setup only after the choice in the dialog, and then starts over at the work queue', async () => {
    leegmaken.mockClear();
    huidigeKlok.value = { bestuurbaar: true, datum: '2026-12-01', modus: 'test' };
    const w = await toon('#testomgeving-legen');
    expect(w.find('nldd-sheet').attributes('open')).toBeUndefined();
    const dialoog = w.find('nldd-modal-dialog');
    expect(dialoog.attributes('open')).toBeDefined();
    expect(dialoog.attributes('supporting-text')).toContain('niet terug te draaien');
    // Keeping the data empties nothing.
    await knop(w, 'Behoud gegevens').trigger('click');
    await flushPromises();
    expect(leegmaken).not.toHaveBeenCalled();
    expect(w.router.currentRoute.value.hash).toBe('');
    await w.router.push({ path: '/backoffice/kroniek', hash: '#testomgeving-legen' });
    await flushPromises();
    await knop(w, 'Leeg testomgeving').trigger('click');
    await flushPromises();
    expect(leegmaken).toHaveBeenCalledTimes(1);
    expect(huidigeKlok.value?.datum).toBe('2026-10-13');
    expect(w.router.currentRoute.value.fullPath).toBe('/backoffice');
  });
});
