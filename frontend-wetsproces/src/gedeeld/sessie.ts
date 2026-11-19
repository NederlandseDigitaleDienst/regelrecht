// The session behind every call to the server, in one place.
//
// The API clients (burger/api.ts, backoffice/api.ts) never say how a request
// proves who sends it: they hand their request to `metSessie`, which adds
// that. In this app the proof is the session cookie the server set at the
// login, so the default only asks the browser to send it along. A client that
// logs in another way, such as an app with a token from another login
// protocol, puts its own `Sessie` in place with `gebruikSessie`, and no
// client changes.

/** How a request proves who sends it. */
export interface Sessie {
  /** The request `init`, with what proves who sends it. */
  bij(init: RequestInit): RequestInit;
}

/** The session cookie of this origin, which the browser sends along. */
export const cookieSessie: Sessie = {
  bij: (init) => ({ ...init, credentials: 'same-origin' }),
};

let huidige: Sessie = cookieSessie;

/** Use `sessie` for every call from now on. */
export function gebruikSessie(sessie: Sessie): void {
  huidige = sessie;
}

/** The request `init` with the proof of the session in use. */
export function metSessie(init: RequestInit = {}): RequestInit {
  return huidige.bij(init);
}
