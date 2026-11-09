use std::path::Path;

use crate::error::{CorpusError, Result};

/// Construct the per-source token env-var name from a slug.
///
/// Single source of truth — used by the resolver and surfaced in
/// operator-facing error messages so the name shown to the operator
/// matches what the resolver actually looks up. Keeping it in one place
/// guarantees that if the naming scheme ever changes (lowercase prefix,
/// extra separator, etc.) the resolver and the diagnostic log can't
/// silently drift apart and confuse an operator who follows the error
/// message.
pub fn token_env_name(auth_ref: &str) -> String {
    format!(
        "CORPUS_AUTH_{}_TOKEN",
        auth_ref.to_uppercase().replace('-', "_")
    )
}

/// Auth configuration for a corpus source, loaded from `corpus-auth.yaml`.
#[derive(Debug, Clone, serde::Deserialize)]
pub struct AuthConfig {
    pub sources: Vec<SourceAuth>,
}

/// Auth entry for a single source.
#[derive(Clone, serde::Deserialize)]
pub struct SourceAuth {
    pub id: String,
    pub token: String,
}

impl std::fmt::Debug for SourceAuth {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SourceAuth")
            .field("id", &self.id)
            .field("token", &"***")
            .finish()
    }
}

/// Where a resolved server token came from. Makes the strict-vs-legacy
/// outcome observable at every call-site: a `LegacyShared` origin on a
/// path that should never see the shared token is a bug you can now
/// assert on (and log), instead of an invisible property of *which*
/// resolver function happened to be called.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TokenOrigin {
    /// Per-source environment variable `CORPUS_AUTH_{KEY}_TOKEN`.
    ServiceEnv,
    /// Per-source entry in the `corpus-auth.yaml` auth file.
    ServiceAuthFile,
    /// Legacy shared `CORPUS_GIT_TOKEN` (harvester-era single token).
    /// Never produced for strict lookups.
    LegacyShared,
    /// No token found — the request will go out unauthenticated.
    None,
}

/// Outcome of a token lookup: the token (if any) plus where it came from.
#[derive(Clone)]
pub struct TokenDecision {
    token: Option<String>,
    origin: TokenOrigin,
}

/// Redacts the token, like [`SourceAuth`]'s `Debug` — decisions are meant
/// to be logged (that's the point of [`TokenOrigin`]), so a derived
/// `Debug` would put the raw secret one `{:?}` away from the log output.
impl std::fmt::Debug for TokenDecision {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("TokenDecision")
            .field("token", &self.token.as_ref().map(|_| "***"))
            .field("origin", &self.origin)
            .finish()
    }
}

impl TokenDecision {
    /// The resolved token, if any.
    pub fn token(&self) -> Option<&str> {
        self.token.as_deref()
    }

    /// Where the token came from ([`TokenOrigin::None`] when absent).
    pub fn origin(&self) -> TokenOrigin {
        self.origin
    }

    /// Consume the decision, keeping only the token. For call-sites that
    /// feed the token straight into a backend and don't need the origin.
    pub fn into_token(self) -> Option<String> {
        self.token
    }
}

/// What to resolve a token *for*. Constructed via [`TokenContext::for_source`]
/// (the canonical route — strictness comes from
/// [`Source::strict_auth`](crate::models::Source::strict_auth)) or
/// [`TokenContext::strict`] (for lookups keyed by user-derived input where no
/// [`Source`](crate::models::Source) object exists yet, e.g. the
/// create-traject preflight).
///
/// There is deliberately **no** constructor that requests the legacy
/// `CORPUS_GIT_TOKEN` fallback for a bare key: the fallback is only
/// reachable through a `Source` whose `strict_auth` is `false` (i.e. an
/// operator-managed manifest source). A new call-site therefore cannot
/// opt into the legacy fallback for a user-derived key by picking the
/// "wrong" function — the exfiltration vector this module exists to close.
#[derive(Debug, Clone, Copy)]
pub struct TokenContext<'a> {
    /// Lookup key: the source's `auth_ref` when set, else its id.
    key: &'a str,
    /// Strict lookups never fall back to the legacy shared token.
    strict: bool,
}

