<script setup lang="ts">
/**
 * One daily-jobs chart (stacked succeeded/failed bars + added line) for a
 * single job type. Data comes in as plain entries; colors are resolved from
 * the NLDD tokens at mount and re-resolved when the color scheme flips
 * (data-scheme attribute for the explicit picker, the media query for
 * 'auto') - the tokens are light-dark() pairs the canvas can't track itself.
 */
import { computed, onMounted, onUnmounted, shallowRef } from 'vue';
import { use } from 'echarts/core';
import { CanvasRenderer } from 'echarts/renderers';
import { BarChart, LineChart } from 'echarts/charts';
import {
  GridComponent,
  TooltipComponent,
  LegendComponent,
} from 'echarts/components';
import VChart from 'vue-echarts';
import {
  buildDailyJobsOption,
  type DailyJobsEntry,
} from '../charts/dailyJobsChart';
import { resolveChartColors, type ChartColors } from '../charts/chartColors';

use([
  CanvasRenderer,
  BarChart,
  LineChart,
  GridComponent,
  TooltipComponent,
  LegendComponent,
]);

const props = defineProps<{
  title: string;
  entries: DailyJobsEntry[];
}>();

const colors = shallowRef<ChartColors | null>(null);

// resolveChartColors returns null while the design system's color-scheme
// hasn't been applied yet (light-dark() would resolve to black then); retry a
// few frames until it lands.
let retryFrame: number | null = null;
function refreshColors(attemptsLeft = 30) {
  if (retryFrame) {
    cancelAnimationFrame(retryFrame);
    retryFrame = null;
  }
  const resolved = resolveChartColors();
  if (resolved) {
    colors.value = resolved;
  } else if (attemptsLeft > 0) {
    retryFrame = requestAnimationFrame(() => refreshColors(attemptsLeft - 1));
  } else {
    console.warn(
      'DailyJobsChart: color-scheme never left "normal"; chart not rendered',
    );
  }
}

let schemeObserver: MutationObserver | null = null;
let darkMedia: MediaQueryList | null | undefined = null;
const onSchemeChange = () => refreshColors();
onMounted(() => {
  refreshColors();
  schemeObserver = new MutationObserver(onSchemeChange);
  schemeObserver.observe(document.documentElement, {
    attributes: true,
    attributeFilter: ['data-scheme'],
  });
  darkMedia = window.matchMedia?.('(prefers-color-scheme: dark)');
  darkMedia?.addEventListener?.('change', onSchemeChange);
});
onUnmounted(() => {
  schemeObserver?.disconnect();
  darkMedia?.removeEventListener?.('change', onSchemeChange);
  if (retryFrame) cancelAnimationFrame(retryFrame);
});

const option = computed(() =>
  colors.value ? buildDailyJobsOption(props.entries, colors.value) : null,
);
</script>

<template>
  <nldd-card>
    <nldd-container padding="16">
      <nldd-title size="display-6">
        <h3>{{ title }}</h3>
      </nldd-title>
      <nldd-spacer size="12"></nldd-spacer>
      <div class="daily-jobs-chart">
        <v-chart v-if="option" :option="option" autoresize />
      </div>
    </nldd-container>
  </nldd-card>
</template>
