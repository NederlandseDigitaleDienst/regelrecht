<template>
  <div class="optimization-path">
    <span class="optimization-path-title">{{ title }}</span>
    <v-chart class="chart" :option="option" autoresize @click="onPoint" />
    <p class="optimization-path-hint">{{ hint }}</p>
  </div>
</template>

<!--
  The path the policy assistant takes toward a target: one point per
  measurement it makes on the way.

  Why this is shared and not per case: the shape is the same (measure, adjust,
  measure again), only the quantity differs. Terugbetaalregimes measures a
  percentage of debtors with payment problems; nieuwkomersbekostiging measures
  amounts, three side by side (po, vo, implementation burden), because a target
  like "level the two without the spending going up" is about the ratio
  between those items. Hence series with their own unit instead of a fixed
  percentage axis.

  The points are clickable: during a running optimization you can take an
  intermediate state you like, without waiting for it to converge.
-->

<script setup lang="ts">
import { computed } from 'vue';
import { use } from 'echarts/core';
import { CanvasRenderer } from 'echarts/renderers';
import { LineChart } from 'echarts/charts';
import { GridComponent, TooltipComponent, LegendComponent } from 'echarts/components';
import VChart from 'vue-echarts';

use([CanvasRenderer, LineChart, GridComponent, TooltipComponent, LegendComponent]);

/**
 * One measurement. A series missing from a measurement gets a gap instead of
 * a zero: not measured is something else than zero.
 */
export interface Measurement {
  iteration: number | string;
  values?: Record<string, number | null | undefined>;
}

/** A line in the chart. `axis` is 0 (left) or 1 (right), for two quantities side by side. */
export interface Series {
  key: string;
  label: string;
  color: string;
  axis?: 0 | 1;
  formatter?: (value: number) => string;
}

/** What ECharts passes to a tooltip formatter with `trigger: 'axis'`, as far as used here. */
interface TooltipPoint {
  axisValue?: string | number;
  marker?: unknown;
  seriesName?: string;
  seriesIndex?: number;
  value?: unknown;
}

const props = withDefaults(
  defineProps<{
    /** The measurements, in ascending order. */
    measurements?: Measurement[];
    /** What is drawn. */
    series: Series[];
    /** Axis titles, per index. */
    axisTitles?: string[];
    /** Formatters per axis. */
    axisFormatters?: ((value: number) => string)[];
    title?: string;
    hint?: string;
    /** Index of the point shown as selected, or null. */
    selected?: number | null;
  }>(),
  {
    measurements: () => [],
    axisTitles: () => [],
    axisFormatters: () => [],
    title: 'Optimalisatiepad',
    hint: 'Klik op een punt om die tussenstand te bekijken.',
    selected: null,
  },
);

const emit = defineEmits<{ select: [index: number] }>();

/** The measurement that belongs to the clicked point index. */
function onPoint(e: { dataIndex?: unknown } | undefined) {
  if (typeof e?.dataIndex !== 'number') return;
  emit('select', e.dataIndex);
}

const hasSecondAxis = computed(() => props.series.some((s) => s.axis === 1));

function formatValue(s: Partial<Series>, value: unknown): string {
  if (value === null || value === undefined) return '—';
  // A number as text goes through the formatter too, as before; what is no
  // number comes through unformatted.
  const num = typeof value === 'number' ? value : Number(value);
  return s.formatter && Number.isFinite(num) ? s.formatter(num) : String(value);
}

const option = computed(() => ({
  tooltip: {
    trigger: 'axis',
    formatter: (params: TooltipPoint[]) => {
      const heading = `meting ${params[0]?.axisValue ?? ''}`;
      const lines = params.map((p) => {
        const s: Partial<Series> = (p.seriesIndex === undefined ? undefined : props.series[p.seriesIndex]) ?? {};
        return `${p.marker} ${p.seriesName}: ${formatValue(s, p.value)}`;
      });
      return [heading, ...lines].join('<br>');
    },
  },
  legend: props.series.length > 1
    ? { top: 0, data: props.series.map((s) => s.label), textStyle: { fontSize: 11 } }
    : undefined,
  grid: {
    left: 52,
    right: hasSecondAxis.value ? 52 : 16,
    top: props.series.length > 1 ? 28 : 16,
    bottom: 32,
  },
  xAxis: {
    type: 'category',
    data: props.measurements.map((m) => m.iteration),
    name: 'meting',
    nameLocation: 'end',
  },
  yAxis: [0, 1].filter((i) => i === 0 || hasSecondAxis.value).map((i) => ({
    type: 'value',
    name: props.axisTitles[i] ?? '',
    axisLabel: {
      formatter: props.axisFormatters[i] ? (v: number) => props.axisFormatters[i]!(v) : undefined,
    },
    // Not from zero: a target run often moves within a narrow band, and an
    // axis from zero then shows a flat line with nothing to see.
    scale: true,
  })),
  series: props.series.map((s) => ({
    name: s.label,
    type: 'line',
    smooth: true,
    showSymbol: true,
    symbolSize: 8,
    yAxisIndex: hasSecondAxis.value ? (s.axis ?? 0) : 0,
    lineStyle: { width: 2 },
    color: s.color,
    data: props.measurements.map((m) => m.values?.[s.key] ?? null),
    // The selected point larger, so it shows which intermediate state you are looking at.
    markPoint: props.selected === null ? undefined : {
      symbol: 'circle',
      symbolSize: 14,
      itemStyle: { color: s.color, opacity: 0.35 },
      label: { show: false },
      data: [{ coord: [props.selected, props.measurements[props.selected]?.values?.[s.key] ?? null] }],
    },
  })),
}));
</script>

<style scoped>
.optimization-path { display: flex; flex-direction: column; gap: var(--primitives-space-8); }
.optimization-path-title { font-size: 0.85em; font-weight: 600; color: var(--semantics-content-secondary-color); }
.optimization-path-hint { margin: 0; font-size: 0.8em; color: var(--semantics-content-secondary-color); }
.chart { height: 220px; width: 100%; cursor: pointer; }
</style>