impl<'a> TokenContext<'a> {
    /// Context for a [`Source`](crate::models::Source): key is `auth_ref`
    /// (falling back to the source id), strictness follows
    /// [`strict_auth`](crate::models::Source::strict_auth).
    pub fn for_source(source: &'a crate::models::Source) -> Self {
        Self {
            key: source.auth_ref.as_deref().unwrap_or(&source.id),
            strict: source.strict_auth,
        }
    }

    /// Strict context for a bare auth key derived from untrusted input
    /// (e.g. repo coordinates from a create-traject request). Never falls
    /// back to the legacy shared `CORPUS_GIT_TOKEN`: an unknown key cleanly
    /// resolves to no token instead of shipping the central token (via
    /// `Authorization: Bearer`) to a repo the requester controls.
    pub fn strict(key: &'a str) -> Self {
        Self { key, strict: true }
    }

    /// The env var an operator must set for this lookup to succeed via the
    /// environment — for diagnostics.
    pub fn env_name(&self) -> String {
        token_env_name(self.key)
    }
}

/// The one entry point for answering "which server token do we use for this
/// source". All server-side GitHub credential lookups (backend construction,
/// index scans, fetches, push preflights, worker writes) go through
/// [`CredentialResolver::resolve`], so the strict-vs-legacy decision lives in
/// exactly one place instead of being re-encoded per call-site.
///
/// Resolution order:
/// 1. Per-source environment variable `CORPUS_AUTH_{KEY}_TOKEN`
///    (uppercased, hyphens → underscores). Use this for multi-source
///    setups that need isolation between source repos.
/// 2. `corpus-auth.yaml` file (if it exists) — explicit per-source
///    entries, same purpose as (1) but file-backed.
/// 3. **Non-strict contexts only:** legacy shared `CORPUS_GIT_TOKEN`. The
///    harvester era used this single global token for one upstream repo.
///    We accept it as a fallback so deployments that still set only the
///    legacy variable keep working for *every* manifest GitHub source
///    without duplicating the secret under each per-source name.
///    Per-source vars/file entries above still win. Strict contexts skip
///    this step entirely — see [`TokenContext::strict`] for why.
/// 4. No token (unauthenticated, lower rate limits).
///
/// This layer knows nothing about sessions, user OAuth, or HTTP status
/// codes; user-token fallbacks live in the editor-api on top of the
/// [`TokenDecision`] this returns.
#[derive(Debug, Clone, Copy)]
pub struct CredentialResolver<'a> {
    auth_file: Option<&'a Path>,
}

impl<'a> CredentialResolver<'a> {
    /// Build a resolver that consults the given `corpus-auth.yaml` (if any)
    /// as its file-backed lookup step.
    pub fn new(auth_file: Option<&'a Path>) -> Self {
        Self { auth_file }
    }

    /// Resolve a token for the given context. See the type-level docs for
    /// the resolution order.
    pub fn resolve(&self, ctx: TokenContext<'_>) -> Result<TokenDecision> {
        // 1. Per-source environment variable
        if let Some(token) = non_empty_env(&token_env_name(ctx.key)) {
            return Ok(TokenDecision {
                token: Some(token),
                origin: TokenOrigin::ServiceEnv,
            });
        }

        // 2. Auth file
        if let Some(token) = find_in_auth_file(ctx.key, self.auth_file)? {
            return Ok(TokenDecision {
                token: Some(token),
                origin: TokenOrigin::ServiceAuthFile,
            });
        }

        // 3. Legacy shared CORPUS_GIT_TOKEN — never for strict contexts:
        // a strict key derives from user input, and falling through here
        // would ship the central token to a user-chosen repo.
        if !ctx.strict {
            if let Some(token) = non_empty_env("CORPUS_GIT_TOKEN") {
                return Ok(TokenDecision {
                    token: Some(token),
                    origin: TokenOrigin::LegacyShared,
                });
            }
        }

        // 4. No token
        Ok(TokenDecision {
            token: None,
            origin: TokenOrigin::None,
        })
    }

