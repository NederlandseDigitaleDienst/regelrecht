<script setup lang="ts">
import { ref } from 'vue';
import { useMarkings, type Marking } from '../composables/useMarkings';
import { useMarkingClusters, type MarkingCluster } from '../composables/useMarkingClusters';
import { MARKING_COLUMNS, MARKING_SORT_OPTIONS } from '../constants';
import DataTable from '../components/DataTable.vue';
import StatusBadge from '../components/StatusBadge.vue';
import PaginationControls from '../components/PaginationControls.vue';
import MarkingDetailPanel from '../components/MarkingDetailPanel.vue';
import MarkingClusters from '../components/MarkingClusters.vue';

const {
  data, loading, error,
  sort, order, filters,
  currentPage, totalPages,
  setSort, setFilter, setFilters, goToPage, filterParams,
} = useMarkings();

// The backlog reads the same filtered set as the list below it.
const {
  data: clusters,
  loading: clustersLoading,
  refresh: refreshClusters,
} = useMarkingClusters(filterParams);

// Read-only detail: no polling needed, unlike the jobs sheet.
const selected = ref<Marking | null>(null);
const detailOpen = ref(false);

function openDetail(row: Marking) {
  selected.value = row;
  detailOpen.value = true;
}

function closeDetail() {
  detailOpen.value = false;
}

// Clicking a cluster filters the list to it, which is how a reader gets from
// "this change is wanted in four places" to the four articles themselves.
//
// Both fields, because `resolution` has two possible values: filtering on it
// alone lands the reader on half the corpus rather than on those four
// articles. `resolved_by` is what identifies the cluster.
//
// A cluster that names no change (`resolved_by` is nullable) can only be
// narrowed by resolution, which is honest: there is nothing else to match on.
function focusCluster(cluster: MarkingCluster) {
  // One update, one request. Two `setFilter` calls would race: each fires its
  // own un-awaited refresh, and the first would go out with only `resolution`
  // set.
  setFilters({
    resolution: cluster.resolution,
    resolved_by: cluster.resolved_by || '',
  });
  refreshClusters();
}

// Every filter change has to reach the clusters too, or the backlog describes
// a different set than the rows under it until the next poll tick. A count
// that groups something other than the visible rows is worse than no count:
// nothing about the page looks wrong while it happens.
function onFilterChange(key: string, value: string) {
  setFilter(key, value);
  refreshClusters();
}
</script>

<template>
  <MarkingClusters
    :clusters="clusters"
    :loading="clustersLoading"
    @select="focusCluster"
  />

  <DataTable
    :columns="MARKING_COLUMNS"
    :sort-options="MARKING_SORT_OPTIONS"
    :data="data"
    :loading="loading"
    :error="error"
    :sort="sort"
    :order="order"
    :filters="filters"
    :clickable-rows="true"
    empty-text="Geen markeringen"
    empty-supporting-text="Ze verschijnen hier zodra een verrijking een constructie markeert die het formaat niet kan uitdrukken"
    @sort="setSort"
    @filter-change="onFilterChange"
    @row-click="openDetail"
  >
    <!-- A closed pair, so a badge reads faster than a sentence and keeps the
         column narrow. What it means in full is in the detail panel. -->
    <template #cell-resolution="{ row }">
      <nldd-cell width="fit-content">
        <nldd-badge
          :color="row.resolution === 'operation' ? 'accent' : 'neutral'"
          size="sm"
          :text="row.resolution === 'operation' ? 'bewerking' : 'formaat'"
        />
      </nldd-cell>
    </template>
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

  <MarkingDetailPanel :row="selected" :is-open="detailOpen" @close="closeDetail" />
</template>
