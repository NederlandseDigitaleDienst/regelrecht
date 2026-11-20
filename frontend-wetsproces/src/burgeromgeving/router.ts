// The routes of the citizen's entry: the screen to get in and the citizen's
// screens of src/burger/, nothing of the back office. What getting in means
// is the transport's (vervoer.ts).
import { createRouter } from 'vue-router';
import type { RouteLocationNormalized } from 'vue-router';
import { burgerRoutes } from '../burger/routes';
import type { Toegang, Vervoer } from './vervoer';

/** Where `to` sends the citizen instead, given `toegang`. */
export function omleiding(to: RouteLocationNormalized, toegang: Toegang): string | undefined {
  if (to.path === '/') return toegang.binnen ? '/burger' : '/toegang';
  if (to.name !== 'toegang' && !toegang.binnen) return '/toegang';
  return undefined;
}

export function maakBurgerRouter(vervoer: Vervoer) {
  const router = createRouter({
    history: vervoer.history(),
    routes: [
      { path: '/', component: { render: () => null } },
      { path: '/toegang', name: 'toegang', component: vervoer.toegangScherm },
      ...burgerRoutes,
      { path: '/:pad(.*)*', redirect: '/' },
    ],
  });
  router.beforeEach(async (to) => omleiding(to, await vervoer.toegang()));
  return router;
}
