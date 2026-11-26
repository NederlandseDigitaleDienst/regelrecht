// The login, apart from the API clients: which login the server offers, the
// accounts of the test login, and logging in and out. This is the one module
// that knows the server logs in with a session cookie; the clients do not.
import { apiFetch, apiFetchJson, useAuth } from '@regelrecht/frontend-shared';
import type { AuthStatus } from '@regelrecht/frontend-shared';
import { onbereikbaar } from '../gedeeld/sessie';
import { markeerUitgelogd } from './uitgelogd';

/** An account of the test login (`GET /auth/testpersonen`). */
export interface Persona {
  soort: 'burger' | 'medewerker' | 'beheerder';
  sub: string;
  rollen: string[];
  claims?: Record<string, unknown>;
  identiteit?: string;
  /** What the number is called: BSN, or KvK-nummer for a business. */
  label?: string;
  /** A citizen's or a business's made-up name. */
  naam?: string;
  niveau?: string;
  cel?: string | null;
  cel_naam?: string | null;
  /** Whether the account is of an external party's own server: its staff, not the executor's. */
  partij?: boolean;
}

/**
 * What the simulated login of the citizens' side asks (`GET /auth/simulatie`):
 * the number that identifies an applicant, and for a case with a login step
 * what that step offers.
 */
export interface Simulatie {
  identiteit: {
    claim: string;
    label: string;
    soort: 'bsn' | 'kvk' | 'anders';
    cijfers: number | null;
  };
  inlogstap: {
    cel: string;
    cel_naam: string;
    dienst: string;
    wachtwoord: string;
    gebruikersnamen: string[];
  } | null;
}

export const inloggenApi = {
  /** Asked anew every time: a test login changes it without a page load. */
  status: () => apiFetchJson<AuthStatus>('/auth/status', { credentials: 'same-origin' }).catch(onbereikbaar),
  testpersonen: () => apiFetchJson<Persona[]>('/auth/testpersonen', { credentials: 'same-origin' }).catch(onbereikbaar),
  simulatie: () => apiFetchJson<Simulatie>('/auth/simulatie', { credentials: 'same-origin' }).catch(onbereikbaar),
  testlogin: (p: Persona, naam: string) =>
    apiFetch('/auth/testlogin', {
      method: 'POST',
      credentials: 'same-origin',
      headers: { 'content-type': 'application/json' },
      body: JSON.stringify({ sub: p.sub, rollen: p.rollen, claims: p.claims ?? {}, naam }),
    }).catch(onbereikbaar),
  /**
   * An applicant of the simulated DigiD or eHerkenning, by number and name.
   * The server checks the number (a BSN of the test range, or eight digits
   * for a KvK number) and gives the account the citizen's role and no other.
   */
  digid: (nummer: string, naam: string) =>
    apiFetch('/auth/testlogin', {
      method: 'POST',
      credentials: 'same-origin',
      headers: { 'content-type': 'application/json' },
      body: JSON.stringify({ nummer, naam }),
    }).catch(onbereikbaar),
  /**
   * Through the login step of the case: the cell of the step checks the user
   * name and the password and records the authentication.
   */
  inlogstap: (gebruikersnaam: string, wachtwoord: string) =>
    apiFetch('/auth/testlogin', {
      method: 'POST',
      credentials: 'same-origin',
      headers: { 'content-type': 'application/json' },
      body: JSON.stringify({ gebruikersnaam, wachtwoord }),
    }).catch(onbereikbaar),
  /** Through Keycloak: the browser leaves the app and comes back to `terug`. */
  oidc: (terug: string) => useAuth().login(terug),
  /**
   * The server ends the session and sends the browser on (to Keycloak's
   * logout, or to `/`). The tab remembers it, so the screen that logs in says
   * so instead of logging the person straight back in (uitgelogd.ts).
   */
  uitloggen: () => {
    markeerUitgelogd();
    useAuth().logout();
  },
};
