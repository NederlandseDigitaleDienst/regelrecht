// `?law_id=` opens the jobs sheet for that law and filters on it. A repeated
// key arrives as an array; the view takes the first value.
import { describe, it, expect, vi, beforeEach } from 'vitest';
import { shallowMount } from '@vue/test-utils';
import { ref, reactive } from 'vue';
import type { LocationQuery } from 'vue-router';
import JobsView from './JobsView.vue';
import { useLawJobsSheet } from '../composables/useLawJobsSheet';

const routeState: { query: LocationQuery } = { query: {} };

vi.mock('vue-router', () => ({
  useRoute: () => routeState,
  useRouter: () => ({ replace: vi.fn() }),
}));

const setLawIdFilter = vi.fn();
vi.mock('../composables/useJobs', () => ({
  useJobs: () => ({
    data: ref([]), totalCount: ref(0), loading: ref(false), error: ref(null),
    sort: ref('created_at'), order: ref('desc'), filters: reactive({}),
    viewMode: ref('grouped'),
    currentPage: ref(1), totalPages: ref(1),
    setSort: vi.fn(), setFilter: vi.fn(), goToPage: vi.fn(),
    toggleViewMode: vi.fn(), setLawIdFilter,
    refresh: vi.fn(),
  }),
}));

vi.mock('../composables/useNewHarvestJob', () => ({
  useNewHarvestJob: () => ({ lastJobCreated: ref(null), open: vi.fn() }),
}));

describe('JobsView - ?law_id=', () => {
  beforeEach(() => {
    setLawIdFilter.mockReset();
    useLawJobsSheet().close();
  });

  it('opens the sheet for the first law of a repeated ?law_id=', () => {
    routeState.query = { law_id: ['BWBR0001', 'BWBR0002'] };
    shallowMount(JobsView);
    expect(setLawIdFilter).toHaveBeenCalledWith('BWBR0001');
    expect(useLawJobsSheet().lawId.value).toBe('BWBR0001');
    expect(useLawJobsSheet().isOpen.value).toBe(true);
  });

  it('opens nothing for a valueless ?law_id', () => {
    routeState.query = { law_id: null };
    shallowMount(JobsView);
    expect(setLawIdFilter).not.toHaveBeenCalled();
    expect(useLawJobsSheet().isOpen.value).toBe(false);
  });
});
