// The login, apart from the API clients: which login the server offers, the
// accounts of the test login, and logging in and out. This is the one module
// that knows the server logs in with a session cookie; the clients do not.
import { apiFetch, apiFetchJson, useAuth } from '@regelrecht/frontend-shared';
import type { AuthStatus } from '@regelrecht/frontend-shared';
import { onbereikbaar } from '../gedeeld/sessie';

/** An account of the test login (`GET /auth/testpersonen`). */
export interface Persona {
  soort: 'burger' | 'medewerker' | 'beheerder';
  sub: string;
  rollen: string[];
  claims?: Record<string, unknown>;
  identiteit?: string;
  /** A citizen's made-up name. */
  naam?: string;
  niveau?: string;
  cel?: string;
  cel_naam?: string;
}

export const inloggenApi = {
  /** Asked anew every time: a test login changes it without a page load. */
  status: () => apiFetchJson<AuthStatus>('/auth/status', { credentials: 'same-origin' }).catch(onbereikbaar),
  testpersonen: () => apiFetchJson<Persona[]>('/auth/testpersonen', { credentials: 'same-origin' }).catch(onbereikbaar),
  testlogin: (p: Persona, naam: string) =>
    apiFetch('/auth/testlogin', {
      method: 'POST',
      credentials: 'same-origin',
      headers: { 'content-type': 'application/json' },
      body: JSON.stringify({ sub: p.sub, rollen: p.rollen, claims: p.claims ?? {}, naam }),
    }).catch(onbereikbaar),
  /**
   * A citizen of the simulated DigiD, by number and name. The server checks
   * the number (nine digits, the eleven test, the test range) and gives the
   * account the citizen's role and no other.
   */
  digid: (bsn: string, naam: string) =>
    apiFetch('/auth/testlogin', {
      method: 'POST',
      credentials: 'same-origin',
      headers: { 'content-type': 'application/json' },
      body: JSON.stringify({ bsn, naam }),
    }).catch(onbereikbaar),
  /** Through Keycloak: the browser leaves the app and comes back to `terug`. */
  oidc: (terug: string) => useAuth().login(terug),
  /** The server ends the session and sends the browser on (to Keycloak's logout, or to `/`). */
  uitloggen: () => useAuth().logout(),
};
