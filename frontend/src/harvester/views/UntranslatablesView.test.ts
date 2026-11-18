import { shallowMount } from '@vue/test-utils';
import { describe, it, expect, vi } from 'vitest';
import { ref, reactive } from 'vue';
import UntranslatablesView from './UntranslatablesView.vue';
import DataTable from '../components/DataTable.vue';
import UntranslatableDetailPanel from '../components/UntranslatableDetailPanel.vue';
import { UNTRANSLATABLE_COLUMNS, UNTRANSLATABLE_SORT_OPTIONS } from '../constants';

// DataTable is a generic component (`<script setup generic>`), whose props
// test-utils cannot type; read them as a plain record.
function tableProps(table: { props(): unknown }): Record<string, unknown> {
  return table.props() as Record<string, unknown>;
}

const setSort = vi.fn();
const setFilter = vi.fn();
const goToPage = vi.fn();

vi.mock('../composables/useUntranslatables', () => ({
  useUntranslatables: () => ({
    data: ref([{ id: '1', law_id: 'test_law', construct: 'rounding', accepted: false }]),
    loading: ref(false),
    error: ref(null),
    sort: ref('created_at'),
    order: ref('desc'),
    filters: reactive({}),
    currentPage: ref(1),
    totalPages: ref(1),
    setSort,
    setFilter,
    goToPage,
  }),
}));

describe('UntranslatablesView', () => {
  it('wires the DataTable with the untranslatable columns and clickable rows', () => {
    const w = shallowMount(UntranslatablesView);
    const table = w.findComponent(DataTable);
    expect(tableProps(table).columns).toBe(UNTRANSLATABLE_COLUMNS);
    expect(tableProps(table).sortOptions).toBe(UNTRANSLATABLE_SORT_OPTIONS);
    expect(tableProps(table).clickableRows).toBe(true);
  });

  it('forwards sort/filter/page events to the composable', () => {
    const w = shallowMount(UntranslatablesView);
    const table = w.findComponent(DataTable);
    table.vm.$emit('sort', 'law_id', 'asc');
    table.vm.$emit('filter-change', 'provider', 'claude');
    expect(setSort).toHaveBeenCalledWith('law_id', 'asc');
    expect(setFilter).toHaveBeenCalledWith('provider', 'claude');
  });

  it('opens the detail panel with the clicked row', async () => {
    const w = shallowMount(UntranslatablesView);
    const panel = w.findComponent(UntranslatableDetailPanel);
    expect(panel.props('isOpen')).toBe(false);

    const row = { id: '1', construct: 'rounding' };
    w.findComponent(DataTable).vm.$emit('row-click', row);
    await w.vm.$nextTick();

    expect(panel.props('isOpen')).toBe(true);
    expect(panel.props('row')).toEqual(row);
  });
});
