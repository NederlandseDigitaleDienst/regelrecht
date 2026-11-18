import { ensureAuthReady, useAuth } from '../composables/useAuth';
import { useGithubAuth } from '../composables/useGithubAuth';

// Global 401/428 interceptor for the editor SPA.
//
// The backend runs a BFF/session model: the browser holds an opaque HttpOnly
// session cookie, never a token. When the session expires the backend answers
// API calls with 401. Without this guard each call site surfaces a raw "401"
// error; with it, a 401 on an `/api/*` call bounces the user through
// `/auth/login?return_url=<current path>` so they re-authenticate (silently if
// the Keycloak SSO session is still alive) and land back where they were.
//
// The router already does this for *navigation* (router.ts `beforeEach`); this
// closes the gap for fetch calls fired while a page is open. We reuse the same
// `useAuth().login()` redirect so there is one auth path, not two.
//
// 428 (Precondition Required) gets the same treatment for the GitHub link:
// the backend answers a traject write with 428 when this deployment requires
// the acting user's own GitHub token (`github.user_oauth` flag or
// GITHUB_USER_TOKEN_REQUIRED), the write target has no configured service
// token (a configured token takes precedence and needs no link — see
// `user_write_token_for_backend` in the editor-api), and the user hasn't
// linked (or it expired).
// Instead of a dead-end error toast, the guard bounces straight into the
// GitHub consent flow via `useGithubAuth().connect()`, which returns to the
// current page with a `?github=connected|error|denied` marker. Note the
// redirect leaves the page, so work not yet accepted by the backend stays
// behind - the 428 write itself was refused, and the linking flow is a
// one-time detour per user.
//
// Deliberately NOT handled here:
// - 403 (authenticated but missing role) is not a re-login case - redirecting
//   would loop. Call sites show their own "no access" message.
// - Only same-origin `/api/*` responses are intercepted, so `/auth/*` (incl.
//   `/auth/status`), `/data/*`, `/wasm/*` and cross-origin fetches pass through.
// - 401s on sessions that were never authenticated. Public pages legitimately
//   call `/api/*` endpoints that 401 without a session (e.g. `/library` loads
//   `/api/favorites`, which the editor tolerates as "no favorites"), and local
//   dev runs with OIDC disabled. We only redirect a session that loaded
//   authenticated against a configured IdP and then got a 401 - i.e. its
//   session expired. This mirrors the router guard's `oidcConfigured` gate.

/**
 * True when the fetch target is a same-origin `/api/...` URL.
 * Accepts the same input shapes as `fetch`: string, URL, or Request.
 * @param {RequestInfo | URL} input
 * @returns {boolean}
 */
export function isApiUrl(input: RequestInfo | URL): boolean {
  const raw = input instanceof Request ? input.url : input;
  let url;
  try {
    // Resolve against the full current URL (not just the origin) so bare
    // relative paths resolve exactly as the browser's fetch does - e.g. on
    // `/trajects/123`, `fetch('api/x')` hits `/trajects/api/x`, not `/api/x`.
    url = new URL(raw, window.location.href);
  } catch {
    return false;
  }
  return url.origin === window.location.origin && url.pathname.startsWith('/api/');
}

// Module-level latch: once a 401/428 triggers a redirect we are leaving the
// page, so further hits from in-flight calls must not fire a second redirect.
let redirecting = false;

/**
 * Wrap `window.fetch` so a 401 on a same-origin `/api/*` call redirects to the
 * OIDC login, and a 428 redirects into the GitHub connect flow. Idempotent-safe
 * to call once at app start (before mount).
 */
export function installApiAuthGuard() {
  const originalFetch = window.fetch.bind(window);

  window.fetch = async (input, init) => {
    const response = await originalFetch(input, init);
    if (response.status === 401 && !redirecting && isApiUrl(input)) {
      // Resolve auth status before deciding: an early `/api/*` 401 can race the
      // `/auth/status` check, leaving the refs at their defaults otherwise.
      await ensureAuthReady();
      const { authenticated, oidcConfigured } = useAuth();
      // Only an expired session (loaded authenticated, IdP configured) redirects.
      // Anonymous visitors on public pages and OIDC-off dev fall through.
      if (oidcConfigured.value && authenticated.value && !redirecting) {
        redirecting = true;
        // Land on the bare "je bent uitgelogd" page (a public SPA route)
        // instead of silently bouncing to /auth/login (which could dead-end on
        // a blank page). window.location already reflects the page the user is
        // on, so pass it as return_url for the page's re-login button.
        const returnUrl = window.location.pathname + window.location.search + window.location.hash;
        window.location.assign('/uitgelogd?return_url=' + encodeURIComponent(returnUrl));
      }
    }
    if (response.status === 428 && !redirecting && isApiUrl(input)) {
      // CONTRACT: 428 is reserved editor-wide for the GitHub write-token
      // requirement (documented on `github_oauth::user_write_token` in the
      // editor-api). This guard keys on nothing but the status code, so a
      // backend endpoint returning 428 for any other precondition would
      // silently hijack its callers into the koppel-flow - use a different
      // status there instead. The requirement only fires for an
      // authenticated session on a deployment with the OAuth App configured,
      // so no auth/configured re-checks are needed here.
      redirecting = true;
      // No explicit returnUrl, same reasoning as the 401 branch: connect()
      // defaults to the page the user is on, and the callback bounces back
      // to it with a `?github=...` marker.
      useGithubAuth().connect();
    }
    return response;
  };
}
