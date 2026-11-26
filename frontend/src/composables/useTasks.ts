/**
 * useTasks - de open taken van de ingelogde gebruiker (module-singleton).
 *
 * Poll-patroon van useTrajectDocumentJobs (keep-stale on failure), maar met
 * één gedeelde module-state zoals useBwbHarvest: het sidebar-item (badge) en
 * het taken-panel kijken naar dezelfde lijst. Interval bewust ruim (30s) - taken
 * zijn laagfrequent; na eigen acties (enrich-aanvraag, resolve) wordt direct
 * ge-refreshed.
 */
import { ref, computed, watch, onUnmounted, getCurrentInstance, toValue } from 'vue';
import type { MaybeRefOrGetter, WatchSource } from 'vue';
import { apiFetch } from '@regelrecht/frontend-shared';
import type { RunningJob, Task, TaskPayload } from '../lib/taskCategories';
import { jobInTraject, taskInTraject } from '../lib/taskReview';

/** The payload fields read in the editor; the backend stores free-form JSON. */
export interface TaskRecordPayload extends TaskPayload {
  yaml_path?: string | null;
}

/** One task as `/api/tasks` returns it (Task in pipeline/src/tasks.rs). */
export interface TaskRecord extends Task {
  id: string;
  task_type: string;
  status: 'open' | 'approved' | 'rejected' | 'dismissed';
  assignee_account_id: string | null;
  traject_id: string | null;
  job_id: string | null;
  title: string;
  payload: TaskRecordPayload | null;
  created_at: string;
  resolved_at: string | null;
  resolved_by: string | null;
}

/** A running task-flow job (RunningTaskJob in pipeline/src/tasks.rs). */
export interface RunningTaskJob extends RunningJob {
  job_id: string;
  job_type: string;
  law_id: string;
  traject_ref: string | null;
  target_path: string | null;
  status: string;
  created_at: string;
  /** `article` or `law` for a scoped enrichment; absent for the classic run. */
  mode?: string | null;
  /** The work list of a scoped enrichment. */
  articles?: string[] | null;
  /** Set when a member asked to stop the run and it is winding down. */
  cancel_requested_at?: string | null;
}

/** `GET /api/tasks` (TaskListResponse in tasks_api.rs). */
interface TaskListResponse {
  tasks?: TaskRecord[];
  open_count?: number;
  running?: RunningTaskJob[];
}

/** A result file of the task's job (ResultFile in tasks_api.rs). */
export interface ResultFile {
  path: string;
  content: string;
}

/** `GET /api/tasks/:id` (TaskDetailResponse in tasks_api.rs: the task, flattened). */
export interface TaskDetail extends TaskRecord {
  results: ResultFile[];
}

/** One part of an enrichment (JobTaskSummary in enrich_review.rs). */
export interface JobTaskSummary {
  id: string;
  status: TaskRecord['status'];
  title: string;
  article: string | null;
}

/** `GET /api/tasks/jobs/:id` (JobTasksResponse in enrich_review.rs). */
export interface JobTasksResponse {
  job_id: string;
  law_id: string | null;
  tasks: JobTaskSummary[];
  /** `article` or `law` for a scoped enrichment. */
  mode?: string | null;
  /** The engineer's instruction (article mode). */
  instruction?: string | null;
  /** What the reviewer should know that is not a change (Signal in stelsel.rs). */
  signals?: EnrichSignal[];
}

/**
 * Something a scoped enrichment reports that is not a change: an edit outside
 * its scope that was put back, a reader elsewhere it broke, a problem the
 * agent saw in another law. Carries no verdict.
 */
export interface EnrichSignal {
  kind: string;
  law_id?: string | null;
  article?: string | null;
  text: string;
}

/** One verdict in `POST /api/tasks/jobs/:id/apply` (Decision in enrich_review.rs). */
export interface EnrichDecision {
  task_id: string;
  action: 'approved' | 'rejected';
  content?: string | null;
}

export type ResolveAction = 'approved' | 'rejected' | 'dismissed';

/** `POST /api/tasks/jobs/:id/apply` (ApplyResponse in enrich_review.rs). */
export interface ApplyResponse {
  law_id: string;
  accepted: number;
  total: number;
  etag: string | null;
  pr: { url: string; number: number } | null;
}

const POLL_INTERVAL_MS = 30_000;

// Module-level shared state - all callers of useTasks() share these refs.
const tasks = ref<TaskRecord[]>([]);
const openCount = ref(0);
// Lopende taak-flow-aanvragen (pending/processing) - de "Bezig"-sectie.
// Zelfde keep-stale-gedrag als tasks/openCount: een poll-fout laat 'm staan.
const running = ref<RunningTaskJob[]>([]);
const error = ref<unknown>(null);

