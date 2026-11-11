/**
 * The trajecten of the signed-in user, and the one the route is in, as a store
 * the whole editor shares.
 *
 * The active traject lives in `route.params.trajectRef` (per browser tab), so
 * `activeTrajectRef`, `activeTraject` and `trajectMissing` are getters on the
 * route rather than state. The list loads once: main.ts calls `ensureReady()`
 * at startup, and the operations that change it reload it.
 */
import { computed, ref } from 'vue';
import { useRoute } from 'vue-router';
import { defineStore } from 'pinia';
import { apiFetch, apiFetchJson } from '../lib/apiFetch';
import { useAuth } from '../composables/useAuth';

/** One traject as `GET /api/trajects` lists it (TrajectSummary in trajects.rs). */
export interface TrajectSummary {
  id: string;
  name: string;
  description: string;
  scope: string;
  status: string;
  role: string;
  // `null` when the backend forgot `fill_ref()`; see the guard in activeTraject.
  ref: string | null;
}

/** Body of `POST /api/trajects` (CreateTrajectRequest in trajects.rs). */
export interface CreateTrajectRequest {
  name: string;
  description?: string;
  scope?: string;
  repo_owner?: string | null;
  repo_name?: string | null;
  base_branch?: string | null;
  repo_path?: string | null;
}

/** Body of `PATCH /api/trajects/:id` (UpdateTrajectRequest in trajects.rs). */
export interface UpdateTrajectRequest {
  name?: string;
  description?: string;
  scope?: string;
  status?: string;
  repo_path?: string;
}

// Pure predicate: the URL names a traject (`trajectRef`) but, for a user who
// may use the editor (`authorized`) whose membership list has finished loading
// (`!loading`) without error (`!errored`), that ref is not among their
// memberships (`activeTraject` is null). That means the traject either does not
// exist or the user has no access - we deliberately do not distinguish the two
// (no information leak about traject existence).
//
// `authorized` mirrors `canEdit`'s `!oidcConfigured || authenticated` idiom so
// the message also works in OIDC-not-configured (dev) deployments. The
// `!loading` + `!errored` gates cover the trajects-list fetch: a network
// failure leaves the list empty but is NOT a missing traject, so `errored`
// suppresses a false "does not exist" for a transient blip. The auth side
// needs no gate because the `requiresAuth` router guard awaits `/auth/status`
// before EditorView mounts, so `authorized` is already settled by the time this
// runs.
export function isTrajectMissing(
  trajectRef: string | null | undefined,
  authorized: boolean,
  loading: boolean,
  errored: boolean,
  activeTraject: unknown,
): boolean {
  return trajectRef != null && authorized && !loading && !errored && activeTraject == null;
}

