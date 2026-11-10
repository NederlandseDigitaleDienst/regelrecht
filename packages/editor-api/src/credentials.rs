//! One credential service for traject reads and writes.
//!
//! Every editor-api handler that touches a traject backend has to answer the
//! same question — *which* token authenticates this request, and what happens
//! when there is none* — and used to answer it by hand: a write-token
//! resolution, a separate read-token resolution, a loose writability gate, and
//! a `WriteContext { token_override: … }` built literally at the persist site.
//! Four steps, each independently forgettable: a new call-site that skipped one
//! still compiled and silently fell back to the service token (the bug class
//! behind the pr952 promote regression).
//!
//! [`TrajectCredentials`] folds those steps into two entry points —
//! [`TrajectCredentials::for_read`] and [`TrajectCredentials::for_write`] — plus
//! the two backend-less variants the index scan and the create-traject
//! preflight need. The write path returns a [`WriteAuthorization`], the **only**
//! way an editor handler puts a `token_override` into a [`WriteContext`]: the
//! writability gate (403) and the per-user token are resolved together, so they
//! can no longer drift apart.
//!
//! ## What the decision depends on
//!
//! * **writable-at-rest** — whether the target backend has its own write
//!   credential (a configured `CORPUS_AUTH_*` service token on the GitHub API
//!   backend, or a natively writable local dir). This is the
//!   [`writable`](crate::state::BackendEntry::writable) flag captured once at
//!   backend registration, **not** a runtime `RepoBackend::is_writable()` probe:
//!   the old code used `is_writable()` as a stand-in for "has a service token",
//!   overloading a method whose meaning could shift under it. Passing the
//!   registration-time capability in explicitly keeps the credential decision
//!   decoupled from what `is_writable()` happens to mean.
//! * **override-capability** — [`RepoBackend::supports_token_override`]: only a
//!   backend that authenticates each persist against GitHub can route a write
//!   through the acting user's own token. A local/clone backend that ignores the
//!   override must never be handed a per-user token (or demanded one).
//! * **requiredness** — whether this deployment routes writes through the acting
//!   user's own GitHub token (the `GITHUB_USER_TOKEN_REQUIRED` env var **or** the
//!   `github.user_oauth` feature flag). See [`write_requires_user_token`] and the
//!   deliberately-more-tolerant [`user_token_write_mode`].
//! * **the link state** — whether the caller has a valid / expired / absent
//!   sealed GitHub-token cookie riding on this request.
//!
//! ## Precedence, in one place
//!
//! A configured service token always wins: a backend that is writable at rest
//! keeps reading and writing with its own token, byte-identical to the
//! pre-user-token flow, even for a linked user. Rerouting working service-token
//! traffic through a personal token would 403 every member GitHub refuses on
//! that repo (no direct push access, or an org's OAuth-App access
//! restrictions). Only a token-less, override-capable backend — the traject
//! writable-own repo on a user-chosen repo, which has no service token *by
//! design* (fail-closed against shipping the central token to a user-picked
//! repo) — falls through to the user's own token, or the 428 connect-flow when
//! they haven't linked one.

use axum::http::{HeaderMap, StatusCode};
use uuid::Uuid;

use regelrecht_corpus::auth::{CredentialResolver, TokenContext, TokenDecision, token_env_name};
use regelrecht_corpus::backend::{EditorUser, RepoBackend, WriteContext};

use crate::github_oauth::{self, GithubOAuth};
use crate::state::AppState;

/// Whether traject writes must carry the acting user's own GitHub token.
///
/// Two switches, OR-ed:
/// * the `GITHUB_USER_TOKEN_REQUIRED` env var — static, deployment-wide
///   override;
/// * the `github.user_oauth` feature flag — the same toggle that shows the
///   GitHub-koppeling UI, so switching the feature on in the Functies menu
///   also switches enforcement on. Linking is never offered-but-inert.
///
/// A flag-read failure propagates as 500: on a token-less backend the
/// requiredness decision determines whether a write may proceed at all,
/// and that must not be guessed when the flag store is unreadable. (In
/// practice the session store shares the same database, so requests
/// rarely get this far with a broken pool.)
///
/// `pub`: the `/auth/github/status` handler reports this to the frontend as
/// `required` so the UI can nudge the user to link before their first write.
pub async fn write_requires_user_token(
    state: &AppState,
    oauth: &GithubOAuth,
) -> Result<bool, (StatusCode, String)> {
    if oauth.require_user_token {
        return Ok(true);
    }
    // Without a database there is no flag store either (the toggle PUT 503s),
    // so the env var is the only switch — e.g. OIDC-off local dev.
    let Some(pool) = &state.pool else {
        return Ok(false);
    };
    crate::feature_flags::flag_enabled(pool, crate::feature_flags::GITHUB_USER_OAUTH)
        .await
        .map_err(|e| {
            tracing::error!(error = %e, "failed to read github.user_oauth feature flag");
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                "Kon de feature-flags niet lezen; probeer het opnieuw.".to_string(),
            )
        })
}

