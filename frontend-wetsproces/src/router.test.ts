// @vitest-environment node
import { describe, expect, it } from 'vitest';
import type { RouteLocationNormalized, RouteRecordRaw } from 'vue-router';
import { createMemoryHistory, createRouter } from 'vue-router';
import { backofficeRoutes } from './backoffice/routes';
import { omleiding } from './router';
import type { Ik } from './gedeeld/types';

const naar = (path: string, kant?: 'burger' | 'backoffice', name?: string) =>
  ({ path, fullPath: path, name, matched: kant ? [{ meta: { kant } }] : [] }) as unknown as RouteLocationNormalized;

const geen: Ik = {
  sub: 's',
  naam: null,
  burger: false,
  identiteit: null,
  niveau: null,
  cellen: [],
  partijen_invoeren: false,
  klok_zetten: false,
};
const burger: Ik = { ...geen, burger: true, identiteit: '999990019' };
const medewerker: Ik = {
  ...geen,
  niveau: 'lezer',
  cellen: [{ cel: 'c', cel_naam: 'C', lezen: true, behandelen: false, beslissen: false }],
};

describe('omleiding', () => {
  it('opens each account on its own side, and nobody on the login', () => {
    expect([null, burger, medewerker].map((w) => omleiding(naar('/'), w))).toEqual(['/inloggen', '/burger', '/backoffice']);
  });

  it('keeps a citizen out of the back office and an employee out of the public side', () => {
    expect(omleiding(naar('/backoffice', 'backoffice'), burger)).toBe('/burger');
    expect(omleiding(naar('/burger', 'burger'), medewerker)).toBe('/backoffice');
    expect(omleiding(naar('/burger', 'burger'), null)).toBe('/inloggen');
    // A deeper screen without a login comes back after it.
    expect(omleiding(naar('/backoffice/kroniek/c', 'backoffice', 'kroniek'), null)).toEqual({
      name: 'inloggen',
      query: { terug: '/backoffice/kroniek/c' },
    });
    expect(omleiding(naar('/burger/aanvraag', 'burger'), burger)).toBeUndefined();
  });

  it('opens the simulation only for whoever may set the clock', () => {
    expect(omleiding(naar('/backoffice/simulatie', 'backoffice', 'simulatie'), medewerker)).toBe('/backoffice');
    expect(omleiding(naar('/backoffice/simulatie', 'backoffice', 'simulatie'), { ...medewerker, klok_zetten: true })).toBeUndefined();
  });

  it('keeps someone without the right out also at the old address of the clock', async () => {
    // The real routes with empty screens: the redirect and the guard are what count here.
    const leeg = (routes: RouteRecordRaw[]): RouteRecordRaw[] =>
      routes.map((r) => ({ ...r, ...('component' in r && r.component ? { component: { render: () => null } } : {}), ...(r.children ? { children: leeg(r.children) } : {}) }) as RouteRecordRaw);
    const router = createRouter({ history: createMemoryHistory(), routes: leeg(backofficeRoutes) });
    router.beforeEach((to) => omleiding(to, medewerker));
    await router.push('/backoffice/klok');
    expect(router.currentRoute.value.path).toBe('/backoffice');
  });

  it('opens the admin panel only for whoever enters what an external party did', () => {
    const beheer = { path: '/backoffice/beheer', name: 'beheer', matched: [{ meta: { kant: 'backoffice' } }, { meta: { beheer: true } }] };
    const route = beheer as unknown as RouteLocationNormalized;
    expect(omleiding(route, { ...medewerker, niveau: 'beslisser' })).toBe('/backoffice');
    expect(omleiding(route, { ...medewerker, niveau: 'admin', partijen_invoeren: true })).toBeUndefined();
  });

  it('always lets the login open', () => {
    expect(omleiding(naar('/inloggen', undefined, 'inloggen'), burger)).toBeUndefined();
  });
});
