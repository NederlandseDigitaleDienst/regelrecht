import { describe, it, expect, beforeEach, vi } from 'vitest';

// Replace window.location with a plain object that records href assignments
// (so login()/logout() don't trigger a real navigation in happy-dom).
function stubLocation({ pathname = '/', search = '', hash = '' } = {}) {
  const loc = { href: '', pathname, search, hash, origin: 'http://localhost' };
  Object.defineProperty(window, 'location', {
    configurable: true,
    writable: true,
    value: loc,
  });
  return loc;
}

// Each test imports a fresh module so useAuth's module-singleton refs and
// readyPromise start clean, and so vi.doMock('./apiFetch', …) is applied.
async function freshAuth(apiFetchJson: ReturnType<typeof vi.fn>) {
  vi.resetModules();
  vi.doMock('./apiFetch', () => ({ apiFetchJson }));
  return import('./useAuth');
}

describe('login / logout URL construction', () => {
  let loc: ReturnType<typeof stubLocation>;
  beforeEach(() => {
    loc = stubLocation({ pathname: '/editor', search: '?a=1', hash: '#h' });
  });

  it('login() with no argument uses the current location as return_url', async () => {
    const { useAuth } = await freshAuth(vi.fn().mockResolvedValue({}));
    useAuth().login();
    expect(loc.href).toBe(
      '/auth/login?return_url=' + encodeURIComponent('/editor?a=1#h'),
    );
  });

  it('login(returnUrl) forwards an explicit string', async () => {
    const { useAuth } = await freshAuth(vi.fn().mockResolvedValue({}));
    useAuth().login('/bibliotheek');
    expect(loc.href).toBe(
      '/auth/login?return_url=' + encodeURIComponent('/bibliotheek'),
    );
  });

  it('login ignores a non-string (PointerEvent) arg and falls back to location', async () => {
    const { useAuth } = await freshAuth(vi.fn().mockResolvedValue({}));
    useAuth().login({ type: 'pointerdown' });
    expect(loc.href).toBe(
      '/auth/login?return_url=' + encodeURIComponent('/editor?a=1#h'),
    );
  });

  it('logout() redirects to /auth/logout', async () => {
    const { useAuth } = await freshAuth(vi.fn().mockResolvedValue({}));
    useAuth().logout();
    expect(loc.href).toBe('/auth/logout');
  });
});

describe('ensureAuthReady / checkAuth', () => {
  beforeEach(() => stubLocation());

  it('populates auth state from /auth/status', async () => {
    const apiFetchJson = vi.fn().mockResolvedValue({
      authenticated: true,
      oidc_configured: true,
      person: { name: 'Jane' },
    });
    const { useAuth, ensureAuthReady } = await freshAuth(apiFetchJson);
    await ensureAuthReady();
    const { authenticated, oidcConfigured, person, loading } = useAuth();
    expect(apiFetchJson).toHaveBeenCalledWith('/auth/status');
    expect(authenticated.value).toBe(true);
    expect(oidcConfigured.value).toBe(true);
    expect(person.value).toEqual({ name: 'Jane' });
    expect(loading.value).toBe(false);
  });

  it('treats a failed /auth/status as unauthenticated', async () => {
    const { useAuth, ensureAuthReady } = await freshAuth(
      vi.fn().mockRejectedValue(new Error('500')),
    );
    await ensureAuthReady();
    const { authenticated, oidcConfigured, loading } = useAuth();
    expect(authenticated.value).toBe(false);
    expect(oidcConfigured.value).toBe(false);
    expect(loading.value).toBe(false);
  });

  it('fetches /auth/status only once across calls (readyPromise singleton)', async () => {
    const apiFetchJson = vi.fn().mockResolvedValue({
      authenticated: false,
      oidc_configured: false,
    });
    const { useAuth, ensureAuthReady } = await freshAuth(apiFetchJson);
    await ensureAuthReady();
    useAuth();
    useAuth();
    await ensureAuthReady();
    expect(apiFetchJson).toHaveBeenCalledTimes(1);
  });
});

describe('roles / hasRole / hasAnyRole', () => {
  beforeEach(() => stubLocation());

  it('exposes the roles from /auth/status and gates on them', async () => {
    const apiFetchJson = vi.fn().mockResolvedValue({
      authenticated: true,
      oidc_configured: true,
      person: { name: 'Jane', roles: ['editor-reader', 'harvester-writer'] },
    });
    const { useAuth, ensureAuthReady } = await freshAuth(apiFetchJson);
    await ensureAuthReady();
    const { roles, hasRole, hasAnyRole } = useAuth();
    expect(roles.value).toEqual(['editor-reader', 'harvester-writer']);
    expect(hasRole('harvester-writer')).toBe(true);
    expect(hasRole('harvester-admin')).toBe(false);
    expect(hasAnyRole(['harvester-reader', 'harvester-writer'])).toBe(true);
    expect(hasAnyRole(['harvester-admin', 'regelrecht-admin'])).toBe(false);
  });

  it('defaults roles to an empty array when unauthenticated', async () => {
    const apiFetchJson = vi.fn().mockResolvedValue({
      authenticated: false,
      oidc_configured: true,
    });
    const { useAuth, ensureAuthReady } = await freshAuth(apiFetchJson);
    await ensureAuthReady();
    const { roles, hasAnyRole } = useAuth();
    expect(roles.value).toEqual([]);
    expect(hasAnyRole(['harvester-reader'])).toBe(false);
  });

  it('defaults roles to an empty array when person carries no roles field', async () => {
    const apiFetchJson = vi.fn().mockResolvedValue({
      authenticated: true,
      oidc_configured: true,
      person: { name: 'No Roles' },
    });
    const { useAuth, ensureAuthReady } = await freshAuth(apiFetchJson);
    await ensureAuthReady();
    const { roles } = useAuth();
    expect(roles.value).toEqual([]);
  });
});
