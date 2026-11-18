<script setup lang="ts">
import { ref } from 'vue';
import { useUntranslatables, type Untranslatable } from '../composables/useUntranslatables';
import { UNTRANSLATABLE_COLUMNS, UNTRANSLATABLE_SORT_OPTIONS } from '../constants';
import DataTable from '../components/DataTable.vue';
import StatusBadge from '../components/StatusBadge.vue';
import PaginationControls from '../components/PaginationControls.vue';
import UntranslatableDetailPanel from '../components/UntranslatableDetailPanel.vue';

const {
  data, loading, error,
  sort, order, filters,
  currentPage, totalPages,
  setSort, setFilter, goToPage,
} = useUntranslatables();

// Read-only detail: no polling needed, unlike the jobs sheet.
const selected = ref<Untranslatable | null>(null);
const detailOpen = ref(false);

function openDetail(row: Untranslatable) {
  selected.value = row;
  detailOpen.value = true;
}

function closeDetail() {
  detailOpen.value = false;
}
</script>

<template>
  <DataTable
    :columns="UNTRANSLATABLE_COLUMNS"
    :sort-options="UNTRANSLATABLE_SORT_OPTIONS"
    :data="data"
    :loading="loading"
    :error="error"
    :sort="sort"
    :order="order"
    :filters="filters"
    :clickable-rows="true"
    empty-text="No untranslatables"
    empty-supporting-text="They appear here after an enrichment flags a construct"
    @sort="setSort"
    @filter-change="setFilter"
    @row-click="openDetail"
  >
    <template #cell-accepted="{ row }">
      <StatusBadge :status="row.accepted ? 'accepted' : 'open'" />
    </template>
    <template #pagination>
      <PaginationControls
        :current-page="currentPage"
        :total-pages="totalPages"
        @page-change="goToPage"
      />
    </template>
  </DataTable>
  <UntranslatableDetailPanel :row="selected" :is-open="detailOpen" @close="closeDetail" />
</template>
