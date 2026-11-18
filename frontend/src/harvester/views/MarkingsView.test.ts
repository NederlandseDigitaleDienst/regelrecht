import { shallowMount } from '@vue/test-utils';
import { describe, it, expect, vi } from 'vitest';
import { ref, reactive } from 'vue';
import MarkingsView from './MarkingsView.vue';
import DataTable from '../components/DataTable.vue';
import MarkingDetailPanel from '../components/MarkingDetailPanel.vue';
import MarkingClusters from '../components/MarkingClusters.vue';
import { MARKING_COLUMNS, MARKING_SORT_OPTIONS } from '../constants';

// DataTable is a generic component (`<script setup generic>`), whose props
// test-utils cannot type; read them as a plain record.
function tableProps(table: { props(): unknown }): Record<string, unknown> {
  return table.props() as Record<string, unknown>;
}

const setSort = vi.fn();
const setFilter = vi.fn();
const goToPage = vi.fn();
const filterParams = vi.fn(() => new URLSearchParams());
const refreshClusters = vi.fn();
const setFilters = vi.fn();

vi.mock('../composables/useMarkings', () => ({
  useMarkings: () => ({
    data: ref([
      {
        id: '1',
        law_id: 'test_law',
        article: '5',
        about: 'de eerstvolgende werkdag',
        resolution: 'operation',
        resolved_by: 'een WORKING_DAY-bewerking',
        target: ['datum_van_betaling'],
        accepted: false,
      },
    ]),
    loading: ref(false),
    error: ref(null),
    sort: ref('created_at'),
    order: ref('desc'),
    filters: reactive({}),
    currentPage: ref(1),
    totalPages: ref(1),
    setSort,
    setFilter,
    setFilters,
    goToPage,
    filterParams,
  }),
}));

vi.mock('../composables/useMarkingClusters', () => ({
  useMarkingClusters: () => ({
    refresh: refreshClusters,
    data: ref([
      {
        resolution: 'operation',
        resolved_by: 'een WORKING_DAY-bewerking',
        markings: 3,
        laws: 2,
        articles: 3,
        providers: ['opencode'],
        all_accepted: false,
      },
    ]),
    loading: ref(false),
    error: ref(null),
  }),
}));

describe('MarkingsView', () => {
  it('wires the DataTable with the marking columns and clickable rows', () => {
    const w = shallowMount(MarkingsView);
    const table = w.findComponent(DataTable);
    expect(tableProps(table).columns).toBe(MARKING_COLUMNS);
    expect(tableProps(table).sortOptions).toBe(MARKING_SORT_OPTIONS);
    expect(tableProps(table).clickableRows).toBe(true);
  });

  it('forwards sort and filter events to the composable', () => {
    const w = shallowMount(MarkingsView);
    const table = w.findComponent(DataTable);
    table.vm.$emit('sort', 'resolved_by', 'asc');
    table.vm.$emit('filter-change', 'provider', 'claude');
    expect(setSort).toHaveBeenCalledWith('resolved_by', 'asc');
    expect(setFilter).toHaveBeenCalledWith('provider', 'claude');
  });

  it('opens the detail panel with the clicked row', async () => {
    const w = shallowMount(MarkingsView);
    const panel = w.findComponent(MarkingDetailPanel);
    expect(panel.props('isOpen')).toBe(false);

    const row = { id: '1', about: 'werkdag' };
    w.findComponent(DataTable).vm.$emit('row-click', row);
    await w.vm.$nextTick();

    expect(panel.props('isOpen')).toBe(true);
    expect(panel.props('row')).toEqual(row);
  });

  it('shows the backlog above the list', () => {
    const w = shallowMount(MarkingsView);
    const clusters = w.findComponent(MarkingClusters);
    expect(clusters.exists()).toBe(true);
    expect(clusters.props('clusters')).toHaveLength(1);
  });

  // Getting from "this change is wanted in four places" to the four articles
  // is the whole point of showing the backlog beside the list. Filtering on
  // `resolution` alone would land the reader on half the corpus, since it has
  // two possible values; `resolved_by` is what identifies the cluster.
  it('filters the list to the picked cluster, on both fields at once', () => {
    const w = shallowMount(MarkingsView);
    setFilters.mockClear();

    w.findComponent(MarkingClusters).vm.$emit('select', {
      resolution: 'operation',
      resolved_by: 'een WORKING_DAY-bewerking',
    });

    // One update rather than two calls: two would fire two un-awaited
    // refreshes, and the first would go out with only `resolution` set.
    expect(setFilters).toHaveBeenCalledTimes(1);
    expect(setFilters).toHaveBeenCalledWith({
      resolution: 'operation',
      resolved_by: 'een WORKING_DAY-bewerking',
    });
  });

  // A cluster that names no change can only be narrowed by resolution. Passing
  // the null through would filter on the string "null" and return nothing.
  it('clears the resolved_by filter for a cluster that names no change', () => {
    const w = shallowMount(MarkingsView);
    setFilters.mockClear();

    w.findComponent(MarkingClusters).vm.$emit('select', {
      resolution: 'model',
      resolved_by: null,
    });

    expect(setFilters).toHaveBeenCalledWith({ resolution: 'model', resolved_by: '' });
  });

  // The backlog groups over the filtered set, so a filter change has to reach
  // it at once. Without this the clusters describe a different selection than
  // the rows under them until the next 20s poll tick, and nothing about the
  // page looks wrong while it happens.
  it('refreshes the backlog when a filter changes', () => {
    const w = shallowMount(MarkingsView);
    refreshClusters.mockClear();

    w.findComponent(DataTable).vm.$emit('filter-change', 'provider', 'claude');
    expect(refreshClusters).toHaveBeenCalled();
  });

  it('refreshes the backlog when a cluster is picked', () => {
    const w = shallowMount(MarkingsView);
    refreshClusters.mockClear();

    w.findComponent(MarkingClusters).vm.$emit('select', {
      resolution: 'operation',
      resolved_by: 'x',
    });
    expect(refreshClusters).toHaveBeenCalled();
  });
});
