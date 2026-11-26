<script setup lang="ts">
/**
 * RunDetailSheet - the details of one run of a law, in a sheet over its
 * Verwerkingen page. The same flow as a task in the task list and a job in the
 * harvester: a list, and a sheet with the details of the row you clicked. The
 * route does not carry the open run, as it does not carry the open task.
 *
 * `jobId` opens the sheet; closing it emits `close`, and the parent clears
 * `jobId`. nldd-sheet has imperative show()/hide(), mirrored from a watcher
 * (same pattern as TasksListPane and TrajectMenu).
 */
import { computed, nextTick, ref, watch } from 'vue';
import type { NLDDSheet } from '@nldd/design-system/sheet';
import RunDetailPane from './RunDetailPane.vue';
import { useTrajectRunsStore } from '../stores/trajectRuns';
import { runLawTitle } from '../lib/trajectRuns';

const props = withDefaults(
  defineProps<{
    jobId?: string | null;
    /** The law the page is about, under the title. */
    lawName?: string | null;
  }>(),
  { jobId: null, lawName: null },
);

const emit = defineEmits<{
  close: [];
  'open-tasks': [lawId: string];
}>();

const store = useTrajectRunsStore();
const title = computed(() => {
  const run = store.byId(props.jobId);
  return run ? runLawTitle(run) : 'Verwerking';
});

const sheetEl = ref<NLDDSheet | null>(null);
watch(
  () => props.jobId,
  async (jobId) => {
    await nextTick();
    if (jobId) sheetEl.value?.show();
    else sheetEl.value?.hide();
  },
  { immediate: true },
);
</script>

<template>
  <!-- Teleported to body: the page hangs in a split-view pane, and a sheet
       left there as a sibling takes the pane height through ::slotted
       flex-grow. Same reason as the details sheet in TasksListPane. -->
  <Teleport to="body">
    <nldd-sheet ref="sheetEl" placement="right" width="520px" @close="emit('close')">
      <nldd-page sticky-header>
        <nldd-top-title-bar
          slot="header"
          text="Verwerkingsdetails"
          dismiss-text="Sluit"
          @dismiss="emit('close')"
        ></nldd-top-title-bar>
        <nldd-simple-section v-if="jobId">
          <nldd-title size="display-3">
            <h3>{{ title }}</h3>
            <p v-if="lawName" slot="supporting-text">{{ lawName }}</p>
          </nldd-title>
          <nldd-spacer size="16"></nldd-spacer>
          <RunDetailPane :job-id="jobId" @open-tasks="emit('open-tasks', $event)"></RunDetailPane>
        </nldd-simple-section>
      </nldd-page>
    </nldd-sheet>
  </Teleport>
</template>
