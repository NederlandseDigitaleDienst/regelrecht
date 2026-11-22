// What the screen that logs in does with Keycloak. Someone without a session
// goes on to Keycloak at once, the way they came (the access wall of the
// platform logged them in there already, so Keycloak sends them straight
// back). The screen stays only where going on would loop or hide something:
// just after logging out, after a login that failed or came back without a
// session, for an account this server does not admit, and when the server
// fails. With the test login the screen is the login itself, so none of this
// applies.

/** What the screen does: go to Keycloak, go in, or show one of its messages. */
export type Stap =
  | { soort: 'doorsturen' }
  | { soort: 'binnen' }
  | { soort: 'uitgelogd' }
  | { soort: 'mislukt' }
  | { soort: 'geen_toegang' }
  | { soort: 'storing' }
  | { soort: 'testlogin' };

/** What the screen knows when it opens. */
export interface Toestand {
  /** Whether the server logs in through Keycloak. */
  oidc: boolean;
  /** Whether the session is logged in. */
  ingelogd: boolean;
  /**
   * For a session that is logged in, what the server says of the account
   * (`GET /api/ik`): it admits it, it has no role here (403), or the answer
   * failed.
   */
  toegang: 'ja' | 'rol' | 'storing';
  /** What the server put in the address after a failed return from Keycloak (`?fout=`). */
  fout: 'rol' | 'inloggen' | null;
  /** Whether the browser is back from Keycloak (`?na=keycloak`): without a session then, the login failed. */
  naKeycloak: boolean;
  /** Whether this tab just logged out. */
  uitgelogd: boolean;
}

/** What the screen that logs in does, given `t`. */
export function stap(t: Toestand): Stap {
  if (!t.oidc) return { soort: 'testlogin' };
  if (t.ingelogd) {
    if (t.toegang === 'ja') return { soort: 'binnen' };
    return t.toegang === 'rol' ? { soort: 'geen_toegang' } : { soort: 'storing' };
  }
  // Each of these would bring the same failure back, or log the person
  // straight in again: the screen says what happened instead.
  if (t.fout === 'rol') return { soort: 'geen_toegang' };
  if (t.fout === 'inloggen' || t.naKeycloak) return { soort: 'mislukt' };
  if (t.uitgelogd) return { soort: 'uitgelogd' };
  return { soort: 'doorsturen' };
}
