<script setup lang="ts">
/**
 * RunsListPane - the runs of one law in the traject, on the law's Verwerkingen
 * page: what is still going on top, what is done below, each newest first. A
 * run opens its details in a sheet (RunDetailSheet), the way a task in the
 * task list and a job in the harvester open theirs.
 *
 * This pane drives the shared runs store (see stores/trajectRuns.ts): it loads
 * the law's runs and polls while it is mounted, and the details read from the
 * same store.
 */
import { computed, onBeforeUnmount, watch } from 'vue';
import { useTrajectRunsStore } from '../stores/trajectRuns';
import { isActive, runLawTitle, runSupportingText, splitRuns } from '../lib/trajectRuns';
import type { TrajectRun } from '../lib/trajectRuns';

const props = withDefaults(
  defineProps<{
    trajectRef?: string | null;
    lawId?: string | null;
    /** The law's BWB id, so the harvest that fetched it is listed too. */
    bwbId?: string | null;
  }>(),
  { trajectRef: null, lawId: null, bwbId: null },
);

const emit = defineEmits<{ select: [jobId: string] }>();

const store = useTrajectRunsStore();

watch(
  () => [props.trajectRef, props.lawId, props.bwbId] as const,
  ([trajectRef, lawId, bwbId]) => {
    // Without a law there is nothing this page should list: an unfiltered
    // load would show the runs of every law under this one.
    store.follow(trajectRef && lawId ? { trajectRef, lawId, bwbId } : null);
  },
  { immediate: true },
);
onBeforeUnmount(() => store.stop());

const groups = computed(() => {
  const { active, done } = splitRuns(store.runs);
  return [
    { key: 'active', title: 'Bezig', runs: active },
    { key: 'done', title: 'Afgerond', runs: done },
  ].filter((g) => g.runs.length > 0);
});

// A finished run says how it ended with its icon; a running one shows an
// activity indicator instead (in the template).
function runIcon(run: TrajectRun): string {
  if (run.status === 'failed') return 'exclamation-circle-filled';
  if (run.status === 'cancelled') return 'stop';
  return 'done';
}
</script>

<template>
  <nldd-activity-indicator v-if="!store.loaded" text="Verwerkingen laden" show-text></nldd-activity-indicator>
  <nldd-inline-dialog
    v-else-if="store.error && store.runs.length === 0"
    variant="alert"
    :text="store.error"
  ></nldd-inline-dialog>
  <nldd-inline-dialog
    v-else-if="store.runs.length === 0"
    text="Nog geen verwerkingen"
    supporting-text="Wanneer iemand in dit traject dit regelwerk laat ophalen of verrijken, staat dat hier met de uitkomst."
  ></nldd-inline-dialog>
  <template v-else>
    <template v-for="(group, index) in groups" :key="group.key">
      <nldd-spacer v-if="index > 0" size="24"></nldd-spacer>
      <nldd-title size="display-5"><h4>{{ group.title }}</h4></nldd-title>
      <nldd-spacer size="8"></nldd-spacer>
      <nldd-list appearance="simple">
        <nldd-list-item
          v-for="run in group.runs"
          :key="run.job_id"
          size="md"
          button
          @click="emit('select', run.job_id)"
        >
          <nldd-cell v-if="isActive(run)" vertical-alignment="center">
            <!-- A short label: the row is a button, and its accessible name
                 already carries the title. -->
            <nldd-activity-indicator size="20" timing="instant" text="Bezig"></nldd-activity-indicator>
          </nldd-cell>
          <nldd-icon-cell
            v-else
            size="20"
            :icon="runIcon(run)"
            :color="run.status === 'failed' ? 'critical' : undefined"
          ></nldd-icon-cell>
          <nldd-spacer-cell size="8"></nldd-spacer-cell>
          <nldd-text-cell :text="runLawTitle(run)" :supporting-text="runSupportingText(run)"></nldd-text-cell>
          <nldd-spacer-cell size="8"></nldd-spacer-cell>
          <nldd-icon-cell size="20" icon="chevron-right"></nldd-icon-cell>
        </nldd-list-item>
      </nldd-list>
    </template>
  </template>
</template>
