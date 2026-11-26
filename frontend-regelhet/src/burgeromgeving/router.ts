// The routes of the citizen's entry: the screen to get in and the citizen's
// screens of src/burger/, nothing of the back office. What getting in means
// is the transport's (vervoer.ts).
import { createRouter } from 'vue-router';
import type { RouteLocationNormalized, RouteLocationRaw } from 'vue-router';
import { burgerRoutes } from '../burger/routes';
import { BURGER, terugNaar } from '../gedeeld/terug';
import type { Toegang, Vervoer } from './vervoer';

/**
 * Where `to` sends the citizen instead, given `toegang`. Someone who is not in
 * and asked for a screen deeper than the dossier, such as a decision from a
 * link, gets that screen back after getting in (`terug`, read by the screen
 * to get in with `terugNaar`).
 */
export function omleiding(to: RouteLocationNormalized, toegang: Toegang): RouteLocationRaw | undefined {
  if (to.path === '/') return toegang.binnen ? '/burger' : '/toegang';
  if (to.name !== 'toegang' && !toegang.binnen) {
    return terugNaar(to.fullPath, BURGER, '') ? { name: 'toegang', query: { terug: to.fullPath } } : '/toegang';
  }
  return undefined;
}

export function maakBurgerRouter(vervoer: Vervoer) {
  const router = createRouter({
    history: vervoer.history(),
    routes: [
      { path: '/', component: { render: () => null } },
      { path: '/toegang', name: 'toegang', component: vervoer.toegangScherm, meta: { paginanaam: 'Inloggen' } },
      ...burgerRoutes,
      { path: '/:pad(.*)*', redirect: '/' },
    ],
  });
  router.beforeEach(async (to) => omleiding(to, await vervoer.toegang()));
  return router;
}
