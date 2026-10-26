<script setup lang="ts">
/**
 * A bar chart for simulation results: categories on one axis, one or more
 * series of numbers. ECharts (as in the editor's harvester dashboards), with
 * colours taken from the design tokens at mount and re-resolved when the
 * colour scheme flips.
 */
import { computed, onMounted, onUnmounted, shallowRef } from 'vue';
import { use } from 'echarts/core';
import { CanvasRenderer } from 'echarts/renderers';
import { BarChart } from 'echarts/charts';
import { GridComponent, LegendComponent, TooltipComponent } from 'echarts/components';
import VChart from 'vue-echarts';
import { resolveChartColors, resolveCssColor, SERIES_KEYS, type ChartColors } from '../simulation/chartColors';
import { intlLocale } from '../data/format';

use([CanvasRenderer, BarChart, GridComponent, LegendComponent, TooltipComponent]);

/** One series of bars; `color` is a CSS expression, such as a design token. */
export interface BarSeries {
  name: string;
  values: (number | null | undefined)[];
  color?: string;
}

const props = withDefaults(
  defineProps<{
    categories: string[];
    series: BarSeries[];
    unit?: 'percent' | 'euro' | 'number';
    horizontal?: boolean;
    height?: string;
  }>(),
  {
    unit: 'number',
    horizontal: false,
    height: '280px',
  },
);

const colors = shallowRef<ChartColors | null>(null);
let retry: number | null = null;
function refreshColors(attempts = 30) {
  if (retry) cancelAnimationFrame(retry);
  retry = null;
  const resolved = resolveChartColors();
  if (resolved) colors.value = resolved;
  else if (attempts > 0) retry = requestAnimationFrame(() => refreshColors(attempts - 1));
}

let observer: MutationObserver | null = null;
let media: MediaQueryList | null = null;
const onMedia = () => refreshColors();
onMounted(() => {
  refreshColors();
  observer = new MutationObserver(() => refreshColors());
  observer.observe(document.documentElement, { attributes: true, attributeFilter: ['data-scheme', 'data-theme', 'class'] });
  media = window.matchMedia('(prefers-color-scheme: dark)');
  media.addEventListener('change', onMedia);
});
onUnmounted(() => {
  observer?.disconnect();
  media?.removeEventListener('change', onMedia);
  if (retry) cancelAnimationFrame(retry);
});

// Per aanroep opgebouwd en niet eenmalig op moduleniveau: een `Intl`-formatter
// legt zijn taal vast bij het maken, en de grafiek hoort na een taalwissel mee
// te veranderen.
function fmt(v: number | null | undefined): string {
  if (v === null || v === undefined) return '–';
  if (props.unit === 'percent') return `${Math.round(v)}%`;
  if (props.unit === 'euro') return new Intl.NumberFormat(intlLocale(), { style: 'currency', currency: 'EUR', maximumFractionDigits: 0 }).format(v);
  return new Intl.NumberFormat(intlLocale(), { maximumFractionDigits: 1 }).format(v);
}

const option = computed(() => {
  const c = colors.value;
  if (!c) return null;
  const valueAxis = {
    type: 'value',
    max: props.unit === 'percent' ? 100 : undefined,
    axisLabel: { color: c.textSecondary, formatter: (v: number) => fmt(v) },
    splitLine: { lineStyle: { color: c.grid } },
  };
  const categoryAxis = {
    type: 'category',
    data: props.categories,
    axisLabel: { color: c.text, interval: 0, width: props.horizontal ? 210 : 90, overflow: 'truncate', rotate: props.horizontal ? 0 : props.categories.length > 6 ? 30 : 0 },
    axisLine: { lineStyle: { color: c.grid } },
    axisTick: { show: false },
    inverse: props.horizontal,
  };
  return {
    animationDuration: 300,
    grid: { left: props.horizontal ? 220 : 56, right: 56, top: props.series.length > 1 ? 36 : 12, bottom: props.horizontal ? 28 : 56 },
    legend: props.series.length > 1 ? { top: 0, textStyle: { color: c.text } } : undefined,
    tooltip: { trigger: 'axis', valueFormatter: (v: number | null | undefined) => fmt(v) },
    xAxis: props.horizontal ? valueAxis : categoryAxis,
    yAxis: props.horizontal ? categoryAxis : valueAxis,
    // `i % length` is always a valid index into SERIES_KEYS.
    series: props.series.map((s, i) => ({
      name: s.name,
      type: 'bar',
      data: s.values.map((v) => (v === null || v === undefined ? null : Math.round(v * 100) / 100)),
      itemStyle: { color: (s.color && resolveCssColor(s.color)) || c[SERIES_KEYS[i % SERIES_KEYS.length]!], borderRadius: 3 },
      barMaxWidth: 28,
      label: props.series.length === 1 ? { show: true, position: props.horizontal ? 'right' : 'top', color: c.textSecondary, formatter: (p: { value: number | null }) => fmt(p.value) } : undefined,
    })),
  };
});
</script>

<template>
  <VChart v-if="option" :option="option" :style="{ height, width: '100%' }" autoresize />
  <div v-else :style="{ height }"></div>
</template>
