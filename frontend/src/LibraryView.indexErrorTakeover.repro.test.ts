// Guards the fix for the 2026-07-29 bug report "taakmenu crasht na ophalen
// wetgeving in private repo": when the traject-scoped corpus listing fails
// (502 because the traject's writable-own repo could not be scanned),
// LibraryView's top-level index-error pane used to take over the WHOLE route -
// including /taken and /instellingen/details, which do not read the corpus
// index at all. After the fix the takeover is gated to library mode; the
// non-library modes keep their panes and get a non-blocking warning banner,
// and the backend's own explanation is surfaced instead of a generic fallback.
//
// Harness: test-utils/libraryViewHarness.ts, shallow-mount with every
// composable stubbed, deliberately narrow to this one surface.
import { describe, it, expect, vi, beforeEach } from 'vitest';
import { shallowMount } from '@vue/test-utils';
import { ref, nextTick } from 'vue';
import * as harness from './test-utils/libraryViewHarness';

/** The route fields LibraryView reads; tests reassign params freely. */
interface RouteStub {
  name: string;
  params: Record<string, string>;
  query: Record<string, string>;
  fullPath: string;
}

const routeState: RouteStub = {
  name: 'taken-traject',
  params: { trajectRef: 'traject-abcd1234', categorie: 'alle' },
  query: {},
  fullPath: '/trajecten/traject-abcd1234/taken',
};
vi.mock('vue-router', () => harness.vueRouter(() => routeState));
vi.mock('./composables/useAuth', () => harness.auth());

// The ref the view watches: the traject switcher leaves LibraryView mounted
// and only swaps the route param, which is what LibraryView's
// `watch(activeTrajectRef)` reacts to, so a test switches traject in-place by
// writing to this exact ref.
const activeTrajectRef = ref('traject-abcd1234');
vi.mock('./composables/useTrajects', () =>
  harness.trajectsModule(() => ({
    activeTrajectRef,
    activeTraject: ref({ name: 'Afgeschermde test Kandidaatstellingsprocedure Kieswet' }),
  })),
);

// The 502 body below is the backend's own text for a traject whose
// writable-own (private) repo could not be scanned - see
// `require_traject_index` in corpus_handlers.rs.
//
// `vi.mock` factories are hoisted above module-level consts, so the shared
// stubs have to be hoisted too.
const { ApiErrorStub, apiFetch, apiFetchJson } = vi.hoisted(() => {
  class ApiErrorStub extends Error {
    status: number | undefined;
    body: string;
    constructor(message: string, { status, body = '' }: { status?: number; body?: string } = {}) {
      super(message);
      this.name = 'ApiError';
      this.status = status;
      this.body = body;
    }
  }
  const BODY =
    'De bibliotheek van dit traject is niet beschikbaar: de traject-repo kon niet ' +
    'worden gescand (Trees API for org/afgeschermd-traject@main: 409)';
  const fail = () => {
    throw new ApiErrorStub('Failed to load corpus: 502', { status: 502, body: BODY });
  };
  // Every traject-scoped corpus call fails; favorites/changed-laws resolve
  // (they swallow their own errors anyway, so failing them would prove nothing
  // about the takeover).
  return {
    ApiErrorStub,
    apiFetch: vi.fn(async (url: unknown, _init?: unknown) => {
      if (String(url).includes('/corpus/laws')) fail();
      return { ok: true, status: 200, json: async () => [] };
    }),
    apiFetchJson: vi.fn(async (url: unknown, _init?: unknown) => {
      if (String(url).includes('/corpus/laws')) fail();
      return [];
    }),
  };
});

vi.mock('./lib/apiFetch', () =>
  harness.apiFetchModule({
    apiFetch: (...a: Parameters<typeof apiFetch>) => apiFetch(...a),
    apiFetchJson: (...a: Parameters<typeof apiFetchJson>) => apiFetchJson(...a),
    ApiError: ApiErrorStub,
  }),
);
vi.mock('./composables/useFeatureFlags', () => harness.featureFlags());
vi.mock('./composables/useDocumentsManager', () => harness.documentsManager());
vi.mock('./composables/useTrajectDocumentJobs', () => harness.trajectDocumentJobs());
vi.mock('./composables/useDocumentUpload', () => harness.documentUpload());
vi.mock('./composables/useDocumentTaskReview', () => harness.documentTaskReview());

import LibraryView from './LibraryView.vue';

async function mountAfterFailedIndex() {
  const wrapper = shallowMount(LibraryView, { global: { stubs: { teleport: true } } });
  await nextTick();
  await nextTick();
  await nextTick();
  return wrapper;
}

