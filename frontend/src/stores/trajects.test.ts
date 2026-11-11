import { describe, it, expect, vi, beforeEach, afterEach } from 'vitest';
import { isTrajectMissing, useTrajectsStore, type TrajectSummary } from './trajects';
import { jsonResponse } from '../test-utils/fetchStubs';

// The route and the login state the store reads. The route is reactive, as a
// real one is, so the store's getters follow it; the global test setup gives
// every test a fresh Pinia.
const route = await vi.hoisted(async () => {
  const { reactive } = await import('vue');
  return reactive({ params: {} as Record<string, string> });
});
vi.mock('vue-router', () => ({ useRoute: () => route }));
vi.mock('../composables/useAuth', () => ({
  useAuth: () => ({ authenticated: { value: true }, oidcConfigured: { value: true } }),
}));

// Signature: isTrajectMissing(trajectRef, authorized, loading, errored, activeTraject)
describe('isTrajectMissing', () => {
  const ref = 'zorgtoeslag-12345678';
  const member = { ref, name: 'Zorgtoeslag' };

  it('is true when the URL names a traject not in the membership list', () => {
    // authorized, list done loading, no error, ref not found → missing
    expect(isTrajectMissing(ref, true, false, false, null)).toBe(true);
  });

  it('is false when the traject is one of the memberships', () => {
    expect(isTrajectMissing(ref, true, false, false, member)).toBe(false);
  });

  it('is false while the membership list is still loading', () => {
    expect(isTrajectMissing(ref, true, true, false, null)).toBe(false);
  });

  it('is false when the trajects fetch errored (e.g. network blip)', () => {
    // empty list from a failed fetch must not read as "does not exist"
    expect(isTrajectMissing(ref, true, false, true, null)).toBe(false);
  });

  it('is false when the user is not authorized', () => {
    expect(isTrajectMissing(ref, false, false, false, null)).toBe(false);
  });

  it('is false for the global view (no traject in the URL)', () => {
    expect(isTrajectMissing(null, true, false, false, null)).toBe(false);
  });
});

describe('useTrajectsStore', () => {
  const TRAJECT: TrajectSummary = {
    id: '00000000-0000-0000-0000-0000abcd1234',
    name: 'Testtraject',
    description: '',
    scope: '',
    status: 'bezig',
    role: 'owner',
    ref: 'testtraject-abcd1234',
  };

  beforeEach(() => {
    vi.stubGlobal('fetch', vi.fn(async () => jsonResponse([TRAJECT])));
  });
  afterEach(() => {
    vi.unstubAllGlobals();
    for (const key of Object.keys(route.params)) delete route.params[key];
  });

  it('takes the active traject from the route and finds it in the list', async () => {
    const store = useTrajectsStore();
    await store.ensureReady();
    expect(store.activeTrajectRef).toBeNull();

    route.params.trajectRef = 'testtraject-abcd1234';
    expect(store.activeTrajectRef).toBe('testtraject-abcd1234');
    expect(store.activeTraject?.name).toBe('Testtraject');
    expect(store.trajectMissing).toBe(false);
  });

  it('reports a traject the user has no access to as missing, once the list is in', async () => {
    const store = useTrajectsStore();
    route.params.trajectRef = 'onbekend-99998888';
    expect(store.trajectMissing).toBe(false); // still loading
    await store.ensureReady();

    expect(store.activeTraject).toBeNull();
    expect(store.trajectMissing).toBe(true);
  });

  it('loads the list once, however often it is asked for', async () => {
    const store = useTrajectsStore();
    await Promise.all([store.ensureReady(), store.ensureReady()]);
    expect(fetch).toHaveBeenCalledTimes(1);

    await store.refreshTrajects();
    expect(fetch).toHaveBeenCalledTimes(2);
  });
});