let timer: ReturnType<typeof setInterval> | null = null;
let consumers = 0;

async function refresh(): Promise<void> {
  try {
    const res = await apiFetch('/api/tasks', { allowStatuses: [401] });
    if (res.status === 401) return;
    const json = (await res.json()) as TaskListResponse | null;
    tasks.value = Array.isArray(json?.tasks) ? json.tasks : [];
    openCount.value = Number.isFinite(json?.open_count) ? (json?.open_count as number) : tasks.value.length;
    running.value = Array.isArray(json?.running) ? json.running : [];
    error.value = null;
  } catch (e) {
    // Keep-stale: een poll-fout wist de badge/lijst niet.
    error.value = e;
  }
}

async function fetchTask(taskId: string): Promise<TaskDetail> {
  const res = await apiFetch(`/api/tasks/${encodeURIComponent(taskId)}`);
  return res.json() as Promise<TaskDetail>;
}

/**
 * De onderdelen van één verrijking: alle review-taken van dezelfde enrich-job,
 * met per taak het artikelnummer en de status. Levert `{ job_id, law_id, tasks }`.
 */
async function fetchJobTasks(jobId: string): Promise<JobTasksResponse> {
  const res = await apiFetch(`/api/tasks/jobs/${encodeURIComponent(jobId)}`);
  return res.json() as Promise<JobTasksResponse>;
}

/**
 * Verwerk een verrijking: alle oordelen in één keer, één schrijfactie, één
 * commit. `decisions` is een lijst `{ task_id, action, content? }`; `etag` is
 * de ETag van de wet zoals de beoordelaar hem zag en gaat als `If-Match` mee -
 * dát is het enige moment waarop deze flow nog op verouderdheid controleert.
 */
async function applyEnrichment(
  jobId: string,
  decisions: readonly EnrichDecision[],
  etag?: string | null,
): Promise<ApplyResponse> {
  const headers: Record<string, string> = { 'Content-Type': 'application/json' };
  if (etag) headers['If-Match'] = etag;
  const res = await apiFetch(`/api/tasks/jobs/${encodeURIComponent(jobId)}/apply`, {
    method: 'POST',
    headers,
    body: JSON.stringify({ decisions }),
    // 412 lossen we hieronder zelf op met een uitlegbare tekst; de rest gooit.
    allowStatuses: [412],
    // Zelfde proxy-guard als saveLaw: alleen text/plain-bodies zijn van onze
    // eigen editor-api, al het andere is een tussenliggende proxy.
    errorMessage: (status, body, contentType) =>
      contentType.startsWith('text/plain') && body ? body : `Verwerken mislukt: ${status}`,
  });
  if (res.status === 412) {
    throw new Error(
      'Het regelwerk is intussen door iemand anders gewijzigd. Herlaad de pagina om de ' +
        'laatste wijzigingen te zien en beoordeel de verrijking daarna opnieuw.',
    );
  }
  const json = (await res.json()) as ApplyResponse;
  await refresh();
  return json;
}

async function resolveTask(taskId: string, action: ResolveAction): Promise<void> {
  await apiFetch(`/api/tasks/${encodeURIComponent(taskId)}/resolve`, {
    method: 'POST',
    headers: { 'Content-Type': 'application/json' },
    body: JSON.stringify({ action }),
  });
  await refresh();
}

// Kaal: vraagt de verrijking aan en raakt de gedeelde takenlijst niet. Zonder
// een refresh erachteraan blijft `running` leeg, slaat de bezig-staat van de
// lege Machine-/YAML-pane niet om en begint usePollWhile nooit te pollen. Een
// view die een wet toont neemt daarom useEnrichState, dat die refresh eraan
// vastknoopt; wie deze rechtstreeks aanroept (TasksListPane, dat zelf al pollt)
// zet er zelf een refresh() achter.
async function requestEnrich(
  trajectRef: string,
  lawId: string,
  options?: string | EnrichOptions,
): Promise<EnrichRequestResult> {
  // A bare string is the provider, as callers have always passed it.
  const body = typeof options === 'string' ? { provider: options } : enrichBody(options);
  // Without any option there is no body, and the deployment's default runs.
  const init = Object.keys(body).length
    ? {
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify(body),
      }
    : {};
  const res = await apiFetch(
    `/api/trajects/${encodeURIComponent(trajectRef)}/corpus/laws/${encodeURIComponent(lawId)}/enrich`,
    { method: 'POST', allowStatuses: [409, 429], ...init }
  );
  let running: RunningScope | null = null;
  if (res.status === 409 && typeof res.json === 'function') {
    // The 409 names the run that is in the way, so the editor can say it is
    // article 3 and not the whole law. An older backend answers in plain text.
    try {
      const conflict = (await res.json()) as { mode?: string | null; articles?: string[] | null };
      running = { mode: conflict.mode ?? null, articles: conflict.articles ?? null };
    } catch {
      running = null;
    }
  }
  return { alreadyRunning: res.status === 409, tooMany: res.status === 429, running };
}

