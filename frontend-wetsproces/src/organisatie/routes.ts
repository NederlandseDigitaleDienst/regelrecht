// The routes of an organisation's front: one page of tiles per side, under the
// paths a process uses for its sides, so the login and the router of the
// shared shell (router.ts) send each account to its own side as they do in a
// process. The front has no sections; its bar carries the organisation's name
// (gedeeld/uiterlijk.ts).
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
    name: 'organisatie-backoffice',
    component: () => import('./OrganisatieHome.vue'),
    props: { kant: 'backoffice' },
    meta: { kant: 'backoffice' },
  },
  // Where the server sends a citizen whose login through Keycloak failed.
  { path: '/toegang', redirect: (to) => ({ name: 'inloggen', query: to.query }) },
];
