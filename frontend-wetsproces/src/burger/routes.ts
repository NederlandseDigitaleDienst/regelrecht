// The routes of the public side. Every screen loads as a chunk of its own,
// apart from the back office, so this side can be bundled on its own. The
// sections of this side are the shell's tabs (`meta.tabs`, gedeeld/schil.ts).
// The dossier and a decision in it are one screen, as the editor's sidebar
// and main pane: the decision opens beside the dossier.
import { RouterView } from 'vue-router';
import type { RouteRecordRaw } from 'vue-router';

export const burgerRoutes: RouteRecordRaw[] = [
  {
    path: '/burger',
    component: RouterView,
    meta: {
      kant: 'burger',
      tabs: [
        { naam: 'dossier', tekst: 'Mijn dossier', pad: '/burger', icoon: 'document', ook: ['besluit'] },
        { naam: 'aanvraag', tekst: 'Aanvraag doen', pad: '/burger/aanvraag', icoon: 'add' },
      ],
    },
    children: [
      { path: '', name: 'dossier', component: () => import('./DossierView.vue') },
      { path: 'besluit/:id', name: 'besluit', component: () => import('./DossierView.vue'), props: true },
      { path: 'aanvraag', name: 'aanvraag', component: () => import('./AanvraagView.vue') },
    ],
  },
];
