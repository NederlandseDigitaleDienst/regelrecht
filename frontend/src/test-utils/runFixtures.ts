import type { TrajectRun } from '../lib/trajectRuns';

/** A finished enrichment run; override what a test is about. */
export function makeRun(overrides: Partial<TrajectRun> = {}): TrajectRun {
  return {
    job_id: 'j1',
    job_type: 'enrich',
    law_id: 'wet_op_de_zorgtoeslag',
    target_path: null,
    law_name: null,
    status: 'completed',
    attempts: 1,
    max_attempts: 3,
    created_at: '2026-11-12T10:00:00Z',
    started_at: '2026-11-12T10:00:05Z',
    completed_at: '2026-11-12T10:12:05Z',
    requested_by_name: 'Alice Voorbeeld',
    error: null,
    provider: null,
    articles_total: null,
    articles_with_machine_readable: null,
    coverage_score: null,
    converted_with: null,
    progress: null,
    tasks_open: 0,
    tasks_approved: 0,
    tasks_rejected: 0,
    tasks_dismissed: 0,
    tasks_open_mine: 0,
    ...overrides,
  };
}
