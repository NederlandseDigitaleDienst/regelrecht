// @vitest-environment node
import { describe, expect, it, vi } from 'vitest';
import type { RouteLocationNormalized, RouteRecordRaw } from 'vue-router';
import { createMemoryHistory, createRouter } from 'vue-router';
import { backofficeRoutes } from './backoffice/routes';
import { herstelNaChunkfout, isChunkfout, omleiding } from './router';
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
  cellen: [{ cel: 'c', cel_naam: 'C', lezen: true, beslissen: false }],
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

  it('opens the server of an external party on its own screen, and nowhere else', () => {
    const arts: Ik = { ...geen, niveau: 'beslisser', partijen_invoeren: true, partij: { cel: 'g', cel_naam: 'G' } };
    expect(omleiding(naar('/'), arts)).toBe('/backoffice');
    for (const [pad, naam] of [['/backoffice', 'werkvoorraad'], ['/backoffice/kroniek', 'kroniek'], ['/backoffice/beheer', 'beheer']] as const) {
      expect(omleiding(naar(pad, 'backoffice', naam), arts)).toBe('/backoffice/verzoeken');
    }
    expect(omleiding(naar('/backoffice/verzoeken', 'backoffice', 'partij'), arts)).toBeUndefined();
    // The clock is for its admin only; without it the party's screen again.
    expect(omleiding(naar('/backoffice/simulatie', 'backoffice', 'simulatie'), arts)).toBe('/backoffice');
    expect(omleiding(naar('/backoffice/simulatie', 'backoffice', 'simulatie'), { ...arts, klok_zetten: true })).toBeUndefined();
    // A reader of the party enters nothing, so has no screen there.
    expect(omleiding(naar('/backoffice', 'backoffice', 'werkvoorraad'), { ...arts, niveau: 'lezer', partijen_invoeren: false })).toBe('/inloggen');
    // The executor's employees never get the party's screen.
    expect(omleiding(naar('/backoffice/verzoeken', 'backoffice', 'partij'), medewerker)).toBe('/backoffice');
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

describe('herstelNaChunkfout', () => {
  const opslag = () => {
    const m = new Map<string, string>();
    return { getItem: (k: string) => m.get(k) ?? null, setItem: (k: string, v: string) => void m.set(k, v) };
  };

  it('knows the words each browser gives a script that is gone', () => {
    expect(isChunkfout(new TypeError('Failed to fetch dynamically imported module: /assets/a.js'))).toBe(true);
    expect(isChunkfout(new TypeError('Importing a module script failed.'))).toBe(true);
    expect(isChunkfout(new TypeError('error loading dynamically imported module'))).toBe(true);
    expect(isChunkfout(new Error('Failed to fetch dynamically imported module'))).toBe(false);
    expect(isChunkfout(new TypeError('x is undefined'))).toBe(false);
  });

  it('loads the page anew once when a script is gone, and not again when it is still gone', () => {
    const o = opslag();
    const laad = vi.fn();
    const fout = new TypeError('Importing a module script failed.');
    expect(herstelNaChunkfout(fout, '/burger', o, laad)).toBe(true);
    expect(laad).toHaveBeenCalledWith('/burger');
    expect(herstelNaChunkfout(fout, '/burger', o, laad)).toBe(false);
    expect(laad).toHaveBeenCalledTimes(1);
    // Another error is no reason to reload.
    expect(herstelNaChunkfout(new Error('anders'), '/inloggen', o, laad)).toBe(false);
  });

  it('does not reload without storage, where it cannot tell the second miss', () => {
    const laad = vi.fn();
    const zonder = { getItem: () => { throw new Error('geen opslag'); }, setItem: () => undefined };
    expect(herstelNaChunkfout(new TypeError('Importing a module script failed.'), '/burger', zonder, laad)).toBe(false);
    expect(laad).not.toHaveBeenCalled();
  });
});
