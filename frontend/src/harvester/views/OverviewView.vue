<script setup lang="ts">
import { computed } from 'vue';
import { apiFetch } from '@regelrecht/frontend-shared';
import {
  useDashboardStats,
  type DailyCounts,
  type RecentFailure,
} from '../composables/useDashboardStats';
import { useJobDetail } from '../composables/useJobDetail';
import { JOB_STATUSES, type TableColumn } from '../constants';
import { formatNumber, formatStatus } from '../formatters';
import DataTable from '../components/DataTable.vue';
import DailyJobsChart from '../components/DailyJobsChart.vue';
import DetailPanel from '../components/DetailPanel.vue';
import StatusBadge from '../components/StatusBadge.vue';

const { stats, loading, error } = useDashboardStats();
const { job: detailJob, isOpen: detailOpen, open: openDetail, close: closeDetail } =
  useJobDetail();

// Columns for the recent-failures list. No `sortable`/`filter` on any column
// (and no sortOptions passed to DataTable) so the table toolbar renders empty.
const FAILED_COLUMNS: TableColumn<RecentFailure>[] = [
  { key: 'law_id', label: 'Law ID', minWidth: '160px' },
  { key: 'job_type', label: 'Type', width: 'fit-content', text: (row) => formatStatus(row.job_type) },
  { key: 'failed_at', label: 'Wanneer', width: 'fit-content' },
  { key: 'error', label: 'Reden', minWidth: '240px', text: (row) => row.error },
];

// Top-level totals shown as standalone KPI cards; the per-type detail lives in
// the panels below.
const topKpis = computed(() => {
  const s = stats.value;
  if (!s) return [];
  return [
    { label: 'Jobs totaal', value: s.jobs.total },
    { label: 'Open untranslatables', value: s.open_untranslatables },
  ];
});

// One titled section per job type: status breakdown + executed counts next to
// the daily chart. The list of types comes from `by_type` rather than from a
// constant here, so a type the API knows can never be missing from the page —
// that hardcoding is what hid three of the five types.
// `daily` is absent while an older API is deployed - then chartEntries stays
// empty and the chart simply doesn't render.
const typePanels = computed(() => {
  const s = stats.value;
  if (!s) return [];
  const daily = s.daily ?? [];
  return Object.keys(s.jobs.by_type ?? {}).map((key) => ({
    key,
    label: formatStatus(key),
    total: s.jobs.by_type[key] ?? 0,
    statuses: JOB_STATUSES.map((status) => ({
      status,
      count: s.jobs.by_type_status[key]?.[status] ?? 0,
    })),
    today: s.executed.today[key] ?? 0,
    last7d: s.executed.last_7d[key] ?? 0,
    // Cast: `key` comes from `by_type`, so it names a type's counts, never
    // `date`, and the API puts every type's counts on every day.
    chartEntries: daily.map((d) => ({ date: d.date, ...(d[key] as DailyCounts) })),
  }));
});

// Failed rows only carry a subset of the job; fetch the full job so the detail
// sheet shows the complete info. Fall back to the summary row (with the reason)
// if the fetch fails.
async function onFailureClick(row: RecentFailure) {
  try {
    const resp = await apiFetch(`/api/harvest-admin/jobs/${encodeURIComponent(row.id)}`);
    openDetail(await resp.json());
  } catch {
    openDetail({
      ...row,
      status: 'failed',
      completed_at: row.failed_at,
      result: { error: row.error },
    });
  }
}
</script>

<template>
  <nldd-simple-section v-if="loading && !stats">
    <nldd-inline-dialog text="Laden…"></nldd-inline-dialog>
  </nldd-simple-section>
  <nldd-simple-section v-else-if="error && !stats">
    <nldd-inline-dialog variant="alert" :text="'Kan dashboard niet laden: ' + error"></nldd-inline-dialog>
  </nldd-simple-section>

  <template v-else-if="stats">
    <!-- Top-level totals: same half/half rhythm as the per-type sections
         below, so every card on the page shares one width -->
    <nldd-one-half-one-half-section>
      <nldd-card v-for="(kpi, i) in topKpis" :key="kpi.label" :slot="i === 0 ? 'left' : 'right'">
        <nldd-container padding="16">
          <!-- A figure, not a heading: nldd-text with a display size. -->
          <nldd-text size="md" color="secondary">{{ kpi.label }}</nldd-text>
          <nldd-text size="display-2">{{ formatNumber(kpi.value) }}</nldd-text>
        </nldd-container>
      </nldd-card>
    </nldd-one-half-one-half-section>

    <!-- One titled section per job type: details next to the daily chart -->
    <nldd-one-half-one-half-section v-for="panel in typePanels" :key="panel.key">
      <nldd-title slot="header" size="display-6"><h3>{{ panel.label }}</h3></nldd-title>

      <nldd-card slot="left">
        <nldd-container padding="16">
          <nldd-text size="display-3">{{ formatNumber(panel.total) }}</nldd-text>
          <nldd-text size="md" color="secondary">jobs totaal</nldd-text>

          <nldd-spacer size="16"></nldd-spacer>

          <nldd-list appearance="simple">
            <nldd-list-item v-for="item in panel.statuses" :key="item.status">
              <nldd-cell width="fit-content">
                <StatusBadge :status="item.status" size="md" />
              </nldd-cell>
              <nldd-text-cell :text="formatNumber(item.count)" horizontal-alignment="right" />
            </nldd-list-item>
          </nldd-list>

          <nldd-spacer size="12"></nldd-spacer>
          <nldd-divider></nldd-divider>
          <nldd-spacer size="12"></nldd-spacer>

          <nldd-list appearance="simple">
            <nldd-list-item>
              <nldd-text-cell text="Uitgevoerd vandaag" color="secondary" />
              <nldd-text-cell :text="formatNumber(panel.today)" horizontal-alignment="right" />
            </nldd-list-item>
            <nldd-list-item>
              <nldd-text-cell text="Afgelopen 7 dagen" color="secondary" />
              <nldd-text-cell :text="formatNumber(panel.last7d)" horizontal-alignment="right" />
            </nldd-list-item>
          </nldd-list>
        </nldd-container>
      </nldd-card>

      <DailyJobsChart
        v-if="panel.chartEntries.length"
        slot="right"
        :title="'Per dag'"
        :entries="panel.chartEntries"
      />
    </nldd-one-half-one-half-section>

    <!-- Recent failures -->
    <nldd-simple-section>
      <nldd-title slot="header" size="display-6"><h3>Gefaalde jobs</h3></nldd-title>
    </nldd-simple-section>
    <DataTable
      :columns="FAILED_COLUMNS"
      :data="stats.recent_failures"
      :clickable-rows="true"
      empty-text="Geen gefaalde jobs"
      empty-supporting-text="Er zijn geen recente mislukte jobs"
      @row-click="onFailureClick"
    />

    <DetailPanel :job="detailJob" :is-open="detailOpen" @close="closeDetail" />
  </template>
</template>
