<script setup lang="ts">
/**
 * TasksListPane - the task list itself, in the main pane of Home. It shows the
 * category chosen in the panel (TasksCategoriesPane); the route carries that
 * choice and this component reads it as props.
 *
 * The list is grouped by what the work is about (a law, or Werkdocumenten) and
 * sorted within each group in the order of the regelwerk; see
 * `lib/taskGroups.ts`. Running jobs sit at the bottom of the group they belong
 * to. They are not tasks, but they are on your plate: "alle" should not be
 * selective, and a conversion of a werkdocument belongs to that context too.
 *
 * Every row does its main thing when you click it: a review opens the article
 * in the editor, a failure opens its details, a running job opens what it is
 * working on. Whatever else a row offers sits in a separate "more" segment
 * next to it, so the one thing you came to do is never behind a menu. The two
 * are segments of the row (nldd-list-item-segment), because a row that is
 * itself a link cannot hold a second control.
 *
 * The actions with side effects that carry their own error handling (the file
 * picker, cancelling a conversion, opening a running conversion) are emitted;
 * LibraryView already owns that wiring, including the error modals. The rest
 * (navigating, dismissing, requesting again) happens here.
 *
 * Mounting starts the shared 30s poll through `useTasks()`; the route behind it
 * requires login, so anonymous visitors never poll.
 */
import { computed, nextTick, onMounted, ref, watch } from 'vue';
import { useRouter } from 'vue-router';
import type { NLDDSheet } from '@nldd/design-system/sheet';
import { useTrajectTasks } from '../composables/useTasks';
import type { RunningTaskJob, TaskRecord, TaskRecordPayload } from '../composables/useTasks';
import { useCorpus } from '../corpus';
import { homeTarget } from '../composables/useLastVisitedRoute';
import { reviewTarget, taskNamesTraject } from '../lib/taskReview';
import { taskTitle, taskRowTitle, taskRowLabel, runningTitle, runningRowTitle } from '../lib/taskTitle';
import { groupTasks } from '../lib/taskGroups';
import {
  filterTasks,
  filterRunning,
  taskContext,
  taskLawId,
  jobContext,
  jobLawId,
  WACHTEN,
  WERKDOCUMENTEN,
  WET,
} from '../lib/taskCategories';

const props = withDefaults(
  defineProps<{
    trajectRef?: string | null;
    categorie?: string | null;
    lawId?: string | null;
  }>(),
  { trajectRef: null, categorie: null, lawId: null },
);

// `upload` opens the file picker, `cancel-job` cancels a running conversion,
// `view-job` opens the job view of a running conversion - all three belong to
// LibraryView, which owns viewingJobPath and the error modals.
const emit = defineEmits<{
  upload: [];
  'cancel-job': [job: RunningTaskJob];
  'view-job': [path: string];
}>();

// A failed job's task carries the raw error in its (free-form) payload.
interface TaskWithError extends TaskRecord {
  payload: (TaskRecordPayload & { error?: string | null }) | null;
}

const router = useRouter();
const { tasks, running, resolveTask, requestEnrich, refresh } = useTrajectTasks(() => props.trajectRef);
const { displayName } = useCorpus();

// runningTitle may pass a job without a law_id; displayName (via humanizeLawId)
// renders a missing id as '' either way, so '' stands in for it here.
function jobLawName(lawId: string | null | undefined): string {
  return displayName(lawId ?? '');
}

// The shared list already runs its 30s poll, but that was started elsewhere
// and is halfway through an interval when you arrive. Whoever opens a list
// wants to see it current, so opening it is the moment to refresh.
onMounted(refresh);

const groups = computed(() =>
  groupTasks(
    filterTasks(tasks.value, props.categorie, props.lawId),
    filterRunning(running.value, props.categorie, props.lawId),
    displayName,
  ),
);

// A heading per group, unless the page title already names the one context
// the list can hold: a single law, or Werkdocumenten.
const showHeadings = computed(
  () => !(props.categorie === WET && props.lawId) && props.categorie !== WERKDOCUMENTEN,
);

const isEmpty = computed(() => groups.value.length === 0);

// Menu ids: the prefix keeps the id spaces of tasks and jobs apart, and from
// any other id on the page.
function taskMenuId(task: TaskRecord): string {
  return `task-menu-${task.id}`;
}
function jobMenuId(job: RunningTaskJob): string {
  return `job-menu-${job.job_id}`;
}

