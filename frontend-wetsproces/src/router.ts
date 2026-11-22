// The routes of a web build with the login: the login and the sides the
// build carries, the public side and the back office in the combined build
// (main.ts), the back office alone in its own (backofficeomgeving/). This
// module imports neither side, so a build holds only the sides it passes.
// Which side an account sees follows from what it may do (`GET /api/ik`);
// the server checks every call itself.
import { createRouter, createWebHistory } from 'vue-router';
import type { RouteLocationNormalized, RouteLocationRaw, RouteRecordRaw } from 'vue-router';
import { isMedewerker, laadIk, startpagina } from './gedeeld/ik';
import { ALLE_KANTEN, terugNaar } from './gedeeld/terug';
import type { Ik } from './gedeeld/types';

/**
 * Where `to` sends `wie` instead, or nothing when `wie` may open it. Someone
 * not logged in who asked for a screen deeper than a side's start, such as
 * a decision from a link, gets it back after the login (`terug`, read by
 * the login screen).
 */
export function omleiding(to: RouteLocationNormalized, wie: Ik | null): RouteLocationRaw | undefined {
  if (to.path === '/') return startpagina(wie);
  if (to.name === 'inloggen') return undefined;
  if (!wie && terugNaar(to.fullPath, ALLE_KANTEN, '')) return { name: 'inloggen', query: { terug: to.fullPath } };
  const kant = to.matched.find((r) => r.meta.kant)?.meta.kant;
  if (kant === 'burger' && !wie?.burger) return startpagina(wie);
  if (kant === 'backoffice' && !isMedewerker(wie)) return startpagina(wie);
  if (to.name === 'simulatie' && !wie?.klok_zetten) return '/backoffice';
  if (to.matched.some((r) => r.meta.beheer) && !wie?.partijen_invoeren) return '/backoffice';
  return undefined;
}

/** The router of a web build with the sides `kanten`. */
export function maakRouter(kanten: RouteRecordRaw[]) {
  const router = createRouter({
    history: createWebHistory(),
    routes: [
      { path: '/', component: { render: () => null } },
      { path: '/inloggen', name: 'inloggen', component: () => import('./inloggen/InloggenView.vue') },
      ...kanten,
      { path: '/:pad(.*)*', redirect: '/' },
    ],
  });
  router.beforeEach(async (to) => omleiding(to, await laadIk()));
  return router;
}

declare module 'vue-router' {
  interface RouteMeta {
    /** The side a route belongs to. */
    kant?: 'burger' | 'backoffice';
    /** A screen of the admin panel, for whoever enters what an external party did. */
    beheer?: boolean;
  }
}
