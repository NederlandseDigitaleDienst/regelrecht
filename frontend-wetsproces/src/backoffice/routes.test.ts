// @vitest-environment node
import { describe, expect, it } from 'vitest';
import { createMemoryHistory, createRouter } from 'vue-router';
import { backofficeRoutes } from './routes';

describe('the routes of the back office', () => {
  it('names the simulation tab and still takes the address of the clock', async () => {
    const router = createRouter({ history: createMemoryHistory(), routes: backofficeRoutes });
    const tab = backofficeRoutes[0]!.meta!.tabs!.find((t) => t.naam === 'simulatie');
    expect(tab).toMatchObject({ tekst: 'Simulatie', pad: '/backoffice/simulatie', recht: 'klok_zetten' });
    await router.push('/backoffice/klok');
    expect(router.currentRoute.value.name).toBe('simulatie');
  });
});
