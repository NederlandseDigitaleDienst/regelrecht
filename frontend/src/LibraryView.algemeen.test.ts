// The Algemeen page of a law: an overview with where the law comes from (its
// published text and its folder on GitHub), and a YAML tab with the whole file.
// Mounts LibraryView `shallow` with the composable stubs from
// test-utils/libraryViewHarness.ts.
import { describe, it, expect, vi, beforeEach } from 'vitest';
import { shallowMount, type VueWrapper } from '@vue/test-utils';
import { nextTick, ref } from 'vue';
import * as harness from './test-utils/libraryViewHarness';

const routeState: { name: string; params: Record<string, string>; query: Record<string, string>; fullPath: string; hash: string } = {
  name: 'corpus-juris',
  params: { lawId: 'wet_a', articleNumber: 'algemeen' },
  query: {},
  fullPath: '/corpus-juris/wet_a/algemeen',
  hash: '',
};
vi.mock('vue-router', () => harness.vueRouter(() => routeState));
vi.mock('./composables/useAuth', () => harness.auth());

const activeTrajectRef = ref<string | null>(null);
vi.mock('./composables/useTrajects', () =>
  harness.trajectsModule(() => ({ activeTrajectRef, activeTraject: ref({ name: 'Traject A' }) })),
);

// What the server sends: the law's file, and its list entry for `?ids=`.
const server = vi.hoisted(() => ({
  lawText: '',
  entry: {} as Record<string, unknown>,
  listUrls: [] as string[],
}));
vi.mock('./lib/apiFetch', () =>
  harness.apiFetchModule({
    apiFetch: vi.fn(async () => ({ ok: true, status: 200, json: async () => [], text: async () => server.lawText })),
    apiFetchJson: vi.fn(async (url: string) => {
      if (!url.includes('ids=')) return [];
      server.listUrls.push(url);
      return [server.entry];
    }),
  }),
);
vi.mock('./composables/useFeatureFlags', () => harness.featureFlags());
vi.mock('./composables/useDocumentsManager', () => harness.documentsManager());
vi.mock('./composables/useTrajectDocumentJobs', () => harness.trajectDocumentJobs());
vi.mock('./composables/useDocumentUpload', () => harness.documentUpload());
vi.mock('./composables/useDocumentTaskReview', () => harness.documentTaskReview());

import LibraryView from './LibraryView.vue';

const mountLibrary = () =>
  shallowMount(LibraryView, { global: { stubs: { teleport: true } } }) as VueWrapper;

async function settle() {
  for (let i = 0; i < 6; i++) await nextTick();
}

const LAW_TEXT = [
  '$id: wet_a',
  'bwb_id: BWBR0000001',
  'url: https://wetten.overheid.nl/BWBR0000001/2025-01-01',
  'articles: []',
  '',
].join('\n');
const FOLDER = 'https://github.com/example-org/central-corpus/tree/main/regulation/nl/wet/wet_a';

beforeEach(() => {
  activeTrajectRef.value = null;
  routeState.params = { lawId: 'wet_a', articleNumber: 'algemeen' };
  routeState.hash = '';
  server.lawText = LAW_TEXT;
  server.entry = { law_id: 'wet_a', source_url: FOLDER };
  server.listUrls = [];
});

describe('LibraryView Algemeen', () => {
  it('links to the published text and to the folder on GitHub', async () => {
    const wrapper = mountLibrary();
    await settle();
    expect(wrapper.get('[data-testid="algemeen-link-official"]').attributes('href')).toBe(
      'https://wetten.overheid.nl/BWBR0000001/2025-01-01',
    );
    expect(wrapper.get('[data-testid="algemeen-link-official"] nldd-text-cell').attributes('text')).toBe(
      'wetten.overheid.nl',
    );
    expect(wrapper.get('[data-testid="algemeen-link-github"]').attributes('href')).toBe(FOLDER);
    expect(wrapper.find('[data-testid="algemeen-yaml"]').exists()).toBe(false);
  });

  it('falls back to the BWB id without a url, and leaves GitHub out for a local law', async () => {
    server.lawText = '$id: wet_a\nbwb_id: BWBR0000001\narticles: []\n';
    server.entry = { law_id: 'wet_a' };
    const wrapper = mountLibrary();
    await settle();
    expect(wrapper.get('[data-testid="algemeen-link-official"]').attributes('href')).toBe(
      'https://wetten.overheid.nl/BWBR0000001',
    );
    expect(wrapper.find('[data-testid="algemeen-link-github"]').exists()).toBe(false);
  });

  it('never turns a url that is not a web address into a link', async () => {
    server.lawText = "$id: wet_a\nbwb_id: BWBR0000001\nurl: 'javascript:alert(1)'\narticles: []\n";
    const wrapper = mountLibrary();
    await settle();
    expect(wrapper.get('[data-testid="algemeen-link-official"]').attributes('href')).toBe(
      'https://wetten.overheid.nl/BWBR0000001',
    );
  });

  it('shows the whole file on the YAML tab', async () => {
    routeState.hash = '#yaml';
    const wrapper = mountLibrary();
    await settle();
    expect(wrapper.get('[data-testid="algemeen-yaml"]').text()).toBe(LAW_TEXT.trim());
  });

  it('asks the traject for the GitHub folder inside a traject', async () => {
    activeTrajectRef.value = 'traject-aaaa1111';
    routeState.params = { trajectRef: 'traject-aaaa1111', lawId: 'wet_a', articleNumber: 'algemeen' };
    const wrapper = mountLibrary();
    await settle();
    expect(server.listUrls.some((u) => u.includes('/trajects/traject-aaaa1111/'))).toBe(true);
    expect(wrapper.get('[data-testid="algemeen-link-github"]').attributes('href')).toBe(FOLDER);
  });
});
