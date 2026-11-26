// How a call reaches the server and proves who sends it, in one place.
//
// The API clients (burger/api.ts, backoffice/api.ts) never say how: they
// call `haal` and `stuur` in http.ts, which hand the call to the session in
// use. In the browser that is the session cookie the server set at the login
// (`cookieSessie`). The citizen's entry (src/burgeromgeving/) puts the
// `Sessie` of its transport in place with `gebruikSessie`: in the app a call
// goes to a command of the Rust core, which holds the key and the session of
// FSC voor Burgers, and no client changes.
import { apiFetchJson } from '@regelrecht/frontend-shared';

/** How a call reaches the server and proves who sends it. */
export interface Sessie {
  /** `GET pad`, as JSON. */
  haal<T>(pad: string): Promise<T>;
  /**
   * `methode pad` with the JSON body `body`, as JSON. `sleutel`, when given,
   * is the idempotency key: the server answers a request sent again under the
   * same key as the first time and records nothing twice.
   */
  stuur<T>(methode: string, pad: string, body: unknown, sleutel?: string): Promise<T>;
}

/** What a screen says when the server could not be reached or answered with a page of the platform. */
export const NIET_BEREIKBAAR =
  'De server is op dit moment niet bereikbaar. Wat u hebt ingevuld blijft staan; probeer het over een moment opnieuw.';

/**
 * The reason in an error answer of the server (`{ "fout": "<why>" }`), for a
 * screen to show. Any other body is never shown as it is: a 5xx is often an
 * HTML page of the platform in front of the server (a restart, a deploy),
 * and a 4xx without a reason is axum's own rejection, written for a
 * developer. Both transports take their messages from here.
 */
export function reden(body: string, status?: number): string {
  try {
    const parsed: unknown = JSON.parse(body);
    if (parsed && typeof parsed === 'object' && 'fout' in parsed && typeof parsed.fout === 'string') {
      return parsed.fout;
    }
  } catch {
    // Not JSON: a page of the platform, or axum's rejection in plain text.
  }
  if (status !== undefined && status >= 400 && status < 500) {
    return `De server nam dit verzoek niet aan (status ${status}). Wat u hebt ingevuld blijft staan; probeer het opnieuw.`;
  }
  return NIET_BEREIKBAAR;
}

const errorMessage = (status: number, body: string) => reden(body, status);

/** A call that got no answer the screen can read: no answer at all, or a page that is not the JSON it asked for. */
export class Onbereikbaar extends Error {
  constructor(oorzaak: unknown) {
    super(NIET_BEREIKBAAR, { cause: oorzaak });
    this.name = 'Onbereikbaar';
  }
}

/**
 * `fetch` rejects with a TypeError when the request never reached the
 * server, and reading a 200 that is a page of the platform as JSON fails with
 * a SyntaxError. Both become `Onbereikbaar`; the original goes to the console
 * for whoever looks. Any other error passes on as it is.
 */
export function onbereikbaar(e: unknown): never {
  if (e instanceof TypeError || e instanceof SyntaxError) {
    console.error('a call to the server failed', e);
    throw new Onbereikbaar(e);
  }
  throw e;
}

/** The session cookie of this origin, which the browser sends along. */
export const cookieSessie: Sessie = {
  haal: <T>(pad: string) => apiFetchJson<T>(pad, { credentials: 'same-origin', errorMessage }).catch(onbereikbaar),
  stuur: <T>(methode: string, pad: string, body: unknown, sleutel?: string) => {
    const headers: Record<string, string> = { 'content-type': 'application/json' };
    if (sleutel) headers['idempotency-key'] = sleutel;
    return apiFetchJson<T>(pad, {
      method: methode,
      headers,
      body: JSON.stringify(body),
      credentials: 'same-origin',
      errorMessage,
    }).catch(onbereikbaar);
  },
};

let huidige: Sessie = cookieSessie;

/** Use `sessie` for every call from now on. */
export function gebruikSessie(sessie: Sessie): void {
  huidige = sessie;
}

/** The session in use. */
export function sessie(): Sessie {
  return huidige;
}
