<script setup lang="ts">
/**
 * TasksListPane - the task list itself, in the main pane of Home. It shows the
 * category chosen in the panel (TasksCategoriesPane); the route carries that
 * choice and this component reads it as props.
 *
 * "Alle taken" is one list, oldest first, with each row's context (a law, or
 * Werkdocumenten) as its supporting text. The other categories are grouped by
 * that context and sorted within each group in the order of the regelwerk.
 * Both orders are in `lib/taskGroups.ts`. Running jobs sit below the tasks, in
 * the list or in the group they belong to. They are not tasks, but they are on
 * your plate: "alle" should not be selective, and a conversion of a
 * werkdocument belongs to that context too.
 *
 * Clicking a task opens its details in a sheet: what it is about, when it
 * arrived, what the run behind it came to, and the actions that settle it
 * (Beoordelen and Verwerpen for a review, Probeer opnieuw and Markeer als
 * gedaan for a failure). A running job opens what it is working on. Whatever
 * else a row offers sits in a separate "more" segment next to it. The two are
 * segments of the row (nldd-list-item-segment), because a row that is itself a
 * control cannot hold a second one.
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
import type { NLDDModalDialog } from '@nldd/design-system/modal-dialog';
import { useTrajectTasks } from '../composables/useTasks';
import type { RunningTaskJob, TaskRecord, TaskRecordPayload } from '../composables/useTasks';
import { useCorpus } from '../corpus';
import { homeTarget } from '../composables/useLastVisitedRoute';
import { reviewTarget, taskNamesTraject } from '../lib/taskReview';
import {
  contextName,
  taskTitle,
  taskRowTitle,
  taskRowLabel,
  runningTitle,
  runningRowTitle,
} from '../lib/taskTitle';
import { groupTasks, listByAge } from '../lib/taskGroups';
import { taskDetailRows } from '../lib/taskDetails';
import { useTrajectRunsStore } from '../stores/trajectRuns';
import { useFeatureFlagsStore } from '../stores/featureFlags';
import type { TaskGroup } from '../lib/taskGroups';
import {
  filterTasks,
  filterRunning,
  taskContext,
  taskLawId,
  jobContext,
  jobLawId,
  ALLE,
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
const { tasks, running, resolveTask, requestEnrich, stopRun, refresh } = useTrajectTasks(() => props.trajectRef);
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

// "Alle taken" is not grouped: it is one list by age, and each row names its
// context instead of a heading doing it.
const byAge = computed(() => props.categorie === ALLE);

const groups = computed((): TaskGroup<TaskRecord, RunningTaskJob>[] => {
  const shown = filterTasks(tasks.value, props.categorie, props.lawId);
  const jobs = filterRunning(running.value, props.categorie, props.lawId);
  if (!byAge.value) return groupTasks(shown, jobs, displayName);
  if (shown.length === 0 && jobs.length === 0) return [];
  return [{ key: ALLE, context: null, lawId: null, name: 'Alle taken', ...listByAge(shown, jobs, displayName) }];
});

// A heading per group, unless the list is not grouped, or the page title
// already names the one context the list can hold: a single law, or
// Werkdocumenten.
const showHeadings = computed(
  () => !byAge.value && !(props.categorie === WET && props.lawId) && props.categorie !== WERKDOCUMENTEN,
);

// Without headings, the row says what it is about. Under a heading the
// supporting text stays empty: it would repeat the heading on every row.
function taskSupportingText(task: TaskRecord): string | undefined {
  return byAge.value ? contextName(taskContext(task), taskLawId(task), displayName) : undefined;
}
function jobSupportingText(job: RunningTaskJob): string | undefined {
  const context = byAge.value ? contextName(jobContext(job), jobLawId(job), displayName) : undefined;
  if (!job.cancel_requested_at) return context;
  return context ? `Stoppen… · ${context}` : 'Stoppen…';
}

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
// Clicking a task opens it here: what it is about and what the run behind it
// came to, together with what you can do about it. For a failure that includes
// the raw error, which does not belong in the row: it is a technical string of
// sometimes two hundred characters ("no push token for traject source '…'
// (expected env CORPUS_AUTH_…)").
//
// One sheet for the whole list, not one per row; `detailTask` decides what it
// shows. nldd-sheet has imperative show()/hide(), mirrored from a watcher (same
// pattern as TrajectMenu).
const detailSheetEl = ref<NLDDSheet | null>(null);
const detailTask = ref<TaskWithError | null>(null);
const detailError = ref<string | null>(null);
watch(detailTask, async (task) => {
  await nextTick();
  if (task) detailSheetEl.value?.show();
  else detailSheetEl.value?.hide();
});

// The run behind the task, from the traject's runs. Loaded when a sheet opens,
// once: the facts it adds (model, how far the review is) do not need to tick
// along while you read them.
const runs = useTrajectRunsStore();
const detailRows = computed(() =>
  detailTask.value ? taskDetailRows(detailTask.value, runs.byId(detailTask.value.job_id), displayName) : [],
);

function showDetails(task: TaskRecord): void {
  detailError.value = null;
  detailTask.value = task;
  // A law's task asks for that law's runs only: the server filters them, so the
  // cap on the traject's whole list cannot leave the run out.
  if (task.job_id && props.trajectRef) {
    void runs.load({ trajectRef: props.trajectRef, lawId: taskLawId(task) });
  }
}
function closeDetails(): void {
  detailTask.value = null;
}

// "Verwerpen" asks first, as it does in the editor: the proposal is gone
// afterwards. The dialog opens over the sheet, so the task stays in view.
const rejectConfirm = ref<NLDDModalDialog | null>(null);
async function confirmReject(): Promise<void> {
  rejectConfirm.value?.hide();
  const task = detailTask.value;
  if (!task) return;
  try {
    await resolveTask(task.id, 'rejected');
    closeDetails();
  } catch {
    detailError.value = 'Verwerpen is mislukt. Probeer het opnieuw.';
  }
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
function taskColor(task: TaskRecord): 'critical' | undefined {
  return task.task_type === 'job_failed' ? 'critical' : undefined;
}

// "Beoordelen" in the sheet is a real link: the destination is fully
// deep-linkable (the review hangs off the URL - EditorView reads `?task=` on
// entry), so it should do what a link does: middle click, ctrl/cmd click, "open
// in new tab", "copy link address". `router.resolve` builds the URL, so the
// route definitions stay the single source of truth. A task without a usable
// destination gets no Beoordelen at all instead of a broken link.
function reviewHref(task: TaskRecord): string | undefined {
  const target = reviewTarget(task);
  return target ? router.resolve(target).href : undefined;
}

// A plain click stays SPA navigation; the button's <a> does no
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

// With the scope modes on, a running enrichment can be stopped from its row,
// in the same "more" menu where a conversion is cancelled.
const featureFlags = useFeatureFlagsStore();
function stoppable(job: RunningTaskJob): boolean {
  return featureFlags.isEnabled('enrich.scoped') && job.job_type === 'enrich';
}
// Rows with a menu: a conversion (cancel) and a stoppable enrichment.
function hasMenu(job: RunningTaskJob): boolean {
  return jobContext(job) === WERKDOCUMENTEN || stoppable(job);
}

// Stopping asks first, like Verwerpen: what the run did so far is lost.
const stopConfirm = ref<NLDDModalDialog | null>(null);
const stopTarget = ref<RunningTaskJob | null>(null);
const stopError = ref<string | null>(null);
function askStop(job: RunningTaskJob): void {
  stopTarget.value = job;
  stopError.value = null;
  stopConfirm.value?.show();
}
async function confirmStop(): Promise<void> {
  stopConfirm.value?.hide();
  const job = stopTarget.value;
  stopTarget.value = null;
  if (!job || !props.trajectRef) return;
  try {
    await stopRun(props.trajectRef, job.job_id);
  } catch {
    stopError.value = 'Stoppen is mislukt. Probeer het opnieuw.';
  }
}

function openJob(job: RunningTaskJob): void {
  if (!jobDestination(job)) return;
  if (jobContext(job) === WERKDOCUMENTEN) viewDocument(job);
  else viewLaw(job);
}
</script>

<template>
  <template v-if="stopError">
    <nldd-inline-dialog variant="alert" :text="stopError"></nldd-inline-dialog>
    <nldd-spacer size="16"></nldd-spacer>
  </template>
  <template v-if="!isEmpty">
    <template v-for="(group, index) in groups" :key="group.key">
      <template v-if="showHeadings">
        <nldd-spacer v-if="index > 0" size="24"></nldd-spacer>
        <nldd-title size="display-5"><h4>{{ group.name }}</h4></nldd-title>
        <nldd-spacer size="8"></nldd-spacer>
      </template>

      <nldd-list appearance="simple">
        <nldd-list-item v-for="task in group.tasks" :key="task.id" size="md">
          <!-- Main segment: the task's details, with what you can do about
               it. A failure shows in the critical color. -->
          <nldd-list-item-segment
            width="full"
            button
            :accessible-label="`${taskRowLabel(task, displayName)}, toon details`"
            @click="showDetails(task)"
          >
            <nldd-icon-cell size="20" :icon="taskIcon(task)" :color="taskColor(task)"></nldd-icon-cell>
            <nldd-spacer-cell size="8"></nldd-spacer-cell>
            <nldd-text-cell
              :text="taskRowTitle(task)"
              :supporting-text="taskSupportingText(task)"
              :color="taskColor(task)"
            ></nldd-text-cell>
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
          <nldd-list-item v-if="hasMenu(job)" size="md">
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
              <nldd-text-cell
              :text="runningRowTitle(job, jobLawName)"
              :supporting-text="jobSupportingText(job)"
            ></nldd-text-cell>
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
            <nldd-text-cell
              :text="runningRowTitle(job, jobLawName)"
              :supporting-text="jobSupportingText(job)"
            ></nldd-text-cell>
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
        <nldd-menu v-else-if="stoppable(job)" :id="jobMenuId(job)">
          <!-- Stopping ends the run without a proposal; a run that is already
               winding down has nothing left to stop. -->
          <nldd-menu-item
            destructive
            text="Stop verrijking"
            :disabled="!!job.cancel_requested_at || undefined"
            @select="askStop(job)"
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
          <template v-if="detailError">
            <nldd-inline-dialog variant="alert" :text="detailError"></nldd-inline-dialog>
            <nldd-spacer size="16"></nldd-spacer>
          </template>
          <!-- Laid out like the table on a law's Algemeen page. -->
          <nldd-list v-if="detailRows.length" appearance="box-tinted" accessible-label="Gegevens van de taak">
            <nldd-list-item v-for="row in detailRows" :key="row.label">
              <nldd-text-cell :text="row.label" width="200px"></nldd-text-cell>
              <nldd-spacer-cell size="16"></nldd-spacer-cell>
              <nldd-text-cell :text="row.value" horizontal-alignment="right"></nldd-text-cell>
            </nldd-list-item>
          </nldd-list>
          <template v-if="detailTask.payload?.error">
            <nldd-spacer size="24"></nldd-spacer>
            <nldd-rich-text>
              <h4>Foutmelding</h4>
              <p>{{ detailTask.payload.error }}</p>
            </nldd-rich-text>
          </template>
        </nldd-simple-section>
        <!-- A review: judge the proposal in the editor, or reject it here. -->
        <nldd-container v-if="detailTask && detailTask.task_type !== 'job_failed'" slot="footer" padding="16">
          <template v-if="reviewHref(detailTask)">
            <nldd-button
              appearance="primary"
              size="md"
              width="full"
              text="Beoordelen"
              :href="reviewHref(detailTask)"
              @click="onReviewClick($event, detailTask)"
            ></nldd-button>
            <nldd-spacer size="8"></nldd-spacer>
          </template>
          <nldd-button
            appearance="secondary"
            size="md"
            width="full"
            text="Verwerpen"
            @click="rejectConfirm?.show()"
          ></nldd-button>
        </nldd-container>
        <!-- A failure: what you can do about it, right under what went wrong. -->
        <nldd-container v-else-if="detailTask" slot="footer" padding="16">
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

    <nldd-modal-dialog
      ref="stopConfirm"
      variant="alert"
      icon="exclamation-triangle"
      text="Verrijking stoppen?"
      supporting-text="Wat de verrijking tot nu toe deed gaat verloren. Er komt geen voorstel."
    >
      <nldd-button
        slot="actions"
        appearance="primary"
        text="Laat doorlopen"
        @click="stopConfirm?.hide()"
      ></nldd-button>
      <nldd-button
        slot="actions"
        appearance="destructive"
        text="Stop verrijking"
        @click="confirmStop"
      ></nldd-button>
    </nldd-modal-dialog>

    <!-- Same question as Verwerp voorstel in the editor (AppShell). -->
    <nldd-modal-dialog
      ref="rejectConfirm"
      variant="alert"
      icon="exclamation-triangle"
      text="Voorstel verwerpen?"
      supporting-text="De taak wordt afgesloten en het voorstel gaat verloren."
    >
      <nldd-button
        slot="actions"
        appearance="primary"
        text="Behoud voorstel"
        @click="rejectConfirm?.hide()"
      ></nldd-button>
      <nldd-button
        slot="actions"
        appearance="destructive"
        text="Verwerp voorstel"
        @click="confirmReject"
      ></nldd-button>
    </nldd-modal-dialog>
  </Teleport>
</template>