export const useTrajectsStore = defineStore('trajects', () => {
  const trajects = ref<TrajectSummary[]>([]);
  const loading = ref(true);
  const error = ref<unknown>(null);
  let readyPromise: Promise<void> | null = null;

  // The route is read here and never returned: the store exposes what it
  // derives from it, as the Pinia docs ask of `useRoute()` in a setup store.
  const route = useRoute();
  const { authenticated, oidcConfigured } = useAuth();

  // `null` for any route without a traject param - that's the "global browse"
  // mode where edits are not available. The ref is the URL form
  // `{slug}-{8hex}` returned on `t.ref`; the backend resolver looks up the
  // matching traject by the trailing 8 hex chars of its UUID. `trajectRef` is
  // a single (non-repeatable) route param, so a string.
  const activeTrajectRef = computed(() => (route.params.trajectRef as string | undefined) || null);
  // Guard against `t.ref` being null/undefined: the backend serialises it as
  // `null` when a `TrajectSummary` is built without calling `fill_ref()`
  // (defensive contract - see editor-api/trajects.rs). Skip those rather than
  // risk a `null === null` match against a missing `activeTrajectRef`.
  const activeTraject = computed(
    () => trajects.value.find((t) => t.ref && t.ref === activeTrajectRef.value) || null,
  );
  const trajectMissing = computed(() =>
    isTrajectMissing(
      activeTrajectRef.value,
      !oidcConfigured.value || authenticated.value,
      loading.value,
      error.value != null,
      activeTraject.value,
    ),
  );

  async function loadTrajects(): Promise<void> {
    // Reset on each call so refresh-path consumers (createTraject →
    // refreshTrajects) see the menu flip back to its loading placeholder
    // while the new list is in flight, instead of holding the stale label
    // for the duration of the round-trip.
    loading.value = true;
    // Clear any prior error so it reflects only this fetch - otherwise a single
    // historical network blip stays sticky and would permanently suppress the
    // trajectMissing message (which gates on `!error`) for the whole session.
    error.value = null;
    try {
      // Raw fetch + ok-branch on purpose: a non-ok status (e.g. 401 on a
      // public page) keeps the previous list without setting `error` -
      // only a network failure surfaces through the catch.
      const resp = await fetch('/api/trajects');
      if (resp.ok) {
        trajects.value = (await resp.json()) as TrajectSummary[];
      }
    } catch (e) {
      error.value = e;
    } finally {
      loading.value = false;
    }
  }

  /** Load the list unless it already loads; resolves when it is there. */
  function ensureReady(): Promise<void> {
    if (!readyPromise) readyPromise = loadTrajects();
    return readyPromise;
  }

  /** Load the list again. */
  function refreshTrajects(): Promise<void> {
    readyPromise = loadTrajects();
    return readyPromise;
  }

  async function createTraject(payload: CreateTrajectRequest): Promise<TrajectSummary> {
    const created = await apiFetchJson<TrajectSummary>('/api/trajects', {
      method: 'POST',
      headers: { 'content-type': 'application/json' },
      body: JSON.stringify(payload),
      errorMessage: (status, body) => body || `Create failed: ${status}`,
    });
    await refreshTrajects();
    return created;
  }

  // Owner-only partial update (backend: PATCH /api/trajects/:id → 204). Send
  // only the fields that change; anything omitted keeps its current value. The
  // backend answers a refused change (400/403) with a Dutch explanation in the
  // body, so that body is the message the caller shows.
  async function updateTraject(trajectId: string, patch: UpdateTrajectRequest): Promise<void> {
    await apiFetch(`/api/trajects/${encodeURIComponent(trajectId)}`, {
      method: 'PATCH',
      headers: { 'content-type': 'application/json' },
      body: JSON.stringify(patch),
      errorMessage: (status, body) => body || `Opslaan mislukt: ${status}`,
    });
    // The traject list carries name and status, which this endpoint can change.
    await refreshTrajects();
  }

  // Point the traject's own source at another GitHub repo, e.g. after the repo
  // moved to a new owner (backend: PUT /api/trajects/:id/repo → 204).
  // editor-admin only. The backend first proves the new repo works (token,
  // push access, base branch, traject branch) and answers a refusal with a
  // Dutch explanation in the body, which is the message the caller shows.
  async function moveTrajectRepo(trajectId: string, repoOwner: string, repoName: string): Promise<void> {
    await apiFetch(`/api/trajects/${encodeURIComponent(trajectId)}/repo`, {
      method: 'PUT',
      headers: { 'content-type': 'application/json' },
      body: JSON.stringify({ repo_owner: repoOwner, repo_name: repoName }),
      errorMessage: (status, body) => body || `Opslaan mislukt: ${status}`,
    });
  }

  // Owner-only hard delete (backend: DELETE /api/trajects/:id → 204). The
  // upstream branch on GitHub is deliberately left untouched by the backend.
  async function deleteTraject(trajectId: string): Promise<void> {
    await apiFetch(`/api/trajects/${encodeURIComponent(trajectId)}`, {
      method: 'DELETE',
      errorMessage: (status, body) => body || `Delete failed: ${status}`,
    });
    await refreshTrajects();
  }

  // A contributor leaves a traject (backend: POST /api/trajects/:id/leave).
  // Mirror of deleteTraject from the member's side: refresh the list so the
  // traject they just left drops out immediately.
  async function leaveTraject(trajectId: string): Promise<void> {
    await apiFetch(`/api/trajects/${encodeURIComponent(trajectId)}/leave`, {
      method: 'POST',
      errorMessage: (status, body) => body || `Verlaten mislukt: ${status}`,
    });
    await refreshTrajects();
  }

  return {
    trajects,
    loading,
    error,
    activeTrajectRef,
    activeTraject,
    trajectMissing,
    ensureReady,
    refreshTrajects,
    createTraject,
    updateTraject,
    moveTrajectRepo,
    deleteTraject,
    leaveTraject,
  };
});
