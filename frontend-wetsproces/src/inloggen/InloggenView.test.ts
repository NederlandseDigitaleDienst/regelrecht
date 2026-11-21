// The login through Keycloak leads back to the screen of a link, but only to
// a screen of the build's own sides.
import { flushPromises, mount } from '@vue/test-utils';
import { describe, expect, it, vi } from 'vitest';
import { createMemoryHistory, createRouter } from 'vue-router';
import { BURGER } from '../gedeeld/terug';
import InloggenView from './InloggenView.vue';

const oidc = vi.fn<(terug: string) => void>();
vi.mock('./api', () => ({
  inloggenApi: {
    status: async () => ({ authenticated: false, oidc_configured: true }),
    testpersonen: async () => [],
    oidc: (terug: string) => oidc(terug),
  },
}));

async function klikInloggen(terug: string, naar?: readonly string[]) {
  oidc.mockReset();
  const router = createRouter({ history: createMemoryHistory(), routes: [{ path: '/:p(.*)*', component: { render: () => null } }] });
  await router.push({ path: '/inloggen', query: { terug } });
  const w = mount(InloggenView, { props: naar ? { naar } : {}, global: { plugins: [router] } });
  await flushPromises();
  await w.findAll('nldd-button').find((b) => b.attributes('text') === 'Inloggen')!.trigger('click');
  return oidc.mock.calls[0]?.[0];
}

describe('the login through Keycloak', () => {
  it("returns to a link's screen of either side in a build with both", async () => {
    expect(await klikInloggen('/backoffice/kroniek/c')).toBe('/backoffice/kroniek/c');
  });

  it("returns to the start for a screen the build does not serve", async () => {
    expect(await klikInloggen('/backoffice/kroniek/c', BURGER)).toBe('/');
    expect(await klikInloggen('//voorbeeld.org/x')).toBe('/');
  });
});