function dismiss(task: TaskRecord): void {
  closeDetails();
  resolveTask(task.id, 'dismissed');
}

// --- Task details sheet ---
// The raw error does not belong in the row: it is a technical string of
// sometimes two hundred characters ("no push token for traject source '…'
// (expected env CORPUS_AUTH_…)"). Clicking a failed task opens it here,
// together with what you can do about it.
//
// One sheet for the whole list, not one per row; `detailTask` decides what it
// shows. nldd-sheet has imperative show()/hide(), mirrored from a watcher (same
// pattern as TrajectMenu).
const detailSheetEl = ref<NLDDSheet | null>(null);
const detailTask = ref<TaskWithError | null>(null);
watch(detailTask, async (task) => {
  await nextTick();
  if (task) detailSheetEl.value?.show();
  else detailSheetEl.value?.hide();
});
function showDetails(task: TaskRecord): void {
  detailTask.value = task;
}
function closeDetails(): void {
  detailTask.value = null;
}

// "Probeer opnieuw" on a failed task. Two mechanisms behind one intent: a
// conversion can only run again through a new upload (the job runs with
// max_attempts=1 on purpose, see corpus_handlers.rs), an enrichment you simply
// request again.
async function retry(task: TaskRecord): Promise<void> {
  closeDetails();
  if (taskContext(task) === WERKDOCUMENTEN) {
    emit('upload');
    return;
  }
  const lawId = taskLawId(task);
  if (!lawId || !props.trajectRef) return;
  await requestEnrich(props.trajectRef, lawId);
  await refresh();
}

// Two icons, by action: a filled exclamation circle means something broke
// (same circle family as the Prioriteit entry, which carries the open variant),
// 'eyeglasses' means this is for you to look at and judge.
function taskIcon(task: TaskRecord): string {
  return task.task_type === 'job_failed' ? 'exclamation-circle-filled' : 'eyeglasses';
}

// A review row is a real link: the destination is fully deep-linkable (the
// review hangs off the URL - EditorView reads `?task=` on entry), so it should
// do what a link does: middle click, ctrl/cmd click, "open in new tab", "copy
// link address". `router.resolve` builds the URL, so the route definitions stay
// the single source of truth. A task without a usable destination gets no href
// and a disabled button instead of a broken link.
function reviewHref(task: TaskRecord): string | undefined {
  const target = reviewTarget(task);
  return target ? router.resolve(target).href : undefined;
}

// A plain click stays SPA navigation; the segment's <a> does no
// preventDefault, so without this the browser reloads the whole page. Same
// rule as router-link: only an unmodified primary click is taken over, the
// rest (new tab, new window) is left to the browser. Enter on the focused link
// fires a click too, so the keyboard takes the same path.
function onReviewClick(event: MouseEvent, task: TaskRecord): void {
  if (event.defaultPrevented || event.button !== 0) return;
  if (event.metaKey || event.ctrlKey || event.shiftKey || event.altKey) return;
  const target = reviewTarget(task);
  if (!target) return;
  event.preventDefault();
  router.push(target);
}

// A running conversion: the .md does not exist yet, so we want the job view
// ("Aan het converteren…"), not the document. Through an emit rather than
// router.push, so LibraryView sets viewingJobPath before navigating -
// otherwise it first opens the path as a plain document (the jobs list has not
// been polled yet on entry) and the 404/"failed" state flashes.
function viewDocument(job: RunningTaskJob): void {
  if (!job.target_path) return;
  emit('view-job', job.target_path);
}

// A running enrichment cannot be cancelled (that endpoint exists only for
// conversions), but the law itself exists, so the row opens it in the library.
function viewLaw(job: RunningTaskJob): void {
  const lawId = jobLawId(job);
  if (!lawId) return;
  router.push(homeTarget({ trajectRef: props.trajectRef || undefined, lawId }));
}

// Whether a running job has something to open: the document a conversion
// writes, or the law an enrichment works on.
function jobDestination(job: RunningTaskJob): boolean {
  return jobContext(job) === WERKDOCUMENTEN ? Boolean(job.target_path) : Boolean(jobLawId(job));
}

function openJob(job: RunningTaskJob): void {
  if (!jobDestination(job)) return;
  if (jobContext(job) === WERKDOCUMENTEN) viewDocument(job);
  else viewLaw(job);
}
</script>

