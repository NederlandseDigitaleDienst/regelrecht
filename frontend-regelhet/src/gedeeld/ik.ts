// Who is logged in, for the whole app: asked once when the app opens and
// again after a login. The screens use it to hide what the account may not
// do; the server checks every call itself, so this is no security.
import { ref } from 'vue';
import { gedeeldApi } from './api';
import { ApiError } from '@regelrecht/frontend-shared';
import { foutTekst, nietIngelogd } from './http';
import type { Ik } from './types';

/** The account, or null when nobody is logged in. */
export const ik = ref<Ik | null>(null);
/** Why the app cannot open for the account: no role of this server (`rol`), or a server that fails (`storing`). */
export const geenToegang = ref<{ soort: 'rol' | 'storing'; kop: string; reden: string } | null>(null);

let geladen: Promise<Ik | null> | null = null;

/** Ask the server who is logged in; `opnieuw` asks again after a login. */
export function laadIk(opnieuw = false): Promise<Ik | null> {
  if (geladen && !opnieuw) return geladen;
  geladen = gedeeldApi
    .ik()
    .then((wie) => {
      ik.value = wie;
      geenToegang.value = null;
      return wie;
    })
    .catch((e: unknown) => {
      ik.value = null;
      if (nietIngelogd(e)) geenToegang.value = null;
      else if (e instanceof ApiError && e.status === 403) {
        geenToegang.value = { soort: 'rol', kop: 'Geen toegang', reden: 'Dit account heeft geen rol in deze proefopstelling.' };
      } else geenToegang.value = { soort: 'storing', kop: 'De server is niet bereikbaar', reden: foutTekst(e) };
      return null;
    });
  return geladen;
}

/**
 * Whether the account works in the back office: it reads a cell, or it may set
 * the clock, or enters what its party did, or (on the front of an
 * organisation, which runs no cell and works for no party) it holds a step
 * of the ladder.
 */
export const isMedewerker = (wie: Ik | null): boolean =>
  !!wie &&
  (wie.cellen.length > 0 || wie.klok_zetten || (!!wie.partij && wie.partijen_invoeren) || (!wie.burger && !!wie.niveau && !wie.partij));

/** Where the app opens for `wie`: its own side, or the login. */
export function startpagina(wie: Ik | null): string {
  if (wie?.burger) return '/burger';
  if (isMedewerker(wie)) return '/backoffice';
  return '/inloggen';
}
