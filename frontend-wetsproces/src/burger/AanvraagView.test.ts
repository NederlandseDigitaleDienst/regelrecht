// After submitting, the sheet says the application is in and offers two ways
// on: open it in the dossier, or start another on an empty form.
import { flushPromises, mount } from '@vue/test-utils';
import { beforeEach, describe, expect, it, vi } from 'vitest';
import { createMemoryHistory, createRouter } from 'vue-router';
import type { Casus, Gram, Indieningsvorm } from '../gedeeld/types';
import { burgerRoutes } from './routes';

const casus: Casus = {
  id: 'voorbeeld',
  cel: 'gemeente',
  cel_naam: 'gemeente Voorbeeld',
  artikel: 'wet#1',
  besluit: 'wet#1',
  uitkomst: 'verleend',
  andere_cellen: [],
  indieningen: [{ artikel: 'wet#1', besluit: 'wet#1', cel: 'gemeente', cel_naam: 'gemeente Voorbeeld', soort: 'AANVRAAG' }],
  vandaag: '2026-10-13',
};
const vorm: Indieningsvorm = {
  soort: 'AANVRAAG',
  basis: 'wet#1',
  besluit: 'wet#1',
  cel: 'gemeente',
  velden: [{ naam: 'reden', type: 'string', mag_leeg: true, basis: 'wet#1' }],
  systeem: {},
  verborgen: [],
};
const nieuw: Gram = { id: 'n', kind: 'AANVRAAG', basis: 'wet#1', recorded_on: '2026-10-13', recording_actor: 'gemeente' };

const dossier = vi.fn<() => Gram[]>();
const indienen = vi.fn(async () => nieuw);
vi.mock('./api', () => ({
  burgerApi: {
    casus: async () => casus,
    formulier: async () => ({ indieningen: { 'wet#1': vorm }, besluiten: {}, teksten: {} }),
    ik: async () => null,
    dossier: async () => ({ grammen: dossier(), stand: { rollen: { n: 'indiening' }, herroepen: {}, antwoorden: { n: { soort: 'open' } } }, vervolg: [] }),
    tijdlijn: async () => null,
    indienen: (...a: unknown[]) => indienen(...(a as [])),
  },
}));

async function dienIn(pad = '/burger#nieuwe-aanvraag') {
  const router = createRouter({ history: createMemoryHistory(), routes: burgerRoutes });
  await router.push(pad);
  const w = mount({ template: '<RouterView />' }, { global: { plugins: [router], stubs: { teleport: true } } });
  await flushPromises();
  await w.find('form').trigger('submit');
  await flushPromises();
  return { w, router };
}
const knop = (w: Awaited<ReturnType<typeof dienIn>>['w'], tekst: string) =>
  w.findAll('nldd-inline-dialog nldd-button').find((b) => b.attributes('text') === tekst)!;

describe('AanvraagView', () => {
  beforeEach(() => {
    dossier.mockReset().mockReturnValue([]);
    indienen.mockClear();
  });

  it('says the application is in, and the dossier beside it loads it', async () => {
    const { w } = await dienIn();
    expect(indienen).toHaveBeenCalledTimes(1);
    const dialoog = w.find('nldd-inline-dialog[variant="success"]');
    expect(dialoog.attributes('text')).toBe('Aanvraag ingediend');
    // The day of submission, as recorded, is named here and not on the form before it.
    expect(dialoog.attributes('supporting-text')).toBe(
      'Ingediend bij gemeente Voorbeeld op 13 oktober 2026. Dat is de dagtekening van uw aanvraag. ' +
        'Het besluit verschijnt in uw dossier zodra een medewerker klaar is.',
    );
    expect(w.find('form').exists()).toBe(false);
    // Loaded when the dossier and the sheet opened, and again after submitting.
    expect(dossier.mock.calls.length).toBeGreaterThanOrEqual(3);
  });

  it('opens what was submitted in the dossier, which closes the sheet', async () => {
    const { w, router } = await dienIn();
    await knop(w, 'Bekijk aanvraag').trigger('click');
    await flushPromises();
    expect(router.currentRoute.value.path).toBe('/burger/dossier/n');
    expect(router.currentRoute.value.hash).toBe('');
  });

  it('starts another application on an empty form, under the case\'s own article', async () => {
    const { w, router } = await dienIn('/burger?artikel=wet%231&op=x#nieuwe-aanvraag');
    await knop(w, 'Doe nog een aanvraag').trigger('click');
    await flushPromises();
    expect(router.currentRoute.value.hash).toBe('#nieuwe-aanvraag');
    expect(router.currentRoute.value.query).toEqual({});
    expect(w.find('nldd-inline-dialog[variant="success"]').exists()).toBe(false);
    expect(w.find('form').exists()).toBe(true);
    expect(w.text()).not.toContain('Dagtekening');
  });
});
