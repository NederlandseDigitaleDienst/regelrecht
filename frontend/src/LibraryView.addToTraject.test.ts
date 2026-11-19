// "Toevoegen aan traject" on the open rulework: inside a traject, a rulework
// that comes from a source the traject only reads along with can be brought
// into the traject from the law view itself.
import { describe, it, expect, vi, beforeEach } from 'vitest';
import { shallowMount, type VueWrapper } from '@vue/test-utils';
import { nextTick, reactive } from 'vue';
import * as harness from './test-utils/libraryViewHarness';
import { installTestCorpus } from './test-utils/corpusSetup';
import type { CorpusLaw } from './corpus';

const TRAJECT = 'traject-aaaa1111';

const routeState: { name: string; params: Record<string, string>; query: Record<string, string>; fullPath: string; hash: string } = reactive({
  name: 'library-traject',
  params: { trajectRef: TRAJECT, lawId: 'wet_a' },
  query: {},
  fullPath: `/trajecten/${TRAJECT}/wet_a`,
  hash: '',
});
vi.mock('vue-router', () => harness.vueRouter(() => routeState));
vi.mock('./composables/useAuth', () => harness.auth());

// The promote call goes through apiFetchJson; everything else answers empty.
const { ApiErrorStub, apiFetchJson, promote } = vi.hoisted(() => {
  class ApiErrorStub extends Error {
    status: number | undefined;
    constructor(message: string, { status }: { status?: number } = {}) {
      super(message);
      this.name = 'ApiError';
      this.status = status;
    }
  }
  const promote = { status: 200, onSuccess: () => {} };
  const apiFetchJson = vi.fn(async (url: unknown, _init?: unknown): Promise<unknown> => {
    if (String(url).endsWith('/promote')) {
      if (promote.status !== 200) {
        throw new ApiErrorStub(`promote failed: ${promote.status}`, { status: promote.status });
      }
      promote.onSuccess();
    }
    return [];
  });
  return { ApiErrorStub, apiFetchJson, promote };
});
vi.mock('./lib/apiFetch', () =>
  harness.apiFetchModule({
    apiFetchJson: (...a: Parameters<typeof apiFetchJson>) => apiFetchJson(...a),
    ApiError: ApiErrorStub,
  }),
);
vi.mock('./composables/useDocumentsManager', () => harness.documentsManager());
vi.mock('./composables/useTrajectDocumentJobs', () => harness.trajectDocumentJobs());
vi.mock('./composables/useDocumentUpload', () => harness.documentUpload());
vi.mock('./composables/useDocumentTaskReview', () => harness.documentTaskReview());

import LibraryView from './LibraryView.vue';

// The open law's list entry: its `source_priority` says where it comes from.
const entries = { list: [] as CorpusLaw[] };

// Reads of the open law's body, and of its list entry (`?ids=`).
const reads = { body: 0, entry: 0 };

function mountLibrary() {
  const content = { laws: { wet_a: '$id: wet_a\narticles: []\n' }, list: entries.list };
  const { transport } = installTestCorpus({ '': content, [TRAJECT]: content }, routeState.params.trajectRef ?? null);
  const { law, list } = transport;
  transport.law = (...args) => {
    reads.body++;
    return law(...args);
  };
  // The answer book as it stands at the time of the read, so a test can move
  // the law to the traject's own repository the way a promote does.
  transport.list = async (trajectRef, query) => {
    if (!query.ids) return list(trajectRef, query);
    reads.entry++;
    const laws = entries.list.filter((l) => query.ids?.includes(l.law_id));
    return { laws, total: laws.length };
  };
  return shallowMount(LibraryView, { global: { stubs: { teleport: true }, plugins: [harness.pinia()] } });
}
async function settle() {
  for (let i = 0; i < 8; i++) await nextTick();
}
const addButton = (wrapper: VueWrapper) => wrapper.find('nldd-icon-button[data-testid="add-open-law-to-traject"]');
const addError = (wrapper: VueWrapper) => wrapper.find('nldd-inline-dialog[data-testid="add-open-law-error"]');
const promoteCalls = () => apiFetchJson.mock.calls.filter(([url]) => String(url).endsWith('/promote'));