    /// Convenience for the common source-driven lookup:
    /// `resolve(TokenContext::for_source(source))`. Every scan, fetch and
    /// backend construction for a [`Source`](crate::models::Source) should
    /// go through here — scans, fetches and backend construction resolving
    /// through different rules is exactly how a promote-write can succeed
    /// while the subsequent index scan of the same repo silently fails.
    pub fn resolve_source(&self, source: &crate::models::Source) -> Result<TokenDecision> {
        self.resolve(TokenContext::for_source(source))
    }
}

/// Read an env var, treating "unset" and "set but empty" the same way
/// (empty tokens would send a bogus `Authorization` header downstream).
fn non_empty_env(key: &str) -> Option<String> {
    match std::env::var(key) {
        Ok(v) if !v.is_empty() => Some(v),
        _ => None,
    }
}

/// Read the auth file (if any) and look up `auth_ref` in it. Returns
/// `None` when the file is absent or the ref isn't listed. The single
/// parser for the auth-file shape — a future change to the file format
/// only needs to land here.
fn find_in_auth_file(auth_ref: &str, auth_file: Option<&Path>) -> Result<Option<String>> {
    let Some(path) = auth_file else {
        return Ok(None);
    };
    if !path.exists() {
        return Ok(None);
    }
    let content = std::fs::read_to_string(path).map_err(|e| {
        CorpusError::Config(format!(
            "Failed to read auth file {}: {}",
            path.display(),
            e
        ))
    })?;
    let config: AuthConfig = serde_yaml_ng::from_str(&content).map_err(|e| {
        CorpusError::Config(format!(
            "Failed to parse auth file {}: {}",
            path.display(),
            e
        ))
    })?;
    Ok(config
        .sources
        .iter()
        .find(|s| s.id == auth_ref)
        .map(|entry| entry.token.clone()))
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;
    use std::io::Write;
    use std::sync::Mutex;
    use tempfile::NamedTempFile;

    // Tests in this module mutate the process-wide environment via
    // `CORPUS_GIT_TOKEN` / per-source vars. cargo's default parallel
    // test runner would then race two tests reading/writing the same
    // env vars and produce flakes. Serialize the env-mutating tests
    // behind one mutex so they take turns. Tests that only read a
    // unique-named var don't need it.
    static ENV_LOCK: Mutex<()> = Mutex::new(());

    fn resolve_key(key: &str, strict: bool, auth_file: Option<&Path>) -> TokenDecision {
        CredentialResolver::new(auth_file)
            .resolve(if strict {
                TokenContext::strict(key)
            } else {
                // Non-strict bare-key contexts are deliberately not
                // constructible outside this crate; the tests reach the
                // non-strict path the same way production does — through
                // a Source with `strict_auth: false`.
                TokenContext { key, strict: false }
            })
            .unwrap()
    }

    /// RAII guard: set an env var for the duration of a test, restoring
    /// the previous value (or absence) on drop — even on assert failure,
    /// so one failing case can't poison the environment for the next.
    struct EnvVarGuard {
        key: String,
        prev: Option<String>,
    }

    impl EnvVarGuard {
        fn set(key: &str, value: &str) -> Self {
            let prev = std::env::var(key).ok();
            unsafe { std::env::set_var(key, value) };
            Self {
                key: key.to_string(),
                prev,
            }
        }

        fn unset(key: &str) -> Self {
            let prev = std::env::var(key).ok();
            unsafe { std::env::remove_var(key) };
            Self {
                key: key.to_string(),
                prev,
            }
        }
    }

    impl Drop for EnvVarGuard {
        fn drop(&mut self) {
            match &self.prev {
                Some(v) => unsafe { std::env::set_var(&self.key, v) },
                None => unsafe { std::env::remove_var(&self.key) },
            }
        }
    }

    /// Full decision table: every combination of
    /// `{strict, per-source env set, auth-file entry present, legacy set}`
    /// against the expected `TokenDecision`. Pins the three standing
    /// guarantees in one place: env wins over file, per-source wins over
    /// legacy, and strict lookups never see the legacy token.
    #[test]
    fn decision_table_covers_all_strict_env_file_legacy_combinations() {
        let _g = ENV_LOCK.lock().unwrap();

        let key = "decision-table-src";
        let env_key = token_env_name(key);
        assert_eq!(env_key, "CORPUS_AUTH_DECISION_TABLE_SRC_TOKEN");

        let yaml = format!("sources:\n  - id: {key}\n    token: file-token\n");
        let mut file = NamedTempFile::new().unwrap();
        file.write_all(yaml.as_bytes()).unwrap();

        // (strict, env, file, legacy) -> (expected token, expected origin)
        type Case = (bool, bool, bool, bool, Option<&'static str>, TokenOrigin);
        #[rustfmt::skip]
        let cases: &[Case] = &[
            // Non-strict: env > file > legacy > none.
            (false, true,  true,  true,  Some("env-token"),    TokenOrigin::ServiceEnv),
            (false, true,  true,  false, Some("env-token"),    TokenOrigin::ServiceEnv),
            (false, true,  false, true,  Some("env-token"),    TokenOrigin::ServiceEnv),
            (false, true,  false, false, Some("env-token"),    TokenOrigin::ServiceEnv),
            (false, false, true,  true,  Some("file-token"),   TokenOrigin::ServiceAuthFile),
            (false, false, true,  false, Some("file-token"),   TokenOrigin::ServiceAuthFile),
            (false, false, false, true,  Some("legacy-token"), TokenOrigin::LegacyShared),
            (false, false, false, false, None,                 TokenOrigin::None),
            // Strict: identical, except legacy is never consulted.
            (true,  true,  true,  true,  Some("env-token"),    TokenOrigin::ServiceEnv),
            (true,  true,  true,  false, Some("env-token"),    TokenOrigin::ServiceEnv),
            (true,  true,  false, true,  Some("env-token"),    TokenOrigin::ServiceEnv),
            (true,  true,  false, false, Some("env-token"),    TokenOrigin::ServiceEnv),
            (true,  false, true,  true,  Some("file-token"),   TokenOrigin::ServiceAuthFile),
            (true,  false, true,  false, Some("file-token"),   TokenOrigin::ServiceAuthFile),
            (true,  false, false, true,  None,                 TokenOrigin::None),
            (true,  false, false, false, None,                 TokenOrigin::None),
        ];

        for &(strict, env, has_file, legacy, want_token, want_origin) in cases {
            let _env = if env {
                EnvVarGuard::set(&env_key, "env-token")
            } else {
                EnvVarGuard::unset(&env_key)
            };
            let _legacy = if legacy {
                EnvVarGuard::set("CORPUS_GIT_TOKEN", "legacy-token")
            } else {
                EnvVarGuard::unset("CORPUS_GIT_TOKEN")
            };
            let auth_file = has_file.then(|| file.path());

            let decision = resolve_key(key, strict, auth_file);
            let case = format!("strict={strict} env={env} file={has_file} legacy={legacy}");
            assert_eq!(decision.token(), want_token, "token mismatch for {case}");
            assert_eq!(decision.origin(), want_origin, "origin mismatch for {case}");
        }
    }

    #[test]
    fn resolve_reads_per_source_env_var() {
        // Use a unique env var name to avoid test interference
        let key = "CORPUS_AUTH_TEST_SOURCE_123_TOKEN";
        unsafe { std::env::set_var(key, "env-token-value") };

        let decision = resolve_key("test-source-123", false, None);
        assert_eq!(decision.token(), Some("env-token-value"));
        assert_eq!(decision.origin(), TokenOrigin::ServiceEnv);

        unsafe { std::env::remove_var(key) };
    }

    #[test]
    fn resolve_reads_auth_file() {
        let yaml = r#"
sources:
  - id: amsterdam
    token: file-token-123
  - id: rotterdam
    token: file-token-456
"#;
        let mut file = NamedTempFile::new().unwrap();
        file.write_all(yaml.as_bytes()).unwrap();

        let decision = resolve_key("amsterdam", false, Some(file.path()));
        assert_eq!(decision.token(), Some("file-token-123"));
        assert_eq!(decision.origin(), TokenOrigin::ServiceAuthFile);

        let decision = resolve_key("rotterdam", false, Some(file.path()));
        assert_eq!(decision.token(), Some("file-token-456"));
    }

    #[test]
    fn resolve_without_any_source_is_none() {
        let _g = ENV_LOCK.lock().unwrap();
        // Make sure the legacy CORPUS_GIT_TOKEN doesn't leak from the
        // test runner's environment and turn this assertion into a flake.
        let _legacy = EnvVarGuard::unset("CORPUS_GIT_TOKEN");
        let decision = resolve_key("nonexistent", false, None);
        assert_eq!(decision.token(), None);
        assert_eq!(decision.origin(), TokenOrigin::None);
    }

    #[test]
    fn resolve_falls_back_to_legacy_corpus_git_token() {
        let _g = ENV_LOCK.lock().unwrap();
        // No per-source var, no auth file → resolver picks up
        // CORPUS_GIT_TOKEN so deployments that still set only the legacy
        // single-token variable keep working across all sources.
        let _legacy = EnvVarGuard::set("CORPUS_GIT_TOKEN", "legacy-token-value");
        let decision = resolve_key("any-source-here", false, None);
        assert_eq!(decision.token(), Some("legacy-token-value"));
        assert_eq!(decision.origin(), TokenOrigin::LegacyShared);
    }

    #[test]
    fn per_source_env_wins_over_legacy_corpus_git_token() {
        let _g = ENV_LOCK.lock().unwrap();
        let _legacy = EnvVarGuard::set("CORPUS_GIT_TOKEN", "legacy-loses");
        let _per_source = EnvVarGuard::set("CORPUS_AUTH_PRIORITY_TEST_TOKEN", "per-source-wins");
        let decision = resolve_key("priority-test", false, None);
        assert_eq!(decision.token(), Some("per-source-wins"));
        assert_eq!(decision.origin(), TokenOrigin::ServiceEnv);
    }

    #[test]
    fn resolve_with_missing_auth_file_is_none() {
        let _g = ENV_LOCK.lock().unwrap();
        let _legacy = EnvVarGuard::unset("CORPUS_GIT_TOKEN");
        let decision = resolve_key("test", false, Some(Path::new("/nonexistent/auth.yaml")));
        assert_eq!(decision.token(), None);
    }

    #[test]
    fn strict_context_skips_legacy_corpus_git_token() {
        // The whole point of a strict context: an unknown auth key
        // must NOT fall through to `CORPUS_GIT_TOKEN`. If it did, an
        // attacker who can pick the key (via repo coords on the
        // create-traject endpoint) would harvest the operator's
        // central token on the next outgoing GitHub call. This test
        // pins the strict behaviour so a future refactor of the
        // resolver can't silently regress it.
        let _g = ENV_LOCK.lock().unwrap();
        let _legacy = EnvVarGuard::set("CORPUS_GIT_TOKEN", "central-secret");
        let decision = CredentialResolver::new(None)
            .resolve(TokenContext::strict("attacker-supplied-key"))
            .unwrap();
        assert_eq!(
            decision.token(),
            None,
            "strict resolver must NOT leak the legacy token"
        );
        assert_eq!(decision.origin(), TokenOrigin::None);
    }

    #[test]
    fn strict_context_still_uses_per_source_env() {
        // The strict variant still honours the explicit per-source env
        // var — that's the path operators are supposed to configure.
        // Holds `ENV_LOCK` like the sibling strict test: even though
        // the key here is unique, env-mutating tests run serialised so
        // a concurrent test holding the lock and mutating arbitrary
        // env vars can't race with this one.
        let _g = ENV_LOCK.lock().unwrap();
        let _per_source =
            EnvVarGuard::set("CORPUS_AUTH_STRICT_PER_SOURCE_OK_TOKEN", "per-source-value");
        let decision = CredentialResolver::new(None)
            .resolve(TokenContext::strict("strict-per-source-ok"))
            .unwrap();
        assert_eq!(decision.token(), Some("per-source-value"));
        assert_eq!(decision.origin(), TokenOrigin::ServiceEnv);
    }

    fn local_source(auth_ref: &str, strict_auth: bool) -> crate::models::Source {
        crate::models::Source {
            id: "src".to_string(),
            name: "Src".to_string(),
            source_type: crate::models::SourceType::Local {
                local: crate::models::LocalSource {
                    path: std::path::PathBuf::from("unused"),
                },
            },
            scopes: vec![],
            priority: 0,
            auth_ref: Some(auth_ref.to_string()),
            strict_auth,
            enriched: None,
            load_all: false,
            library_only: false,
        }
    }

    #[test]
    fn resolve_source_strict_source_skips_legacy_token() {
        // A `strict_auth` source (traject writable-own, user-supplied repo
        // coords) must resolve strictly on EVERY path that goes through the
        // source object — most importantly the index scan, which used to
        // fall back to `CORPUS_GIT_TOKEN` while the push path resolved
        // strictly.
        let _g = ENV_LOCK.lock().unwrap();
        let _legacy = EnvVarGuard::set("CORPUS_GIT_TOKEN", "central-secret");
        let resolver = CredentialResolver::new(None);
        let strict = resolver
            .resolve_source(&local_source("user-picked-ref", true))
            .unwrap();
        let legacy = resolver
            .resolve_source(&local_source("user-picked-ref", false))
            .unwrap();
        assert_eq!(
            strict.token(),
            None,
            "strict source must not leak the legacy token"
        );
        assert_eq!(strict.origin(), TokenOrigin::None);
        assert_eq!(
            legacy.token(),
            Some("central-secret"),
            "manifest sources keep the legacy single-PAT fallback"
        );
        assert_eq!(legacy.origin(), TokenOrigin::LegacyShared);
    }

    #[test]
    fn resolve_source_uses_auth_ref_over_source_id() {
        // The lookup key is `auth_ref` when present (RFC-010: `auth_ref`
        // in the manifest references an entry in the auth config), the
        // source id otherwise.
        let _g = ENV_LOCK.lock().unwrap();
        let _per_ref = EnvVarGuard::set("CORPUS_AUTH_REF_KEY_WINS_TOKEN", "ref-token");
        let decision = CredentialResolver::new(None)
            .resolve_source(&local_source("ref-key-wins", false))
            .unwrap();
        assert_eq!(decision.token(), Some("ref-token"));

        let mut no_ref = local_source("unused", false);
        no_ref.auth_ref = None;
        no_ref.id = "id-key-wins".to_string();
        let _per_id = EnvVarGuard::set("CORPUS_AUTH_ID_KEY_WINS_TOKEN", "id-token");
        let decision = CredentialResolver::new(None)
            .resolve_source(&no_ref)
            .unwrap();
        assert_eq!(decision.token(), Some("id-token"));
    }

    #[test]
    fn empty_env_var_counts_as_unset() {
        // An empty per-source var must not shadow the auth file (or
        // produce an empty Authorization header downstream).
        let _g = ENV_LOCK.lock().unwrap();
        let key = "empty-env-src";
        let _empty = EnvVarGuard::set(&token_env_name(key), "");
        let _legacy = EnvVarGuard::unset("CORPUS_GIT_TOKEN");

        let yaml = format!("sources:\n  - id: {key}\n    token: file-wins\n");
        let mut file = NamedTempFile::new().unwrap();
        file.write_all(yaml.as_bytes()).unwrap();

        let decision = resolve_key(key, false, Some(file.path()));
        assert_eq!(decision.token(), Some("file-wins"));
        assert_eq!(decision.origin(), TokenOrigin::ServiceAuthFile);
    }
}
