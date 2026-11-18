import { describe, it, expect, beforeEach, vi } from 'vitest';

// Replace window.location with a plain object that records href assignments
// (so connect() doesn't trigger a real navigation in happy-dom).
function stubLocation({ pathname = '/', search = '', hash = '' } = {}) {
  const loc = { href: '', pathname, search, hash, origin: 'http://localhost' };
  Object.defineProperty(window, 'location', {
    configurable: true,
    writable: true,
    value: loc,
  });
  return loc;
}

// Fresh module per test so the module-singleton status/readyPromise start clean
// and vi.doMock('./apiFetch', …) applies.
async function freshGithub({
  apiFetchJson,
  apiFetch,
}: { apiFetchJson?: ReturnType<typeof vi.fn>; apiFetch?: ReturnType<typeof vi.fn> } = {}) {
  vi.resetModules();
  vi.doMock('./apiFetch', () => ({
    apiFetchJson: apiFetchJson || vi.fn().mockResolvedValue({}),
    apiFetch: apiFetch || vi.fn().mockResolvedValue(new Response(null, { status: 204 })),
  }));
  return import('./useGithubAuth');
}

describe('connect URL construction', () => {
  let loc: ReturnType<typeof stubLocation>;
  beforeEach(() => {
    loc = stubLocation({ pathname: '/editor', search: '?a=1', hash: '#h' });
  });

  it('connect() with no argument uses the current location as return_url', async () => {
    const { useGithubAuth } = await freshGithub();
    useGithubAuth().connect();
    expect(loc.href).toBe(
      '/auth/github/login?return_url=' + encodeURIComponent('/editor?a=1#h'),
    );
  });

  it('connect(returnUrl) forwards an explicit string', async () => {
    const { useGithubAuth } = await freshGithub();
    useGithubAuth().connect('/editor/traject-x');
    expect(loc.href).toBe(
      '/auth/github/login?return_url=' + encodeURIComponent('/editor/traject-x'),
    );
  });

  it('connect ignores a non-string (PointerEvent) arg and falls back to location', async () => {
    const { useGithubAuth } = await freshGithub();
    useGithubAuth().connect({ type: 'pointerdown' });
    expect(loc.href).toBe(
      '/auth/github/login?return_url=' + encodeURIComponent('/editor?a=1#h'),
    );
  });
});

describe('ensureGithubReady / status', () => {
  beforeEach(() => stubLocation());

  it('populates status from /auth/github/status', async () => {
    const apiFetchJson = vi.fn().mockResolvedValue({
      connected: true,
      configured: true,
      github_login: 'octocat',
      scopes: 'repo',
      expired: false,
      required: false,
    });
    const { useGithubAuth, ensureGithubReady } = await freshGithub({ apiFetchJson });
    await ensureGithubReady();
    const { status, loading } = useGithubAuth();
    expect(apiFetchJson).toHaveBeenCalledWith('/auth/github/status');
    expect(status.value?.connected).toBe(true);
    expect(status.value?.github_login).toBe('octocat');
    expect(loading.value).toBe(false);
  });

  it('treats a failed status as unconfigured (hides the UI)', async () => {
    const { useGithubAuth, ensureGithubReady } = await freshGithub({
      apiFetchJson: vi.fn().mockRejectedValue(new Error('500')),
    });
    await ensureGithubReady();
    const { status } = useGithubAuth();
    expect(status.value?.configured).toBe(false);
    expect(status.value?.connected).toBe(false);
  });

  it('fetches status only once across calls (readyPromise singleton)', async () => {
    const apiFetchJson = vi.fn().mockResolvedValue({ connected: false, configured: true });
    const { useGithubAuth, ensureGithubReady } = await freshGithub({ apiFetchJson });
    await ensureGithubReady();
    useGithubAuth();
    useGithubAuth();
    await ensureGithubReady();
    expect(apiFetchJson).toHaveBeenCalledTimes(1);
  });

  it('disconnect() POSTs to the disconnect endpoint then re-fetches status', async () => {
    const apiFetch = vi.fn().mockResolvedValue(new Response(null, { status: 204 }));
    const apiFetchJson = vi
      .fn()
      .mockResolvedValueOnce({ connected: true, configured: true, github_login: 'octocat' })
      .mockResolvedValueOnce({ connected: false, configured: true });
    const { useGithubAuth, ensureGithubReady } = await freshGithub({ apiFetchJson, apiFetch });
    await ensureGithubReady();
    const gh = useGithubAuth();
    await gh.disconnect();
    expect(apiFetch).toHaveBeenCalledWith('/auth/github/disconnect', { method: 'POST' });
    expect(gh.status.value?.connected).toBe(false);
  });
});
