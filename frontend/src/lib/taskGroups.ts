/**
 * taskGroups - the order of the task list: grouped by what the work is about,
 * and within a group in the order of the regelwerk itself.
 *
 * The server returns tasks newest first. An enrichment creates one task per
 * changed article, and two enrichments running side by side interleave, so in
 * server order article 10 of one law sits between article 31 and article 3 of
 * another. Grouping per context and sorting by article number turns that into
 * a list you can work through from top to bottom.
 *
 * Group order: a group with a failed task comes first, because a failure is
 * stuck until someone steps in and should not sit below a scroll. Otherwise the
 * order follows the categories panel (TasksCategoriesPane): Werkdocumenten
 * first, then each law by name, so the list reads in the same order as the
 * panel next to it and a group does not move when a task is added or resolved.
 * Work without a recognisable context goes last.
 *
 * Within a group: failed tasks first, because nothing moves until someone
 * steps in; then the reviews in article order (a task without an article
 * covers the whole regelwerk and goes before its articles); then what is still
 * running, since there is nothing to do about that yet.
 */
import { isPrioriteit, jobContext, jobLawId, taskContext, taskLawId, WERKDOCUMENTEN, WET } from './taskCategories';
import type { RunningJob, Task, TaskContext } from './taskCategories';

export interface TaskGroup<T extends Task, J extends RunningJob> {
  /** Stable key for rendering: `werkdocumenten`, `wet:<law_id>` or `overig`. */
  key: string;
  context: TaskContext | null;
  lawId: string | null;
  /** Display name: the law's name, `Werkdocumenten`, or `Overig`. */
  name: string;
  tasks: T[];
  running: J[];
}

const OVERIG = 'overig';

function groupKey(context: TaskContext | null, lawId: string | null): string {
  if (context === WERKDOCUMENTEN) return WERKDOCUMENTEN;
  if (context === WET && lawId) return `${WET}:${lawId}`;
  return OVERIG;
}

// Numeric-aware, so article 2 comes before article 10 and 1a after 1.
const collator = new Intl.Collator('nl', { numeric: true, sensitivity: 'base' });

function fileName(path: string | null | undefined): string {
  return path?.split('/').pop() ?? '';
}

// What a task is sorted on within its group: the article for a law, the file
// name for a werkdocument. A law task without an article covers the whole
// regelwerk and sorts before its articles.
function sortLabel(task: Task): string {
  if (taskContext(task) === WERKDOCUMENTEN) return fileName(task.payload?.target_path);
  const article = task.payload?.article;
  return article == null ? '' : String(article);
}

function compareTasks(a: Task, b: Task): number {
  const failed = Number(isPrioriteit(b)) - Number(isPrioriteit(a));
  if (failed !== 0) return failed;
  return collator.compare(sortLabel(a), sortLabel(b));
}

/**
 * Group tasks and running jobs by context, in the order described above.
 * `nameFor` turns a law_id into its display name (useCorpusLaws' displayName).
 * Ties keep the server order: `Array.prototype.sort` is stable.
 */
export function groupTasks<T extends Task, J extends RunningJob>(
  tasks: readonly T[],
  running: readonly J[],
  nameFor: (lawId: string) => string = (id) => id,
): TaskGroup<T, J>[] {
  const groups = new Map<string, TaskGroup<T, J>>();
  const groupFor = (context: TaskContext | null, lawId: string | null): TaskGroup<T, J> => {
    const key = groupKey(context, lawId);
    let group = groups.get(key);
    if (!group) {
      const known = key !== OVERIG;
      const name = context === WERKDOCUMENTEN ? 'Werkdocumenten' : known && lawId ? nameFor(lawId) : 'Overig';
      group = { key, context: known ? context : null, lawId: known ? lawId : null, name, tasks: [], running: [] };
      groups.set(key, group);
    }
    return group;
  };

  for (const task of tasks) groupFor(taskContext(task), taskLawId(task)).tasks.push(task);
  for (const job of running) groupFor(jobContext(job), jobLawId(job)).running.push(job);

  const failed = (g: TaskGroup<T, J>) => (g.tasks.some(isPrioriteit) ? 0 : 1);
  const rank = (g: TaskGroup<T, J>) => (g.key === WERKDOCUMENTEN ? 0 : g.key === OVERIG ? 2 : 1);
  return [...groups.values()]
    .map((g) => ({ ...g, tasks: [...g.tasks].sort(compareTasks) }))
    .sort((a, b) => failed(a) - failed(b) || rank(a) - rank(b) || a.name.localeCompare(b.name, 'nl'));
}

const HOUR_MS = 60 * 60 * 1000;

// The hour an item was created in, for ordering by age. Whole hours, not
// milliseconds: an enrichment creates its tasks one by one, so ordering on the
// exact moment scatters the articles of one law. A missing or unreadable
// timestamp counts as the newest, so it ends up at the bottom rather than on
// top.
function createdHour(item: { created_at?: string | null }): number {
  const time = item.created_at ? Date.parse(item.created_at) : Number.NaN;
  return Number.isNaN(time) ? Number.POSITIVE_INFINITY : Math.floor(time / HOUR_MS);
}

// What an item is sorted on within its hour: its context by name, then its
// place in that context (an article number, a file name).
interface SortKey {
  context: string;
  label: string;
}

function contextName(context: TaskContext | null, lawId: string | null, nameFor: (lawId: string) => string): string {
  if (context === WERKDOCUMENTEN) return 'Werkdocumenten';
  return context === WET && lawId ? nameFor(lawId) : '';
}

function byHourThenName<I extends { created_at?: string | null }>(
  items: readonly I[],
  key: (item: I) => SortKey,
): I[] {
  const keyed = items.map((item) => ({ item, hour: createdHour(item), key: key(item) }));
  keyed.sort(
    (a, b) =>
      // Infinity - Infinity is NaN; two items without a time are a tie.
      (a.hour === b.hour ? 0 : a.hour - b.hour) ||
      collator.compare(a.key.context, b.key.context) ||
      collator.compare(a.key.label, b.key.label),
  );
  return keyed.map((k) => k.item);
}

/**
 * The order of "Alle taken": one list without headings, oldest first, so what
 * has waited longest is on top. That list spans every context, and a heading
 * per law would split it back up; each row names its context in its supporting
 * text instead.
 *
 * Age counts in whole hours. Within an hour the list goes by the name of the
 * law (or Werkdocumenten), and within a law in article order, so the tasks one
 * enrichment created sit together as 1, 1a, 2, 10. The tasks come first and
 * the running jobs below them, in the same order: a job is something you wait
 * for, not something to do. Ties keep the server order.
 */
export function listByAge<T extends Task, J extends RunningJob>(
  tasks: readonly T[],
  running: readonly J[],
  nameFor: (lawId: string) => string = (id) => id,
): { tasks: T[]; running: J[] } {
  return {
    tasks: byHourThenName(tasks, (task) => ({
      context: contextName(taskContext(task), taskLawId(task), nameFor),
      label: sortLabel(task),
    })),
    running: byHourThenName(running, (job) => ({
      context: contextName(jobContext(job), jobLawId(job), nameFor),
      label: fileName(job.target_path),
    })),
  };
}