<template>
  <template v-if="!isEmpty">
    <template v-for="(group, index) in groups" :key="group.key">
      <template v-if="showHeadings">
        <nldd-spacer v-if="index > 0" size="24"></nldd-spacer>
        <nldd-title size="display-5"><h4>{{ group.name }}</h4></nldd-title>
        <nldd-spacer size="8"></nldd-spacer>
      </template>

      <nldd-list appearance="simple">
        <nldd-list-item v-for="task in group.tasks" :key="task.id" size="md">
          <!-- Main segment: what you came to do. A failure opens its details
               (what went wrong comes before retrying or dismissing it); a
               review is a link to the article in the editor. -->
          <nldd-list-item-segment
            v-if="task.task_type === 'job_failed'"
            width="full"
            button
            :accessible-label="`${taskRowLabel(task, displayName)}, toon details`"
            @click="showDetails(task)"
          >
            <nldd-icon-cell size="20" :icon="taskIcon(task)" color="critical"></nldd-icon-cell>
            <nldd-spacer-cell size="8"></nldd-spacer-cell>
            <nldd-text-cell :text="taskRowTitle(task)" color="critical"></nldd-text-cell>
            <nldd-spacer-cell size="8"></nldd-spacer-cell>
            <nldd-icon-cell size="20" icon="chevron-right"></nldd-icon-cell>
          </nldd-list-item-segment>
          <nldd-list-item-segment
            v-else
            width="full"
            :href="reviewHref(task)"
            :button="!reviewHref(task) || undefined"
            :disabled="!reviewTarget(task) || undefined"
            :accessible-label="taskRowLabel(task, displayName)"
            @click="onReviewClick($event, task)"
          >
            <nldd-icon-cell size="20" :icon="taskIcon(task)"></nldd-icon-cell>
            <nldd-spacer-cell size="8"></nldd-spacer-cell>
            <nldd-text-cell :text="taskRowTitle(task)"></nldd-text-cell>
            <nldd-spacer-cell size="8"></nldd-spacer-cell>
            <nldd-icon-cell size="20" icon="chevron-right"></nldd-icon-cell>
          </nldd-list-item-segment>

          <!-- More segment: the rest, in a menu. "Markeer als gedaan" stays
               here rather than on the row: it cannot be undone, and resolving
               a task deletes the proposal it carries. -->
          <nldd-list-item-segment
            button
            :popovertarget="taskMenuId(task)"
            :accessible-label="`Acties voor ${taskRowLabel(task, displayName)}`"
          >
            <nldd-icon-cell size="20" icon="more"></nldd-icon-cell>
          </nldd-list-item-segment>
        </nldd-list-item>

        <!-- A running job has at most one thing to open. With only that, the
             row itself is the control (the design system's rule for a single
             action); a conversion can also be cancelled, so it gets segments.
             A job with nothing to open (a law conversion, a harvest) is a
             plain row: a chevron there would promise a destination. -->
        <template v-for="job in group.running" :key="job.job_id">
          <nldd-list-item v-if="jobContext(job) === WERKDOCUMENTEN" size="md">
            <nldd-list-item-segment
              width="full"
              button
              :disabled="!jobDestination(job) || undefined"
              :accessible-label="runningTitle(job, jobLawName)"
              @click="openJob(job)"
            >
              <nldd-cell vertical-alignment="center">
                <nldd-activity-indicator size="20" timing="instant" :text="runningTitle(job, jobLawName)"></nldd-activity-indicator>
              </nldd-cell>
              <nldd-spacer-cell size="8"></nldd-spacer-cell>
              <nldd-text-cell :text="runningRowTitle(job, jobLawName)"></nldd-text-cell>
              <nldd-spacer-cell size="8"></nldd-spacer-cell>
              <nldd-icon-cell size="20" icon="chevron-right"></nldd-icon-cell>
            </nldd-list-item-segment>
            <nldd-list-item-segment
              button
              :popovertarget="jobMenuId(job)"
              :accessible-label="`Acties voor ${runningTitle(job, jobLawName)}`"
            >
              <nldd-icon-cell size="20" icon="more"></nldd-icon-cell>
            </nldd-list-item-segment>
          </nldd-list-item>
          <nldd-list-item
            v-else
            size="md"
            :button="jobDestination(job) || undefined"
            @click="openJob(job)"
          >
            <!-- A plain cell, not an icon cell: an activity indicator is not an
                 icon and does not need that cell's icon scale.
                 timing="instant": the 1000ms anti-flash delay is meant for
                 loaders that are gone again quickly. These stay for minutes,
                 so the delay only leaves a gap where the row is already shown. -->
            <nldd-cell vertical-alignment="center">
              <nldd-activity-indicator size="20" timing="instant" :text="runningTitle(job, jobLawName)"></nldd-activity-indicator>
            </nldd-cell>
            <nldd-spacer-cell size="8"></nldd-spacer-cell>
            <nldd-text-cell :text="runningRowTitle(job, jobLawName)"></nldd-text-cell>
            <template v-if="jobDestination(job)">
              <nldd-spacer-cell size="8"></nldd-spacer-cell>
              <nldd-icon-cell size="20" icon="chevron-right"></nldd-icon-cell>
            </template>
          </nldd-list-item>
        </template>
      </nldd-list>
    </template>

    <!-- The menus live outside the list: a list item's slot only takes cells
         and segments. Each segment above points at its menu through
         popovertarget, which also positions the menu against it. -->
    <template v-for="group in groups" :key="`menus-${group.key}`">
      <nldd-menu v-for="task in group.tasks" :id="taskMenuId(task)" :key="task.id">
        <template v-if="task.task_type === 'job_failed'">
          <nldd-menu-item
            v-if="taskNamesTraject(task)"
            text="Probeer opnieuw"
            @select="retry(task)"
          ></nldd-menu-item>
          <nldd-menu-divider v-if="taskNamesTraject(task)"></nldd-menu-divider>
        </template>
        <!-- "Markeer als gedaan" is `dismissed`: off your list, without a
             verdict. `rejected` (the proposal is wrong) belongs in the review
             UI, where you see what you reject. -->
        <nldd-menu-item text="Markeer als gedaan" @select="dismiss(task)"></nldd-menu-item>
      </nldd-menu>
      <template v-for="job in group.running" :key="job.job_id">
        <nldd-menu v-if="jobContext(job) === WERKDOCUMENTEN" :id="jobMenuId(job)">
          <!-- Cancelling throws away the job and the uploaded source. -->
          <nldd-menu-item
            destructive
            text="Annuleer conversie"
            @select="emit('cancel-job', job)"
          ></nldd-menu-item>
        </nldd-menu>
      </template>
    </template>
  </template>

  <!-- The empty state differs per category: under "Wachten op" nothing is
       running, elsewhere there is nothing to do. -->
  <nldd-inline-dialog
    v-else-if="categorie === WACHTEN"
    text="Niets loopt op dit moment."
    supporting-text="Een verrijking of conversie die je aanvraagt verschijnt hier tot hij klaar is."
  ></nldd-inline-dialog>
  <nldd-inline-dialog v-else text="Geen taken"></nldd-inline-dialog>

  <!-- Teleported to body: this list hangs in a split-view pane, and a sheet
       left there as a sibling takes the pane height through ::slotted
       flex-grow. Same reason as the create sheet in TrajectMenu. -->
  <Teleport to="body">
    <nldd-sheet
      ref="detailSheetEl"
      placement="right"
      width="520px"
      @close="closeDetails"
    >
      <nldd-page sticky-header sticky-footer>
        <nldd-top-title-bar
          slot="header"
          text="Taakdetails"
          dismiss-text="Sluit"
          @dismiss="closeDetails"
        ></nldd-top-title-bar>
        <nldd-simple-section v-if="detailTask">
          <nldd-title size="display-3"><h3>{{ taskTitle(detailTask, displayName) }}</h3></nldd-title>
          <nldd-spacer size="16"></nldd-spacer>
          <nldd-rich-text v-if="detailTask.payload?.error">
            <h4>Foutmelding</h4>
            <p>{{ detailTask.payload.error }}</p>
          </nldd-rich-text>
        </nldd-simple-section>
        <!-- What you can do about it, right under what went wrong. -->
        <nldd-container v-if="detailTask" slot="footer" padding="16">
          <template v-if="taskNamesTraject(detailTask)">
            <nldd-button
              appearance="primary"
              size="md"
              width="full"
              text="Probeer opnieuw"
              @click="retry(detailTask)"
            ></nldd-button>
            <nldd-spacer size="8"></nldd-spacer>
          </template>
          <nldd-button
            appearance="secondary"
            size="md"
            width="full"
            text="Markeer als gedaan"
            @click="dismiss(detailTask)"
          ></nldd-button>
        </nldd-container>
      </nldd-page>
    </nldd-sheet>
  </Teleport>
</template>
