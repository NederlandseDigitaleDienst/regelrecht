<script setup lang="ts">
import { computed, onUnmounted, useId } from 'vue';
import type { NLDDMenuItem } from '@nldd/design-system/menu';
import { formatStatus } from '../formatters';
import type { DirectionLabels, SortOption, TableColumn } from '../constants';

const props = withDefaults(
  defineProps<{
    columns: TableColumn[];
    sortOptions?: SortOption[] | null;
    sort?: string;
    order?: string;
    filters?: Record<string, string>;
  }>(),
  {
    sortOptions: null,
    sort: '',
    order: 'desc',
    filters: () => ({}),
  },
);

const emit = defineEmits<{
  sort: [key: string, order: string];
  'filter-change': [key: string, value: string];
}>();

/** A column whose toolbar filter is set. */
type FilterColumn = TableColumn & { filter: NonNullable<TableColumn['filter']> };

/** One entry of the sort menu. */
interface SortMenuItem {
  key: string;
  order: string;
  value: string;
  text: string;
}

// Unique prefix for menu anchor IDs (avoid collisions when multiple toolbars exist)
const uid = useId();

const sortableColumns = computed<SortOption[]>(
  () => props.sortOptions || props.columns.filter((c) => c.sortable),
);
const filterColumns = computed(() =>
  props.columns.filter((c): c is FilterColumn => Boolean(c.filter)),
);
const dropdownFilters = computed(() => filterColumns.value.filter((c) => c.filter.options));
const textFilters = computed(() => filterColumns.value.filter((c) => c.filter.type === 'text'));

const sortMenuItems = computed(() => {
  const items: SortMenuItem[] = [];
  for (const opt of sortableColumns.value) {
    if (opt.directionLabels) {
      for (const [dir, dirLabel] of Object.entries(opt.directionLabels)) {
        items.push({
          key: opt.key,
          order: dir,
          value: `${opt.key}:${dir}`,
          text: `${opt.label} (${dirLabel})`,
        });
      }
    } else {
      items.push({ key: opt.key, order: 'desc', value: opt.key, text: opt.label });
    }
  }
  return items;
});

const activeSortLabel = computed(() => {
  const opt = sortableColumns.value.find((c) => c.key === props.sort);
  if (!opt) return 'Sort';
  const dirLabel = opt.directionLabels?.[props.order as keyof DirectionLabels];
  return dirLabel ? `Sort: ${opt.label} (${dirLabel})` : `Sort: ${opt.label}`;
});

function getFilterKey(col: TableColumn): string {
  return col.filter?.key || col.key;
}

function getFilterLabel(col: TableColumn): string {
  return col.filter?.label || col.label;
}

function getFilterButtonLabel(col: TableColumn): string {
  const key = getFilterKey(col);
  const value = props.filters[key];
  const label = getFilterLabel(col);
  return `${label}: ${value ? formatStatus(value) : 'All'}`;
}

function onSortSelect(event: Event) {
  const item = (event.target as Element).closest<NLDDMenuItem>('nldd-menu-item');
  if (!item) return;
  const [key, order] = String(item.value).split(':');
  // split() always returns at least one element.
  emit('sort', key!, order || 'desc');
}

function onFilterSelect(col: TableColumn, event: Event) {
  const item = (event.target as Element).closest<NLDDMenuItem>('nldd-menu-item');
  if (!item) return;
  const key = getFilterKey(col);
  // Empty string value means "All" (clear filter)
  emit('filter-change', key, item.value);
}

// Text filter debounce
const debounceTimers: Record<string, ReturnType<typeof setTimeout>> = {};
function onTextFilter(col: TableColumn, event: Event) {
  const key = getFilterKey(col);
  const raw =
    (event as CustomEvent<{ value?: string } | null>).detail?.value ??
    (event.target as { value?: unknown } | null)?.value ??
    '';
  const value = String(raw).trim();
  clearTimeout(debounceTimers[key]);
  debounceTimers[key] = setTimeout(() => {
    emit('filter-change', key, value);
  }, 300);
}

onUnmounted(() => Object.values(debounceTimers).forEach(clearTimeout));
</script>

<template>
  <div v-if="sortableColumns.length > 0 || filterColumns.length > 0" class="table-toolbar">
    <slot name="prefix" />

    <!-- Sort menu -->
    <nldd-button
      v-if="sortableColumns.length > 0"
      :id="`sort-btn-${uid}`"
      expandable
      appearance="neutral-tinted"
      size="sm"
      :text="activeSortLabel"
    />
    <nldd-menu
      v-if="sortableColumns.length > 0"
      :anchor="`sort-btn-${uid}`"
      @select="onSortSelect"
    >
      <nldd-menu-item
        v-for="item in sortMenuItems"
        :key="item.value"
        type="radio"
        :text="item.text"
        :value="item.value"
        :selected="sort === item.key && order === item.order"
      />
    </nldd-menu>

    <!-- Dropdown filter menus -->
    <template v-for="col in dropdownFilters" :key="getFilterKey(col)">
      <nldd-button
        :id="`filter-btn-${uid}-${getFilterKey(col)}`"
        expandable
        appearance="neutral-tinted"
        size="sm"
        :text="getFilterButtonLabel(col)"
      />
      <nldd-menu
        :anchor="`filter-btn-${uid}-${getFilterKey(col)}`"
        @select="onFilterSelect(col, $event)"
      >
        <nldd-menu-item
          type="radio"
          text="All"
          value=""
          :selected="!filters[getFilterKey(col)]"
        />
        <nldd-menu-item
          v-for="v in col.filter.options"
          :key="v"
          type="radio"
          :text="col.filter.optionLabel ? col.filter.optionLabel(v) : formatStatus(v)"
          :value="v"
          :selected="filters[getFilterKey(col)] === v"
        />
      </nldd-menu>
    </template>

    <!-- Text filter inputs -->
    <nldd-search-field
      v-for="col in textFilters"
      :key="getFilterKey(col)"
      size="sm"
      width="200px"
      :placeholder="getFilterLabel(col)"
      :accessible-label="`Filter ${getFilterLabel(col)}`"
      :value="filters[getFilterKey(col)] || ''"
      @input="onTextFilter(col, $event)"
    />
  </div>
</template>
