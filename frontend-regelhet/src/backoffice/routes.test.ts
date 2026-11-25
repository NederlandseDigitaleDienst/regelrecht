// @vitest-environment node
import { describe, expect, it } from 'vitest';
import { createMemoryHistory, createRouter } from 'vue-router';
import { gereedschapVan } from '../gedeeld/schil';
import { backofficeRoutes } from './routes';

describe('the routes of the back office', () => {
  it('has no section for the clock, and opens its sheet from the addresses it had', async () => {
    const router = createRouter({ history: createMemoryHistory(), routes: backofficeRoutes });
    expect(backofficeRoutes[0]!.meta!.tabs!.map((t) => t.naam)).not.toContain('simulatie');
    for (const pad of ['/backoffice/simulatie', '/backoffice/klok']) {
      await router.push(pad);
      expect(router.currentRoute.value.fullPath).toBe('/backoffice#klok');
    }
  });

  it('offers the tools of the test setup in the account menu only to who may set the clock', async () => {
    const router = createRouter({ history: createMemoryHistory(), routes: backofficeRoutes });
    await router.push('/backoffice/kroniek');
    const route = router.currentRoute.value;
    expect(gereedschapVan(route, ['uitvoerder'])).toEqual([]);
    expect(gereedschapVan(route, ['uitvoerder', 'klok_zetten']).map((g) => [g.tekst, g.hash])).toEqual([
      ['Klok instellen', '#klok'],
      ['Testomgeving legen', '#testomgeving-legen'],
    ]);
  });
});
