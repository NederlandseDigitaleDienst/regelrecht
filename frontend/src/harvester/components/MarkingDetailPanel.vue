<script setup lang="ts">
/**
 * MarkingDetailPanel - right-side sheet showing one marking (schema v0.7.0)
 * in full. DetailPanel.vue is job-shaped (hardcoded job fields), so this is a
 * sibling rather than a reuse, for the same reason UntranslatableDetailPanel
 * is. The long free-text fields don't fit a table cell and live here.
 *
 * The legal text excerpt matters most: a marking that cannot quote the text it
 * is about is about something else, so the schema requires it. Reading the
 * marking beside the words it hangs on is how someone decides whether the gap
 * is real.
 */
import { computed, ref, watch } from 'vue';
import StatusBadge from './StatusBadge.vue';
import type { NLDDSheet } from '@nldd/design-system/sheet';
import { formatDate } from '../formatters';
import type { Marking } from '../composables/useMarkings';
import { providerLabel } from '../../lib/enrichProviders';

const props = withDefaults(
  defineProps<{
    row?: Marking | null;
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

const RESOLUTION_LABEL: Record<string, string> = {
  operation: 'bewerking ontbreekt',
  model: 'formaat mist een vorm',
};

const infoFields = computed(() => {
  if (!props.row) return [];
  return ([
    ['Wet', props.row.law_name || props.row.law_id],
    ['Wet-id', props.row.law_id],
    ['Artikel', props.row.article],
    ['Soort', RESOLUTION_LABEL[props.row.resolution] || props.row.resolution],
    ['Provider', props.row.provider && providerLabel(props.row.provider)],
    ['Verrijkingsjob', props.row.enrich_job_id],
    ['Gevonden', formatDate(props.row.created_at)],
  ] satisfies [string, string | null | undefined][]).filter(([, value]) => value != null);
});

// An empty target is a claim rather than a blank: the article stays
// executable. Saying so beats an empty line the reader has to interpret.
const targetText = computed(() => {
  const target = props.row?.target;
  if (!target || !target.length) return 'niets, het artikel blijft werken';
  return target.join(', ');
});

// Long free-text sections, each shown only when present.
const textSections = computed(() => {
  const r = props.row;
  if (!r) return [];
  const out: TextSection[] = [];
  if (r.about) out.push({ title: 'Wat niet uitdrukbaar is', code: r.about });
  if (r.resolved_by) out.push({ title: 'Wat het zou oplossen', code: r.resolved_by });
  if (r.legal_text_excerpt) {
    out.push({ title: 'De wettekst waar het aan hangt', code: r.legal_text_excerpt });
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
      accessible-label="Markering in detail"
      @close="onSheetClose"
    >
      <nldd-page sticky-header>
        <nldd-top-title-bar
          slot="header"
          text="Markering in detail"
          dismiss-text="Sluiten"
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
              <nldd-text-cell text="Blokkeert" color="secondary" width="fit-content" />
              <nldd-spacer-cell size="12" />
              <nldd-text-cell :text="targetText" horizontal-alignment="right" />
            </nldd-list-item>
            <nldd-list-item>
              <nldd-text-cell text="Beoordeeld" color="secondary" width="fit-content" />
              <nldd-spacer-cell size="12" />
              <nldd-cell width="full" horizontal-alignment="right">
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
