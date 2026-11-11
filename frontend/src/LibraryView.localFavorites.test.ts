// Favorites without an account: kept in this browser in Corpus juris, marked
// as such in the sidebar with a way to an account, and moved to the account
// at the first load after login. Mounts LibraryView `shallow` with the
// composable stubs from test-utils/libraryViewHarness.ts.
import { describe, it, expect, vi, beforeEach } from 'vitest';
import { shallowMount, type VueWrapper } from '@vue/test-utils';
import { nextTick, ref } from 'vue';
import * as harness from './test-utils/libraryViewHarness';
import { installTestCorpus } from './test-utils/corpusSetup';
import { LOCAL_FAVORITES_KEY } from './composables/useLocalFavorites';

const routeState: { name: string; params: Record<string, string>; query: Record<string, string>; fullPath: string; hash: string } = {
  name: 'corpus-juris',
  params: { lawId: 'wet_a' },
  query: {},
  fullPath: '/corpus-juris/wet_a',
  hash: '',
};
vi.mock('vue-router', () => harness.vueRouter(() => routeState));

// The login state, with a /auth/status answer a test can hold back: while
// `authLoading` is true, `ensureAuthReady` waits for `auth.answer()`.
const authenticated = ref(false);
const authLoading = ref(false);
const auth = vi.hoisted(() => ({ ready: Promise.resolve(), answer: () => {} }));
vi.mock('./composables/useAuth', () => ({
  useAuth: () => ({ authenticated, loading: authLoading, login: vi.fn() }),
  ensureAuthReady: () => auth.ready,
}));

const activeTrajectRef = ref<string | null>(null);
vi.mock('./composables/useTrajects', () =>
  harness.trajectsModule(() => ({ activeTrajectRef, activeTraject: ref({ name: 'Traject A' }) })),
);

// Every favorites request, in order: method and url. The account's favorites
// answer with what the PUTs stored. Corpus reads go to the in-memory corpus.
const server = vi.hoisted(() => ({ calls: [] as string[], account: new Set<string>() }));
vi.mock('./lib/apiFetch', () =>
  harness.apiFetchModule({
    apiFetch: vi.fn(async (url: string, init?: { method?: string }) => {
      const method = init?.method ?? 'GET';
      server.calls.push(`${method} ${url}`);
      const fav = /^\/api\/favorites\/([^/?]+)$/.exec(url);
      if (fav && method === 'PUT') server.account.add(decodeURIComponent(fav[1]!));
      return { ok: true, status: 200, json: async () => [] };
    }),
    apiFetchJson: vi.fn(async (url: string) => {
      server.calls.push(`GET ${url}`);
      if (url === '/api/favorites') return [...server.account];
      return [];
    }),
  }),
);
vi.mock('./composables/useFeatureFlags', () => harness.featureFlags());
vi.mock('./composables/useDocumentsManager', () => harness.documentsManager());
vi.mock('./composables/useTrajectDocumentJobs', () => harness.trajectDocumentJobs());
vi.mock('./composables/useDocumentUpload', () => harness.documentUpload());
vi.mock('./composables/useDocumentTaskReview', () => harness.documentTaskReview());

import LibraryView from './LibraryView.vue';

// An entry per law, so a favorite resolves to a sidebar row.
function mountLibrary() {
  const content = {
    laws: { wet_a: '$id: wet_a\narticles: []\n' },
    list: ['wet_a', 'wet_b'].map((law_id) => ({ law_id, name: law_id })),
  };
  installTestCorpus({ '': content, 'traject-aaaa1111': content }, activeTrajectRef.value);
  return shallowMount(LibraryView, { global: { stubs: { teleport: true } } }) as VueWrapper;
}

// The login path chains several awaits (the saves, then the account's list,
// then the sidebar's entries), so let the queued tasks run out.
async function settle() {
  for (let i = 0; i < 5; i++) {
    await new Promise((r) => setTimeout(r, 0));
    await nextTick();
  }
}

const star = (wrapper: VueWrapper) =>
  wrapper.findAll('nldd-icon-button').find((b) => (b.attributes('icon') ?? '').startsWith('star'));
