// The screen that logs in through Keycloak: it goes on to Keycloak at once,
// back to the screen of a link of the build's own sides, and stays only
// after a logout, after a failed login and for an account it does not admit.
import { ApiError } from '@regelrecht/frontend-shared';
import { flushPromises, mount } from '@vue/test-utils';
import { beforeEach, describe, expect, it, vi } from 'vitest';
import { createMemoryHistory, createRouter } from 'vue-router';
import { BURGER } from '../gedeeld/terug';
import type { Ik } from '../gedeeld/types';
import InloggenView from './InloggenView.vue';
import { markeerUitgelogd } from './uitgelogd';

const oidc = vi.fn<(terug: string) => void>();
const uitloggen = vi.fn();
const status = vi.fn(async () => ({ authenticated: false, oidc_configured: true }));
vi.mock('./api', () => ({
  inloggenApi: {
    status: () => status(),
    testpersonen: async () => [],
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
  ik.mockReset().mockRejectedValue(new ApiError('niet ingelogd', { status: 401 }));
  sessionStorage.clear();
});

async function open(query: Record<string, string>, naar?: readonly string[]) {
  const router = createRouter({ history: createMemoryHistory(), routes: [{ path: '/:p(.*)*', component: { render: () => null } }] });
  // In the app the router has asked already; here the screen asks itself.
  await router.push({ path: '/inloggen', query });
  const w = mount(InloggenView, { props: naar ? { naar } : {}, global: { plugins: [router] } });
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