/// [`write_requires_user_token`], tolerating an unconfigured OAuth
/// integration: `false` when `github_oauth` is absent (the pre-spike
/// service-token world).
///
/// The fail-mode difference from [`write_requires_user_token`] is
/// **deliberate**, not an oversight to unify away: the writability gate
/// ([`assert_writable`]) must not 500 a write merely because the OAuth
/// integration isn't configured — a deployment can run the whole
/// service-token flow with no `github_oauth` block at all, and its writes to a
/// writable-at-rest backend must keep working. So the gate consults this
/// tolerant variant; the token resolvers, which only run once an OAuth config
/// is known present, consult the strict one.
async fn user_token_write_mode(state: &AppState) -> Result<bool, (StatusCode, String)> {
    match state.config.github_oauth.as_ref() {
        Some(oauth) => write_requires_user_token(state, oauth).await,
        None => Ok(false),
    }
}

/// The single writability gate for traject writes.
///
/// A backend that is writable at rest (configured service token, or a
/// natively writable local dir) always passes. A backend that is
/// read-only at rest still passes when this deployment routes writes
/// through the acting user's own GitHub token AND the backend honors
/// `WriteContext::token_override` — the deployed federation config for a
/// user-created traject repo has no service token on purpose, and 403-ing
/// there would break every write in the user-token mode (the pr952
/// promote bug). [`TrajectCredentials::for_write`] runs this before
/// resolving the token, so the two can't be applied out of order.
/// `persist` itself still refuses (`CorpusError::ReadOnly` → 403) if a
/// write somehow reaches it with no token at all.
pub(crate) async fn assert_writable(
    state: &AppState,
    backend: &dyn RepoBackend,
    writable_at_rest: bool,
) -> Result<(), (StatusCode, String)> {
    if writable_at_rest {
        return Ok(());
    }
    if backend.supports_token_override() && user_token_write_mode(state).await? {
        return Ok(());
    }
    Err((StatusCode::FORBIDDEN, "Source is read-only".to_string()))
}

/// A resolved write authorization for a single traject write.
///
/// The **only** way an editor handler puts a `token_override` into a
/// [`WriteContext`]: [`TrajectCredentials::for_write`] produced it, having
/// already run the writability gate and applied the token-precedence rule, so a
/// handler can neither skip the gate nor forget the override. The same resolved
/// token also serves the write path's optimistic-concurrency precondition read
/// ([`WriteAuthorization::read_token`]) — one credential resolution per write
/// instead of a separate write-token and read-token lookup that could disagree.
#[must_use = "a resolved WriteAuthorization must be turned into a WriteContext (or its read token used) — dropping it silently discards the per-user token"]
pub struct WriteAuthorization {
    token_override: Option<String>,
}

/// Redacts the token — a `WriteAuthorization` carries a live credential and is
/// one `{:?}` away from a log line, so a derived `Debug` would leak it. Reports
/// only *whether* an override is present.
impl std::fmt::Debug for WriteAuthorization {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("WriteAuthorization")
            .field(
                "token_override",
                &self.token_override.as_ref().map(|_| "***"),
            )
            .finish()
    }
}

impl WriteAuthorization {
    /// The token to authenticate a **precondition read** on the same write
    /// target with (the `If-Match` check that must see the same bytes the
    /// write will land on). Identical to the write's `token_override` — same
    /// backend, same precedence — so the read and the write can't authenticate
    /// as different identities.
    pub fn read_token(&self) -> Option<&str> {
        self.token_override.as_deref()
    }

