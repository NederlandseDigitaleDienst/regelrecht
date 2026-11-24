// The routes of the back office, each screen a chunk of its own. The
// sections are the shell's tabs (`meta.tabs`, gedeeld/schil.ts); the clock
// shows only to who may set it, the admin panel to who enters what an
// external party did. The work queue and the submission being
// handled are one screen, as the editor's sidebar and main pane.
import { RouterView } from 'vue-router';
import type { RouteRecordRaw } from 'vue-router';

export const backofficeRoutes: RouteRecordRaw[] = [
  {
    path: '/backoffice',
    component: RouterView,
    meta: {
      kant: 'backoffice',
      tabs: [
        { naam: 'werkvoorraad', tekst: 'Werkvoorraad', pad: '/backoffice', icoon: 'inbox', ook: ['behandelen'], recht: 'uitvoerder' },
        { naam: 'kroniek', tekst: 'Kroniek', pad: '/backoffice/kroniek', icoon: 'history', recht: 'uitvoerder' },
        // On the server of an external party, the same screen under the name
        // its own staff know it by.
        { naam: 'beheer', tekst: 'Verzoeken', pad: '/backoffice/beheer', icoon: 'inbox', ook: ['beheer-behandelen'], recht: 'partij' },
        {
          naam: 'beheer',
          tekst: 'Beheerpaneel',
          pad: '/backoffice/beheer',
          icoon: 'form',
          ook: ['beheer-behandelen'],
          recht: 'partijen_invoeren',
        },
        { naam: 'simulatie', tekst: 'Simulatie', pad: '/backoffice/simulatie', icoon: 'calendar', recht: 'klok_zetten' },
      ],
    },
    children: [
      { path: '', name: 'werkvoorraad', component: () => import('./WerkvoorraadView.vue') },
      {
        path: 'behandelen/:cel/:id',
        name: 'behandelen',
        component: () => import('./WerkvoorraadView.vue'),
        props: true,
      },
      { path: 'kroniek/:cel?', name: 'kroniek', component: () => import('./KroniekView.vue'), props: true },
      { path: 'simulatie', name: 'simulatie', component: () => import('./SimulatieView.vue') },
      // The address of this screen before it was called a simulation.
      { path: 'klok', redirect: { name: 'simulatie' } },
      { path: 'beheer', name: 'beheer', component: () => import('./BeheerView.vue'), meta: { beheer: true } },
      {
        path: 'beheer/behandelen/:cel/:id',
        name: 'beheer-behandelen',
        component: () => import('./BeheerView.vue'),
        props: true,
        meta: { beheer: true },
      },
    ],
  },
];