const stored = () => JSON.parse(localStorage.getItem(LOCAL_FAVORITES_KEY) ?? '[]') as string[];

beforeEach(() => {
  localStorage.clear();
  authenticated.value = false;
  authLoading.value = false;
  auth.ready = Promise.resolve();
  activeTrajectRef.value = null;
  routeState.params = { lawId: 'wet_a' };
  server.calls = [];
  server.account = new Set();
});

describe('LibraryView favorites without an account', () => {
  it('stars a law in this browser, and marks the section as local with a way to an account', async () => {
    const wrapper = mountLibrary();
    await settle();
    expect(wrapper.find('[data-testid="local-favorites-notice"]').exists()).toBe(false);

    const button = star(wrapper);
    expect(button?.attributes('text')).toBe('Voeg toe aan favorieten in deze browser');
    await button!.trigger('click');
    await settle();

    expect(stored()).toEqual(['wet_a']);
    expect(server.calls.some((c) => c.startsWith('PUT'))).toBe(false);
    const notice = wrapper.get('[data-testid="local-favorites-notice"]');
    expect(notice.attributes('text')).toBe('Alleen in deze browser');
    expect(notice.get('nldd-button').attributes('text')).toBe('Account aanvragen');
    expect(wrapper.find('nldd-list-item[data-section="favorites"][data-law-id="wet_a"]').exists()).toBe(true);
  });

  it('offers no star inside a traject, where favorites need an account', async () => {
    activeTrajectRef.value = 'traject-aaaa1111';
    routeState.params = { trajectRef: 'traject-aaaa1111', lawId: 'wet_a' };
    const wrapper = mountLibrary();
    await settle();
    expect(star(wrapper)).toBeUndefined();
  });

  it('waits for the login state before it shows or moves anything', async () => {
    localStorage.setItem(LOCAL_FAVORITES_KEY, JSON.stringify(['wet_a']));
    // Logged in, but /auth/status has not answered yet: `authenticated` reads false.
    authLoading.value = true;
    auth.ready = new Promise<void>((resolve) => {
      auth.answer = () => {
        authenticated.value = true;
        authLoading.value = false;
        resolve();
      };
    });
    const wrapper = mountLibrary();
    await settle();

    expect(star(wrapper)).toBeUndefined();
    expect(wrapper.find('[data-testid="local-favorites-notice"]').exists()).toBe(false);
    expect(wrapper.find('nldd-list-item[data-section="favorites"]').exists()).toBe(false);
    expect(server.calls.some((c) => c.startsWith('PUT'))).toBe(false);

    auth.answer();
    await settle();

    expect(server.calls).toContain('PUT /api/favorites/wet_a');
    expect(localStorage.getItem(LOCAL_FAVORITES_KEY)).toBeNull();
    expect(wrapper.find('[data-testid="local-favorites-notice"]').exists()).toBe(false);
    expect(star(wrapper)?.attributes('text')).toBe('Verwijder uit favorieten');
  });

  it('moves the browser favorites to the account at the first load after login', async () => {
    localStorage.setItem(LOCAL_FAVORITES_KEY, JSON.stringify(['wet_a', 'wet_b']));
    authenticated.value = true;
    const wrapper = mountLibrary();
    await settle();

    const puts = server.calls.filter((c) => c.startsWith('PUT'));
    expect(puts.sort()).toEqual(['PUT /api/favorites/wet_a', 'PUT /api/favorites/wet_b']);
    // Saved before the account's list is read, so that list already has them.
    const firstList = server.calls.indexOf('GET /api/favorites');
    expect(server.calls.findIndex((c) => c.startsWith('PUT'))).toBeLessThan(firstList);
    expect(localStorage.getItem(LOCAL_FAVORITES_KEY)).toBeNull();
    expect(wrapper.find('nldd-list-item[data-section="favorites"][data-law-id="wet_b"]').exists()).toBe(true);
    expect(wrapper.find('[data-testid="local-favorites-notice"]').exists()).toBe(false);
    expect(star(wrapper)?.attributes('text')).toBe('Verwijder uit favorieten');
  });
});