beforeEach(() => {
  apiFetchJson.mockClear();
  promote.status = 200;
  // What the backend does on a promote: the law now sits in the own repository.
  promote.onSuccess = () => {
    entries.list = [{ law_id: 'wet_a', source_name: 'Eigen traject-repo', source_priority: 0 }];
  };
  reads.body = 0;
  reads.entry = 0;
  routeState.name = 'library-traject';
  routeState.params = { trajectRef: TRAJECT, lawId: 'wet_a' };
  entries.list = [{ law_id: 'wet_a', source_name: 'Centraal corpus', source_priority: 1 }];
});

describe('LibraryView - toevoegen aan traject vanaf het open regelwerk', () => {
  it('offers the button for a rulework from a source the traject reads along with', async () => {
    const wrapper = mountLibrary();
    await settle();
    expect(addButton(wrapper).attributes('text')).toBe('Toevoegen aan traject');
  });

  it('does not offer it for a rulework in the traject\'s own repository', async () => {
    entries.list = [{ law_id: 'wet_a', source_name: 'Eigen traject-repo', source_priority: 0 }];
    const wrapper = mountLibrary();
    await settle();
    expect(addButton(wrapper).exists()).toBe(false);
  });

  it('does not offer it while the source is unknown', async () => {
    entries.list = [];
    const wrapper = mountLibrary();
    await settle();
    expect(addButton(wrapper).exists()).toBe(false);
  });

  it('does not offer it outside a traject', async () => {
    routeState.name = 'corpus-juris';
    routeState.params = { lawId: 'wet_a' };
    const wrapper = mountLibrary();
    await settle();
    expect(addButton(wrapper).exists()).toBe(false);
  });

  it('promotes the open rulework and then stops offering the button', async () => {
    const wrapper = mountLibrary();
    await settle();
    await addButton(wrapper).trigger('click');
    await settle();

    const calls = promoteCalls();
    expect(calls).toHaveLength(1);
    expect(String(calls[0]![0])).toBe(`/api/trajects/${TRAJECT}/corpus/laws/wet_a/promote`);
    expect(calls[0]![1]).toMatchObject({ method: 'POST' });
    expect(addButton(wrapper).exists()).toBe(false);
    expect(addError(wrapper).exists()).toBe(false);
  });

  it('loads the traject\'s copy and its entry again after adding', async () => {
    const wrapper = mountLibrary();
    await settle();
    const before = { ...reads };
    await addButton(wrapper).trigger('click');
    await settle();

    expect(reads.body).toBe(before.body + 1);
    expect(reads.entry).toBeGreaterThan(before.entry);
  });

  it('offers the button again for the same rulework in another traject', async () => {
    const wrapper = mountLibrary();
    await settle();
    await addButton(wrapper).trigger('click');
    await settle();
    expect(addButton(wrapper).exists()).toBe(false);

    // The other traject reads the law from the central corpus still.
    entries.list = [{ law_id: 'wet_a', source_name: 'Centraal corpus', source_priority: 1 }];
    routeState.params = { trajectRef: 'traject-bbbb2222', lawId: 'wet_a' };
    await settle();
    expect(addButton(wrapper).exists()).toBe(true);
  });

  it('stops offering the button when the backend says the rulework is there already', async () => {
    promote.status = 409;
    // The entry was stale: the law is in the own repository already.
    const wrapper = mountLibrary();
    await settle();
    entries.list = [{ law_id: 'wet_a', source_name: 'Eigen traject-repo', source_priority: 0 }];
    await settle();
    await addButton(wrapper).trigger('click');
    await settle();

    expect(addButton(wrapper).exists()).toBe(false);
    expect(addError(wrapper).exists()).toBe(false);
  });

  it('keeps the button and says so when adding fails', async () => {
    promote.status = 502;
    const wrapper = mountLibrary();
    await settle();
    await addButton(wrapper).trigger('click');
    await settle();

    expect(addButton(wrapper).exists()).toBe(true);
    expect(addError(wrapper).attributes('text')).toContain('Toevoegen aan het traject is mislukt');
  });
});