    /// Consume the authorization into the [`WriteContext`] handed to
    /// `RepoBackend::persist`. Takes ownership so the token can't be reused
    /// past the single write it authorizes.
    pub fn into_write_context(self, message: String, author: Option<EditorUser>) -> WriteContext {
        WriteContext {
            message,
            author,
            token_override: self.token_override,
        }
    }
}

/// Per-request credential service for a traject.
///
/// Constructed from the request's identity (editor account + headers, the
/// latter carrying the sealed GitHub-token cookie) and answers "which token"
/// for every read/write against a traject backend. Cheap to build — it just
/// borrows the request context — so handlers construct one per request.
pub struct TrajectCredentials<'a> {
    state: &'a AppState,
    account_id: Uuid,
    headers: &'a HeaderMap,
}

impl<'a> TrajectCredentials<'a> {
    /// Build the service for one request.
    pub fn new(state: &'a AppState, account_id: Uuid, headers: &'a HeaderMap) -> Self {
        Self {
            state,
            account_id,
            headers,
        }
    }

    /// Resolve the per-user GitHub credential for a **read** on `backend`.
    ///
    /// * backend ignores token overrides (local source, clone-based git) →
    ///   `Ok(None)`;
    /// * backend is writable at rest (has a service token) → `Ok(None)` (read
    ///   uses it, as before) — even in the user-token write mode. Rerouting
    ///   working service-token traffic through personal tokens would break
    ///   members without direct repo access;
    /// * backend is token-less and override-capable (the traject writable-own
    ///   Contents-API backend on a user-chosen repo) → the user's own token,
    ///   or the 428 connect-flow when this deployment requires a user token and
    ///   the caller hasn't linked one. Without the user-token mode enabled this
    ///   stays `Ok(None)` — the pre-existing behaviour.
    pub async fn for_read(
        &self,
        backend: &dyn RepoBackend,
        writable_at_rest: bool,
    ) -> Result<Option<String>, (StatusCode, String)> {
        if !backend.supports_token_override() || writable_at_rest {
            return Ok(None);
        }
        self.user_read_token().await
    }

    /// Resolve a [`WriteAuthorization`] for a write on `backend`: the
    /// writability gate ([`assert_writable`]) followed by the per-user token,
    /// applying the same precedence as [`Self::for_read`].
    ///
    /// The gate and the token are resolved together on purpose — see
    /// [`WriteAuthorization`].
    pub async fn for_write(
        &self,
        backend: &dyn RepoBackend,
        writable_at_rest: bool,
    ) -> Result<WriteAuthorization, (StatusCode, String)> {
        assert_writable(self.state, backend, writable_at_rest).await?;
        let token_override = if !backend.supports_token_override() || writable_at_rest {
            // Local/clone backend (ignores the override) or a backend with its
            // own service token: the write uses the backend's configured token
            // and stamps the commit with the session identity — the pre-user-
            // token flow. Routing writes on a service-token repo through the
            // user's personal token instead would 403 every member GitHub
            // refuses on that repo.
            None
        } else {
            self.user_write_token().await?
        };
        Ok(WriteAuthorization { token_override })
    }

    /// The read-path candidate token **without** a backend to scope it to.
    ///
    /// Exists for the traject **index scan**, which resolves a candidate token
    /// ahead of `build_traject_corpus` so the enumeration of a token-less
    /// writable-own repo can authenticate as the acting user. Because there is
    /// no backend to scope the decision to, callers must treat the result as a
    /// *candidate*: apply it only where a server-side token is absent (the
    /// corpus-side `ScanTokenOverride` enforces that) and defer the `Err` (428
    /// connect-flow) until a traject-scoped read actually needs the
    /// writable-own source.
    pub async fn read_scan_candidate(&self) -> Result<Option<String>, (StatusCode, String)> {
        self.user_read_token().await
    }

