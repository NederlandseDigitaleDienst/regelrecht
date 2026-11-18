import { DATE_FORMATTER } from './constants';
import type { Job } from './composables/useJobs';

export function formatDate(value: string | number | Date | null | undefined): string | null {
  if (value === null || value === undefined) return null;
  const date = new Date(value);
  if (isNaN(date.getTime())) return String(value);
  return DATE_FORMATTER.format(date);
}

export function formatNumber(value: unknown): string {
  if (value === null || value === undefined) return '—';
  const num = Number(value);
  return Number.isFinite(num) ? num.toLocaleString('nl-NL') : String(value);
}

export function formatCoverageScore(value: unknown): string | null {
  if (value === null || value === undefined) return null;
  const num = Number(value);
  if (Number.isFinite(num)) return `${Math.round(num * 100)}%`;
  return String(value);
}

export function truncateUuid(value: unknown): string {
  const str = String(value);
  return str.length > 8 ? str.substring(0, 8) : str;
}

// snake_case status → human-readable sentence case, e.g.
// 'harvest_failed' → 'Harvest failed', 'enriching' → 'Enriching'.
export function formatStatus(value: unknown): string {
  if (value === null || value === undefined || value === '') return '';
  const s = String(value).replace(/_/g, ' ');
  return s.charAt(0).toUpperCase() + s.slice(1);
}

/** The job fields `jobSubtitle` reads. */
export type JobSubtitleFields = Pick<
  Job,
  'job_type' | 'status' | 'attempts' | 'created_at' | 'started_at' | 'completed_at'
>;

export function jobSubtitle(job: JobSubtitleFields): string {
  const type = job.job_type
    ? job.job_type.charAt(0).toUpperCase() + job.job_type.slice(1)
    : 'Job';
  switch (job.status) {
    case 'completed':
      return `${type} completed at ${formatDate(job.completed_at)}`;
    case 'failed': {
      const attempts = job.attempts > 1 ? ` after ${job.attempts} attempts` : '';
      return `${type} failed${attempts} at ${formatDate(job.completed_at)}`;
    }
    case 'processing':
      return `${type} started at ${formatDate(job.started_at)}`;
    default:
      return `${type} created at ${formatDate(job.created_at)}`;
  }
}
