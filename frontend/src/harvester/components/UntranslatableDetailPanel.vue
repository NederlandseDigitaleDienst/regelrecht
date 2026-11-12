<script setup lang="ts">
/**
 * UntranslatableDetailPanel - right-side sheet showing one untranslatable
 * (RFC-012) in full. DetailPanel.vue is job-shaped (hardcoded job fields), so
 * this is a sibling rather than a reuse. The long free-text fields (reason,
 * suggestion, legal text excerpt) don't fit a table cell and live here.
 */
import { computed, ref, watch } from 'vue';
import StatusBadge from './StatusBadge.vue';
import type { NLDDSheet } from '@nldd/design-system/sheet';
import { formatDate } from '../formatters';
import type { Untranslatable } from '../composables/useUntranslatables';
import { providerLabel } from '../../lib/enrichProviders';

const props = withDefaults(
  defineProps<{
    row?: Untranslatable | null;
    isOpen?: boolean;
  }>(),
  {
    row: null,
    isOpen: false,
  },
);

const emit = defineEmits<{
  close: [];
}>();

/** A long free-text field, shown as its own section. */
interface TextSection {
  title: string;
  code: string;
}

const sheetRef = ref<NLDDSheet | null>(null);

watch(() => props.isOpen, (open) => {
  if (open) sheetRef.value?.show();
  else sheetRef.value?.hide();
});

const infoFields = computed(() => {
  if (!props.row) return [];
  return ([
    ['Law', props.row.law_name || props.row.law_id],
    ['Law ID', props.row.law_id],
    ['Article', props.row.article],
    ['Construct', props.row.construct],
    ['Provider', props.row.provider && providerLabel(props.row.provider)],
    ['Enrich job', props.row.enrich_job_id],
    ['Captured', formatDate(props.row.created_at)],
  ] satisfies [string, string | null | undefined][]).filter(([, value]) => value != null);
});

// Long free-text sections, each shown only when present.
const textSections = computed(() => {
  const r = props.row;
  if (!r) return [];
  const out: TextSection[] = [];
  if (r.reason) out.push({ title: 'Reason', code: r.reason });
  if (r.suggestion) out.push({ title: 'Suggestion', code: r.suggestion });
  if (r.legal_text_excerpt) {
    out.push({ title: 'Legal text excerpt', code: r.legal_text_excerpt });
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
      accessible-label="Untranslatable details"
      @close="onSheetClose"
    >
      <nldd-page sticky-header>
        <nldd-top-title-bar
          slot="header"
          text="Untranslatable details"
          dismiss-text="Close"
          @dismiss="$emit('close')"
        />
        <nldd-simple-section v-if="row">
          <nldd-list appearance="simple">
            <nldd-list-item v-for="[label, value] in infoFields" :key="label">
              <nldd-text-cell :text="label" color="secondary" width="fit-content" />
              <nldd-spacer-cell size="12" />
              <nldd-text-cell :text="String(value)" horizontal-alignment="right" />
            </nldd-list-item>
            <nldd-list-item>
              <nldd-text-cell text="Accepted" color="secondary" width="fit-content" />
              <nldd-spacer-cell size="12" />
              <nldd-cell width="full" style="align-items: flex-end">
                <StatusBadge :status="row.accepted ? 'accepted' : 'open'" size="md" />
              </nldd-cell>
            </nldd-list-item>
          </nldd-list>

          <template v-for="section in textSections" :key="section.title">
            <nldd-spacer size="16" />
            <nldd-title size="display-6"><h3>{{ section.title }}</h3></nldd-title>
            <nldd-spacer size="4" />
            <nldd-code-viewer wrap>{{ section.code }}</nldd-code-viewer>
          </template>
        </nldd-simple-section>
      </nldd-page>
    </nldd-sheet>
  </Teleport>
</template>
