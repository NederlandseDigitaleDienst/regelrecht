<script setup lang="ts">
import { watch } from 'vue';
import { useLawEntries } from '../composables/useLawEntries';
import { useNewHarvestJob } from '../composables/useNewHarvestJob';
import { LAW_ENTRY_COLUMNS, LAW_ENTRY_SORT_OPTIONS } from '../constants';
import DataTable from '../components/DataTable.vue';
import StatusBadge from '../components/StatusBadge.vue';
import RowActions from '../components/RowActions.vue';
import PaginationControls from '../components/PaginationControls.vue';

const {
  data, totalCount, loading, error,
  sort, order, filters,
  currentPage, totalPages,
  setSort, setFilter, goToPage, refresh,
} = useLawEntries();

const { lastJobCreated, open } = useNewHarvestJob();
watch(lastJobCreated, () => refresh());
</script>

<template>
  <DataTable
    :columns="LAW_ENTRY_COLUMNS"
    :sort-options="LAW_ENTRY_SORT_OPTIONS"
    :data="data"
    :loading="loading"
    :error="error"
    :sort="sort"
    :order="order"
    :filters="filters"
    empty-text="No law entries"
    empty-supporting-text="Start jobs to add laws"
    @sort="setSort"
    @filter-change="setFilter"
  >
    <template #empty-action>
      <nldd-button
        slot="actions"
        appearance="primary"
        text="New harvest job"
        @click="open"
      />
    </template>
    <template #cell-status="{ row }">
      <StatusBadge :status="row.status || 'unknown'" />
    </template>
    <template #cell-_actions="{ row }">
      <RowActions :row="row" @action-complete="refresh" />
    </template>
    <template #pagination>
      <PaginationControls
        :current-page="currentPage"
        :total-pages="totalPages"
        @page-change="goToPage"
      />
    </template>
  </DataTable>
</template>
