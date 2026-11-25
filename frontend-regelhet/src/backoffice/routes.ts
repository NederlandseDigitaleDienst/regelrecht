// The routes of the back office, each screen a chunk of its own. The
// sections are the shell's tabs (`meta.tabs`, gedeeld/schil.ts); the admin
// panel shows only to who enters what an external party did. The tools of
// the test setup are no section but open over any of them from the account
// menu, for who may set the clock: the clock as a sheet (`KLOK`), emptying
// the setup as a dialog (`LEGEN`). The work queue and the submission being
// handled are one screen, as the editor's sidebar and main pane.
import type { RouteRecordRaw } from 'vue-router';

/** The hash of the sheet that sets the clock, on whatever screen of the back office is open. */
export const KLOK = '#klok';
/** The hash of the dialog that empties the test setup. */
export const LEGEN = '#testomgeving-legen';

export const backofficeRoutes: RouteRecordRaw[] = [
  {
    path: '/backoffice',
    component: () => import('./BackofficeKader.vue'),
    meta: {
      kant: 'backoffice',
      gereedschap: [
        { tekst: 'Klok instellen', icoon: 'calendar', hash: KLOK, recht: 'klok_zetten' },
        { tekst: 'Testomgeving legen', icoon: 'delete', destructief: true, hash: LEGEN, recht: 'klok_zetten' },
      ],
      tabs: [
        { naam: 'werkvoorraad', tekst: 'Aanvragen', pad: '/backoffice', icoon: 'inbox', ook: ['behandelen'], recht: 'uitvoerder' },
        { naam: 'kroniek', tekst: 'Kroniek', pad: '/backoffice/kroniek', icoon: 'history', recht: 'uitvoerder' },
        // On the server of an external party, the same screen under the name
        // its own staff know it by.
        { naam: 'partij', tekst: 'Verzoeken', pad: '/backoffice/verzoeken', icoon: 'inbox', recht: 'partij' },
        {
          naam: 'beheer',
          tekst: 'Beheerpaneel',
          pad: '/backoffice/beheer',
          icoon: 'form',
          ook: ['beheer-behandelen'],
          recht: 'partijen_invoeren',
        },
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
      // The addresses of the clock when it was a screen, last called the simulation.
      { path: 'simulatie', redirect: { path: '/backoffice', hash: KLOK } },
      { path: 'klok', redirect: { path: '/backoffice', hash: KLOK } },
      { path: 'verzoeken', name: 'partij', component: () => import('./PartijView.vue') },
      { path: 'beheer/:cel?', name: 'beheer', component: () => import('./BeheerView.vue'), props: true, meta: { beheer: true } },
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
