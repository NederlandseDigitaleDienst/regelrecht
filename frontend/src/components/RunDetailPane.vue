<script setup lang="ts">
/**
 * RunDetailPane - one run of a law, in the details sheet of its Verwerkingen
 * page: its state, who started it and when, and what came of it. Reads the run
 * from the shared runs store, which RunsListPane keeps current, so a running
 * run updates here without its own poll.
 *
 * Where there is something left to do, the pane points at it: an enrichment
 * with open review tasks links to those tasks. The error of a failed run is
 * shown in full here; the list only says it failed.
 *
 * The rows look like the table on the law's Algemeen page: label left, value
 * right, in a tinted box.
 */
import { computed } from 'vue';
import { useTrajectRunsStore } from '../stores/trajectRuns';
import {
  coverageText,
  durationText,
  formatMoment,
  isActive,
  reviewSummary,
  runStatusText,
  timesCurrentAttempt,
} from '../lib/trajectRuns';

const props = withDefaults(
  defineProps<{
    jobId?: string | null;
  }>(),
  { jobId: null },
);

const emit = defineEmits<{
  'open-tasks': [lawId: string];
}>();

const store = useTrajectRunsStore();
const run = computed(() => store.byId(props.jobId));

interface Row {
  label: string;
  value: string;
}

function asText(value: unknown): string | null {
  return typeof value === 'string' && value !== '' ? value : null;
}

const rows = computed<Row[]>(() => {
  const r = run.value;
  if (!r) return [];
  const all: Array<[string, string | null]> = [
    ['Status', runStatusText(r)],
    ['Aangevraagd door', r.requested_by_name],
    ['Aangevraagd', formatMoment(r.created_at)],
    ['Gestart', timesCurrentAttempt(r) ? formatMoment(r.started_at) : null],
    [r.status === 'failed' ? 'Mislukt' : 'Klaar', formatMoment(r.completed_at)],
    [isActive(r) ? 'Loopt al' : 'Duur', durationText(r)],
    ['Pogingen', r.attempts > 1 ? `${r.attempts} van ${r.max_attempts}` : null],
    // The worker's own note on what it is doing, while it does it.
    ['Fase', isActive(r) ? asText((r.progress as Record<string, unknown> | null)?.phase) : null],
    ['Taalmodel', r.provider],
    [
      'Uitvoerbare artikelen',
      r.articles_total != null && r.articles_with_machine_readable != null
        ? `${r.articles_with_machine_readable} van ${r.articles_total}`
        : null,
    ],
    ['Nieuw verrijkt', coverageText(r.coverage_score)],
    ['Conversie', r.converted_with],
    ['Beoordeling', reviewSummary(r)],
  ];
  return all
    .filter((entry): entry is [string, string] => entry[1] !== null)
    .map(([label, value]) => ({ label, value }));
});

// A failure's error is the point of opening it. On a run waiting for its next
// attempt it explains why there is a next attempt.
const showError = computed(
  () => !!run.value?.error && (run.value.status === 'failed' || run.value.status === 'pending'),
);

// Only an enrichment leaves review tasks, and it names its law by `$id`; a
// harvest carries a BWB id.
const lawId = computed(() => (run.value?.job_type === 'enrich' ? run.value.law_id : null));

// Review tasks go to whoever requested the run, and Taken shows a member only
// their own. Someone else's open proposals get a line saying whose they are,
// not a button to a list where they are not.
const othersOpen = computed(() => (run.value ? run.value.tasks_open - run.value.tasks_open_mine : 0));
</script>

<template>
  <nldd-inline-dialog
    v-if="!run && store.loaded"
    text="Deze verwerking staat niet (meer) in de lijst"
    supporting-text="De lijst toont de laatste 200 verwerkingen van dit regelwerk."
  ></nldd-inline-dialog>
  <nldd-activity-indicator v-else-if="!run" text="Verwerking laden" show-text></nldd-activity-indicator>
  <template v-else>
    <nldd-list appearance="box-tinted" accessible-label="Gegevens van de verwerking">
      <nldd-list-item v-for="row in rows" :key="row.label">
        <nldd-text-cell :text="row.label" width="200px"></nldd-text-cell>
        <nldd-spacer-cell size="16"></nldd-spacer-cell>
        <nldd-text-cell :text="row.value" horizontal-alignment="right"></nldd-text-cell>
      </nldd-list-item>
    </nldd-list>

    <template v-if="showError">
      <nldd-spacer size="24"></nldd-spacer>
      <nldd-rich-text>
        <h4>Foutmelding</h4>
        <!-- A pre keeps the line breaks of a CLI or model error. -->
        <pre>{{ run.error }}</pre>
      </nldd-rich-text>
    </template>

    <template v-if="othersOpen > 0">
      <nldd-spacer size="24"></nldd-spacer>
      <nldd-rich-text>
        <p>
          {{ othersOpen === 1 ? '1 voorstel wacht' : `${othersOpen} voorstellen wachten` }}
          op beoordeling door {{ run.requested_by_name || 'de aanvrager' }}.
        </p>
      </nldd-rich-text>
    </template>

    <template v-if="lawId && run.tasks_open_mine > 0">
      <nldd-spacer size="24"></nldd-spacer>
      <nldd-button
        appearance="primary"
        :text="run.tasks_open_mine === 1 ? 'Beoordeel het open voorstel' : `Beoordeel ${run.tasks_open_mine} open voorstellen`"
        @click="emit('open-tasks', lawId)"
      ></nldd-button>
    </template>
  </template>
</template>