    /// The write-path user token **without** a backend to scope it to.
    ///
    /// `Ok(None)` when this deployment does not require a user token; the
    /// linked token when it does and the caller has one; `Err((428, …))` when
    /// it does and the caller hasn't (or it expired). Used by
    /// [`Self::for_new_repo_preflight`] and exercised directly by the
    /// requiredness tests.
    ///
    /// Callers that have a resolved backend must prefer [`Self::for_write`],
    /// which applies the service-token-first precedence; a bare caller is
    /// responsible for that precedence itself (the create-traject preflight
    /// resolves the configured token first — see
    /// [`Self::for_new_repo_preflight`]).
    pub async fn user_write_token(&self) -> Result<Option<String>, (StatusCode, String)> {
        self.user_token_when_required(
            "Je GitHub-koppeling is verlopen. Koppel je account opnieuw om op te slaan.",
            "Koppel je GitHub-account om in dit traject op te slaan. \
             De wijziging wordt met jouw eigen GitHub-toegang weggeschreven.",
        )
        .await
    }

    /// The read-path analogue of [`Self::user_write_token`] (same requiredness
    /// gate and 428 semantics, read-flavoured messages).
    async fn user_read_token(&self) -> Result<Option<String>, (StatusCode, String)> {
        self.user_token_when_required(
            "Je GitHub-koppeling is verlopen. Koppel je account opnieuw om de \
             inhoud van dit traject te kunnen lezen.",
            "Koppel je GitHub-account om de inhoud van dit traject te kunnen \
             lezen. De traject-repo wordt met jouw eigen GitHub-toegang gelezen.",
        )
        .await
    }

    /// Resolve the token for a **create-traject preflight** on a user-supplied
    /// repo, following the same precedence as the write path.
    ///
    /// A configured per-repo service token goes first — the eventual writes on
    /// this repo run over that token too, so preflighting with the user's
    /// personal token would validate an access path the traject will never use.
    /// The lookup is **strict** ([`TokenContext::strict`]): `auth_ref` derives
    /// from user-supplied repo coords, so an unknown ref must NOT fall back to
    /// the legacy shared `CORPUS_GIT_TOKEN` (that would ship the central token
    /// to a user-picked repo — a token-exfiltration vector).
    ///
    /// Only for a token-less ref does the acting user's OWN GitHub token come
    /// into play: the preflight then validates *their* push access to the
    /// chosen repo. Outcomes:
    /// * configured service token → it;
    /// * no service token, user-token required + linked → the user's token;
    /// * no service token, required + unlinked/expired → 428 (connect-flow);
    /// * no service token, not required → 503 with the `token_env_name` hint so
    ///   the operator knows which env var to set.
    pub async fn for_new_repo_preflight(
        &self,
        auth_ref: &str,
    ) -> Result<String, (StatusCode, String)> {
        let auth_file = {
            let corpus = self.state.corpus.read().await;
            corpus.auth_file.clone()
        };
        let service_token = CredentialResolver::new(auth_file.as_deref())
            .resolve(TokenContext::strict(auth_ref))
            .map(TokenDecision::into_token)
            .map_err(|e| {
                tracing::error!(error = %e, "auth lookup failed for new traject repo");
                (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    "auth lookup failed".to_string(),
                )
            })?;
        match service_token {
            Some(service_token) => Ok(service_token),
            None => self.user_write_token().await?.ok_or_else(|| {
                let env_name = token_env_name(auth_ref);
                tracing::warn!(
                    auth_ref = %auth_ref,
                    env_name = %env_name,
                    "no token configured for user-supplied repo"
                );
                (
                    StatusCode::SERVICE_UNAVAILABLE,
                    format!(
                        "deze repo is nog niet door je beheerder geconfigureerd \
                             (verwacht env var {env_name})"
                    ),
                )
            }),
        }
    }

