import { usePollingFetch } from './usePollingFetch';

/** One cluster of markings naming the same change (`MarkingCluster` in packages/admin/src/models.rs). */
export type MarkingCluster = {
  resolution: string;
  resolved_by: string | null;
  /** Markings in this cluster. */
  markings: number;
  /** Distinct laws they sit in. */
  laws: number;
  /** Distinct articles blocked. */
  articles: number;
  providers: string[];
  providers_on_these_laws: number;
  all_accepted: boolean;
};

// The backlog, read off the corpus: markings grouped by the change that would
// resolve them. Reads GET /api/harvest-admin/markings/clusters.
//
// Takes `filterParams` from `useMarkings` so the backlog always describes the
// same set of markings as the list beside it. A count that groups something
// other than the visible rows is worse than no count: nothing about the page
// looks wrong while it happens.
//
// There is no paging. A backlog is short by construction, and truncating it
// would cut off the tail of single-marking clusters, which is the part worth
// looking at twice: `resolved_by` is free text, so two agents asking for the
// same change in different words land in two clusters of one.
export function useMarkingClusters(filterParams?: () => URLSearchParams) {
  function buildUrl() {
    const params = filterParams ? filterParams() : new URLSearchParams();
    const query = params.toString();
    return `/api/harvest-admin/markings/clusters${query ? `?${query}` : ''}`;
  }

  const { data, loading, error, refresh, startPolling, stopPolling } =
    usePollingFetch<MarkingCluster>(buildUrl);

  refresh();
  startPolling();

  return { data, loading, error, refresh, startPolling, stopPolling };
}
