import { describe, it, expect, beforeEach, afterEach, vi } from 'vitest';
import { ref } from 'vue';
import { jsonResponse } from '../test-utils/fetchStubs';
import { useFeatureFlagsStore } from './featureFlags';

// Whether OIDC is configured decides what a failed write does; each test sets
// it. The global test setup gives every test a fresh Pinia, so the store
// starts from the defaults (plus localStorage) each time.
const oidcConfigured = ref(false);
vi.mock('../composables/useAuth', () => ({ useAuth: () => ({ oidcConfigured }) }));

async function freshFlags() {
  const store = useFeatureFlagsStore();
  await store.load();
  return store;
}

function stubFetch(putResponse: () => Response) {
  vi.stubGlobal(
    'fetch',
    vi.fn((input, init) => {
      const method = init?.method ?? 'GET';
      if (method === 'PUT') return Promise.resolve(putResponse());
      // GET /api/feature-flags - empty server set, so DEFAULTS apply.
      return Promise.resolve(
        jsonResponse({}),
      );
    }),
  );
}

// A flag not in DEFAULTS: unknown keys default to `true` (isEnabled's `?? true`),
// which is all these tests about the store mechanics need.
const KEY = 'test.flag';

describe('useFeatureFlagsStore', () => {
  beforeEach(() => {
    localStorage.clear();
    oidcConfigured.value = false;
  });
  afterEach(() => {
    vi.unstubAllGlobals();
    vi.restoreAllMocks();
  });

  it('keeps a toggle locally when the backend write is rejected (e.g. 401 in OIDC-off dev)', async () => {
    stubFetch(() => new Response('unauthorized', { status: 401 }));
    const { isEnabled, toggle } = await freshFlags();

    expect(isEnabled(KEY)).toBe(true); // unknown key defaults on
    await toggle(KEY);

    // Kept (not reverted) and persisted so it survives a refresh.
    expect(isEnabled(KEY)).toBe(false);
    const saved = JSON.parse(localStorage.getItem('regelrecht-feature-flags') || '{}');
    expect(saved[KEY]).toBe(false);
  });

  it('reverts the toggle when OIDC is configured and the write fails (prod - no sticky override)', async () => {
    // With OIDC configured, a failed write must not persist a local override
    // that would beat the server on the next load.
    oidcConfigured.value = true;
    stubFetch(() => new Response('unauthorized', { status: 401 }));
    const { isEnabled, toggle } = await freshFlags();

    expect(isEnabled(KEY)).toBe(true); // default
    await toggle(KEY);

    // Reverted in memory; nothing sticky left in localStorage to override the server.
    expect(isEnabled(KEY)).toBe(true);
    const saved = JSON.parse(localStorage.getItem('regelrecht-feature-flags') || '{}');
    expect(KEY in saved).toBe(false);
  });

  it('lets a server (DB) value win over a stale local override', async () => {
    // A toggle saved during a no-DB session leaves the flag off in localStorage;
    // once the flag is set in the DB the server value must win, otherwise the
    // feature stays hidden despite the admin-set flag.
    localStorage.setItem('regelrecht-feature-flags', JSON.stringify({ [KEY]: false }));
    vi.stubGlobal(
      'fetch',
      vi.fn((input, init) => {
        const method = init?.method ?? 'GET';
        if (method === 'PUT') return Promise.resolve(new Response('{}', { status: 200 }));
        return Promise.resolve(
          jsonResponse({ [KEY]: true }),
        );
      }),
    );
    const { isEnabled } = await freshFlags();

    expect(isEnabled(KEY)).toBe(true);
  });

  it('keeps a local override for a flag the server does not define (no-DB dev)', async () => {
    // Server returns an empty set (no DB row for this key), so the local toggle
    // is the only source and must survive.
    localStorage.setItem('regelrecht-feature-flags', JSON.stringify({ [KEY]: false }));
    stubFetch(() => new Response('{}', { status: 200 }));
    const { isEnabled } = await freshFlags();

    expect(isEnabled(KEY)).toBe(false);
  });

  it('clears the local override when the server write succeeds', async () => {
    stubFetch(
      () =>
        jsonResponse({ [KEY]: false }),
    );
    const { isEnabled, toggle } = await freshFlags();

    await toggle(KEY);

    expect(isEnabled(KEY)).toBe(false);
    // Server is authoritative - no lingering local override.
    const saved = JSON.parse(localStorage.getItem('regelrecht-feature-flags') || '{}');
    expect(KEY in saved).toBe(false);
  });
});
