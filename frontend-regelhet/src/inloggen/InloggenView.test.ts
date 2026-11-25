// The screen that logs in through Keycloak: it goes on to Keycloak at once,
// back to the screen of a link of the build's own sides, and stays only
// after a logout, after a failed login and for an account it does not admit.
import { ApiError } from '@regelrecht/frontend-shared';
import { flushPromises, mount } from '@vue/test-utils';
import { beforeEach, describe, expect, it, vi } from 'vitest';
import { createMemoryHistory, createRouter } from 'vue-router';
import { BURGER } from '../gedeeld/terug';
import type { Ik } from '../gedeeld/types';
import type { Persona } from './api';
import InloggenView from './InloggenView.vue';
import { markeerUitgelogd } from './uitgelogd';

const oidc = vi.fn<(terug: string) => void>();
const uitloggen = vi.fn();
const status = vi.fn(async () => ({ authenticated: false, oidc_configured: true }));
const testpersonen = vi.fn<() => Promise<Persona[]>>(async () => []);
const testlogin = vi.fn();
vi.mock('./api', () => ({
  inloggenApi: {
    status: () => status(),
    testpersonen: () => testpersonen(),
    testlogin: () => testlogin(),
    oidc: (terug: string) => oidc(terug),
    uitloggen: () => uitloggen(),
  },
}));
// `GET /api/ik`: the account, or the error the server answers with. The real
// `laadIk` turns that into who is in and why not, as the app does.
const ik = vi.fn<() => Promise<Ik>>(async () => {
  throw new ApiError('niet ingelogd', { status: 401 });
});
vi.mock('../gedeeld/api', () => ({ gedeeldApi: { ik: () => ik() } }));

beforeEach(() => {
  oidc.mockReset();
  uitloggen.mockReset();
  status.mockReset().mockResolvedValue({ authenticated: false, oidc_configured: true });
  testpersonen.mockReset().mockResolvedValue([]);
  testlogin.mockReset();
  ik.mockReset().mockRejectedValue(new ApiError('niet ingelogd', { status: 401 }));
  sessionStorage.clear();
});

async function open(query: Record<string, string>, naar?: readonly string[]) {
  const router = createRouter({ history: createMemoryHistory(), routes: [{ path: '/:p(.*)*', component: { render: () => null } }] });
  // In the app the router has asked already; here the screen asks itself.
  await router.push({ path: '/inloggen', query });
  const w = mount(InloggenView, { props: naar ? { naar } : {}, global: { plugins: [router], stubs: { teleport: true } } });
  await flushPromises();
  return { w, router };
}

const dialoog = (w: Awaited<ReturnType<typeof open>>['w']) => w.find('nldd-inline-dialog').attributes('text');

describe('the login through Keycloak', () => {
  it("goes on at once, back through this screen to a link's screen of either side in a build with both", async () => {
    await open({ terug: '/backoffice/kroniek/c' });
    expect(oidc).toHaveBeenCalledWith('/inloggen?terug=/backoffice/kroniek/c&na=keycloak');
  });

  it('goes back to the start for a screen the build does not serve', async () => {
    await open({ terug: '/backoffice/kroniek/c' }, BURGER);
    expect(oidc).toHaveBeenLastCalledWith('/inloggen?na=keycloak');
    await open({ terug: '//voorbeeld.org/x' });
    expect(oidc).toHaveBeenLastCalledWith('/inloggen?na=keycloak');
  });

  it('does not go round again when Keycloak sent the browser back without a session', async () => {
    const { w } = await open({ terug: '/backoffice/kroniek/c', na: 'keycloak' });
    expect(oidc).not.toHaveBeenCalled();
    expect(dialoog(w)).toBe('Inloggen is niet gelukt');
  });

  it('says so after a logout instead of logging straight back in, once', async () => {
    markeerUitgelogd();
    const { w } = await open({});
    expect(oidc).not.toHaveBeenCalled();
    expect(dialoog(w)).toBe('U bent uitgelogd');
    await w.findAll('nldd-button').find((b) => b.attributes('text') === 'Opnieuw inloggen')!.trigger('click');
    expect(oidc).toHaveBeenCalledOnce();
    // The next visit goes on as usual.
    oidc.mockReset();
    await open({});
    expect(oidc).toHaveBeenCalledOnce();
  });

  it('says a login failed instead of trying again by itself', async () => {
    const { w } = await open({ fout: 'inloggen' });
    expect(oidc).not.toHaveBeenCalled();
    expect(dialoog(w)).toBe('Inloggen is niet gelukt');
  });

  it('tells an account without a role of this server so (403) and offers another account, no loop', async () => {
    status.mockResolvedValue({ authenticated: true, oidc_configured: true });
    ik.mockRejectedValue(new ApiError('geen rol', { status: 403 }));
    const { w } = await open({ terug: '/backoffice/kroniek/c' });
    expect(oidc).not.toHaveBeenCalled();
    expect(dialoog(w)).toBe('Geen toegang');
    await w.findAll('nldd-button').find((b) => b.attributes('text') === 'Met een ander account inloggen')!.trigger('click');
    expect(uitloggen).toHaveBeenCalledOnce();
  });

  it('says the same when the callback found the missing role (?fout=rol)', async () => {
    const { w } = await open({ fout: 'rol' });
    expect(oidc).not.toHaveBeenCalled();
    expect(dialoog(w)).toBe('Geen toegang');
  });

  it('tells a server that fails apart from a missing role, and offers to try again instead of logging out', async () => {
    status.mockResolvedValue({ authenticated: true, oidc_configured: true });
    ik.mockRejectedValue(new ApiError('kapot', { status: 500 }));
    const { w } = await open({});
    expect(oidc).not.toHaveBeenCalled();
    expect(dialoog(w)).toBe('De server is niet bereikbaar');
    const knoppen = w.findAll('nldd-button').map((b) => b.attributes('text'));
    expect(knoppen).toContain('Opnieuw proberen');
    expect(knoppen).not.toContain('Met een ander account inloggen');
  });

  it('lets an account it admits in, to the screen of the link', async () => {
    status.mockResolvedValue({ authenticated: true, oidc_configured: true });
    ik.mockResolvedValue({ sub: 'm', burger: false, cellen: [{ cel: 'c' }], klok_zetten: false } as unknown as Ik);
    const { router } = await open({ terug: '/backoffice/kroniek/c', na: 'keycloak' });
    expect(oidc).not.toHaveBeenCalled();
    expect(router.currentRoute.value.fullPath).toBe('/backoffice/kroniek/c');
  });

  it('sends nobody anywhere when the server does not answer', async () => {
    status.mockRejectedValue(new Error('De server is niet bereikbaar.'));
    const { w } = await open({});
    expect(oidc).not.toHaveBeenCalled();
    expect(dialoog(w)).toBe('Inloggen lukt niet');
  });
});

