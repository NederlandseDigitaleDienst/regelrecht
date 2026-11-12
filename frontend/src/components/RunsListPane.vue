<script setup lang="ts">
/**
 * RunsListPane - the runs of the traject in the secondary sidebar of
 * Verwerkingen: what is still going on top, what is done below, each newest
 * first. A run opens its details in main (RunDetailPane); the route carries
 * which one.
 *
 * This pane drives the shared runs store (see stores/trajectRuns.ts): it loads
 * the runs and polls while it is mounted, and the details read from the same
 * store.
 */
import { computed, onBeforeUnmount, watch } from 'vue';
import { useTrajectRunsStore } from '../stores/trajectRuns';
import { useCorpus } from '../corpus';
import { isActive, runTitle, runSupportingText, splitRuns } from '../lib/trajectRuns';
import type { TrajectRun } from '../lib/trajectRuns';

const props = withDefaults(
  defineProps<{
    trajectRef?: string | null;
    jobId?: string | null;
  }>(),
  { trajectRef: null, jobId: null },
);

const emit = defineEmits<{ select: [jobId: string] }>();

const store = useTrajectRunsStore();
const { displayName } = useCorpus();

watch(
  () => props.trajectRef,
  (ref_) => {
    store.follow(ref_);
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
  return run.status === 'failed' ? 'exclamation-circle-filled' : 'done';
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
    supporting-text="Wat iemand in dit traject laat ophalen, verrijken of converteren, staat hier met de uitkomst."
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
          :current="run.job_id === jobId || undefined"
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
          <nldd-text-cell
            :text="runTitle(run, displayName)"
            :supporting-text="runSupportingText(run)"
          ></nldd-text-cell>
          <nldd-spacer-cell size="8"></nldd-spacer-cell>
          <nldd-icon-cell size="20" icon="chevron-right"></nldd-icon-cell>
        </nldd-list-item>
      </nldd-list>
    </template>
  </template>
</template>
