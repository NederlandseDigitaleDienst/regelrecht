/**
 * Trajectloader: calls around a traject whose repo is also worked on outside
 * the editor (by hand or by an agent, in a git clone).
 *
 * Two branches, never one shared tip: the working branch belongs to the clone,
 * the traject branch to the editor. "Bijwerken" brings the working branch into
 * the traject on GitHub; "Voorstel naar werkbranch" opens a pull request the
 * other way. Backend: packages/editor-api/src/traject_sync.rs.
 */
import { apiFetchJson } from './apiFetch';

/** GET /api/trajects/{ref}/base-status */
export interface BaseStatus {
  repo: string;
  base_branch: string;
  traject_branch: string;
  /** Commits on the working branch the traject does not have yet. */
  behind_by: number;
  /** Commits on the traject branch the working branch does not have yet. */
  ahead_by: number;
  /** Files the traject changed since the two diverged. */
  traject_files: string[];
  /** Files the working branch changed since the two diverged. */
  base_files: string[];
  /** Files changed on both sides. */
  overlap: string[];
}

/** POST /api/trajects/{ref}/sync-base */
export interface SyncResult {
  outcome: 'fast_forward' | 'merged' | 'up_to_date';
  sha: string | null;
}

/** POST /api/trajects/{ref}/propose */
export interface ProposeResult {
  number: number;
  url: string;
}

export interface PreflightLaw {
  path: string;
  law_id: string | null;
  error: string | null;
}

export interface PreflightFinding {
  level: 'blocker' | 'warning' | 'info';
  message: string;
}

/** POST /api/trajects/preflight */
export interface PreflightReport {
  repo: string;
  is_private: boolean;
  default_branch: string;
  branches: string[];
  base_branch: string;
  repo_path: string | null;
  law_file_count: number;
  law_count: number;
  invalid_count: number;
  unchecked_count: number;
  laws: PreflightLaw[];
  annotations_under_root: number;
  annotations_at_repo_root: number;
  seed_overlap: { source_id: string; source_name: string; law_ids: string[] }[];
  findings: PreflightFinding[];
}

export interface PreflightRequest {
  repo_owner: string;
  repo_name: string;
  base_branch?: string;
  repo_path?: string;
}

const trajectPath = (ref: string, tail: string) => `/api/trajects/${encodeURIComponent(ref)}/${tail}`;

// The central MinBZK corpus is the default traject target. Its working branch
// (`development`) moves for reasons unrelated to any one traject, so the
// prompt would be noise there.
export const CENTRAL_REPO = 'MinBZK/regelrecht-corpus';

/** Inspect a repo before a traject is created on it. */
export function preflightRepo({ repo_owner, repo_name, base_branch, repo_path }: PreflightRequest) {
  const body: PreflightRequest = { repo_owner, repo_name };
  if (base_branch) body.base_branch = base_branch;
  if (repo_path) body.repo_path = repo_path;
  return apiFetchJson<PreflightReport>('/api/trajects/preflight', {
    method: 'POST',
    headers: { 'content-type': 'application/json' },
    body: JSON.stringify(body),
    errorMessage: (status: number, text: string) => text || `Controle mislukt: ${status}`,
  });
}

export function fetchBaseStatus(trajectRef: string) {
  return apiFetchJson<BaseStatus>(trajectPath(trajectRef, 'base-status'), {
    errorMessage: (status: number, text: string) => text || `Status ophalen mislukt: ${status}`,
  });
}

/**
 * `auto`: the background loop. The backend then answers a missing GitHub link
 * with 409 instead of 428, so an unprompted call never opens the connect flow.
 */
export function syncBase(trajectRef: string, { auto = false }: { auto?: boolean } = {}) {
  return apiFetchJson<SyncResult>(trajectPath(trajectRef, auto ? 'sync-base?auto=true' : 'sync-base'), {
    method: 'POST',
    errorMessage: (status: number, text: string) => text || `Bijwerken mislukt: ${status}`,
  });
}

export function proposeToBase(trajectRef: string) {
  return apiFetchJson<ProposeResult>(trajectPath(trajectRef, 'propose'), {
    method: 'POST',
    errorMessage: (status: number, text: string) => text || `Voorstel openen mislukt: ${status}`,
  });
}

// Counted in files, not commits: a merge or a fast-forward adds commits on
// one side without changing anything the other side lacks, and a prompt on
// commit counts would come back after every sync.
const behindFiles = (status: BaseStatus | null) => status?.base_files?.length ?? 0;
const aheadFiles = (status: BaseStatus | null) => status?.traject_files?.length ?? 0;
const plural = (n: number, one: string, many: string) => `${n} ${n === 1 ? one : many}`;

/** Whether "Bijwerken" has anything to bring in. */
export function canSync(status: BaseStatus | null): boolean {
  return behindFiles(status) > 0;
}

/** Whether the traject holds edits the working branch does not have. */
export function canPropose(status: BaseStatus | null): boolean {
  return aheadFiles(status) > 0;
}

/** Whether the sync notice has something to say for this status. */
export function needsSyncPrompt(status: BaseStatus | null): status is BaseStatus {
  if (!status || status.repo === CENTRAL_REPO) return false;
  return canSync(status) || canPropose(status);
}

/** Dutch one-liner for the notice title. */
export function syncSummary(status: BaseStatus): string {
  const parts: string[] = [];
  if (canSync(status)) {
    parts.push(
      `werkbranch ${status.base_branch} heeft wijzigingen in ${plural(behindFiles(status), 'bestand', 'bestanden')} die nog niet in dit traject staan`,
    );
  }
  if (canPropose(status)) {
    parts.push(
      `dit traject heeft wijzigingen in ${plural(aheadFiles(status), 'bestand', 'bestanden')} die nog niet op de werkbranch staan`,
    );
  }
  const text = parts.join('; ');
  return text.charAt(0).toUpperCase() + text.slice(1);
}

/** Supporting text: where a merge may need a person, if anywhere. */
export function syncDetail(status: BaseStatus): string {
  if (status.overlap?.length) {
    return `Aan beide kanten gewijzigd: ${status.overlap.join(', ')}. Git voegt dat zelf samen zolang het om verschillende regels gaat; alleen dezelfde regels geven een conflict.`;
  }
  if (canSync(status)) {
    return 'Geen bestanden aan beide kanten gewijzigd; bijwerken kan zonder conflict.';
  }
  return '';
}

/** A preflight report's findings, by level. */
export function findingsByLevel(report: Pick<PreflightReport, 'findings'> | null): Record<PreflightFinding['level'], string[]> {
  const out: Record<PreflightFinding['level'], string[]> = { blocker: [], warning: [], info: [] };
  for (const f of report?.findings ?? []) (out[f.level] ?? out.info).push(f.message);
  return out;
}