// vi.fn(impl) always has an implementation.
const defaultApiFetch = apiFetch.getMockImplementation()!;

beforeEach(() => {
  apiFetch.mockClear().mockImplementation(defaultApiFetch);
  apiFetchJson.mockClear();
  routeState.name = 'taken-traject';
  routeState.params = { trajectRef: 'traject-abcd1234', categorie: 'alle' };
  routeState.query = {};
  activeTrajectRef.value = 'traject-abcd1234';
});

// The warning banner LibraryView now raises over the non-library modes.
const NOTIFICATION_TEXT = 'De regelwerken van dit traject zijn niet geladen';
// The fullscreen takeover that still owns the library routes themselves.
const FULLSCREEN_TEXT = 'De regelwerken zijn niet geladen';

describe('LibraryView index-error scoping', () => {
  it('probes the traject corpus even with nothing curated (fresh private repo)', async () => {
    await mountAfterFailedIndex();
    // No favorites, no changed laws => the ids set is empty, but LibraryView
    // still probes `?limit=1` so a broken traject repo is not shown as a
    // normal-looking empty library.
    expect(apiFetch).toHaveBeenCalledWith(
      '/api/trajects/traject-abcd1234/corpus/laws?limit=1',
      expect.anything(),
    );
  });

  it('keeps the TAKEN route standing and shows a non-blocking warning banner', async () => {
    const wrapper = await mountAfterFailedIndex();
    // The task panes survive the unrelated 502 - the user can still work.
    expect(wrapper.findComponent({ name: 'TasksCategoriesPane' }).exists()).toBe(true);
    expect(wrapper.findComponent({ name: 'TasksListPane' }).exists()).toBe(true);
    const html = wrapper.html();
    // ...with a warning notification instead of the fullscreen takeover.
    expect(html).toContain(NOTIFICATION_TEXT);
    const notification = wrapper.find('nldd-notification');
    expect(notification.exists()).toBe(true);
    expect(notification.attributes('variant')).toBe('warning');
    // Beschrijft een toestand, geen gebeurtenis: hij mag niet wegtellen, want
    // dan verdwijnt de retry-knop met hem mee.
    expect(notification.attributes('duration')).toBe('0');
  });

  it('keeps the taken panes even when the corpus call fails mid-flight (no flash)', async () => {
    let rejectCorpus: (() => void) | undefined;
    apiFetch.mockImplementation(async (url: unknown) => {
      if (String(url).includes('/corpus/laws')) {
        return new Promise((_, rej) => {
          rejectCorpus = () =>
            rej(new ApiErrorStub('Failed to load corpus: 502', { status: 502, body: 'kapot' }));
        });
      }
      return { ok: true, status: 200, json: async () => [] };
    });
    const wrapper = shallowMount(LibraryView, { global: { stubs: { teleport: true } } });
    await nextTick();
    await nextTick();
    // The task list is on screen while the corpus probe is still in flight...
    expect(wrapper.findComponent({ name: 'TasksCategoriesPane' }).exists()).toBe(true);
    // Assigned once the mount reached the corpus probe, two ticks ago.
    rejectCorpus!();
    for (let i = 0; i < 5; i++) await nextTick();
    // ...and it stays put once the unrelated corpus call fails - only the
    // warning banner appears on top.
    expect(wrapper.findComponent({ name: 'TasksCategoriesPane' }).exists()).toBe(true);
    expect(wrapper.html()).toContain(NOTIFICATION_TEXT);
  });

  it('keeps the INSTELLINGEN route standing under a corpus 502', async () => {
    routeState.name = 'instellingen-traject';
    routeState.params = { trajectRef: 'traject-abcd1234', tab: 'details' };
    const wrapper = await mountAfterFailedIndex();
    expect(wrapper.findComponent({ name: 'TrajectDetailsPane' }).exists()).toBe(true);
    expect(wrapper.html()).toContain(NOTIFICATION_TEXT);
    routeState.name = 'taken-traject';
    routeState.params = { trajectRef: 'traject-abcd1234', categorie: 'alle' };
  });

  it('keeps the WERKDOCUMENTEN route standing under a corpus 502', async () => {
    routeState.name = 'werkdocumenten-traject';
    routeState.params = { trajectRef: 'traject-abcd1234' };
    const wrapper = await mountAfterFailedIndex();
    // Werkdocumenten live in the traject repo's docs tree, not in the
    // wettenindex, so the document list has to survive the corpus 502 too.
    expect(wrapper.find('nldd-navigation-split-view').exists()).toBe(true);
    expect(wrapper.html()).not.toContain(FULLSCREEN_TEXT);
    expect(wrapper.html()).toContain(NOTIFICATION_TEXT);
    routeState.name = 'taken-traject';
    routeState.params = { trajectRef: 'traject-abcd1234', categorie: 'alle' };
  });

  // The banner/takeover must track the scope it belongs to. `retryLoadCorpus`
  // clears the error itself, but the traject switcher reaches `loadIndex()`
  // through `watch(activeTrajectRef)` - if only the retry path cleared the
  // error, switching away from a broken traject would carry its error onto a
  // perfectly healthy one.
  it('drops the error when switching to a healthy traject (banner does not stick)', async () => {
    const wrapper = await mountAfterFailedIndex();
    expect(wrapper.html()).toContain(NOTIFICATION_TEXT);

    // Same view, other traject - and this one indexes fine.
    apiFetch.mockImplementation(async () => ({ ok: true, status: 200, json: async () => [] }));
    routeState.params = { trajectRef: 'traject-99998888', categorie: 'alle' };
    activeTrajectRef.value = 'traject-99998888';
    for (let i = 0; i < 6; i++) await nextTick();

    expect(wrapper.html()).not.toContain(NOTIFICATION_TEXT);
    expect(wrapper.find('nldd-notification').exists()).toBe(false);
    expect(wrapper.findComponent({ name: 'TasksCategoriesPane' }).exists()).toBe(true);
  });

  it('drops the fullscreen takeover when switching to a healthy traject', async () => {
    routeState.name = 'library-traject';
    routeState.params = { trajectRef: 'traject-abcd1234' };
    const wrapper = await mountAfterFailedIndex();
    expect(wrapper.html()).toContain(FULLSCREEN_TEXT);

    apiFetch.mockImplementation(async () => ({ ok: true, status: 200, json: async () => [] }));
    routeState.params = { trajectRef: 'traject-99998888' };
    activeTrajectRef.value = 'traject-99998888';
    for (let i = 0; i < 6; i++) await nextTick();

    expect(wrapper.html()).not.toContain(FULLSCREEN_TEXT);
    routeState.name = 'taken-traject';
    routeState.params = { trajectRef: 'traject-abcd1234', categorie: 'alle' };
  });

  it('still shows the fullscreen index-error page on the library route itself', async () => {
    routeState.name = 'library-traject';
    routeState.params = { trajectRef: 'traject-abcd1234' };
    const wrapper = await mountAfterFailedIndex();
    const html = wrapper.html();
    // The library route DOES depend on the index, so it keeps the fullscreen
    // takeover (unchanged) - and does not fall back to the warning banner.
    expect(html).toContain(FULLSCREEN_TEXT);
    expect(wrapper.find('nldd-notification').exists()).toBe(false);
    routeState.name = 'taken-traject';
    routeState.params = { trajectRef: 'traject-abcd1234', categorie: 'alle' };
  });

  it('surfaces the backend explanation while it fits the cap', async () => {
    const wrapper = await mountAfterFailedIndex();
    expect(wrapper.html()).toContain('traject-repo kon niet worden gescand');
  });

  it('still surfaces the backend reason once GitHub\'s JSON body pushes it long', async () => {
    // What a real 409 "Git Repository is empty" looks like once the Trees
    // error body is interpolated into the 502 message. The old 300-char cap
    // dropped this whole sentence to the useless generic fallback (the bug
    // report's screenshots); it must now come through, clamped.
    const githubBody =
      '{"message":"Git Repository is empty.","documentation_url":' +
      '"https://docs.github.com/rest/git/trees/trees#get-a-tree","status":"409"}';
    const long =
      'De bibliotheek van dit traject is niet beschikbaar: de traject-repo kon niet ' +
      `worden gescand (GitHub API error 409: Trees API for example-org/regelrecht-corpus-example@main: ${githubBody})`;
    expect(long.length).toBeGreaterThan(300);
    apiFetch.mockImplementation(async (url: unknown) => {
      if (String(url).includes('/corpus/laws')) {
        throw new ApiErrorStub('Failed to load corpus: 502', { status: 502, body: long });
      }
      return { ok: true, status: 200, json: async () => [] };
    });
    const wrapper = await mountAfterFailedIndex();
    const html = wrapper.html();
    // The reason reaches the user instead of "De gegevens konden niet worden opgehaald."
    expect(html).toContain('traject-repo kon niet worden gescand');
    expect(html).not.toContain('De gegevens konden niet worden opgehaald');
  });
});
