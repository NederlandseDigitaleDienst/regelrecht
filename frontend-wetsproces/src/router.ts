// The routes of the app: the login, the public side and the back office.
// Which side an account sees follows from what it may do (`GET /api/ik`);
// the server checks every call itself.
import { createRouter, createWebHistory } from 'vue-router';
import type { RouteLocationNormalized } from 'vue-router';
import { backofficeRoutes } from './backoffice/routes';
import { burgerRoutes } from './burger/routes';
import { isMedewerker, laadIk, startpagina } from './gedeeld/ik';
import type { Ik } from './gedeeld/types';

/** Where `to` sends `wie` instead, or nothing when `wie` may open it. */
export function omleiding(to: RouteLocationNormalized, wie: Ik | null): string | undefined {
  if (to.path === '/') return startpagina(wie);
  if (to.name === 'inloggen') return undefined;
  const kant = to.matched.find((r) => r.meta.kant)?.meta.kant;
  if (kant === 'burger' && !wie?.burger) return startpagina(wie);
  if (kant === 'backoffice' && !isMedewerker(wie)) return startpagina(wie);
  return undefined;
}

export function maakRouter() {
  const router = createRouter({
    history: createWebHistory(),
    routes: [
      { path: '/', component: { render: () => null } },
      { path: '/inloggen', name: 'inloggen', component: () => import('./inloggen/InloggenView.vue') },
      ...burgerRoutes,
      ...backofficeRoutes,
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
  }
}
