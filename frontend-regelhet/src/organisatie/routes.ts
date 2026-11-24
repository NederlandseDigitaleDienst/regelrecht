// The routes of an organisation's front, under the paths a process uses for
// its sides, so the login and the router of the shared shell (router.ts) send
// each account to its own side as they do in a process. A citizen gets one
// page of tiles. An employee gets the work queue over every process, and the
// tiles of the processes as a second tab (`meta.tabs`, gedeeld/schil.ts). The
// bar carries the organisation's name (gedeeld/uiterlijk.ts).
import { RouterView } from 'vue-router';
import type { RouteRecordRaw } from 'vue-router';

export const organisatieRoutes: RouteRecordRaw[] = [
  {
    path: '/burger',
    name: 'organisatie-burger',
    component: () => import('./OrganisatieHome.vue'),
    props: { kant: 'burger' },
    meta: { kant: 'burger' },
  },
  {
    path: '/backoffice',
    component: RouterView,
    meta: {
      kant: 'backoffice',
      tabs: [
        { naam: 'organisatie-werkvoorraad', tekst: 'Werkvoorraad', pad: '/backoffice', icoon: 'inbox' },
        { naam: 'organisatie-regelingen', tekst: 'Regelingen', pad: '/backoffice/regelingen', icoon: 'building' },
      ],
    },
    children: [
      { path: '', name: 'organisatie-werkvoorraad', component: () => import('./WerkvoorraadView.vue') },
      {
        path: 'regelingen',
        name: 'organisatie-regelingen',
        component: () => import('./OrganisatieHome.vue'),
        props: { kant: 'backoffice' },
      },
    ],
  },
  // Where the server sends a citizen whose login through Keycloak failed.
  { path: '/toegang', redirect: (to) => ({ name: 'inloggen', query: to.query }) },
];
