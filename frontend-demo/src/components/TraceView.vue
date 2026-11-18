<script setup lang="ts">
import { computed, nextTick, ref, watch } from 'vue';
import { traceLaws } from '../data/traceLaws';
import { paletteColor } from '../data/palette';
import { useDemo } from '../store/demoStore';
import { useI18n } from '../i18n/index.js';

// The engine's text trace, with each law in its own colour and a bar to step
// from one law to the next. A presenter walks it law by law on a hall screen
// (zorgtoeslagwet, then the Zvw, then the penitentiaire beginselenwet), and in
// two thousand lines of box drawing the handover from one law to the next is
// what the audience needs to see. traceLaws.ts says which line belongs where.

const props = withDefaults(defineProps<{
  text?: string;
}>(), {
  text: '',
});
const { t } = useI18n();
const { corpus } = useDemo();

const parsed = computed(() => traceLaws(props.text, (id) => !!corpus.value?.lawById(id)));
/**
 * The law's colour as CSS variables: -500 for the stripe and the tint, -700
 * for text. The primitives flip in the dark scheme, so -700 is a dark ink on
 * white and a light one on the dark surface without a rule per scheme.
 */
const colorOf = (law: string): Record<string, string> | undefined => {
  const i = parsed.value.laws.indexOf(law);
  if (i < 0) return undefined;
  return { '--law-color': paletteColor(i), '--law-ink': paletteColor(i, 700) };
};
// A line with `start` always has its `law` (see traceLaws), hence `line.law!` in the template.
const nameOf = (law: string) => corpus.value?.lawById(law)?.name ?? law;

/** Index into `parsed.sections` of the handover in view; -1 before the first jump. */
const current = ref(-1);
// Filled by the `:ref` callback on each line `<div>`; null once a line unmounts.
const lineEls = ref<(HTMLElement | null)[]>([]);
const nav = ref<HTMLElement | null>(null);
watch(() => props.text, () => {
  current.value = -1;
});

async function show(section: number) {
  current.value = section;
  await nextTick();
  const line = parsed.value.sections[section];
  const el = line === undefined ? undefined : lineEls.value[line];
  if (!el) return;
  // Below the bar that stays on top; it wraps onto a second row when the
  // trace crosses many laws, so its height is measured, not assumed.
  el.style.scrollMarginTop = `${(nav.value?.offsetHeight ?? 0) + 16}px`;
  el.scrollIntoView({ block: 'start', behavior: 'smooth' });
}
function step(delta: number) {
  const n = parsed.value.sections.length;
  if (!n) return;
  show(Math.min(n - 1, Math.max(0, current.value + delta)));
}
/** The next place this law takes over, after the one in view; wraps round. */
function jumpTo(law: string) {
  const { sections, lines } = parsed.value;
  const order = sections.map((line, i) => ({ line, i })).filter(({ line }) => lines[line]!.law === law); // `sections` holds indices into `lines`
  if (!order.length) return;
  show((order.find(({ i }) => i > current.value) ?? order[0]!).i); // non-empty: checked above
}
const currentLine = computed(() => (current.value < 0 ? -1 : parsed.value.sections[current.value]));
</script>

<template>
  <div class="trace-view">
    <nldd-container v-if="parsed.laws.length" ref="nav" class="trace-view__nav" layout="wrap" gap="8" padding-block="8" vertical-alignment="center" role="navigation" :aria-label="t('trace.nav.label')">
      <nldd-icon-button size="sm" appearance="neutral-tinted" icon="chevron-up" :text="t('trace.nav.prev')" :disabled="current <= 0 || undefined" @click="step(-1)"></nldd-icon-button>
      <nldd-icon-button size="sm" appearance="neutral-tinted" icon="chevron-down" :text="t('trace.nav.next')" :disabled="current >= parsed.sections.length - 1 || undefined" @click="step(1)"></nldd-icon-button>
      <nldd-text size="sm" color="secondary">{{ t('trace.nav.position', { n: current + 1, total: parsed.sections.length }) }}</nldd-text>
      <nldd-button
        v-for="law in parsed.laws"
        :key="law"
        class="trace-view__law"
        size="xs"
        appearance="inherit-tinted"
        :text="nameOf(law)"
        :style="colorOf(law)"
        @click="jumpTo(law)"
      ></nldd-button>
    </nldd-container>
    <div class="trace-view__lines">
      <div
        v-for="(line, i) in parsed.lines"
        :key="i"
        :ref="(el) => (lineEls[i] = el as HTMLElement | null)"
        :class="['trace-view__line', { 'trace-view__line--start': line.start, 'trace-view__line--current': i === currentLine }]"
        :style="line.law ? colorOf(line.law) : undefined"
      >{{ line.text }}<span v-if="line.start" class="trace-view__badge">{{ nameOf(line.law!) }}</span></div>
    </div>
  </div>
</template>
