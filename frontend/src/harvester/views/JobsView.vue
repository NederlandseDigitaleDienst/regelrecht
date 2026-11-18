<script setup lang="ts">
import { watch } from 'vue';
import { useRoute, useRouter } from 'vue-router';
import { singleQuery } from '../../lib/routeParams';
import { useJobs, type Job, type JobSummary } from '../composables/useJobs';
import { useJobDetail } from '../composables/useJobDetail';
import { useNewHarvestJob } from '../composables/useNewHarvestJob';
import { useLawJobsSheet } from '../composables/useLawJobsSheet';
import { JOB_COLUMNS, JOB_SORT_OPTIONS, GROUPED_SORT_OPTIONS } from '../constants';
import DataTable from '../components/DataTable.vue';
import GroupedTable from '../components/GroupedTable.vue';
import DetailPanel from '../components/DetailPanel.vue';
import LawJobsSheet from '../components/LawJobsSheet.vue';
import StatusBadge from '../components/StatusBadge.vue';
import PaginationControls from '../components/PaginationControls.vue';
import ViewToggle from '../components/ViewToggle.vue';

const route = useRoute();
const router = useRouter();

const {
  data, totalCount, loading, error,
  sort, order, filters,
  viewMode,
  currentPage, totalPages,
  setSort, setFilter, goToPage,
  toggleViewMode, setLawIdFilter,
  refresh,
} = useJobs({ initialViewMode: route.query.view });

const { job: detailJob, isOpen: detailOpen, open: openDetail, close: closeDetail } = useJobDetail();
const { isOpen: lawJobsSheetOpen, open: openLawJobs } = useLawJobsSheet();

// Sync viewMode to URL so a refresh keeps the selected view. Grouped is the
// default - omit the param in that case to keep the URL clean.
watch(viewMode, (mode) => {
  const { view, ...rest } = route.query;
  router.replace({ query: mode === 'flat' ? { ...rest, view: 'flat' } : rest });
});

// When the sheet closes, drop law_id from URL so a refresh doesn't reopen it.
watch(lawJobsSheetOpen, (open) => {
  if (!open && route.query.law_id) {
    const { law_id, ...rest } = route.query;
    router.replace({ query: rest });
  }
});

// Handle incoming law_id query param (runs synchronously during setup)
const initialLawId = singleQuery(route.query.law_id);
if (initialLawId) {
  setLawIdFilter(initialLawId);
  openLawJobs(initialLawId);
}

const { lastJobCreated, open: openNewHarvestJob } = useNewHarvestJob();
watch(lastJobCreated, () => refresh());

// nldd-segmented-control's `change`, forwarded by ViewToggle.
function onViewChange(event: Event) {
  const value = (event as CustomEvent<{ value?: string } | undefined>).detail?.value;
  if (value && value !== viewMode.value) toggleViewMode();
}
</script>

<template>
  <!-- Flat view -->
  <!-- `data` holds the rows of the current view mode: jobs when flat, per-law
       summaries when grouped (switching modes clears it first). -->
  <DataTable
    v-if="viewMode === 'flat'"
    :columns="JOB_COLUMNS"
    :sort-options="JOB_SORT_OPTIONS"
    :data="data as Job[]"
    :loading="loading"
    :error="error"
    :sort="sort"
    :order="order"
    :filters="filters"
    :clickable-rows="true"
    empty-text="No jobs"
    @sort="setSort"
    @filter-change="setFilter"
    @row-click="openDetail"
  >
    <template #toolbar-prefix>
      <ViewToggle :value="viewMode" @change="onViewChange" />
    </template>
    <template #cell-status="{ row }">
      <StatusBadge :status="row.status || 'unknown'" />
    </template>
    <template #empty-action>
      <nldd-button
        slot="actions"
        appearance="primary"
        text="New harvest job"
        @click="openNewHarvestJob"
      />
    </template>
    <template #pagination>
      <PaginationControls
        :current-page="currentPage"
        :total-pages="totalPages"
        @page-change="goToPage"
      />
    </template>
  </DataTable>

  <!-- Grouped view -->
  <GroupedTable
    v-else
    :sort-options="GROUPED_SORT_OPTIONS"
    :data="data as JobSummary[]"
    :loading="loading"
    :error="error"
    :sort="sort"
    :order="order"
    :filters="filters"
    empty-text="No jobs"
    @sort="setSort"
    @filter-change="setFilter"
    @view-jobs="openLawJobs"
  >
    <template #toolbar-prefix>
      <ViewToggle :value="viewMode" @change="onViewChange" />
    </template>
    <template #empty-action>
      <nldd-button
        slot="actions"
        appearance="primary"
        text="New harvest job"
        @click="openNewHarvestJob"
      />
    </template>
    <template #pagination>
      <PaginationControls
        :current-page="currentPage"
        :total-pages="totalPages"
        @page-change="goToPage"
      />
    </template>
  </GroupedTable>

  <DetailPanel :job="detailJob" :is-open="detailOpen" @close="closeDetail" />
  <LawJobsSheet />
</template>
