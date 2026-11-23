// The routes of the public side. Every screen loads as a chunk of its own,
// apart from the back office, so this side can be bundled on its own. This
// side has no sections, so its bar carries its name and its one action
// (`meta.titel`, `meta.actie`, gedeeld/schil.ts).
// The dossier and a record in it are one screen, as the editor's sidebar
// and main pane: the record opens beside the dossier. Every record has one
// address, `/burger/dossier/<id>`; `/burger/besluit/<id>`, the address of a
// decision before, still works for links already given out. The application
// form is a sheet over the dossier, so `/burger/aanvraag` is that same screen
// with the sheet open.
import { RouterView } from 'vue-router';
import type { RouteRecordRaw } from 'vue-router';

export const burgerRoutes: RouteRecordRaw[] = [
  {
    path: '/burger',
    component: RouterView,
    meta: {
      kant: 'burger',
      titel: 'Parkeervergunningen',
      actie: {
        tekst: 'Nieuwe aanvraag',
        pad: '/burger/aanvraag',
        icoon: 'add',
        uitleg: {
          tekst: 'Hier vraagt u de kaart aan',
          toelichting: 'U hebt nog geen aanvraag gedaan. Met deze knop begint u er een.',
        },
      },
    },
    children: [
      { path: '', name: 'dossier', component: () => import('./DossierView.vue') },
      {
        path: 'dossier/:id',
        alias: 'besluit/:id',
        name: 'stuk',
        component: () => import('./DossierView.vue'),
        props: true,
      },
      { path: 'aanvraag', name: 'aanvraag', component: () => import('./DossierView.vue') },
    ],
  },
];
