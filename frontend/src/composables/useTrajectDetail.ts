/**
 * useTrajectDetail - fetch one traject's full detail (`GET /api/trajects/:id`),
 * including its `sources`, for the read-only Traject-info sheet.
 *
 * Same endpoint and fresh-state-per-call shape as `useTrajectMembers`, but
 * keeps the whole `TrajectDetail` object (that composable discards `sources`).
 * The `id` argument is the traject **UUID** - the same value
 * `TrajectMenu.openMembersForActive` passes for member management - not the
 * URL `ref` form.
 */
import { ref } from 'vue';
import { apiFetchJson } from '../lib/apiFetch';
import type { TrajectSummary } from '../stores/trajects';

/** One source of a traject (TrajectSourceDto in trajects.rs). */
export interface TrajectSource {
  source_id: string;
  name: string;
  source_type: string;
  gh_owner: string | null;
  gh_repo: string | null;
  gh_branch: string | null;
  gh_base_branch: string | null;
  gh_path: string | null;
  gh_ref: string | null;
  local_path: string | null;
  priority: number;
  auth_ref: string | null;
  is_writable_own: boolean;
}

/** A traject member (TrajectMember in trajects.rs). */
export interface TrajectMember {
  account_id: string;
  email: string;
  name: string;
  role: string;
}

/** An invite for an address without an account yet (TrajectInvite in trajects.rs). */
export interface TrajectInvite {
  email: string;
  role: string;
}

/** `GET /api/trajects/:id` (TrajectDetail in trajects.rs: the summary, flattened). */
export interface TrajectDetail extends TrajectSummary {
  members: TrajectMember[];
  pending_invites: TrajectInvite[];
  sources: TrajectSource[];
}

/**
 * Pick the writable-own source from a `TrajectDetail`. That is the source the
 * traject pushes edits to, so its repo/branch fields are what the info sheet
 * shows. Returns `null` when there is no detail or no writable source (a
 * defensive shape the backend shouldn't produce, but the UI must not crash on).
 */
export function writableSource(
  detail: { sources?: readonly TrajectSource[] | null } | null | undefined,
): TrajectSource | null {
  if (!detail || !Array.isArray(detail.sources)) return null;
  return detail.sources.find((s) => s.is_writable_own) ?? null;
}

/**
 * `auth_ref` of the shared central corpus source — the default writable source a
 * traject gets when it is created without its own repo. Mirrors
 * `CENTRAL_WRITABLE_AUTH_REF` in the backend (`packages/editor-api/src/
 * trajects.rs`), which is also the marker the backend checks before it refuses
 * to move that source's root path.
 */
const CENTRAL_AUTH_REF = 'minbzk-central';

/**
 * True when this source is the shared central corpus. Its layout is fixed for
 * every traject that writes to it, so the settings pane shows its root path
 * read-only instead of offering an edit that the backend would refuse.
 */
export function isCentralSource(source: Pick<TrajectSource, 'auth_ref'> | null | undefined): boolean {
  return !!source && source.auth_ref === CENTRAL_AUTH_REF;
}

/**
 * Build a GitHub tree URL pointing at the traject branch:
 * `https://github.com/{owner}/{repo}/tree/{branch}`. Slashes inside the branch
 * name are left intact (GitHub serves `tree/feature/x` directly; percent-
 * encoding the `/` would break it). Returns `null` when the source is missing
 * owner/repo/branch, so the caller can render plain text instead of a dead link.
 */
export function branchTreeUrl(
  source: Pick<TrajectSource, 'gh_owner' | 'gh_repo' | 'gh_branch'> | null | undefined,
): string | null {
  if (!source) return null;
  const { gh_owner, gh_repo, gh_branch } = source;
  if (!gh_owner || !gh_repo || !gh_branch) return null;
  return `https://github.com/${gh_owner}/${gh_repo}/tree/${gh_branch}`;
}

export function useTrajectDetail() {
  const detail = ref<TrajectDetail | null>(null);
  const loading = ref(false);
  const error = ref<unknown>(null);

  async function load(trajectId: string): Promise<void> {
    // Reset before the await so a reopen against a different traject can't
    // briefly flash the previous traject's data (mirrors useTrajectMembers).
    loading.value = true;
    error.value = null;
    detail.value = null;
    try {
      detail.value = await apiFetchJson<TrajectDetail>(`/api/trajects/${trajectId}`, {
        errorMessage: (status) => `Kon traject niet laden: ${status}`,
      });
    } catch (e) {
      error.value = e;
    } finally {
      loading.value = false;
    }
  }

  return { detail, loading, error, load };
}
