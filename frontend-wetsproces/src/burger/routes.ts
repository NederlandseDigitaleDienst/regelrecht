// The routes of the public side. Every screen loads as a chunk of its own,
// apart from the back office, so this side can be bundled on its own later.
import type { RouteRecordRaw } from 'vue-router';

export const burgerRoutes: RouteRecordRaw[] = [
  {
    path: '/burger',
    component: () => import('./BurgerLayout.vue'),
    meta: { kant: 'burger' },
    children: [
      { path: '', name: 'dossier', component: () => import('./DossierView.vue') },
      { path: 'aanvraag', name: 'aanvraag', component: () => import('./AanvraagView.vue') },
      { path: 'besluit/:id', name: 'besluit', component: () => import('./BesluitView.vue'), props: true },
    ],
  },
];