/**
 * What a stop request did (StopOutcome in pipeline/src/traject_runs.rs):
 * `removed` (it had not started), `stopping` (the worker ends it within half a
 * minute) or `done` (nothing left to stop).
 */
export type StopState = 'removed' | 'stopping' | 'done';

/**
 * Stop a running enrichment of the traject. Any member with write access may
 * stop any member's run. Refreshes the shared task list afterwards, so a
 * removed run leaves the "Bezig" list at once.
 */
async function stopRun(trajectRef: string, jobId: string): Promise<StopState> {
  const res = await apiFetch(
    `/api/trajects/${encodeURIComponent(trajectRef)}/runs/${encodeURIComponent(jobId)}`,
    { method: 'DELETE' },
  );
  let state: StopState = 'done';
  try {
    const json = (await res.json()) as { state?: StopState } | null;
    if (json?.state) state = json.state;
  } catch {
    // A body we cannot read still means the request went through.
  }
  await refresh();
  return state;
}

/** What sending the requests of a chain selection came to, per law. */
export interface PlanOutcome {
  started: string[];
  /** Laws with an enrichment already running in this traject (409). */
  conflicts: { lawId: string; running: RunningScope | null }[];
  /** Laws refused because too many enrichments run at once (429). */
  tooMany: string[];
  /** Laws whose request the server refused, with its reason. */
  failed: { lawId: string; message: string }[];
}

/**
 * Send the requests of a chain selection, one per law, one after the other:
 * the server keeps one enrichment per law and traject, and a burst would hit
 * the per-account cap halfway. A refusal for one law does not stop the
 * others; the outcome says which went where. Refreshes the shared list once.
 */
async function requestEnrichPlan(
  trajectRef: string,
  requests: readonly { lawId: string; options: EnrichOptions }[],
): Promise<PlanOutcome> {
  const outcome: PlanOutcome = { started: [], conflicts: [], tooMany: [], failed: [] };
  for (const { lawId, options } of requests) {
    try {
      const result = await requestEnrich(trajectRef, lawId, options);
      if (result.alreadyRunning) outcome.conflicts.push({ lawId, running: result.running ?? null });
      else if (result.tooMany) outcome.tooMany.push(lawId);
      else outcome.started.push(lawId);
    } catch (e) {
      // The editor-api says in plain text what is wrong with a request (an
      // article that does not exist, nothing to enrich); a 5xx says nothing.
      const status = (e as { status?: number }).status;
      const body = (e as { body?: string }).body;
      outcome.failed.push({
        lawId,
        message: status && status < 500 && body ? body : 'Verrijking aanvragen mislukt.',
      });
    }
  }
  await refresh();
  return outcome;
}

/** The scope of a running enrichment, as the 409 and `/api/tasks` report it. */
export interface RunningScope {
  mode: string | null;
  articles: string[] | null;
}

export interface EnrichRequestResult {
  alreadyRunning: boolean;
  tooMany: boolean;
  running?: RunningScope | null;
}

/**
 * What an enrichment may be asked for (EnrichRequest in
 * packages/editor-api/src/enrich_scope.rs). Every field is optional; an empty
 * object is the classic whole-law run on the deployment's provider.
 */
export interface EnrichOptions {
  provider?: string;
  model?: string;
  /** `article`: one article from the article panel; `law`: the whole law. */
  mode?: 'article' | 'law';
  articles?: string[];
  onlyMissing?: boolean;
  /** Send the stelselcontext along (default on in the article mode). */
  context?: boolean;
  feedbackRounds?: number;
  instruction?: string;
}

/** The request body for `options`, leaving out what was not chosen. */
export function enrichBody(options?: EnrichOptions): Record<string, unknown> {
  if (!options) return {};
  const body: Record<string, unknown> = {};
  if (options.provider) body.provider = options.provider;
  if (options.model) body.model = options.model;
  if (options.mode) body.mode = options.mode;
  if (options.articles?.length) body.articles = options.articles;
  if (options.onlyMissing) body.only_missing = true;
  if (options.context !== undefined) body.context = options.context;
  if (options.feedbackRounds !== undefined) body.feedback_rounds = options.feedbackRounds;
  const instruction = options.instruction?.trim();
  if (instruction) body.instruction = instruction;
  return body;
}

