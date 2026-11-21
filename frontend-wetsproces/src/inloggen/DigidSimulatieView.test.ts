// The simulated DigiD of the citizens' build: pick a test citizen or type a
// name and a number, and the screen logs in by number and name only. The
// server checks the number; the screen shows its reason when it refuses.
import { flushPromises, mount } from '@vue/test-utils';
import { beforeEach, describe, expect, it, vi } from 'vitest';
import { createMemoryHistory, createRouter } from 'vue-router';
import { ApiError } from '@regelrecht/frontend-shared';
import DigidSimulatieView from './DigidSimulatieView.vue';

/** Records each login; the answer comes from `antwoord`, outside the spy. */
const digid = vi.fn<(bsn: string, naam: string) => void>();
let antwoord: () => Promise<Response> = async () => new Response('{}');
vi.mock('./api', () => ({
  inloggenApi: {
    status: async () => ({ authenticated: false, oidc_configured: false }),
    testpersonen: async () => [
      { soort: 'burger', sub: 'burger-999990032', rollen: ['wetsproces-burger'], identiteit: '999990032', naam: 'Anna Voorbeeld' },
      { soort: 'burger', sub: 'burger-999990020', rollen: ['wetsproces-burger'], identiteit: '999990020', naam: 'Bram Proef' },
    ],
    digid: (bsn: string, naam: string) => {
      digid(bsn, naam);
      return antwoord();
    },
    testlogin: vi.fn(),
  },
}));

async function toon() {
  const leeg = { render: () => null };
  const router = createRouter({
    history: createMemoryHistory(),
    routes: [
      { path: '/toegang', component: DigidSimulatieView },
      { path: '/burger', component: leeg },
    ],
  });
  await router.push('/toegang');
  const w = mount({ template: '<RouterView />' }, { global: { plugins: [router] } });
  await flushPromises();
  return { w, router };
}

const knop = (w: Awaited<ReturnType<typeof toon>>['w'], tekst: string) =>
  w.findAll('nldd-button').find((b) => b.attributes('text') === tekst)!;
const veld = (w: Awaited<ReturnType<typeof toon>>['w'], label: string) =>
  w.findAll('nldd-form-field').find((f) => f.attributes('label') === label)!.find('nldd-text-field');

beforeEach(() => {
  digid.mockReset();
  antwoord = async () => new Response('{}');
});

describe('the simulated DigiD', () => {
  it('says it is a simulation, not DigiD', async () => {
    const { w } = await toon();
    expect(w.find('nldd-top-title-bar').attributes('text')).toBe('Gesimuleerde DigiD');
    expect(w.find('nldd-banner').attributes('text')).toBe('Dit is een simulatie, geen echte DigiD');
  });

  it('logs in as a test citizen picked from the list, by number and name', async () => {
    const { w, router } = await toon();
    const opties = w.findAll('option').map((o) => o.text());
    expect(opties).toEqual(['Anna Voorbeeld, BSN 999990032', 'Bram Proef, BSN 999990020']);
    await w.find('select').setValue('999990020');
    await knop(w, 'Inloggen als deze testpersoon').trigger('click');
    await flushPromises();
    expect(digid).toHaveBeenCalledWith('999990020', 'Bram Proef');
    expect(router.currentRoute.value.path).toBe('/burger');
  });

  it('logs in with a name and a number typed in', async () => {
    const { w, router } = await toon();
    await veld(w, 'Naam').trigger('input', { detail: { value: 'Chris Testpersoon' } });
    await veld(w, 'BSN').trigger('input', { detail: { value: '999990044' } });
    await knop(w, 'Inloggen').trigger('click');
    await flushPromises();
    expect(digid).toHaveBeenCalledWith('999990044', 'Chris Testpersoon');
    expect(router.currentRoute.value.path).toBe('/burger');
  });

  it("shows the server's reason for a number it refuses, and stays", async () => {
    antwoord = async () => {
      throw new ApiError('x', { status: 400, body: JSON.stringify({ fout: 'Dit BSN haalt de elfproef niet. Kijk de cijfers na.' }) });
    };
    const { w, router } = await toon();
    await veld(w, 'BSN').trigger('input', { detail: { value: '999990033' } });
    await knop(w, 'Inloggen').trigger('click');
    await flushPromises();
    expect(w.find('nldd-inline-dialog').attributes('supporting-text')).toBe('Dit BSN haalt de elfproef niet. Kijk de cijfers na.');
    expect(router.currentRoute.value.path).toBe('/toegang');
  });
});
