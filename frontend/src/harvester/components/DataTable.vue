<script setup lang="ts" generic="Row extends Record<string, unknown>">
import StatusBadge from './StatusBadge.vue';
import TableToolbar from './TableToolbar.vue';
import { formatDate, formatCoverageScore, truncateUuid } from '../formatters';
import { useTableFilters } from '../composables/useTableFilters';
import type { SortOption, TableColumn } from '../constants';

const props = withDefaults(
  defineProps<{
    columns: TableColumn<Row>[];
    data: Row[];
    loading?: boolean;
    error?: string | null;
    sort?: string;
    order?: string;
    filters?: Record<string, string>;
    clickableRows?: boolean;
    emptyText?: string;
    emptySupportingText?: string;
    sortOptions?: SortOption[] | null;
  }>(),
  {
    loading: false,
    error: null,
    sort: '',
    order: 'desc',
    filters: () => ({}),
    clickableRows: false,
    emptyText: 'No data found',
    emptySupportingText: '',
    sortOptions: null,
  },
);

const emit = defineEmits<{
  sort: [key: string, order: string];
  'filter-change': [key: string, value: string];
  'row-click': [row: Row];
}>();

const { hasActiveFilters, clearFilters } = useTableFilters(() => props.filters, emit);

function formatCellValue(value: unknown, key: string): string | null {
  if (value === null || value === undefined || value === '') return null;
  if (key === 'id') return truncateUuid(value);
  if (key === 'coverage_score') return formatCoverageScore(value);
  // `*_at` columns hold API timestamps (ISO strings).
  if (key.endsWith('_at')) return formatDate(value as string | number | Date | null | undefined);
  return String(value);
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
    <nldd-inline-dialog v-else-if="data.length === 0" :text="emptyText" :supporting-text="emptySupportingText">
      <slot name="empty-action" />
    </nldd-inline-dialog>

    <nldd-list v-else appearance="simple">
      <!-- Data rows -->
      <slot name="rows" :data="data" :columns="columns">
        <nldd-list-item
          v-for="row in data"
          :key="row.id || row.law_id"
          size="md"
          :button="clickableRows"
          @click="clickableRows && emit('row-click', row)"
        >
          <template v-for="(col, idx) in columns" :key="col.key">
            <nldd-spacer-cell v-if="idx > 0" size="12" :hide-below="col.hideBelow" />
            <nldd-cell
              v-if="$slots['cell-' + col.key]"
              :width="col.width || 'stretch'"
              :min-width="col.minWidth"
              :style="col.align === 'right' ? 'align-items: flex-end' : undefined"
            >
              <slot :name="'cell-' + col.key" :row="row" :value="row[col.key]" />
            </nldd-cell>
            <nldd-cell
              v-else-if="col.key === 'status'"
              :width="col.width || 'stretch'"
              :min-width="col.minWidth"
            >
              <StatusBadge :status="String(row[col.key] || 'unknown')" />
            </nldd-cell>
            <nldd-text-cell
              v-else-if="col.text"
              :text="col.text(row)"
              :overline="col.overline ? col.overline(row) : undefined"
              :supporting-text="col.supportingText ? col.supportingText(row) : undefined"
              :width="col.width || 'stretch'"
              :min-width="col.minWidth"
              :horizontal-alignment="col.align"
              :hide-below="col.hideBelow"
            />
            <nldd-cell
              v-else-if="col.key === 'id' || col.key === 'law_id'"
              :width="col.width || 'stretch'"
              :min-width="col.minWidth"
            >
              <span class="cell-mono" :title="col.key === 'id' ? String(row[col.key]) : undefined">{{
                col.key === 'id' ? formatCellValue(row[col.key], col.key) : row[col.key]
              }}</span>
            </nldd-cell>
            <nldd-text-cell
              v-else-if="col.supportingKey"
              :text="row[col.key] || '—'"
              :supporting-text="row[col.supportingKey]"
              :width="col.width || 'stretch'"
              :min-width="col.minWidth"
              :horizontal-alignment="col.align"
            />
            <nldd-text-cell
              v-else
              :text="formatCellValue(row[col.key], col.key) || '—'"
              :color="formatCellValue(row[col.key], col.key) ? 'content' : 'secondary'"
              :width="col.width || 'stretch'"
              :min-width="col.minWidth"
              :horizontal-alignment="col.align"
            />
          </template>
          <template v-if="clickableRows">
            <nldd-spacer-cell size="12" />
            <nldd-icon-cell icon="chevron-right" size="20" />
          </template>
        </nldd-list-item>
      </slot>
    </nldd-list>

    <slot name="pagination" />
  </nldd-simple-section>
</template>