export function useTasks() {
  // Increment/register-unmount are both gated on an active component
  // instance, and both need the SAME gate: a caller without one (e.g. a
  // plain module/test context) never decrements either, so counting it in
  // would leak a phantom consumer that keeps the poll interval alive
  // forever. Non-component callers therefore don't influence the poll
  // lifecycle at all - use `useTaskActions()` below if you don't want to
  // join it in the first place.
  const hasInstance = !!getCurrentInstance();
  if (hasInstance) consumers += 1;
  if (!timer) {
    // Defer the initial load a microtask tick so a caller that immediately
    // fires its own request (e.g. requestEnrich right after useTasks()) gets
    // to make its apiFetch call first - keeps this composable's eager load
    // from racing a caller's own synchronous follow-up call for the mock/
    // fetch queue.
    Promise.resolve().then(refresh);
    timer = setInterval(refresh, POLL_INTERVAL_MS);
  }
  if (hasInstance) {
    onUnmounted(() => {
      consumers -= 1;
      if (consumers <= 0 && timer) {
        clearInterval(timer);
        timer = null;
      }
    });
  }
  return {
    tasks,
    running,
    error,
    openCount: computed(() => openCount.value),
    refresh,
    fetchTask,
    resolveTask,
    fetchJobTasks,
    applyEnrichment,
    requestEnrich,
    requestEnrichPlan,
    stopRun,
  };
}

/**
 * useTrajectTasks - useTasks(), narrowed to one traject. The task list is
 * account-wide; every view inside a traject (the task panel, its list, the
 * sidebar badge) reads it through here, so work from another traject never
 * shows up there, not even for the same law. Without a traject nothing
 * matches.
 */
export function useTrajectTasks(trajectRef: MaybeRefOrGetter<string | null | undefined>) {
  // `openCount` is the account-wide server total; it would read as this
  // traject's count, so it is left out.
  const { openCount: _accountWide, ...all } = useTasks();
  return {
    ...all,
    tasks: computed(() => all.tasks.value.filter((t) => taskInTraject(t, toValue(trajectRef)))),
    running: computed(() => all.running.value.filter((j) => jobInTraject(j, toValue(trajectRef)))),
  };
}

// useTaskActions - the non-polling half of useTasks(): action helpers only,
// no shared tasks/openCount state and no consumer/timer registration. For
// callers that only need to fetch/resolve/enrich a task (e.g. EditorView's
// review mode and "Verrijk deze wet" action) and would otherwise start the
// 30s poll unconditionally in setup() - including for anonymous visitors -
// which breaks the invariant that anonymous visitors never poll. Callers
// that DO want the shared, polled task list (the taken-lijst in Home:
// TasksSidebarItem/TasksListPane) keep using useTasks().
export function useTaskActions() {
  // `running` is een module-level ref, gedeeld met useTasks(). Meegeven kost
  // niets en start geen poll: een view die alleen wil weten of er iets loopt,
  // leest hem en ververst zelf na een actie of bij mount.
  return {
    fetchTask,
    resolveTask,
    fetchJobTasks,
    applyEnrichment,
    requestEnrich,
    requestEnrichPlan,
    stopRun,
    refresh,
    running,
    tasks,
  };
}

/**
 * Ververs de takenlijst zolang `active` waar is, en stop zodra hij omslaat of
 * de component verdwijnt.
 *
 * Voor de bezig-staat van een verrijking: die kan minuten duren, dus een
 * permanente poll is verspilling, maar wie er op dat moment naar kijkt wil het
 * zien omslaan zonder zelf te verversen. De kosten hangen zo aan kijktijd in
 * plaats van aan looptijd. Bewust korter dan de 30s van useTasks: dit is een
 * gerichte poll die je zelf in gang hebt gezet.
 */
export function usePollWhile(active: WatchSource<boolean>, intervalMs = 10000): void {
  let pollTimer: ReturnType<typeof setInterval> | null = null;
  const stop = () => {
    if (pollTimer) clearInterval(pollTimer);
    pollTimer = null;
  };
  // Bewust NIET immediate: `running` is bij setup nog leeg, dus er valt op dat
  // moment nooit iets te starten - de eerste refresh laat de watch alsnog
  // afgaan.
  //
  // Wat het weglaten van immediate NIET wegneemt: een watch leest zijn bron
  // hoe dan ook meteen bij aanmaak, om de dependency te leggen. Alles wat
  // `active` aanraakt moet dus al gedeclareerd zijn - een computed die state
  // leest die verderop in het bestand pas komt, gooit hier een ReferenceError
  // (temporal dead zone) en dan rendert de hele view niets. useEnrichState
  // ontloopt dat door zijn bronnen als argument binnen te krijgen: die bestaan
  // per definitie al op de aanroepregel.
  watch(active, (on) => {
    stop();
    if (on) pollTimer = setInterval(refresh, intervalMs);
  });
  if (getCurrentInstance()) onUnmounted(stop);
}
