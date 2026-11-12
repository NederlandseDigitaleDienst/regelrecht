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
