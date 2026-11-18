import { computed, ref } from 'vue';
import { apiFetchJson } from './apiFetch';

/** The signed-in person, as `PersonInfo` in `packages/auth` serializes it. */
export interface AuthPerson {
  sub: string;
  email: string;
  name: string;
  /** Keycloak realm roles, such as `editor-reader` or `harvester-writer`. */
  roles: string[];
}

/** The `/auth/status` response, as `AuthStatus` in `packages/auth` serializes it. */
export interface AuthStatus {
  authenticated: boolean;
  oidc_configured: boolean;
  person?: AuthPerson;
}

const authenticated = ref(false);
const oidcConfigured = ref(false);
const person = ref<AuthPerson | null>(null);
const loading = ref(true);

// Realm roles the signed-in user holds, surfaced by `/auth/status`. Empty when
// unauthenticated. Used only to gate role-specific UI (menu items, sections) —
// the backend still enforces authorization per route.
const roles = computed(() => person.value?.roles ?? []);

/** True when the user holds the given realm role. */
export function hasRole(role: string): boolean {
  return roles.value.includes(role);
}

/** True when the user holds at least one of the given realm roles. */
export function hasAnyRole(candidates: readonly string[]): boolean {
  return candidates.some((role) => roles.value.includes(role));
}

let readyPromise: Promise<void> | null = null;

async function checkAuth(): Promise<void> {
  try {
    const status = await apiFetchJson<AuthStatus>('/auth/status');
    authenticated.value = status.authenticated;
    oidcConfigured.value = status.oidc_configured;
    person.value = status.person || null;
  } catch {
    // Auth endpoint not available or errored — treat as no auth configured
  } finally {
    loading.value = false;
  }
}

// Kick off the single shared /auth/status fetch and expose its promise so
// callers outside the Vue component tree (e.g. router guards) can await it
// without touching the reactive `loading` ref.
export function ensureAuthReady(): Promise<void> {
  if (!readyPromise) {
    readyPromise = checkAuth();
  }
  return readyPromise;
}

export function useAuth() {
  ensureAuthReady();

  // Accepts an explicit return URL so callers that know the user's intended
  // destination (e.g. a router guard firing before navigation commits, where
  // `window.location` still points at the source route) can forward it to
  // SSO. Falls back to the current location for the common case. Pass a
  // relative/same-origin path only: `returnUrl` is forwarded verbatim as the
  // SSO `return_url` (the backend validates it, but don't widen the surface by
  // handing it an absolute external URL).
  function login(returnUrl?: unknown): void {
    // Only accept an explicit string: a template that passes `login` as a
    // bare event handler (`@click="login"`) hands us a PointerEvent, which
    // would otherwise stringify into return_url=[object PointerEvent].
    const url = typeof returnUrl === 'string' && returnUrl
      ? returnUrl
      : window.location.pathname + window.location.search + window.location.hash;
    window.location.href = '/auth/login?return_url=' + encodeURIComponent(url);
  }

  function logout(): void {
    window.location.href = '/auth/logout';
  }

  return {
    authenticated,
    oidcConfigured,
    person,
    roles,
    hasRole,
    hasAnyRole,
    loading,
    login,
    logout,
  };
}
