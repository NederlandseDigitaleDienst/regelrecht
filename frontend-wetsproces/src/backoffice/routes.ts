// The routes of the back office, each screen a chunk of its own.
import type { RouteRecordRaw } from 'vue-router';

export const backofficeRoutes: RouteRecordRaw[] = [
  {
    path: '/backoffice',
    component: () => import('./BackofficeLayout.vue'),
    meta: { kant: 'backoffice' },
    children: [
      { path: '', name: 'werkvoorraad', component: () => import('./WerkvoorraadView.vue') },
      { path: 'kroniek/:cel?', name: 'kroniek', component: () => import('./KroniekView.vue'), props: true },
      {
        path: 'behandelen/:cel/:id',
        name: 'behandelen',
        component: () => import('./BehandelenView.vue'),
        props: true,
      },
      { path: 'klok', name: 'klok', component: () => import('./KlokView.vue') },
      { path: 'beheer', name: 'beheer', component: () => import('./BeheerView.vue'), meta: { beheer: true } },
      {
        path: 'beheer/behandelen/:cel/:id',
        name: 'beheer-behandelen',
        component: () => import('./BehandelenView.vue'),
        props: (route) => ({ ...route.params, namens: true }),
        meta: { beheer: true },
      },
    ],
  },
];
