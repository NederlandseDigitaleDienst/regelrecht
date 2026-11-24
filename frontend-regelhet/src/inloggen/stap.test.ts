// @vitest-environment node
import { describe, expect, it } from 'vitest';
import { stap } from './stap';
import type { Toestand } from './stap';

const geen: Toestand = { oidc: true, ingelogd: false, toegang: 'storing', fout: null, naKeycloak: false, uitgelogd: false };

describe('the screen that logs in, with Keycloak', () => {
  it('sends someone without a session on to Keycloak at once', () => {
    expect(stap(geen).soort).toBe('doorsturen');
  });

  it('stays after logging out, after a failed login and when Keycloak sent the browser back without a session', () => {
    expect(stap({ ...geen, uitgelogd: true }).soort).toBe('uitgelogd');
    expect(stap({ ...geen, fout: 'inloggen' }).soort).toBe('mislukt');
    expect(stap({ ...geen, naKeycloak: true }).soort).toBe('mislukt');
    // A failure is told first, also right after a logout.
    expect(stap({ ...geen, fout: 'inloggen', uitgelogd: true }).soort).toBe('mislukt');
  });

  it('tells an account without a role so, whether the server or the callback found it out', () => {
    expect(stap({ ...geen, fout: 'rol' }).soort).toBe('geen_toegang');
    expect(stap({ ...geen, ingelogd: true, toegang: 'rol' }).soort).toBe('geen_toegang');
  });

  it('tells a server that fails apart from an account without a role', () => {
    expect(stap({ ...geen, ingelogd: true, toegang: 'storing' }).soort).toBe('storing');
  });

  it('lets an account it admits in, whatever stale marker the address or the tab carries', () => {
    expect(stap({ ...geen, ingelogd: true, toegang: 'ja' }).soort).toBe('binnen');
    expect(stap({ ...geen, ingelogd: true, toegang: 'ja', fout: 'inloggen', naKeycloak: true, uitgelogd: true }).soort).toBe('binnen');
  });

  it('is the test login itself without Keycloak', () => {
    expect(stap({ ...geen, oidc: false }).soort).toBe('testlogin');
    expect(stap({ ...geen, oidc: false, uitgelogd: true }).soort).toBe('testlogin');
  });
});
