/**
 * taskDetails - what the details sheet of a task lists about it: what the task
 * is about, when it arrived, and what the run that produced it came to. The
 * labels are the ones the run details in Verwerkingen use (RunDetailPane), so
 * the same fact reads the same in both places.
 *
 * The run is optional: the task list knows only the task, and the run comes
 * from the runs store, which may not have loaded yet or may not list it (a
 * list holds at most the last 200 runs: of the law for a law's task, of the
 * whole traject for a werkdocument's). Without it the sheet still shows what
 * the task itself says.
 */
import { taskContext, WERKDOCUMENTEN, WET } from './taskCategories';
import type { Task } from './taskCategories';
import { coverageText, formatMoment, reviewSummary } from './trajectRuns';
import type { TrajectRun } from './trajectRuns';

export interface DetailRow {
  label: string;
  value: string;
}

export function taskDetailRows(
  task: Task,
  run: TrajectRun | null,
  lawName: (lawId: string) => string = (id) => id,
): DetailRow[] {
  const failed = task.task_type === 'job_failed';
  const context = taskContext(task);
  const payload = task.payload;
  const all: Array<[string, string | null | undefined]> = [
    ['Regelwerk', context === WET && payload?.law_id ? lawName(payload.law_id) : null],
    // A failure concerns the whole enrichment, so it names no article.
    [
      'Artikel',
      context === WET && !failed ? (payload?.article ? `Artikel ${payload.article}` : 'Het hele regelwerk') : null,
    ],
    // The whole path: the list shows the file name, the folder tells two
    // documents with the same name apart.
    ['Werkdocument', context === WERKDOCUMENTEN ? payload?.target_path : null],
    [failed ? 'Mislukt' : 'Voorgesteld', formatMoment(task.created_at)],
  ];
  if (run) {
    all.push(
      ['Pogingen', failed && run.attempts > 1 ? `${run.attempts} van ${run.max_attempts}` : null],
      ['Taalmodel', run.provider],
      [
        'Uitvoerbare artikelen',
        run.articles_total != null && run.articles_with_machine_readable != null
          ? `${run.articles_with_machine_readable} van ${run.articles_total}`
          : null,
      ],
      ['Nieuw verrijkt', coverageText(run.coverage_score)],
      ['Conversie', run.converted_with],
      // How far the review of the run's other proposals is: this task is one
      // of them.
      ['Beoordeling', failed ? null : reviewSummary(run)],
    );
  }
  return all
    .filter((entry): entry is [string, string] => typeof entry[1] === 'string' && entry[1] !== '')
    .map(([label, value]) => ({ label, value }));
}
