<script setup lang="ts">
import { computed, nextTick, ref, watch } from 'vue';
import type { NLDDSheet } from '@nldd/design-system/sheet';
import { explainWhy, toBlocks, type WhyErrorKind } from '../why/why';
import { useI18n } from '../i18n/index.js';

// The explanation of one outcome in plain language, written by a language
// model from the calculation (see why/why.ts and server/why.mjs). It streams
// in while it is written, so the reader sees the first sentence in a second or
// two instead of a spinner for twenty.
//
// The caveat above it is not decoration. The tile and the calculation are what
// the law says; this is a model's paraphrase of them, and on a government
// portal a paraphrase that reads as confidently as the outcome has to say what
// it is.

const props = withDefaults(defineProps<{
  open?: boolean;
  // Only the name is read; the tile passes its LawEntry.
  law?: { name?: string } | null;
  /** What the server needs; built by the tile, which knows the outcome. */
  payload?: object | null;
}>(), {
  open: false,
  law: null,
  payload: null,
});
const emit = defineEmits<{ close: [] }>();
const { t } = useI18n();

const sheet = ref<NLDDSheet | null>(null);
const text = ref('');
const error = ref<WhyErrorKind | null>(null); // 'locked' | 'busy' | 'failed'
const loading = ref(false);
let controller: AbortController | null = null;

const blocks = computed(() => toBlocks(text.value));

async function run() {
  controller?.abort();
  controller = new AbortController();
  const own = controller;
  text.value = '';
  error.value = null;
  loading.value = true;
  try {
    await explainWhy(props.payload, { signal: own.signal, onText: (s) => { if (controller === own) text.value = s; } });
  } catch (e) {
    if (own.signal.aborted) return;
    // A WhyError carries its kind; anything else is a plain failure.
    error.value = (e as { kind?: WhyErrorKind } | null)?.kind ?? 'failed';
  } finally {
    if (controller === own) loading.value = false;
  }
}

watch(() => props.open, async (open) => {
  if (!open) {
    // Closing stops the answer: the server kills the model when the
    // connection goes, and nobody is left to read it.
    controller?.abort();
    controller = null;
    return sheet.value?.hide?.();
  }
  await nextTick();
  sheet.value?.show?.();
  run();
});

const errorText = computed(() => (error.value ? t(`wet.tile.why.error.${error.value}`) : ''));
</script>

<template>
  <Teleport to="body">
    <nldd-sheet ref="sheet" placement="right" width="720px" :accessible-label="t('wet.tile.why.title')" @close="emit('close')">
      <nldd-page>
        <nldd-container slot="header" padding="12">
          <nldd-top-title-bar :text="t('wet.tile.why.title')" :supporting-text="law?.name" :dismiss-text="t('wet.tile.why.close')" @dismiss="emit('close')"></nldd-top-title-bar>
        </nldd-container>
        <nldd-container padding="16" gap="16">
          <nldd-banner variant="accent" icon="info" :text="t('wet.tile.why.caveat.title')" :supporting-text="t('wet.tile.why.caveat.body')"></nldd-banner>
          <nldd-inline-dialog v-if="error" variant="alert" :text="t('wet.tile.why.error.title')" :supporting-text="errorText">
            <nldd-button v-if="error !== 'locked'" slot="actions" appearance="secondary" size="sm" :text="t('wet.tile.why.retry')" @click="run"></nldd-button>
          </nldd-inline-dialog>
          <nldd-activity-indicator v-else-if="loading && !text" show-text :text="t('wet.tile.why.loading')" timing="instant" size="24"></nldd-activity-indicator>
          <!-- aria-live so a screen reader reads the explanation as it arrives,
               aria-busy until it is complete: otherwise reading restarts with
               every new sentence. -->
          <nldd-rich-text v-if="blocks.length" aria-live="polite" :aria-busy="loading ? 'true' : 'false'">
            <template v-for="(b, i) in blocks" :key="i">
              <ul v-if="'items' in b"><li v-for="(item, j) in b.items" :key="j">{{ item }}</li></ul>
              <p v-else>{{ b.text }}</p>
            </template>
          </nldd-rich-text>
        </nldd-container>
      </nldd-page>
    </nldd-sheet>
  </Teleport>
</template>