    /// Shared core of the read/write token resolvers: the requiredness gate,
    /// the sealed-cookie resolution, and the 428 semantics — parameterised only
    /// in the user-facing messages so the read path can say "lezen" where the
    /// write path says "opslaan".
    ///
    /// **428 is reserved editor-wide for this flow.** `apiAuthGuard.js`
    /// redirects every same-origin `/api/*` response with status 428 into the
    /// GitHub connect flow, keyed on nothing but the status code — an endpoint
    /// that returns 428 for any other reason would silently hijack its callers
    /// into the koppel-flow. Pick a different status for other preconditions.
    async fn user_token_when_required(
        &self,
        expired_msg: &str,
        missing_msg: &str,
    ) -> Result<Option<String>, (StatusCode, String)> {
        let Some(oauth) = self.state.config.github_oauth.as_ref() else {
            return Ok(None);
        };

        // The override is gated *entirely* on the requiredness decision. With
        // it off (the default), every request keeps using the backend's
        // configured token — byte-identical to pre-spike behaviour, and
        // crucially this holds even for users who HAVE linked GitHub. That
        // matters for the operator-managed central repo: routing a linked
        // user's write there through their personal token would 403 if they
        // lack direct push access, silently breaking saves that worked before.
        // So linking is inert until this deployment opts into the "editor is
        // not in the middle" mode via the feature flag or the env var.
        if !write_requires_user_token(self.state, oauth).await? {
            return Ok(None);
        }

        // From here the user-token mode is on AND the caller established that
        // no configured service token applies (`for_read`/`for_write` return
        // early on writable-at-rest backends): the only outcomes are the user's
        // own token or a 428. `github_oauth::open_link` folds every cookie
        // failure mode (absent, tampered, wrong key, foreign account) into
        // `None` = not linked, which fails closed into the 428 below.
        match github_oauth::open_link(oauth, self.headers, self.account_id) {
            Some(link) if !link.expired => {
                tracing::debug!(
                    github_login = %link.github_login,
                    "authorizing traject access as the linked GitHub user"
                );
                Ok(Some(link.access_token))
            }
            Some(_expired) => Err((StatusCode::PRECONDITION_REQUIRED, expired_msg.to_string())),
            None => Err((StatusCode::PRECONDITION_REQUIRED, missing_msg.to_string())),
        }
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;
    use std::sync::Arc;

    use async_trait::async_trait;
    use regelrecht_corpus::backend::{FileEntry, PersistOutcome};
    use regelrecht_corpus::error::Result as CorpusResult;
    use std::path::Path;
    use tokio::sync::{Mutex, RwLock};

    use crate::config::AppConfig;
    use crate::github_oauth::{GithubOAuth, seal_token_cookie_for_tests};
    use crate::state::{AppState, CorpusState};
    use crate::traject_corpus::TrajectCorpusCache;

    /// Minimal backend whose two credential-relevant capabilities are
    /// configurable, so the decision table can vary `supports_token_override`
    /// independently. Every data method is unreachable in these tests — the
    /// service never touches the wire, only the capability probes.
    struct CapBackend {
        supports_override: bool,
    }

    #[async_trait]
    impl RepoBackend for CapBackend {
        async fn read_file(&self, _relative_path: &Path) -> CorpusResult<Option<String>> {
            unreachable!("credential tests never read")
        }
        async fn write_file(&self, _relative_path: &Path, _content: &str) -> CorpusResult<()> {
            unreachable!("credential tests never write")
        }
        async fn delete_file(&self, _relative_path: &Path) -> CorpusResult<()> {
            unreachable!("credential tests never delete")
        }
        async fn list_files(
            &self,
            _dir: &Path,
            _extension: Option<&str>,
        ) -> CorpusResult<Vec<FileEntry>> {
            unreachable!("credential tests never list")
        }
        async fn persist(&self, _ctx: &WriteContext) -> CorpusResult<PersistOutcome> {
            unreachable!("credential tests never persist")
        }
        async fn ensure_ready(&mut self) -> CorpusResult<()> {
            Ok(())
        }
        fn supports_token_override(&self) -> bool {
            self.supports_override
        }
        fn is_writable(&self) -> bool {
            // Deliberately the *opposite* of what a writable-at-rest backend
            // would report, to prove the credential decision never consults
            // this method (it takes `writable_at_rest` explicitly instead).
            !self.supports_override
        }
    }

    /// AppState with an OAuth config whose `require_user_token` is set from
    /// `required`, and no database (so requiredness is driven purely by the env
    /// switch — no feature-flag DB hit needed for the decision table).
    fn state_with(required: bool, oauth_configured: bool) -> AppState {
        let github_oauth = oauth_configured.then(|| GithubOAuth::for_tests(required));
        AppState {
            corpus: Arc::new(RwLock::new(CorpusState::empty())),
            oidc_client: None,
            end_session_url: None,
            config: Arc::new(AppConfig {
                oidc: None,
                base_url: None,
                github_oauth,
                task_enrich_provider: "claude".to_string(),
            }),
            http_client: regelrecht_auth::http_client(),
            pool: None,
            pipeline_api_url: None,
            harvest_admin_url: None,
            reload_lock: Arc::new(Mutex::new(())),
            integrity: Default::default(),
            trajects: Arc::new(TrajectCorpusCache::new()),
        }
    }

    /// Headers carrying a valid sealed token cookie for `account_id`.
    fn linked_headers(state: &AppState, account_id: Uuid) -> HeaderMap {
        let oauth = state.config.github_oauth.as_ref().unwrap();
        let cookie = seal_token_cookie_for_tests(oauth, account_id, "gho_test_token");
        let mut headers = HeaderMap::new();
        headers.insert(
            axum::http::header::COOKIE,
            axum::http::HeaderValue::from_str(&cookie).unwrap(),
        );
        headers
    }

    fn overridable() -> CapBackend {
        CapBackend {
            supports_override: true,
        }
    }
    fn non_overridable() -> CapBackend {
        CapBackend {
            supports_override: false,
        }
    }

    // --- Reads -------------------------------------------------------------

    #[tokio::test]
    async fn read_writable_at_rest_uses_service_token() {
        // Service token present (writable at rest) → no override, ever, even
        // with the user-token mode on and a linked cookie.
        let state = state_with(true, true);
        let account = Uuid::new_v4();
        let headers = linked_headers(&state, account);
        let creds = TrajectCredentials::new(&state, account, &headers);
        let token = creds.for_read(&overridable(), true).await.unwrap();
        assert_eq!(token, None);
    }

    #[tokio::test]
    async fn read_non_overridable_backend_uses_no_token() {
        // A local/clone backend ignores overrides → no user token demanded even
        // token-less and required.
        let state = state_with(true, true);
        let account = Uuid::new_v4();
        let headers = HeaderMap::new();
        let creds = TrajectCredentials::new(&state, account, &headers);
        let token = creds.for_read(&non_overridable(), false).await.unwrap();
        assert_eq!(token, None);
    }

    #[tokio::test]
    async fn read_tokenless_not_required_stays_none() {
        // Token-less, override-capable, but the deployment does not require a
        // user token → pre-spike behaviour, no override, no 428.
        let state = state_with(false, true);
        let account = Uuid::new_v4();
        let headers = HeaderMap::new();
        let creds = TrajectCredentials::new(&state, account, &headers);
        let token = creds.for_read(&overridable(), false).await.unwrap();
        assert_eq!(token, None);
    }

    #[tokio::test]
    async fn read_tokenless_required_linked_uses_user_token() {
        let state = state_with(true, true);
        let account = Uuid::new_v4();
        let headers = linked_headers(&state, account);
        let creds = TrajectCredentials::new(&state, account, &headers);
        let token = creds.for_read(&overridable(), false).await.unwrap();
        assert_eq!(token.as_deref(), Some("gho_test_token"));
    }

    #[tokio::test]
    async fn read_tokenless_required_unlinked_is_428() {
        let state = state_with(true, true);
        let account = Uuid::new_v4();
        let headers = HeaderMap::new();
        let creds = TrajectCredentials::new(&state, account, &headers);
        let err = creds.for_read(&overridable(), false).await.unwrap_err();
        assert_eq!(err.0, StatusCode::PRECONDITION_REQUIRED);
    }

    #[tokio::test]
    async fn read_without_oauth_config_stays_none() {
        // No `github_oauth` block at all → the whole user-token flow is inert.
        let state = state_with(false, false);
        let account = Uuid::new_v4();
        let headers = HeaderMap::new();
        let creds = TrajectCredentials::new(&state, account, &headers);
        let token = creds.for_read(&overridable(), false).await.unwrap();
        assert_eq!(token, None);
    }

    // --- Writes ------------------------------------------------------------

    #[tokio::test]
    async fn write_writable_at_rest_uses_service_token() {
        let state = state_with(true, true);
        let account = Uuid::new_v4();
        let headers = linked_headers(&state, account);
        let creds = TrajectCredentials::new(&state, account, &headers);
        let auth = creds.for_write(&overridable(), true).await.unwrap();
        assert_eq!(auth.read_token(), None);
        let ctx = auth.into_write_context("msg".to_string(), None);
        assert_eq!(ctx.token_override, None);
    }

    #[tokio::test]
    async fn write_tokenless_not_required_is_403() {
        // Token-less, override-capable, but not in user-token mode: there is no
        // credential to write with → the writability gate 403s (pre-spike:
        // such a backend was simply read-only).
        let state = state_with(false, true);
        let account = Uuid::new_v4();
        let headers = HeaderMap::new();
        let creds = TrajectCredentials::new(&state, account, &headers);
        let err = creds.for_write(&overridable(), false).await.unwrap_err();
        assert_eq!(err.0, StatusCode::FORBIDDEN);
    }

    #[tokio::test]
    async fn write_non_overridable_read_only_is_403() {
        // A read-only-at-rest, non-override backend can never be written.
        let state = state_with(true, true);
        let account = Uuid::new_v4();
        let headers = HeaderMap::new();
        let creds = TrajectCredentials::new(&state, account, &headers);
        let err = creds
            .for_write(&non_overridable(), false)
            .await
            .unwrap_err();
        assert_eq!(err.0, StatusCode::FORBIDDEN);
    }

    #[tokio::test]
    async fn write_tokenless_required_linked_uses_user_token() {
        let state = state_with(true, true);
        let account = Uuid::new_v4();
        let headers = linked_headers(&state, account);
        let creds = TrajectCredentials::new(&state, account, &headers);
        let auth = creds.for_write(&overridable(), false).await.unwrap();
        // The precondition read and the write authenticate as the same user.
        assert_eq!(auth.read_token(), Some("gho_test_token"));
        let ctx = auth.into_write_context("msg".to_string(), None);
        assert_eq!(ctx.token_override.as_deref(), Some("gho_test_token"));
    }

    #[tokio::test]
    async fn write_tokenless_required_unlinked_is_428() {
        let state = state_with(true, true);
        let account = Uuid::new_v4();
        let headers = HeaderMap::new();
        let creds = TrajectCredentials::new(&state, account, &headers);
        let err = creds.for_write(&overridable(), false).await.unwrap_err();
        assert_eq!(err.0, StatusCode::PRECONDITION_REQUIRED);
    }

    // --- Backend-less variants --------------------------------------------

    #[tokio::test]
    async fn scan_candidate_swallows_nothing_but_maps_link_state() {
        // Required + unlinked → 428 (the caller swallows it for seed-only
        // browsing; here we assert the raw decision).
        let state = state_with(true, true);
        let account = Uuid::new_v4();
        let headers = HeaderMap::new();
        let creds = TrajectCredentials::new(&state, account, &headers);
        let err = creds.read_scan_candidate().await.unwrap_err();
        assert_eq!(err.0, StatusCode::PRECONDITION_REQUIRED);

        // Not required → None.
        let state = state_with(false, true);
        let headers = HeaderMap::new();
        let creds = TrajectCredentials::new(&state, account, &headers);
        assert_eq!(creds.read_scan_candidate().await.unwrap(), None);
    }

    #[tokio::test]
    async fn preflight_no_service_token_required_unlinked_is_428() {
        // No CORPUS_AUTH_* env for this ref, user-token required, unlinked →
        // 428, not the 503 "not configured".
        let state = state_with(true, true);
        let account = Uuid::new_v4();
        let headers = HeaderMap::new();
        let creds = TrajectCredentials::new(&state, account, &headers);
        let err = creds
            .for_new_repo_preflight("example-unconfigured-ref")
            .await
            .unwrap_err();
        assert_eq!(err.0, StatusCode::PRECONDITION_REQUIRED);
    }

    #[tokio::test]
    async fn preflight_no_service_token_not_required_is_503() {
        // No service token and the deployment does not require a user token →
        // 503 with the env-var hint (nothing to preflight with).
        let state = state_with(false, true);
        let account = Uuid::new_v4();
        let headers = HeaderMap::new();
        let creds = TrajectCredentials::new(&state, account, &headers);
        let err = creds
            .for_new_repo_preflight("example-unconfigured-ref")
            .await
            .unwrap_err();
        assert_eq!(err.0, StatusCode::SERVICE_UNAVAILABLE);
        assert!(err.1.contains("CORPUS_AUTH_EXAMPLE_UNCONFIGURED_REF_TOKEN"));
    }
}
