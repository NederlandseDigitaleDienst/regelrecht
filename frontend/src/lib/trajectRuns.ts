/**
 * trajectRuns - the runs of a traject (harvests, enrichments, conversions) as
 * the Verwerkingen overview shows them: the shape `GET
 * /api/trajects/{ref}/runs` returns, and the words for it.
 *
 * Pure functions, so the wording is tested without mounting a component. The
 * law name comes in as a function (useCorpusLaws' displayName), because a run
 * carries only the raw law_id.
 */

export type RunStatus = 'pending' | 'processing' | 'completed' | 'failed';
export type RunJobType = 'enrich' | 'traject_harvest' | 'document_convert' | 'law_convert';

/** One run (TrajectRun in packages/pipeline/src/traject_runs.rs). */
export interface TrajectRun {
  job_id: string;
  job_type: RunJobType | string;
  law_id: string;
  target_path: string | null;
  law_name: string | null;
  status: RunStatus | string;
  attempts: number;
  max_attempts: number;
  created_at: string;
  started_at: string | null;
  completed_at: string | null;
  requested_by_name: string | null;
  error: string | null;
  provider: string | null;
  articles_total: number | null;
  articles_with_machine_readable: number | null;
  coverage_score: number | null;
  converted_with: string | null;
  progress: unknown;
  tasks_open: number;
  tasks_approved: number;
  tasks_rejected: number;
  tasks_dismissed: number;
  /** The open review tasks assigned to the member asking: the ones they can pick up. */
  tasks_open_mine: number;
}

/** `GET /api/trajects/{ref}/runs` (TrajectRunList in editor-api). */
export interface TrajectRunList {
  runs?: TrajectRun[];
}

type LawName = (lawId: string) => string;

/** Still going: waiting in the queue or being processed. */
export function isActive(run: TrajectRun): boolean {
  return run.status === 'pending' || run.status === 'processing';
}

function fileName(path: string | null | undefined, fallback: string): string {
  return path?.split('/').pop() || fallback;
}

/** What the run is, in a few words: "Verrijking van Wet op de zorgtoeslag". */
export function runTitle(run: TrajectRun, lawName: LawName = (id) => id): string {
  switch (run.job_type) {
    case 'enrich':
      return `Verrijking van ${lawName(run.law_id)}`;
    case 'traject_harvest':
      // law_id is the BWB id; the name it was requested under reads better.
      return `Ophalen van ${run.law_name || run.law_id}`;
    case 'document_convert':
      return `Conversie van ${fileName(run.target_path, 'werkdocument')}`;
    case 'law_convert':
      return `Conversie van ${fileName(run.target_path, 'document')} naar een regelwerk`;
    default:
      return `${run.job_type} van ${run.law_id}`;
  }
}

/**
 * The state of the run in words. A run that failed once and waits for its next
 * attempt is pending again, but saying "In de wachtrij" would hide that
 * something went wrong.
 */
export function runStatusText(run: TrajectRun): string {
  switch (run.status) {
    case 'processing':
      return 'Bezig';
    case 'pending':
      return run.attempts > 0
        ? `Nieuwe poging (${run.attempts + 1} van ${run.max_attempts})`
        : 'In de wachtrij';
    case 'completed':
      return 'Klaar';
    case 'failed':
      return 'Mislukt';
    default:
      return run.status;
  }
}

/**
 * The run's outcome in one line, or null when there is nothing to say yet.
 * For an enrichment that is how far the law got and how its review went; a
 * failure says so; the error itself is longer and belongs in the details.
 */
export function runOutcome(run: TrajectRun): string | null {
  if (run.status === 'failed') return 'Mislukt';
  if (run.status !== 'completed') return null;
  const review = reviewSummary(run);
  if (run.job_type === 'enrich' && run.articles_total != null && run.articles_with_machine_readable != null) {
    const coverage = `${run.articles_with_machine_readable} van ${run.articles_total} artikelen uitvoerbaar`;
    return review ? `${coverage} · ${review}` : coverage;
  }
  return review;
}

/**
 * How the review of the run's proposals went: "2 overgenomen, 1 afgewezen,
 * 1 open". Null when the run created no review tasks.
 */
export function reviewSummary(run: TrajectRun): string | null {
  const parts: string[] = [];
  if (run.tasks_approved) parts.push(`${run.tasks_approved} overgenomen`);
  if (run.tasks_rejected) parts.push(`${run.tasks_rejected} afgewezen`);
  if (run.tasks_dismissed) parts.push(`${run.tasks_dismissed} als gedaan gemarkeerd`);
  if (run.tasks_open) parts.push(`${run.tasks_open} open`);
  return parts.length ? parts.join(', ') : null;
}

/**
 * The share of previously unenriched articles that got enriched: "75% van de
 * artikelen die nog niet uitvoerbaar waren".
 */
export function coverageText(score: number | null | undefined): string | null {
  return score == null || !Number.isFinite(score)
    ? null
    : `${Math.round(score * 100)}% van de artikelen die nog niet uitvoerbaar waren`;
}

/**
 * Whether the run's timestamps describe the current attempt. A run waiting
 * for its next attempt still carries the start of the one that failed, and
 * timing from there would count the wait as running time.
 */
export function timesCurrentAttempt(run: TrajectRun): boolean {
  return !(run.status === 'pending' && run.attempts > 0);
}

/** `12 nov 2026, 14:03`, in Dutch, in the viewer's time zone. */
export function formatMoment(iso: string | null | undefined): string | null {
  if (!iso) return null;
  const date = new Date(iso);
  if (Number.isNaN(date.getTime())) return null;
  // Made per call, not at module level: a formatter fixes its options when it
  // is created, and this is cheap next to rendering the row.
  return new Intl.DateTimeFormat('nl-NL', { dateStyle: 'medium', timeStyle: 'short' }).format(date);
}

/** How long the run took, or has been running: `45 s`, `12 min`, `1 u 3 min`. */
export function durationText(run: TrajectRun, now: number = Date.now()): string | null {
  if (!run.started_at || !timesCurrentAttempt(run)) return null;
  const start = new Date(run.started_at).getTime();
  const end = run.completed_at ? new Date(run.completed_at).getTime() : now;
  if (!Number.isFinite(start) || !Number.isFinite(end)) return null;
  const secs = Math.max(0, Math.round((end - start) / 1000));
  if (secs < 60) return `${secs} s`;
  const mins = Math.floor(secs / 60);
  if (mins < 60) return `${mins} min`;
  return `${Math.floor(mins / 60)} u ${mins % 60} min`;
}

/** The secondary line of a run in the list: status, moment and requester. */
export function runSupportingText(run: TrajectRun): string {
  const moment = formatMoment(isActive(run) ? run.created_at : (run.completed_at ?? run.created_at));
  return [runOutcome(run) ?? runStatusText(run), moment, run.requested_by_name]
    .filter((part): part is string => Boolean(part))
    .join(' · ');
}

/** The runs split into what is still going and what is done, each newest first. */
export function splitRuns(runs: readonly TrajectRun[]): { active: TrajectRun[]; done: TrajectRun[] } {
  return { active: runs.filter(isActive), done: runs.filter((r) => !isActive(r)) };
}
