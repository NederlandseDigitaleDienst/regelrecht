/**
 * Option-builder for the daily harvest/enrich jobs charts (ECharts).
 *
 * Pure functions so the mapping is unit-testable without a canvas: the Vue
 * wrapper (DailyJobsChart.vue) resolves the design-token colors in the
 * browser and injects them here.
 */

import type { ChartColors } from './chartColors';

/** One day of the chart: jobs created and finished that day. */
export interface DailyJobsEntry {
  date: string;
  added: number;
  succeeded: number;
  failed: number;
}

type SeriesKey = 'added' | 'succeeded' | 'failed';

export const SERIES_LABELS: Record<SeriesKey, string> = {
  added: 'Toegevoegd',
  succeeded: 'Geslaagd',
  failed: 'Gefaald',
};

const AXIS_DATE_FORMAT = new Intl.DateTimeFormat('nl-NL', {
  day: 'numeric',
  month: 'short',
});

// nl-NL grouping (1.500, not the ECharts default 1,500), consistent with
// formatNumber elsewhere on the dashboard.
const AXIS_NUMBER_FORMAT = new Intl.NumberFormat('nl-NL');
export function formatAxisNumber(value: number): string {
  return AXIS_NUMBER_FORMAT.format(value);
}

// 'YYYY-MM-DD' → short Dutch axis label, e.g. '6 jul'.
export function formatAxisDate(isoDate: string): string {
  const date = new Date(`${isoDate}T00:00:00`);
  return Number.isNaN(date.getTime()) ? isoDate : AXIS_DATE_FORMAT.format(date);
}

/**
 * @param {Array<{date: string, added: number, succeeded: number, failed: number}>} entries
 *   One entry per day, ascending (as served by dashboard-stats `daily`).
 * @param {{succeeded: string, failed: string, added: string, text: string,
 *          textSecondary: string, grid: string, surface: string}} colors
 *   Resolved concrete colors (rgb/hex - canvas can't read CSS variables).
 */
export function buildDailyJobsOption(
  entries: DailyJobsEntry[],
  colors: ChartColors,
) {
  // Succeeded under, failed on top: the stack totals "finished that day" and
  // the red cap is immediately visible. Added is a separate event (created vs
  // finished), so it rides on top as a line instead of stacking in.
  const bar = (key: 'succeeded' | 'failed') => ({
    name: SERIES_LABELS[key],
    type: 'bar' as const,
    stack: 'afgerond',
    data: entries.map((e) => e[key]),
    color: colors[key],
    barMaxWidth: 20,
    // 1px surface-colored border keeps stacked segments visually separated.
    itemStyle: { borderColor: colors.surface, borderWidth: 1 },
  });

  const addedLine = {
    name: SERIES_LABELS.added,
    type: 'line' as const,
    data: entries.map((e) => e.added),
    color: colors.added,
    lineStyle: { width: 2 },
    symbol: 'circle',
    symbolSize: 6,
  };
  // A tuple, so each series keeps its own shape for callers.
  const series: [
    ReturnType<typeof bar>,
    ReturnType<typeof bar>,
    typeof addedLine,
  ] = [bar('succeeded'), bar('failed'), addedLine];

  return {
    animation: false,
    grid: { left: 8, right: 8, top: 36, bottom: 8, containLabel: true },
    legend: {
      top: 0,
      left: 0,
      icon: 'roundRect',
      itemWidth: 10,
      itemHeight: 10,
      textStyle: { color: colors.text, fontSize: 12 },
    },
    tooltip: {
      trigger: 'axis' as const,
      axisPointer: { type: 'shadow' as const },
      valueFormatter: formatAxisNumber,
      // Theme the tooltip too - the ECharts default is white-on-light only.
      backgroundColor: colors.surface,
      borderColor: colors.grid,
      textStyle: { color: colors.text },
    },
    xAxis: {
      type: 'category' as const,
      data: entries.map((e) => formatAxisDate(e.date)),
      axisTick: { show: false },
      axisLine: { lineStyle: { color: colors.grid } },
      axisLabel: { color: colors.textSecondary, fontSize: 11 },
    },
    yAxis: {
      type: 'value' as const,
      minInterval: 1,
      splitLine: { lineStyle: { color: colors.grid } },
      axisLabel: {
        color: colors.textSecondary,
        fontSize: 11,
        formatter: formatAxisNumber,
      },
    },
    series,
  };
}
