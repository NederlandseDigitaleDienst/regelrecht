<script setup lang="ts">
import TableToolbar from './TableToolbar.vue';
import { GROUPED_COLUMNS, type SortOption } from '../constants';
import { formatDate } from '../formatters';
import { useTableFilters } from '../composables/useTableFilters';
import type { JobSummary } from '../composables/useJobs';

const props = withDefaults(
  defineProps<{
    data: JobSummary[];
    loading?: boolean;
    error?: string | null;
    sort?: string;
    order?: string;
    filters?: Record<string, string>;
    emptyText?: string;
    sortOptions?: SortOption[] | null;
  }>(),
  {
    loading: false,
    error: null,
    sort: '',
    order: 'desc',
    filters: () => ({}),
    emptyText: 'No data found',
    sortOptions: null,
  },
);

const emit = defineEmits<{
  sort: [key: string, order: string];
  'filter-change': [key: string, value: string];
  'view-jobs': [lawId: string];
}>();

const { hasActiveFilters, clearFilters } = useTableFilters(() => props.filters, emit);

const columns = GROUPED_COLUMNS;

type StatusBarKey = 'completed' | 'failed' | 'processing' | 'pending';

const STATUS_BAR_KEYS: StatusBarKey[] = ['completed', 'failed', 'processing', 'pending'];

const STATUS_BAR_COLORS: Record<StatusBarKey, string> = {
  completed: 'success',
  failed: 'critical',
  processing: 'accent',
  pending: 'neutral',
};

function statusSegments(group: JobSummary) {
  const total = group.total_jobs || 0;
  if (total === 0) return [];
  return STATUS_BAR_KEYS
    .map((key) => ({ key, count: group[key] || 0 }))
    .filter((seg) => seg.count > 0);
}

// A column's raw value, for columns without a text() formatter.
function columnValue(group: JobSummary, key: string): unknown {
  const row: Record<string, unknown> = group;
  return row[key];
}

function statusBarTitle(group: JobSummary): string {
  return STATUS_BAR_KEYS
    .filter((key) => group[key] > 0)
    .map((key) => `${group[key]} ${key}`)
    .join(' · ');
}
</script>

<template>
  <nldd-simple-section>
    <template v-if="data.length > 0 || hasActiveFilters">
      <TableToolbar
        :columns="columns"
        :sort-options="sortOptions"
        :sort="sort"
        :order="order"
        :filters="filters"
        @sort="(key, order) => emit('sort', key, order)"
        @filter-change="(key, value) => emit('filter-change', key, value)"
      >
        <template #prefix>
          <slot name="toolbar-prefix" />
        </template>
      </TableToolbar>
      <nldd-spacer size="16" />
    </template>

    <nldd-inline-dialog v-if="loading && data.length === 0" text="Loading…"></nldd-inline-dialog>
    <nldd-inline-dialog variant="alert" v-else-if="error && data.length === 0" :text="'Failed to load data: ' + error"></nldd-inline-dialog>
    <nldd-inline-dialog
      v-else-if="data.length === 0 && hasActiveFilters"
      text="No results match the current filters."
    >
      <nldd-button
        slot="actions"
        appearance="secondary"
        text="Clear filters"
        @click="clearFilters"
      />
    </nldd-inline-dialog>
    <nldd-inline-dialog v-else-if="data.length === 0" :text="emptyText">
      <slot name="empty-action" />
    </nldd-inline-dialog>

    <nldd-list v-else appearance="simple">
      <!-- Group rows -->
      <nldd-list-item
        v-for="group in data"
        :key="group.law_id"
        size="md"
        button
        @click="emit('view-jobs', group.law_id)"
      >
        <template v-for="(col, idx) in columns" :key="col.key">
          <nldd-spacer-cell v-if="idx > 0" size="12" :hide-below="col.hideBelow" />
          <nldd-cell
            v-if="col.key === 'status_bar'"
            :width="col.width || 'stretch'"
          >
            <!-- One tooltip for the whole bar, with every status in it. A
                 segment with a tooltip-text or a name gets a tooltip of its
                 own, and with it a tab stop inside the row's button. -->
            <nldd-tooltip :text="statusBarTitle(group)" placement="top" timing="instant">
              <nldd-progress-bar
                variant="distribution"
                value-display="none"
                :max="group.total_jobs || 0"
                :accessible-label="statusBarTitle(group)"
              >
                <nldd-progress-bar-segment-indicator
                  v-for="seg in statusSegments(group)"
                  :key="seg.key"
                  :value="seg.count"
                  :color="STATUS_BAR_COLORS[seg.key]"
                ></nldd-progress-bar-segment-indicator>
              </nldd-progress-bar>
            </nldd-tooltip>
          </nldd-cell>
          <nldd-text-cell
            v-else-if="col.text"
            :text="col.text(group)"
            :overline="col.overline ? col.overline(group) : undefined"
            :supporting-text="col.supportingText ? col.supportingText(group) : undefined"
            :width="col.width || 'stretch'"
            :min-width="col.minWidth"
            :horizontal-alignment="col.align"
          />
          <nldd-text-cell
            v-else
            :text="columnValue(group, col.key) != null ? String(columnValue(group, col.key)) : '—'"
            :color="columnValue(group, col.key) != null ? 'content' : 'secondary'"
            :width="col.width || 'stretch'"
            :min-width="col.minWidth"
            :horizontal-alignment="col.align"
            :hide-below="col.hideBelow"
          />
        </template>
        <nldd-spacer-cell size="12" />
        <nldd-icon-cell icon="chevron-right" size="20" />
      </nldd-list-item>
    </nldd-list>

    <slot name="pagination" />
  </nldd-simple-section>
</template>
