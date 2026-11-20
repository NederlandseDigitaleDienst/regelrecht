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

/** The reason in an error answer of the server, or its plain text. */
export function reden(body: string): string {
  try {
    const parsed: unknown = JSON.parse(body);
    if (parsed && typeof parsed === 'object' && 'fout' in parsed && typeof parsed.fout === 'string') {
      return parsed.fout;
    }
  } catch {
    // Not JSON: axum's own rejection of a body that does not parse.
  }
  return body;
}

const errorMessage = (_status: number, body: string) => reden(body);

/** The session cookie of this origin, which the browser sends along. */
export const cookieSessie: Sessie = {
  haal: <T>(pad: string) => apiFetchJson<T>(pad, { credentials: 'same-origin', errorMessage }),
  stuur: <T>(methode: string, pad: string, body: unknown, sleutel?: string) => {
    const headers: Record<string, string> = { 'content-type': 'application/json' };
    if (sleutel) headers['idempotency-key'] = sleutel;
    return apiFetchJson<T>(pad, {
      method: methode,
      headers,
      body: JSON.stringify(body),
      credentials: 'same-origin',
      errorMessage,
    });
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
