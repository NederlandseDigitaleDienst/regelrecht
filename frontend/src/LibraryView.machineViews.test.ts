// Corpus juris shows a law's machine and YAML views only when its current
// version is enriched, labelled as an example; a text-only law says when
// another version has a machine version. A traject keeps all views. Tests the
// decisions LibraryView makes (the rendering is plain v-if on them).
import { describe, it, expect, vi, beforeEach } from 'vitest';
import { shallowMount, type VueWrapper } from '@vue/test-utils';
import { nextTick, ref } from 'vue';
import * as harness from './test-utils/libraryViewHarness';
import { installTestCorpus } from './test-utils/corpusSetup';
import type { CorpusLaw } from './corpus';

interface Internals {
  showMachineViews: boolean;
  shownDetailView: string;
  otherEnrichedVersion: string | null;
  exampleSupportingText: string;
}
type Wrapper = Omit<VueWrapper, 'vm'> & { vm: VueWrapper['vm'] & Internals };

const routeState: { name: string; params: Record<string, string>; query: Record<string, string>; fullPath: string; hash: string } = {
  name: 'corpus-juris',
  params: { lawId: 'wet_a' },
  query: {},
  fullPath: '/corpus-juris/wet_a#machine',
  hash: '#machine',
};
vi.mock('vue-router', () => harness.vueRouter(() => routeState));

// Logged in. Favorites without a login: LibraryView.localFavorites.test.ts.
const authenticated = ref(true);
vi.mock('./composables/useAuth', () => harness.auth(() => ({ authenticated })));

// No traject: Corpus juris. A test that needs a traject sets this ref.
const activeTrajectRef = ref<string | null>(null);
vi.mock('./composables/useTrajects', () =>
  harness.trajectsModule(() => ({ activeTrajectRef, activeTraject: ref({ name: 'Traject A' }) })),
);

// The open law's list entry: what the test sets here is what `?ids=` answers.
// A minimal law body, so the law pane (and its toolbar with the star) renders.
const entries = { list: [] as CorpusLaw[] };
vi.mock('./lib/apiFetch', () => harness.apiFetchModule());

vi.mock('./composables/useDocumentsManager', () => harness.documentsManager());
vi.mock('./composables/useTrajectDocumentJobs', () => harness.trajectDocumentJobs());
vi.mock('./composables/useDocumentUpload', () => harness.documentUpload());
vi.mock('./composables/useDocumentTaskReview', () => harness.documentTaskReview());

import LibraryView from './LibraryView.vue';

// The same corpus globally and in the traject; the route decides which is read.
function mountLibrary() {
  const content = { laws: { wet_a: '$id: wet_a\narticles: []\n' }, list: entries.list };
  installTestCorpus({ '': content, 'traject-aaaa1111': content }, activeTrajectRef.value);
  return shallowMount(LibraryView, { global: { stubs: { teleport: true } } }) as unknown as Wrapper;
}

async function settle() {
  for (let i = 0; i < 6; i++) await nextTick();
}

beforeEach(() => {
  authenticated.value = true;
  activeTrajectRef.value = null;
  routeState.params = { lawId: 'wet_a' };
  entries.list = [];
});

describe('LibraryView machine views in Corpus juris', () => {
  it('shows only the text of a law that is not enriched, even on a #machine link', async () => {
    entries.list = [{ law_id: 'wet_a', enriched: false }];
    const wrapper = mountLibrary();
    await settle();
    expect(wrapper.vm.showMachineViews).toBe(false);
    expect(wrapper.vm.shownDetailView).toBe('tekst');
  });

  it('shows the machine view of an enriched law, naming where it comes from', async () => {
    entries.list = [{ law_id: 'wet_a', enriched: true, source_name: 'Voorbeelden' }];
    const wrapper = mountLibrary();
    await settle();
    expect(wrapper.vm.showMachineViews).toBe(true);
    expect(wrapper.vm.shownDetailView).toBe('machine');
    expect(wrapper.vm.exampleSupportingText).toContain('Voorbeelden');
    expect(wrapper.vm.exampleSupportingText).toContain('niet vastgesteld');
  });

  it('points to another version that has a machine version', async () => {
    entries.list = [{ law_id: 'wet_a', enriched: false, enriched_version: '2025-01-01' }];
    const wrapper = mountLibrary();
    await settle();
    expect(wrapper.vm.otherEnrichedVersion).toBe('1 januari 2025');
  });

  it('treats a law it could not look up as text only', async () => {
    entries.list = [];
    const wrapper = mountLibrary();
    await settle();
    expect(wrapper.vm.showMachineViews).toBe(false);
  });

  it('keeps every view inside a traject', async () => {
    // An entry that would raise the other-version note in Corpus juris: inside
    // a traject the note stays away.
    entries.list = [{ law_id: 'wet_a', enriched: false, enriched_version: '2025-01-01' }];
    activeTrajectRef.value = 'traject-aaaa1111';
    routeState.params = { trajectRef: 'traject-aaaa1111', lawId: 'wet_a' };
    const wrapper = mountLibrary();
    await settle();
    expect(wrapper.vm.showMachineViews).toBe(true);
    expect(wrapper.vm.otherEnrichedVersion).toBeNull();
  });
});

describe('LibraryView favorite button', () => {
  const star = (wrapper: Wrapper) =>
    wrapper.findAll('nldd-icon-button').filter((b) => b.attributes('text') === 'Voeg toe aan favorieten');

  it('offers the star when logged in', async () => {
    const wrapper = mountLibrary();
    await settle();
    expect(star(wrapper)).toHaveLength(1);
  });
});
