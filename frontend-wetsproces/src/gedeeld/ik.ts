// Who is logged in, for the whole app: asked once when the app opens and
// again after a login. The screens use it to hide what the account may not
// do; the server checks every call itself, so this is no security.
import { ref } from 'vue';
import { gedeeldApi } from './api';
import { nietIngelogd } from './http';
import type { Ik } from './types';

/** The account, or null when nobody is logged in. */
export const ik = ref<Ik | null>(null);
/** Why the account cannot use the app, when it is logged in without a role of this server. */
export const geenToegang = ref('');

let geladen: Promise<Ik | null> | null = null;

/** Ask the server who is logged in; `opnieuw` asks again after a login. */
export function laadIk(opnieuw = false): Promise<Ik | null> {
  if (geladen && !opnieuw) return geladen;
  geladen = gedeeldApi
    .ik()
    .then((wie) => {
      ik.value = wie;
      geenToegang.value = '';
      return wie;
    })
    .catch((e: unknown) => {
      ik.value = null;
      geenToegang.value = nietIngelogd(e) ? '' : 'Dit account heeft geen rol in deze proefopstelling.';
      return null;
    });
  return geladen;
}

/** Whether the account works in the back office: it reads a cell, or it may set the clock. */
export const isMedewerker = (wie: Ik | null): boolean => !!wie && (wie.cellen.length > 0 || wie.klok_zetten);

/** Where the app opens for `wie`: its own side, or the login. */
export function startpagina(wie: Ik | null): string {
  if (wie?.burger) return '/burger';
  if (isMedewerker(wie)) return '/backoffice';
  return '/inloggen';
}
