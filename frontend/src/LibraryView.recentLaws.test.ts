// "Recent bekeken" is stored per traject: two trajects hold different laws, so
// a law opened in the one must not turn up under the other, where that id may
// not even resolve. Mounts LibraryView `shallow` with the composable stubs from
// test-utils/libraryViewHarness.ts - deliberately narrow, this file only
// exercises the recent-laws surface.
import { describe, it, expect, vi, beforeEach } from 'vitest';
import { shallowMount, type VueWrapper } from '@vue/test-utils';
import { ref } from 'vue';
import * as harness from './test-utils/libraryViewHarness';

/** A "Recent bekeken" entry as LibraryView stores it. */
interface RecentLawEntry {
  law_id: string;
  name: string;
}

/**
 * Setup state the tests reach through `wrapper.vm`. LibraryView exposes
 * nothing, but test-utils proxies the setup state onto `vm` (refs unwrapped,
 * and writable).
 */
interface LibraryViewInternals {
  recordRecentLaw(lawId: string, name: string): void;
  clearRecent(): void;
  recentLaws: RecentLawEntry[];
  laws: { law_id: string; name?: string | null }[];
  selectedLawId: string | null;
}
type LibraryWrapper = Omit<VueWrapper, 'vm'> & { vm: VueWrapper['vm'] & LibraryViewInternals };

const routeState: { name: string; params: Record<string, string>; query: Record<string, string>; fullPath: string; hash: string } = {
  name: 'library-traject',
  params: { trajectRef: 'traject-aaaa1111', docPath: '' },
  query: {},
  fullPath: '/traject/traject-aaaa1111/bibliotheek',
  hash: '',
};
vi.mock('vue-router', () => harness.vueRouter(() => routeState));
vi.mock('./composables/useAuth', () => harness.auth());

// The ref the view watches: switching traject keeps the view mounted, so the
// test drives the switch by writing to this exact ref.
const activeTrajectRef = ref<string | null>('traject-aaaa1111');
vi.mock('./composables/useTrajects', () =>
  harness.trajectsModule(() => ({ activeTrajectRef, activeTraject: ref({ name: 'Traject A' }) })),
);

vi.mock('./lib/apiFetch', () => harness.apiFetchModule());
vi.mock('./composables/useFeatureFlags', () => harness.featureFlags());
vi.mock('./composables/useDocumentsManager', () => harness.documentsManager());
vi.mock('./composables/useTrajectDocumentJobs', () => harness.trajectDocumentJobs());
vi.mock('./composables/useDocumentUpload', () => harness.documentUpload());
vi.mock('./composables/useDocumentTaskReview', () => harness.documentTaskReview());

import LibraryView from './LibraryView.vue';

const KEY = 'regelrecht-recent-laws';
const mountLibrary = () =>
  shallowMount(LibraryView, { global: { stubs: { teleport: true } } }) as unknown as LibraryWrapper;
const stored = (trajectRef: string): RecentLawEntry[] => JSON.parse(localStorage.getItem(`${KEY}:${trajectRef}`) || '[]');

beforeEach(() => {
  localStorage.clear();
  activeTrajectRef.value = 'traject-aaaa1111';
  routeState.params = { trajectRef: 'traject-aaaa1111', docPath: '' };
});

describe('LibraryView recent bekeken', () => {
  it('bewaart een bekeken wet onder de sleutel van het actieve traject', async () => {
    const wrapper = mountLibrary();
    wrapper.vm.recordRecentLaw('wet_a', 'Wet A');
    await wrapper.vm.$nextTick();
    expect(stored('traject-aaaa1111').map(r => r.law_id)).toEqual(['wet_a']);
    expect(localStorage.getItem(KEY)).toBeNull();
  });

  it('toont de lijst van het andere traject na een wissel, niet die van het eerste', async () => {
    localStorage.setItem(`${KEY}:traject-bbbb2222`, JSON.stringify([{ law_id: 'wet_b', name: 'Wet B' }]));
    const wrapper = mountLibrary();
    wrapper.vm.recordRecentLaw('wet_a', 'Wet A');
    await wrapper.vm.$nextTick();

    activeTrajectRef.value = 'traject-bbbb2222';
    await wrapper.vm.$nextTick();

    expect(wrapper.vm.recentLaws.map(r => r.law_id)).toEqual(['wet_b']);
    // En het eerste traject houdt zijn eigen lijst.
    expect(stored('traject-aaaa1111').map(r => r.law_id)).toEqual(['wet_a']);
  });

  it('houdt de globale corpus apart van een traject', async () => {
    const wrapper = mountLibrary();
    wrapper.vm.recordRecentLaw('wet_a', 'Wet A');
    await wrapper.vm.$nextTick();

    activeTrajectRef.value = null;
    await wrapper.vm.$nextTick();
    expect(wrapper.vm.recentLaws).toEqual([]);

    wrapper.vm.recordRecentLaw('wet_c', 'Wet C');
    await wrapper.vm.$nextTick();
    expect(stored('corpus').map(r => r.law_id)).toEqual(['wet_c']);
    expect(stored('traject-aaaa1111').map(r => r.law_id)).toEqual(['wet_a']);
  });

  it('wist bij "wis recent" alleen het actieve traject', async () => {
    localStorage.setItem(`${KEY}:traject-bbbb2222`, JSON.stringify([{ law_id: 'wet_b', name: 'Wet B' }]));
    const wrapper = mountLibrary();
    wrapper.vm.recordRecentLaw('wet_a', 'Wet A');
    await wrapper.vm.$nextTick();

    wrapper.vm.clearRecent();
    await wrapper.vm.$nextTick();

    expect(wrapper.vm.recentLaws).toEqual([]);
    expect(localStorage.getItem(`${KEY}:traject-aaaa1111`)).toBeNull();
    expect(stored('traject-bbbb2222').map(r => r.law_id)).toEqual(['wet_b']);
  });

  // De wet blijft bij een wissel gewoon openstaan, en de naam wordt opnieuw
  // opgelost, dus de watch die de lijst bijhoudt vuurt nog een keer. Dat is een
  // wissel en geen bezoek.
  it('schrijft de openstaande wet niet in de lijst van het traject waar je heen wisselt', async () => {
    const wrapper = mountLibrary();
    wrapper.vm.laws = [{ law_id: 'wet_a', name: 'Wet A in traject' }];
    wrapper.vm.selectedLawId = 'wet_a';
    await wrapper.vm.$nextTick();
    expect(stored('traject-aaaa1111').map(r => r.law_id)).toEqual(['wet_a']);

    // Wat de wissel in het echt doet: de corpus laadt opnieuw, dus de naam van
    // de nog openstaande wet lost opnieuw op en de watch vuurt.
    activeTrajectRef.value = null;
    wrapper.vm.laws = [{ law_id: 'wet_a', name: 'Wet A in de corpus' }];
    await wrapper.vm.$nextTick();
    await wrapper.vm.$nextTick();

    expect(stored('corpus')).toEqual([]);
    expect(wrapper.vm.recentLaws).toEqual([]);
  });

  it('gooit de oude ongescopete lijst weg', async () => {
    localStorage.setItem(KEY, JSON.stringify([{ law_id: 'wet_oud', name: 'Oud' }]));
    const wrapper = mountLibrary();
    await wrapper.vm.$nextTick();
    expect(localStorage.getItem(KEY)).toBeNull();
    expect(wrapper.vm.recentLaws).toEqual([]);
  });
});
