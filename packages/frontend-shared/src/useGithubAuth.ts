import { ref } from 'vue';
import { apiFetch, apiFetchJson } from './apiFetch';

/**
 * Per-user GitHub link status (spike), as `GithubStatus` in
 * `packages/editor-api` serializes the `/auth/github/status` response.
 * `configured: false` means the deployment has no GitHub OAuth App wired up,
 * so the UI should hide the whole affordance.
 */
export interface GithubStatus {
  connected: boolean;
  configured: boolean;
  github_login?: string;
  scopes?: string;
  /** A linked token exists but has expired, so the user has to reconnect. */
  expired: boolean;
  /** Whether writes require a linked user token in this deployment. */
  required: boolean;
}

const status = ref<GithubStatus | null>(null);
const loading = ref(true);

let readyPromise: Promise<void> | null = null;

async function fetchStatus(): Promise<void> {
  try {
    status.value = await apiFetchJson<GithubStatus>('/auth/github/status');
  } catch {
    // Endpoint missing/errored (e.g. auth-disabled dev mode) — treat as
    // unconfigured so the UI simply hides the GitHub controls.
    status.value = { connected: false, configured: false, expired: false, required: false };
  } finally {
    loading.value = false;
  }
}

export function ensureGithubReady(): Promise<void> {
  if (!readyPromise) {
    readyPromise = fetchStatus();
  }
  return readyPromise;
}

export function useGithubAuth() {
  ensureGithubReady();

  // Redirect the browser into the GitHub consent flow. `returnUrl` is a
  // same-origin relative path the backend validates and bounces back to with
  // a `?github=connected|error|denied` marker. Guards against being passed a
  // bare event object (`@click="connect"`).
  function connect(returnUrl?: unknown): void {
    const url =
      typeof returnUrl === 'string' && returnUrl
        ? returnUrl
        : window.location.pathname + window.location.search + window.location.hash;
    window.location.href = '/auth/github/login?return_url=' + encodeURIComponent(url);
  }

  async function disconnect(): Promise<void> {
    await apiFetch('/auth/github/disconnect', { method: 'POST' });
    await refresh();
  }

  // Force a re-fetch of the shared status (after connect/disconnect, or when a
  // save reported a 428 that may have been resolved in another tab).
  async function refresh(): Promise<void> {
    readyPromise = null;
    return ensureGithubReady();
  }

  return { status, loading, connect, disconnect, refresh };
}
