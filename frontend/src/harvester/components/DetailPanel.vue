<script setup lang="ts">
import { computed, ref, watch } from 'vue';
import StatusBadge from './StatusBadge.vue';
import type { NLDDSheet } from '@nldd/design-system/sheet';
import { formatDate } from '../formatters';
import { progressRows, recentTools } from '../jobProgress';
import type { DetailJob } from '../composables/useJobDetail';

const props = withDefaults(
  defineProps<{
    job?: DetailJob | null;
    isOpen?: boolean;
  }>(),
  {
    job: null,
    isOpen: false,
  },
);

const emit = defineEmits<{
  close: [];
}>();

/** A code block in the sheet: the error, the result or the payload. */
interface CodeSection {
  title: string;
  code: string;
  wrap?: boolean;
  language?: string;
}

const sheetRef = ref<NLDDSheet | null>(null);

watch(() => props.isOpen, (open) => {
  if (open) sheetRef.value?.show();
  else sheetRef.value?.hide();
});

const infoFields = computed(() => {
  if (!props.job) return [];
  return ([
    ['Job ID', props.job.id],
    ['Law ID', props.job.law_id],
    ['Type', props.job.job_type],
    ['Status', props.job.status],
    ['Priority', props.job.priority],
    ['Attempts', props.job.attempts == null ? null : `${props.job.attempts} / ${props.job.max_attempts}`],
    ['Created', formatDate(props.job.created_at)],
    ['Started', formatDate(props.job.started_at)],
    ['Completed', formatDate(props.job.completed_at)],
  ] satisfies [string, string | number | null | undefined][]).filter(([, value]) => value != null);
});

const progressFields = computed(() =>
  progressRows(props.job?.progress, props.job?.status === 'processing'),
);
const progressTools = computed(() => recentTools(props.job?.progress));
// While the job runs this is live; afterwards it is what the job was doing
// when it stopped, which is what explains a timeout.
const progressTitle = computed(() =>
  props.job?.status === 'processing' ? 'Progress' : 'Last progress',
);

const resultJson = computed(() =>
  props.job?.result ? JSON.stringify(props.job.result, null, 2) : null,
);

const payloadJson = computed(() =>
  props.job?.payload ? JSON.stringify(props.job.payload, null, 2) : null,
);

const codeSections = computed(() => {
  const j = props.job;
  if (!j) return [];
  const out: CodeSection[] = [];
  if (j.status === 'failed' && j.result?.error) {
    out.push({ title: 'Error', code: j.result.error, wrap: true });
  }
  if (j.status === 'completed' && resultJson.value) {
    out.push({ title: 'Result', code: resultJson.value, language: 'json' });
  }
  if (payloadJson.value) {
    out.push({ title: 'Payload', code: payloadJson.value, language: 'json' });
  }
  return out;
});

function onSheetClose() {
  if (props.isOpen) emit('close');
}
</script>

<template>
  <Teleport to="body">
    <nldd-sheet
      ref="sheetRef"
      placement="right"
      accessible-label="Job details"
      @close="onSheetClose"
    >
      <nldd-page sticky-header>
        <nldd-top-title-bar
          slot="header"
          text="Job details"
          dismiss-text="Close"
          @dismiss="$emit('close')"
        />
        <nldd-simple-section v-if="job">
          <nldd-list appearance="simple">
            <nldd-list-item v-for="[label, value] in infoFields" :key="label">
              <nldd-text-cell :text="label" color="secondary" width="fit-content" />
              <nldd-spacer-cell size="12" />
              <nldd-cell
                v-if="label === 'Status'"
                width="full"
                style="align-items: flex-end"
              >
                <StatusBadge :status="String(value)" size="md" />
              </nldd-cell>
              <nldd-text-cell v-else :text="String(value)" horizontal-alignment="right" />
            </nldd-list-item>
          </nldd-list>

          <template v-if="progressFields.length">
            <nldd-spacer size="16" />
            <nldd-title size="6"><h3>{{ progressTitle }}</h3></nldd-title>
            <nldd-spacer size="4" />
            <nldd-list appearance="simple" data-testid="job-progress">
              <nldd-list-item v-for="[label, value] in progressFields" :key="label">
                <nldd-text-cell :text="label" color="secondary" width="fit-content" />
                <nldd-spacer-cell size="12" />
                <nldd-text-cell :text="value" horizontal-alignment="right" />
              </nldd-list-item>
            </nldd-list>
            <template v-if="progressTools.length">
              <nldd-spacer size="8" />
              <nldd-code-viewer wrap>{{ progressTools.join('\n') }}</nldd-code-viewer>
            </template>
          </template>

          <template v-for="section in codeSections" :key="section.title">
            <nldd-spacer size="16" />
            <nldd-title size="6"><h3>{{ section.title }}</h3></nldd-title>
            <nldd-spacer size="4" />
            <nldd-code-viewer :wrap="section.wrap || undefined" :language="section.language || undefined">{{ section.code }}</nldd-code-viewer>
          </template>
        </nldd-simple-section>
      </nldd-page>
    </nldd-sheet>
  </Teleport>
</template>