describe('the test login', () => {
  it('shows a failed login in a dialog over the accounts, and stays', async () => {
    status.mockResolvedValue({ authenticated: false, oidc_configured: false });
    testpersonen.mockResolvedValue([{ soort: 'beheerder', sub: 'b', rollen: [] }]);
    testlogin.mockRejectedValue(new ApiError('De server weigert dit', { status: 422 }));
    const { w, router } = await open({});
    expect(w.find('nldd-modal-dialog').attributes('open')).toBeUndefined();
    await w.find('nldd-list-item').trigger('click');
    await flushPromises();
    const melding = w.find('nldd-modal-dialog');
    expect(melding.attributes('open')).toBeDefined();
    expect(melding.attributes('text')).toBe('Inloggen lukt niet');
    expect(melding.attributes('supporting-text')).toBe('De server weigert dit');
    expect(w.find('nldd-list[accessible-label="Testaccounts"]').exists()).toBe(true);
    expect(router.currentRoute.value.path).toBe('/inloggen');
    await melding.find('nldd-button').trigger('click');
    expect(w.find('nldd-modal-dialog').attributes('open')).toBeUndefined();
  });

  it("puts an avatar before every account, falling back on an admin's icon for an admin or decider and a user's for the others", async () => {
    status.mockResolvedValue({ authenticated: false, oidc_configured: false });
    testpersonen.mockResolvedValue([
      { soort: 'burger', sub: 'a', identiteit: '999990019', rollen: [] },
      { soort: 'medewerker', sub: 'm', niveau: 'medewerker', rollen: [] },
      { soort: 'medewerker', sub: 'd', niveau: 'beslisser', rollen: [] },
      { soort: 'beheerder', sub: 'b', rollen: [] },
    ]);
    const { w } = await open({});
    const avatars = w.findAll('nldd-list-item').map((r) => r.find('nldd-cell nldd-avatar'));
    expect(avatars.map((a) => a.attributes('icon'))).toEqual(['user', 'user', 'user-admin', 'user-admin']);
    // Without a made-up name there are no initials, so the icon shows; the name beside it says who it is.
    expect(avatars.map((a) => a.attributes('name'))).toEqual([undefined, undefined, undefined, undefined]);
    expect(avatars.every((a) => a.attributes('decorative') !== undefined)).toBe(true);
  });

  it('names every account by its made-up name, with its role in a tag after it', async () => {
    status.mockResolvedValue({ authenticated: false, oidc_configured: false });
    testpersonen.mockResolvedValue([
      { soort: 'burger', sub: 'a', identiteit: '999990019', naam: 'Iemand Voorbeeld', rollen: [] },
      { soort: 'medewerker', sub: 'd', niveau: 'beslisser', naam: 'Carla Fictief', cel_naam: 'gemeente Voorbeeld', rollen: [] },
      { soort: 'beheerder', sub: 'b', naam: 'Daan Verzonnen', rollen: [] },
    ]);
    const { w } = await open({});
    const rijen = w.findAll('nldd-list-item');
    // The tag stands right after the name, in the cell's own paragraph, each role in a colour of its own.
    expect(rijen.map((r) => r.find('nldd-text-cell p').element.firstChild?.textContent?.trim())).toEqual(['Iemand Voorbeeld, BSN 999990019', 'Carla Fictief', 'Daan Verzonnen']);
    const tags = rijen.map((r) => r.find('nldd-text-cell p nldd-tag'));
    expect(tags.map((t) => t.attributes('text'))).toEqual(['Burger', 'Beslisser', 'Beheerder']);
    expect(new Set(tags.map((t) => t.attributes('color'))).size).toBe(3);
    // A made-up name gives the avatar its initials.
    expect(rijen.map((r) => r.find('nldd-avatar').attributes('name'))).toEqual(['Iemand Voorbeeld', 'Carla Fictief', 'Daan Verzonnen']);
    // The employee's cell stands in the line under the name.
    expect(rijen[1]!.find('nldd-text-cell').attributes('supporting-text')).toContain('voor gemeente Voorbeeld');
    // A business that logs in on its KvK number is no citizen.
    testpersonen.mockResolvedValue([{ soort: 'burger', sub: 'k', identiteit: '12345678', label: 'KvK-nummer', naam: 'Voorbeeld B.V.', rollen: [] }]);
    const bedrijf = (await open({})).w;
    expect(bedrijf.find('nldd-tag').attributes('text')).toBe('Bedrijf');
  });
});
