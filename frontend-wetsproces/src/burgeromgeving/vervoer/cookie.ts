// The transport of the citizen's web environment: plain HTTP with the
// session cookie of the login, as the combined web build has it. See
// ../vervoer.ts. Not deployed yet; the build is `--mode burger`.
import { createWebHistory } from 'vue-router';
import { nietIngelogd } from '../../gedeeld/http';
import { cookieSessie } from '../../gedeeld/sessie';
import type { Sessie } from '../../gedeeld/sessie';
import type { Ik } from '../../gedeeld/types';
import { inloggenApi } from '../../inloggen/api';
import type { Vervoer } from '../vervoer';

export const vervoer: Vervoer = {
  sessie(bijGeenToegang): Sessie {
    const bewaakt = <T>(p: Promise<T>) =>
      p.catch((e: unknown) => {
        if (nietIngelogd(e)) bijGeenToegang('niet_ingelogd');
        throw e;
      });
    return {
      haal: (pad) => bewaakt(cookieSessie.haal(pad)),
      stuur: (methode, pad, body, sleutel) => bewaakt(cookieSessie.stuur(methode, pad, body, sleutel)),
    };
  },
  async toegang() {
    const ik = await cookieSessie.haal<Ik>('/api/ik').catch(() => null);
    return { binnen: !!ik?.burger, naam: ik ? (ik.naam ?? ik.sub) : null };
  },
  toegangScherm: () => import('../../inloggen/InloggenView.vue'),
  afmelden: { tekst: 'Uitloggen', doe: async () => inloggenApi.uitloggen() },
  onderschrift: () => '',
  history: () => createWebHistory(),
};
