//! Traject CRUD endpoints.
//!
//! Trajects group law edits under a named project with its own federated
//! corpus config and a stable branch on the writable source. Edits made
//! while a traject is active are routed through the traject's corpus
//! ([`crate::traject_corpus`]) instead of the globally configured one.

use axum::extract::{Path, State};
use axum::http::StatusCode;
use axum::{Extension, Json};
use serde::{Deserialize, Serialize};
use sqlx::PgPool;
use tower_sessions::Session;
use uuid::Uuid;

use regelrecht_pipeline::tasks;

use crate::accounts::AccountRecord;
use crate::state::AppState;

#[derive(Debug, Serialize, sqlx::FromRow)]
pub struct TrajectSummary {
    pub id: Uuid,
    pub name: String,
    pub description: String,
    pub scope: String,
    pub status: String,
    pub role: String,
    /// URL-form reference: `{slug}-{8hex}`. Built from current `name`
    /// and `id`; the slug part is cosmetic, the trailing 8 hex chars of
    /// the uuid are the actual lookup key (see `resolve_traject_ref`).
    /// Populated post-fetch — sqlx::FromRow doesn't see it in the SELECT.
    ///
    /// `Option<String>` (not `String` with a default of `""`) so a future
    /// code path that fetches a `TrajectSummary` and forgets to call
    /// `fill_ref()` serializes to `"ref": null` instead of `"ref": ""`.
    /// The frontend's `t.ref === activeTrajectRef.value` comparison then
    /// fails loudly (never matches) instead of silently equating two
    /// empty strings against a missing trajectRef.
    #[serde(rename = "ref")]
    #[sqlx(default)]
    pub traject_ref: Option<String>,
}

impl TrajectSummary {
    /// Recompute `traject_ref` from the current `name` and `id`. Called
    /// right after a sqlx fetch and after any in-memory mutation that
    /// might change the slug.
    pub fn fill_ref(&mut self) {
        self.traject_ref = Some(traject_ref(&self.name, self.id));
    }
}

#[derive(Debug, Serialize, sqlx::FromRow)]
pub struct TrajectMember {
    pub account_id: Uuid,
    pub email: String,
    pub name: String,
    pub role: String,
}

#[derive(Debug, Serialize, sqlx::FromRow)]
pub struct TrajectSourceDto {
    pub source_id: String,
    pub name: String,
    pub source_type: String,
    pub gh_owner: Option<String>,
    pub gh_repo: Option<String>,
    pub gh_branch: Option<String>,
    pub gh_base_branch: Option<String>,
    pub gh_path: Option<String>,
    pub gh_ref: Option<String>,
    pub local_path: Option<String>,
    pub priority: i32,
    pub auth_ref: Option<String>,
    pub is_writable_own: bool,
}

#[derive(Debug, Serialize, sqlx::FromRow)]
pub struct TrajectInvite {
    pub email: String,
    pub role: String,
}

#[derive(Debug, Serialize)]
pub struct TrajectDetail {
    #[serde(flatten)]
    pub summary: TrajectSummary,
    pub members: Vec<TrajectMember>,
    pub pending_invites: Vec<TrajectInvite>,
    pub sources: Vec<TrajectSourceDto>,
}

#[derive(Debug, Deserialize)]
pub struct CreateTrajectRequest {
    pub name: String,
    #[serde(default)]
    pub description: String,
    #[serde(default)]
    pub scope: String,
    /// GitHub `owner` for the writable-own source. When `None`, falls
    /// back to the central MinBZK repo (phase-1 default). When set,
    /// `repo_name` and `base_branch` must also be set; the server
    /// derives `auth_ref` deterministically and looks up the matching
    /// `CORPUS_AUTH_{AUTH_REF}_TOKEN` env var.
    #[serde(default)]
    pub repo_owner: Option<String>,
    /// GitHub `repo`. Required when `repo_owner` is set.
    #[serde(default)]
    pub repo_name: Option<String>,
    /// Branch to base the traject branch off of (and target with the
    /// session PR). Required when `repo_owner` is set; the default
    /// branch of the repo if you're unsure.
    #[serde(default)]
    pub base_branch: Option<String>,
    /// Optional sub-path within the repo where regulation YAML files
    /// live. Empty / omitted means "everything under repo root" — the
    /// right default for user repos dedicated to regulations. Set this
    /// when the YAML files sit in a subdirectory like `regulation/nl`
    /// (which is the MinBZK default).
    #[serde(default)]
    pub repo_path: Option<String>,
}

/// GitHub `owner` / `repo` segments end up in a URL path
/// (`/repos/{owner}/{repo}`), in the on-disk session-branch name, and
/// in the commit/PR body. Accept only the character set that GitHub
/// itself allows for these identifiers — alphanumerics plus `-`, `_`,
/// and `.`. Lets us refuse path-traversal-shaped input (`..`, `/`) at
/// the API boundary rather than relying on GitHub to 404 it.
///
/// GitHub additionally disallows the literal segments `.` and `..` and
/// names starting with a leading `.` (hidden/reserved). Reject those
/// here so we don't store a row that GitHub will subsequently 404 on
/// every contents/branches call.
fn valid_repo_segment(s: &str) -> bool {
    if s.is_empty() || s.len() > 100 {
        return false;
    }
    if s == "." || s == ".." || s.starts_with('.') {
        return false;
    }
    s.chars()
        .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_' || c == '.')
}

/// Reject branch names that git itself would refuse as refnames.
///
/// Mirrors the subset of `git-check-ref-format` rules that we can apply
/// to a single component: no control chars, no whitespace, no
/// `~^:?*[\` or `..`, no leading dash, no leading/trailing slash, no
/// `@{` sequence. Bounded length so an oversized input can't bloat the
/// on-disk branch name.
///
/// `base_branch` flows unencoded into a GitHub Contents-API URL
/// (`/repos/{owner}/{repo}/branches/{base_branch}`) and is later
/// persisted + used as a git refname in `commit_and_push_to_branch`.
/// A non-empty trim check would let `"main?spoof=1"` through — GitHub
/// strips the query, validates `main`, and the bogus suffix lands in
/// the DB. This guard refuses any refname-illegal character at the API
/// boundary so the persisted value is always a clean refname.
fn valid_branch_name(s: &str) -> bool {
    if s.is_empty() || s.len() > 200 {
        return false;
    }
    // `git-check-ref-format` rejects the bare `@` as a refname.
    if s == "@" {
        return false;
    }
    // Leading `-`/`/`, trailing `/`, and a trailing `.` are all rejected
    // by `git-check-ref-format` (e.g. `main.` is refused).
    if s.starts_with('-') || s.starts_with('/') || s.ends_with('/') || s.ends_with('.') {
        return false;
    }
    if s.contains("..") || s.contains("@{") || s.contains("//") {
        return false;
    }
    // Per `git-check-ref-format`: no path component may start with `.`
    // (so `.hidden` and `feature/.hidden` are both refused) and none may
    // end with `.lock` (git treats `<ref>.lock` as a lockfile sentinel,
    // so `main.lock` and `feature/foo.lock` are refused). Walk the
    // components once for both rules.
    if s.split('/')
        .any(|c| c.starts_with('.') || c.ends_with(".lock"))
    {
        return false;
    }
    s.chars().all(|c| {
        !c.is_control()
            && c != ' '
            && c != '~'
            && c != '^'
            && c != ':'
            && c != '?'
            && c != '*'
            && c != '['
            && c != '\\'
            // `#` is a legal refname char in git but is the URL fragment
            // delimiter. `format!(".../branches/{base_branch}")` followed
            // by reqwest parsing would silently drop everything after
            // `#`, so a branch name `main#spoof` would 200-pass the
            // pre-flight (it matches `main`) but later git operations
            // reference the non-existent `main#spoof`. Reject up front.
            && c != '#'
    })
}

/// Validate a user-supplied `repo_path` (sub-directory within a repo).
///
/// Empty is allowed and means "repo root". Otherwise the path must be
/// relative (no leading `/`, no Windows prefix) and contain only
/// "normal" components — no `..`, no `.`, no root. Each segment is
/// further constrained to the same character set as `valid_repo_segment`
/// so it can safely flow into a GitHub Contents-API URL without
/// double-encoding or escaping surprises.
///
/// `validate_relative_path` inside `GitBackend::resolve` covers the
/// inner per-file relative path, but the *base* `repo_subpath` (stored
/// on the row as `gh_path`) never reaches that check. Without this
/// guard a caller could ship `repo_path = "../../etc"` and the backend
/// would read/write outside the per-traject clone.
fn valid_repo_path(s: &str) -> bool {
    if s.is_empty() {
        return true;
    }
    let p = std::path::Path::new(s);
    if p.is_absolute() {
        return false;
    }
    for c in p.components() {
        match c {
            std::path::Component::Normal(seg) => {
                let seg_str = seg.to_string_lossy();
                if !valid_repo_segment(&seg_str) {
                    return false;
                }
            }
            // ParentDir (`..`), CurDir (`.`), RootDir, Prefix — all
            // rejected so the stored gh_path is always a clean,
            // forward-traversal-only relative path.
            _ => return false,
        }
    }
    true
}

// Phase-1 default writable source — the central MinBZK corpus repo on
// its `development` branch, using the app-wide
// `CORPUS_AUTH_MINBZK_CENTRAL_TOKEN`. Used when the create-request omits
// the repo_* fields entirely, so existing flows keep working unchanged.
// When the request *does* supply repo coordinates, all four user-facing
// fields (owner/repo/branch/path) are taken from the request and
// `auth_ref` is derived from owner+repo (see `derive_auth_ref`).
const CENTRAL_WRITABLE_OWNER: &str = "MinBZK";
const CENTRAL_WRITABLE_REPO: &str = "regelrecht-corpus";
const CENTRAL_WRITABLE_PATH: &str = "regulation/nl";
const CENTRAL_WRITABLE_BASE_BRANCH: &str = "development";
const CENTRAL_WRITABLE_AUTH_REF: &str = "minbzk-central";
const CENTRAL_WRITABLE_NAME: &str = "MinBZK/regelrecht-corpus";

/// User-facing rejection for a malformed `repo_path`. Shared by `create`
/// and `update` so both entry points describe the same rule in the same
/// words — the UI shows this string verbatim.
const REPO_PATH_INVALID_MSG: &str =
    "repo_path moet een relatief pad zijn zonder '..' segmenten en \
     mag alleen letters, cijfers, en '-', '_', '.' bevatten";

#[derive(Debug, Deserialize)]
pub struct UpdateTrajectRequest {
    pub name: Option<String>,
    pub description: Option<String>,
    pub scope: Option<String>,
    /// Either `"bezig"` or `"afgerond"`.
    pub status: Option<String>,
    /// New sub-path within the writable-own repo (DB column `gh_path`
    /// on the `is_writable_own` source). `None` (field omitted) leaves
    /// the current path alone; `Some("")` — or any whitespace-only
    /// string — means "repo root" and is stored as SQL `NULL`, the same
    /// normalisation `create` applies.
    ///
    /// Everything the editor reads and writes on the own source hangs
    /// off this path: laws, scenarios, annotations and documents. Moving
    /// it therefore moves the editor's whole view of the repo; files
    /// outside the new root stop being regulation as far as the editor
    /// is concerned. Relocating them is the user's job (on the branch),
    /// not this endpoint's.
    pub repo_path: Option<String>,
}

/// Body of `PUT /api/trajects/:id/repo`: the GitHub repository the
/// traject's own source should point at from now on.
#[derive(Debug, Deserialize)]
pub struct MoveRepoRequest {
    pub repo_owner: String,
    pub repo_name: String,
}

#[derive(Debug, Deserialize)]
pub struct AddMemberRequest {
    /// Email of the user to invite. If an account already exists for
    /// this email the row lands in `traject_members` (active). If not,
    /// it goes into `traject_invites` (pending) and is promoted to a
    /// real membership the next time someone with that email claim hits
    /// any authenticated endpoint — see `accounts::ensure_account`.
    pub email: String,
    pub role: String,
}

#[derive(Debug, Deserialize)]
pub struct UpdateMemberRequest {
    pub role: String,
}

#[derive(Debug, Serialize)]
pub struct AddMemberResponse {
    /// `"active"` when a `traject_members` row was created (account
    /// existed), `"pending"` when only a `traject_invites` row was
    /// created (no account yet for this email).
    pub status: &'static str,
    /// Normalised (lowercased + trimmed) email used as the key.
    pub email: String,
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn get_pool(state: &AppState) -> Result<&PgPool, StatusCode> {
    state.pool.as_ref().ok_or(StatusCode::SERVICE_UNAVAILABLE)
}

fn db_err<E: std::fmt::Display>(context: &'static str) -> impl FnOnce(E) -> StatusCode {
    move |e| {
        tracing::error!(error = %e, "{context}");
        StatusCode::INTERNAL_SERVER_ERROR
    }
}

/// Same as [`get_pool`] but for handlers that return `(StatusCode, String)`
/// so the body can carry a user-facing message. Used by `create` where
/// failure reasons (missing token, missing branch, …) need to reach the UI.
fn get_pool_msg(state: &AppState) -> Result<&PgPool, (StatusCode, String)> {
    state.pool.as_ref().ok_or((
        StatusCode::SERVICE_UNAVAILABLE,
        "database not configured".to_string(),
    ))
}

/// Same as [`db_err`] but returning `(StatusCode, String)`. Empty body —
/// 500s caused by a DB issue carry no information the caller can act on,
/// so we just log on the server and let the client see a bare 500.
fn db_err_msg<E: std::fmt::Display>(
    context: &'static str,
) -> impl FnOnce(E) -> (StatusCode, String) {
    move |e| {
        tracing::error!(error = %e, "{context}");
        (StatusCode::INTERNAL_SERVER_ERROR, String::new())
    }
}

/// Slugify a traject name for the branch suffix: lowercase, allowed chars
/// `[a-z0-9-]`, runs of disallowed chars collapsed to a single dash,
/// trimmed and capped at 32 characters.
fn slugify(name: &str) -> String {
    let mut out = String::with_capacity(name.len());
    let mut prev_dash = true; // suppress leading dashes
    for ch in name.chars() {
        let mapped = if ch.is_ascii_alphanumeric() {
            Some(ch.to_ascii_lowercase())
        } else if ch.is_whitespace() || ch == '-' || ch == '_' {
            Some('-')
        } else {
            None
        };
        match mapped {
            Some('-') => {
                if !prev_dash {
                    out.push('-');
                    prev_dash = true;
                }
            }
            Some(c) => {
                out.push(c);
                prev_dash = false;
            }
            None => {}
        }
        if out.len() >= 32 {
            break;
        }
    }
    while out.ends_with('-') {
        out.pop();
    }
    if out.is_empty() {
        out.push_str("traject");
    }
    out
}

fn derive_branch_name(name: &str, traject_id: Uuid) -> String {
    let slug = slugify(name);
    let short = traject_id.simple().to_string()[..8].to_string();
    format!("traject/{slug}-{short}")
}

/// First 8 hex characters of a traject UUID — the suffix used in the
/// URL ref (`{slug}-{short}`). Same length as the branch-name short id so
/// users see one identifier across URL and branch.
///
/// `simple()` emits the 32-char hyphen-less form (`3f4a8b2c…`), and the
/// canonical hyphenated form (`3f4a8b2c-…`) used by Postgres `id::text`
/// places its first hyphen at position 8 — so `left(id::text, 8)` in
/// SQL and `traject_id.simple()[..8]` in Rust produce the SAME first-8
/// hex chars. `resolve_traject_ref`'s DB lookup
/// (`WHERE left(id::text, 8) = $1`) relies on this alignment.
pub fn short_id(traject_id: Uuid) -> String {
    traject_id.simple().to_string()[..8].to_string()
}

/// Build the URL-form ref for a traject from its current name and id.
/// `{slug}-{8hex}`. The slug part is cosmetic — the resolver only cares
/// about the trailing 8-hex chunk, so renaming a traject does not break
/// existing URLs.
pub fn traject_ref(name: &str, traject_id: Uuid) -> String {
    format!("{}-{}", slugify(name), short_id(traject_id))
}

/// Derive the `auth_ref` for a user-supplied `<owner>/<repo>` so the
/// existing token-resolver picks up the right
/// `CORPUS_AUTH_{AUTH_REF_UPPER}_TOKEN` env var without anyone having
/// to hand-maintain the mapping. Operators see a deterministic env-var
/// name derived from the repo coordinates.
///
/// Rule: lowercase, runs of non-alphanumeric characters collapse to a
/// single `-`, leading/trailing dashes trimmed. This is a slug, not a
/// hash — it's lossy and not injective (e.g. `("a-", "-b")` and
/// `("a", "b")` both produce `"a-b"`), so two pathological repo
/// names could share an env var. Acceptable trade-off for legibility;
/// the env-var collision just means both repos use the same token.
///
/// Examples:
///   ("Acme",   "Secret-Repo")        → "acme-secret-repo"
///   ("MinBZK", "regelrecht-corpus")  → "minbzk-regelrecht-corpus"
///   ("a.b",    "c_d")                → "a-b-c-d"
pub fn derive_auth_ref(owner: &str, repo: &str) -> String {
    let mut out = String::new();
    let mut prev_dash = true; // suppress leading dashes
    let combined = format!("{owner}/{repo}");
    for ch in combined.chars() {
        if ch.is_ascii_alphanumeric() {
            out.push(ch.to_ascii_lowercase());
            prev_dash = false;
        } else if !prev_dash {
            out.push('-');
            prev_dash = true;
        }
    }
    while out.ends_with('-') {
        out.pop();
    }
    out
}

/// Resolve a `{slug}-{8hex}` URL ref to a traject UUID. Returns 400 on
/// a malformed ref, 404 when no traject has a uuid starting with the
/// suffix.
///
/// The UUID prefix is uniformly distributed across 32 bits — for N=1k
/// trajects the birthday-bound collision probability is
/// N·(N−1)/(2·2³²) ≈ 1.16×10⁻⁴, which we accept for the readability
/// gain. Ambiguous prefixes surface as 409 Conflict
/// (the URL is genuinely ambiguous; we refuse to guess and the caller
/// must rebuild the ref against a fresh traject). A tracing error on
/// the duplicate branch catches the case in production before it bites
/// a user.
pub async fn resolve_traject_ref(
    pool: &PgPool,
    traject_ref: &str,
) -> Result<Uuid, (StatusCode, String)> {
    // 8-hex suffix preceded by a dash. Anything else (bare UUID, raw
    // slug without suffix, garbage) is a 400 — we don't try to fall
    // back to a bare-uuid lookup because that path no longer exists in
    // the URL contract.
    //
    // Reject non-ASCII up front. Valid refs are slug + 8 hex chars,
    // both ASCII by construction; without this guard a crafted
    // multi-byte sequence like `abcé1234567` passes the length check
    // and then panics on the byte-index slicing below (a multi-byte
    // char straddling `suffix_start` is a char-boundary mid-slice).
    if !traject_ref.is_ascii() {
        return Err((
            StatusCode::BAD_REQUEST,
            "Malformed traject reference".to_string(),
        ));
    }
    let suffix_start = traject_ref.len().checked_sub(8).ok_or_else(|| {
        (
            StatusCode::BAD_REQUEST,
            "Malformed traject reference".to_string(),
        )
    })?;
    // Reject empty- and missing-slug refs in one go:
    //   suffix_start == 0  → ref is exactly 8 hex chars (no dash, no slug)
    //   suffix_start == 1  → ref is `-{8hex}` (the slug part is empty)
    // The SPA router regex requires at least one alphanumeric char
    // before the dash, so the frontend can never produce these; a
    // direct HTTP request still hits the DB lookup if we don't gate
    // here. Aligns both layers on the same minimum shape.
    if suffix_start <= 1 {
        return Err((
            StatusCode::BAD_REQUEST,
            "Malformed traject reference".to_string(),
        ));
    }
    let separator = &traject_ref[suffix_start - 1..suffix_start];
    let suffix = &traject_ref[suffix_start..];
    if separator != "-" || !suffix.chars().all(|c| c.is_ascii_hexdigit()) {
        return Err((
            StatusCode::BAD_REQUEST,
            "Malformed traject reference".to_string(),
        ));
    }
    let suffix_lower = suffix.to_ascii_lowercase();

    // UUID text format starts with the first 8 hex chars (`xxxxxxxx-...`).
    // Equality on `left(id::text, 8)` matches our short id exactly and
    // uses the functional index from migration 0017
    // (`trajects_short_id_idx`) — every traject-scoped request runs this
    // lookup, so the index avoids a seq scan on every save once the
    // table grows.
    let rows: Vec<(Uuid,)> = sqlx::query_as("SELECT id FROM trajects WHERE left(id::text, 8) = $1")
        .bind(&suffix_lower)
        .fetch_all(pool)
        .await
        .map_err(|e| {
            tracing::error!(error = %e, "resolve traject ref query failed");
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                "Failed to resolve traject reference".to_string(),
            )
        })?;
    match rows.len() {
        1 => Ok(rows[0].0),
        0 => Err((StatusCode::NOT_FOUND, "Traject not found".to_string())),
        _ => {
            // Two trajects whose UUIDs share the same first 8 hex
            // chars — astronomical odds, but if it happens the URL is
            // ambiguous. Log loudly and refuse rather than guess.
            tracing::error!(
                suffix = %suffix_lower,
                count = rows.len(),
                "traject ref short id collides; refusing to guess"
            );
            Err((
                StatusCode::CONFLICT,
                "Traject reference is ambiguous (id-suffix collision); contact support".to_string(),
            ))
        }
    }
}

/// Look up the role of an account in a traject. Returns `None` when the
/// account is not a member.
async fn member_role(
    pool: &PgPool,
    traject_id: Uuid,
    account_id: Uuid,
) -> sqlx::Result<Option<String>> {
    let row: Option<(String,)> = sqlx::query_as(
        "SELECT role::text FROM traject_members
         WHERE traject_id = $1 AND account_id = $2",
    )
    .bind(traject_id)
    .bind(account_id)
    .fetch_optional(pool)
    .await?;
    Ok(row.map(|r| r.0))
}

pub(crate) async fn require_membership(
    pool: &PgPool,
    traject_id: Uuid,
    account_id: Uuid,
) -> Result<String, StatusCode> {
    member_role(pool, traject_id, account_id)
        .await
        .map_err(db_err("member lookup failed"))?
        .ok_or(StatusCode::FORBIDDEN)
}

async fn require_owner(
    pool: &PgPool,
    traject_id: Uuid,
    account_id: Uuid,
) -> Result<(), StatusCode> {
    let role = require_membership(pool, traject_id, account_id).await?;
    if role == "owner" {
        Ok(())
    } else {
        Err(StatusCode::FORBIDDEN)
    }
}

/// The realm role that may move any traject's repo, member or not.
const TRAJECT_ADMIN_ROLE: &str = "editor-admin";

/// Allow only a session holding [`TRAJECT_ADMIN_ROLE`]; being the traject's
/// owner is not enough.
///
/// The role is read from the session, so this only opens for a logged-in
/// session that carries it; with auth disabled nobody is admin. An admin
/// gets a 404 for a traject that does not exist.
async fn require_admin(
    pool: &PgPool,
    session: &Session,
    traject_id: Uuid,
) -> Result<(), StatusCode> {
    let is_admin = matches!(
        regelrecht_auth::check_session_role(session, TRAJECT_ADMIN_ROLE).await,
        regelrecht_auth::RoleCheck::Allowed
    );
    if !is_admin {
        return Err(StatusCode::FORBIDDEN);
    }
    let exists: bool = sqlx::query_scalar("SELECT EXISTS (SELECT 1 FROM trajects WHERE id = $1)")
        .bind(traject_id)
        .fetch_one(pool)
        .await
        .map_err(db_err("traject lookup failed"))?;
    if exists {
        Ok(())
    } else {
        Err(StatusCode::NOT_FOUND)
    }
}

fn validate_role(role: &str) -> Result<(), StatusCode> {
    if role == "owner" || role == "contributor" {
        Ok(())
    } else {
        Err(StatusCode::BAD_REQUEST)
    }
}

/// Normalise an email for storage and comparison: trim whitespace,
/// lowercase, and reject obvious non-emails so junk addresses can't
/// accumulate as pending invites that will never promote.
///
/// We intentionally avoid pulling in a full RFC 5322 parser: invite
/// creation is owner-only behind OIDC, so this is correctness/data
/// hygiene rather than a security boundary. The structural check is:
/// exactly one `@`, non-empty local part, non-empty domain part, and a
/// `.` in the domain. The IdP is the source of truth for whether an
/// address is actually deliverable.
fn normalize_email(raw: &str) -> Option<String> {
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return None;
    }
    let at_count = trimmed.bytes().filter(|b| *b == b'@').count();
    if at_count != 1 {
        return None;
    }
    let (local, domain) = trimmed.split_once('@')?;
    if local.is_empty() || domain.is_empty() || !domain.contains('.') {
        return None;
    }
    Some(trimmed.to_lowercase())
}

fn validate_status(status: &str) -> Result<(), StatusCode> {
    if status == "bezig" || status == "afgerond" {
        Ok(())
    } else {
        Err(StatusCode::BAD_REQUEST)
    }
}

// ---------------------------------------------------------------------------
// Endpoints
// ---------------------------------------------------------------------------

/// GET /api/trajects — list trajects the caller is a member of.
pub async fn list(
    State(state): State<AppState>,
    Extension(account): Extension<AccountRecord>,
) -> Result<Json<Vec<TrajectSummary>>, StatusCode> {
    let pool = get_pool(&state)?;
    let mut rows: Vec<TrajectSummary> = sqlx::query_as(
        "SELECT t.id, t.name, t.description, t.scope,
                t.status::text AS status,
                tm.role::text  AS role
         FROM trajects t
         JOIN traject_members tm ON tm.traject_id = t.id
         WHERE tm.account_id = $1
         ORDER BY t.updated_at DESC",
    )
    .bind(account.id)
    .fetch_all(pool)
    .await
    .map_err(db_err("list trajects failed"))?;
    for row in &mut rows {
        row.fill_ref();
    }
    Ok(Json(rows))
}

/// GET /api/trajects/:id — details (members + sources).
pub async fn get(
    State(state): State<AppState>,
    Extension(account): Extension<AccountRecord>,
    Path(id): Path<Uuid>,
) -> Result<Json<TrajectDetail>, StatusCode> {
    let pool = get_pool(&state)?;
    let role = require_membership(pool, id, account.id).await?;

    let mut summary: TrajectSummary = sqlx::query_as(
        "SELECT id, name, description, scope,
                status::text AS status,
                $2           AS role
         FROM trajects WHERE id = $1",
    )
    .bind(id)
    .bind(&role)
    .fetch_one(pool)
    .await
    .map_err(db_err("traject summary fetch failed"))?;
    summary.fill_ref();

    let members: Vec<TrajectMember> = sqlx::query_as(
        "SELECT a.id AS account_id, a.email, a.name, tm.role::text AS role
         FROM traject_members tm
         JOIN accounts a ON a.id = tm.account_id
         WHERE tm.traject_id = $1
         ORDER BY tm.added_at",
    )
    .bind(id)
    .fetch_all(pool)
    .await
    .map_err(db_err("traject members fetch failed"))?;

    // Pending invites carry the email addresses of people the owner has
    // invited but who haven't yet logged in. Only owners — who can act
    // on invites (cancel, re-invite, change role) — get to see them;
    // contributors get an empty list. This keeps invitee emails out of
    // a non-owner's view by default.
    let pending_invites: Vec<TrajectInvite> = if role == "owner" {
        sqlx::query_as(
            "SELECT email, role::text AS role
             FROM traject_invites
             WHERE traject_id = $1
             ORDER BY invited_at",
        )
        .bind(id)
        .fetch_all(pool)
        .await
        .map_err(db_err("traject invites fetch failed"))?
    } else {
        Vec::new()
    };

    let sources: Vec<TrajectSourceDto> = sqlx::query_as(
        "SELECT source_id, name, source_type::text AS source_type,
                gh_owner, gh_repo, gh_branch, gh_base_branch, gh_path, gh_ref,
                local_path, priority, auth_ref, is_writable_own
         FROM traject_corpus_sources
         WHERE traject_id = $1
         ORDER BY priority",
    )
    .bind(id)
    .fetch_all(pool)
    .await
    .map_err(db_err("traject sources fetch failed"))?;

    Ok(Json(TrajectDetail {
        summary,
        members,
        pending_invites,
        sources,
    }))
}

/// Resolved writable-own source coordinates for the create handler.
/// Either the phase-1 MinBZK defaults or the user-supplied repo after
/// validation. The handler picks once up-front and then threads this
/// through the INSERT — no `Option` juggling in the SQL bind list.
struct WritableTarget {
    owner: String,
    repo: String,
    base_branch: String,
    /// Optional sub-path within the repo. `None` means "everything
    /// under repo root" and is persisted as SQL `NULL` — consistent
    /// with how seeded sources without a subpath are stored. `Some` is
    /// always a non-empty, validated relative path (see
    /// `valid_repo_path`).
    path: Option<String>,
    auth_ref: String,
    /// Display name used as the source label in the federation config.
    /// `MinBZK/regelrecht-corpus` for the default, `<owner>/<repo>` for
    /// user-supplied repos.
    display_name: String,
    /// True when resolving this target minted the traject branch on
    /// GitHub. Only the user-supplied-repo arm does that; the MinBZK
    /// default leaves it `false` because its service-token backend
    /// bootstraps its own branch in `ensure_ready`.
    ///
    /// The handler uses this to tell an operator about a branch that was
    /// created but whose traject never made it into the database — see
    /// the failure log in `create`.
    minted_branch: bool,
}

/// Decide whether the create-request asks for the MinBZK default or a
/// user-supplied repo, then validate the latter against the configured
/// token before we open the DB transaction.
///
/// The three `repo_*` fields are all-or-nothing: any subset other than
/// "none" or "all three" is a 400 — partial submissions almost always
/// mean the frontend forgot to wire one field.
async fn resolve_writable_target(
    state: &AppState,
    req: &CreateTrajectRequest,
    account_id: Uuid,
    headers: &axum::http::HeaderMap,
    // The branch this traject will write to, derived once by the caller
    // and reused verbatim for both the GitHub ref-create below and the
    // `gh_branch` column.
    writable_branch: &str,
) -> Result<WritableTarget, (StatusCode, String)> {
    let owner = req
        .repo_owner
        .as_deref()
        .map(str::trim)
        .filter(|s| !s.is_empty());
    let repo = req
        .repo_name
        .as_deref()
        .map(str::trim)
        .filter(|s| !s.is_empty());
    let base = req
        .base_branch
        .as_deref()
        .map(str::trim)
        .filter(|s| !s.is_empty());

    match (owner, repo, base) {
        // No repo coords → phase-1 default.
        (None, None, None) => Ok(WritableTarget {
            owner: CENTRAL_WRITABLE_OWNER.to_string(),
            repo: CENTRAL_WRITABLE_REPO.to_string(),
            base_branch: CENTRAL_WRITABLE_BASE_BRANCH.to_string(),
            path: Some(CENTRAL_WRITABLE_PATH.to_string()),
            auth_ref: CENTRAL_WRITABLE_AUTH_REF.to_string(),
            display_name: CENTRAL_WRITABLE_NAME.to_string(),
            minted_branch: false,
        }),
        // All three filled → user-supplied repo path. Validate.
        (Some(owner), Some(repo), Some(base_branch)) => {
            let auth_ref = validate_repo_coords(owner, repo)?;
            // `base_branch` flows unencoded into a GitHub URL and is
            // later persisted + used as a git refname; reject any
            // refname-illegal character at the boundary so neither
            // GitHub nor git ever sees a malformed ref. See
            // `valid_branch_name` for the exact rule set.
            if !valid_branch_name(base_branch) {
                return Err((
                    StatusCode::BAD_REQUEST,
                    "base_branch bevat tekens die niet zijn toegestaan in een git branch-naam"
                        .to_string(),
                ));
            }
            // Validate the optional repo sub-path at the API boundary.
            // `GitBackend::resolve` only validates the inner *relative*
            // path on each read/write; the base `gh_path` itself is
            // never re-checked downstream, so a traversal like
            // `../../etc` would let the backend read/write outside the
            // per-traject clone. Reject up-front.
            //
            // Trim + treat empty as "no subpath" → stored as SQL NULL
            // (matches how seeded sources without a subpath are stored;
            // avoids the "" vs NULL inconsistency on the row).
            let repo_path: Option<String> = req
                .repo_path
                .as_deref()
                .map(str::trim)
                .filter(|s| !s.is_empty())
                .map(str::to_string);
            if let Some(ref p) = repo_path {
                if !valid_repo_path(p) {
                    return Err((StatusCode::BAD_REQUEST, REPO_PATH_INVALID_MSG.to_string()));
                }
            }
            preflight_repo(
                state,
                account_id,
                headers,
                owner,
                repo,
                base_branch,
                writable_branch,
                &auth_ref,
                "het traject is niet aangemaakt",
            )
            .await?;

            Ok(WritableTarget {
                owner: owner.to_string(),
                repo: repo.to_string(),
                base_branch: base_branch.to_string(),
                // `repo_path` is optional — `None` means "everything
                // under repo root" (persisted as NULL), which the
                // corpus client already handles correctly. The MinBZK
                // default explicitly sets "regulation/nl"; user repos
                // dedicated to regulations typically don't need a
                // subpath. Already validated above as a clean relative
                // path.
                path: repo_path,
                auth_ref,
                display_name: format!("{owner}/{repo}"),
                minted_branch: true,
            })
        }
        // Partial → caller error, refuse rather than guess.
        _ => Err((
            StatusCode::BAD_REQUEST,
            "repo_owner, repo_name en base_branch moeten alle drie worden meegegeven \
             (of alle drie weggelaten worden voor de standaard MinBZK repo)"
                .to_string(),
        )),
    }
}

/// Validate user-supplied GitHub `owner` / `repo` coordinates and derive
/// the `auth_ref` that names their token (`CORPUS_AUTH_{AUTH_REF}_TOKEN`).
/// Shared by `create` and `move_repo` so both refuse the same input with
/// the same words.
fn validate_repo_coords(owner: &str, repo: &str) -> Result<String, (StatusCode, String)> {
    if !valid_repo_segment(owner) || !valid_repo_segment(repo) {
        return Err((
            StatusCode::BAD_REQUEST,
            "repo_owner / repo_name mogen alleen letters, cijfers, en \
             de tekens '-', '_' en '.' bevatten"
                .to_string(),
        ));
    }
    let auth_ref = derive_auth_ref(owner, repo);
    if auth_ref.is_empty() {
        return Err((
            StatusCode::BAD_REQUEST,
            format!("owner/repo \"{owner}/{repo}\" produces an empty auth_ref"),
        ));
    }
    // Reject the rare collision where a user-supplied owner/repo
    // combination derives to the same slug as the hardcoded central
    // writable auth ref. Without this check, a user who happens to point a
    // traject at `MinBZK/central` would silently get the central token
    // routed to their repo, violating the design principle that the
    // central token only ever reaches the central repo. Surface as a 400 so
    // the operator picks a different slug (or omits the repo fields
    // entirely to use the actual MinBZK default).
    // The central repo itself is reserved the same way: a traject pointed at
    // it would write there without being recognised as central, so the
    // guards that keep its layout fixed would not apply.
    let is_central_repo = owner.eq_ignore_ascii_case(CENTRAL_WRITABLE_OWNER)
        && repo.eq_ignore_ascii_case(CENTRAL_WRITABLE_REPO);
    if auth_ref == CENTRAL_WRITABLE_AUTH_REF || is_central_repo {
        return Err((
            StatusCode::BAD_REQUEST,
            format!(
                "repo \"{owner}/{repo}\" botst met de gereserveerde central auth ref; \
                 gebruik een andere repo of laat de repo-velden weg om naar de centrale \
                 MinBZK-repo te wijzen"
            ),
        ));
    }
    Ok(auth_ref)
}

/// Prove that the traject can work on `owner/repo` before anything is
/// stored: resolve the token for `auth_ref`, check push access and the base
/// branch, and make sure the traject branch exists there (minting it off
/// `base_branch` when it does not). Shared by `create` and `move_repo`.
///
/// `consequence` finishes the branch-failure message, so the caller can say
/// what did not happen ("het traject is niet aangemaakt").
#[allow(clippy::too_many_arguments)]
async fn preflight_repo(
    state: &AppState,
    account_id: Uuid,
    headers: &axum::http::HeaderMap,
    owner: &str,
    repo: &str,
    base_branch: &str,
    writable_branch: &str,
    auth_ref: &str,
    consequence: &str,
) -> Result<(), (StatusCode, String)> {
    // Resolve the token to preflight the repo with through the one
    // credential service, following the same precedence rule as the
    // write path: a configured per-repo service token goes first — the
    // eventual writes on this repo run over that token too, so
    // preflighting with the user's personal token would validate an
    // access path the traject will never use. Only for a token-less
    // ref does the acting user's OWN GitHub token come into play
    // (user-OAuth spike): the preflight then validates *their* push
    // access to the chosen repo — the entitlement check GitHub gives us
    // for free, so the editor never needs an all-access credential to
    // police repo choice (the gap that #885 tracks). The service uses a
    // strict lookup for `auth_ref` (derived from user-supplied repo
    // coords), so an unknown ref never falls back to the legacy shared
    // token, and returns 428 when a linked token is required but absent.
    let token = crate::credentials::TrajectCredentials::new(state, account_id, headers)
        .for_new_repo_preflight(auth_ref)
        .await?;

    // Build the shared GitHub client for the pre-flight. A build
    // failure is an infrastructure problem, not a caller error → 503.
    let mut client = regelrecht_github::GithubClient::new().map_err(|e| {
        tracing::error!(error = %e, "failed to build GitHub client for repo preflight");
        (
            StatusCode::SERVICE_UNAVAILABLE,
            "kon de GitHub-client niet initialiseren".to_string(),
        )
    })?;
    // The OAuth config's `api_base` (a pub field, overridable in tests
    // with a wiremock server — `for_tests` uses `api.github.invalid`)
    // wins over the client's own default when set, so the preflight —
    // including WHICH token it authenticates with — is testable without
    // touching github.com.
    if let Some(api_base) = state
        .config
        .github_oauth
        .as_ref()
        .map(|o| o.api_base.as_str())
    {
        client.set_base_url(api_base);
    }
    let info = client
        .validate_repo_access(owner, repo, base_branch, &token)
        .await
        .map_err(|e| repo_access_error_to_status(&e, owner, repo, base_branch))?;

    tracing::info!(
        owner = %owner,
        repo = %repo,
        base_branch = %base_branch,
        default_branch = %info.default_branch,
        is_private = info.is_private,
        "validated user-supplied repo for traject"
    );

    // Mint the traject branch eagerly, right here where we still
    // hold the token that just proved push access. Without this a
    // freshly created traject is dead-on-arrival: the index scan
    // reads `traject/{slug}-{short}`, which does not exist yet, so
    // the Trees API 404s and every traject-scoped corpus endpoint
    // 502s — including the very write UI you'd use to trigger the
    // lazy branch-bootstrap on the write path. A closed loop.
    //
    // In user-token write mode the backend has no service token to
    // bootstrap the branch at `ensure_ready`, so this is the only
    // moment a token is guaranteed in hand. Reuse `ensure_branch`
    // rather than calling `create_branch` directly: it already
    // absorbs the "branch already exists" race (a repeated create,
    // or two members racing), keeping this idempotent (criterion 4).
    // The lazy bootstrap on the write path stays as the safety net
    // for trajects that predate this and sit branch-less in the DB.
    //
    // `writable_branch` is the caller's single derivation, the very
    // same string the INSERT persists as `gh_branch`. Deriving it a
    // second time here would reintroduce the failure mode this fix
    // exists to close: mint branch A, store branch B, traject still
    // dead-on-arrival.
    // The bool says whether this call minted the branch; here it
    // is only of interest to `persist`, which uses it to refuse
    // blind overwrites of base content. Minting it now is exactly
    // what makes that case not arise on the write path.
    regelrecht_corpus::GitHubApiBackend::ensure_branch(
        &client,
        &format!("{owner}/{repo}"),
        writable_branch,
        Some(base_branch),
        Some(&token),
    )
    .await
    .map(|_created| ())
    .map_err(|e| {
        // Push access was just confirmed by the preflight, so a
        // failure here is genuinely exceptional (an upstream GitHub
        // hiccup or a permission revoked mid-flight). Fail loud so no
        // good-looking-but-broken traject is created — nothing has
        // been written to the DB yet at this point.
        tracing::error!(
            error = %e,
            owner = %owner,
            repo = %repo,
            branch = %writable_branch,
            "failed to create traject branch during repo preflight"
        );
        (
            StatusCode::BAD_GATEWAY,
            format!(
                "kon de traject-branch '{writable_branch}' niet aanmaken op \
                 {owner}/{repo}; {consequence}. Probeer het opnieuw of \
                 controleer je GitHub-toegang."
            ),
        )
    })?;
    Ok(())
}

/// Map a `RepoAccessError` to the appropriate HTTP status + a NL message
/// the create-sheet can show as-is. Kept verbose so each failure mode is
/// distinguishable in the UI.
fn repo_access_error_to_status(
    err: &regelrecht_github::RepoAccessError,
    owner: &str,
    repo: &str,
    base_branch: &str,
) -> (StatusCode, String) {
    use regelrecht_github::RepoAccessError as E;
    match err {
        // The *user* is fully authenticated (made it through OIDC + this
        // handler's middleware). What failed is the *operator's*
        // GitHub PAT — that is an upstream credential issue, not a
        // missing user credential. Use BAD_GATEWAY rather than 401 so
        // proxies / browsers don't try to inject WWW-Authenticate or
        // pop a credential dialog at the editor's user.
        E::Unauthorized => (
            StatusCode::BAD_GATEWAY,
            "het token van je beheerder wordt door GitHub geweigerd".to_string(),
        ),
        E::RepoNotFound => (
            StatusCode::NOT_FOUND,
            format!("repo {owner}/{repo} bestaat niet of het token kan 'm niet zien"),
        ),
        E::BranchNotFound => (
            StatusCode::NOT_FOUND,
            format!("branch '{base_branch}' bestaat niet op {owner}/{repo}"),
        ),
        E::NoPushAccess => (
            StatusCode::FORBIDDEN,
            "het geconfigureerde token heeft geen schrijftoegang tot deze repo".to_string(),
        ),
        E::Transport(msg) => {
            tracing::warn!(error = %msg, "transport error validating repo");
            (
                StatusCode::SERVICE_UNAVAILABLE,
                "kon GitHub niet bereiken om de repo te valideren".to_string(),
            )
        }
        E::Other(msg) => {
            tracing::warn!(error = %msg, "unexpected GitHub response validating repo");
            (
                StatusCode::BAD_GATEWAY,
                "onverwacht antwoord van GitHub bij repo-validatie".to_string(),
            )
        }
    }
}

/// POST /api/trajects — create a new traject.
///
/// Seeds the federated config by copying the global registry's sources
/// (with their original priorities) and then attaching the writable own
/// source at priority 0. For a user-supplied repo the traject branch is
/// minted here, during the preflight — a traject whose branch does not
/// exist yet cannot be indexed, so creating it lazily on first write left
/// every fresh traject dead-on-arrival. The lazy bootstrap on the write
/// path (`GitHubApiBackend::persist`) remains as the safety net for
/// trajects created before that, and for the MinBZK default, whose
/// service-token backend bootstraps its branch in `ensure_ready`.
///
/// When the request supplies `repo_owner`/`repo_name`/`base_branch`,
/// the writable-own source points to that user repo and the
/// pre-existing `CORPUS_AUTH_*_TOKEN` env var (named after a
/// deterministic slug of `owner-repo` — see [`derive_auth_ref`])
/// authenticates it. No request omitted → phase-1 MinBZK default.
pub async fn create(
    State(state): State<AppState>,
    Extension(account): Extension<AccountRecord>,
    headers: axum::http::HeaderMap,
    Json(req): Json<CreateTrajectRequest>,
) -> Result<(StatusCode, Json<TrajectSummary>), (StatusCode, String)> {
    let name = req.name.trim();
    if name.is_empty() {
        return Err((StatusCode::BAD_REQUEST, "name is required".to_string()));
    }

    // Mint the traject id client-side so the writable-branch name (derived
    // from it) is known *before* we touch GitHub or the database, and derive
    // that name exactly once. Both consumers — the ref-create in the
    // preflight and the `gh_branch` column on the writable-own source — read
    // this one string, so the branch that gets minted on GitHub and the
    // branch the index scan later reads cannot drift apart. One identity
    // across GitHub, URL ref and DB row.
    let traject_id = Uuid::new_v4();
    let writable_branch = derive_branch_name(name, traject_id);

    // Resolve the writable-own GitHub target up-front. Validation may need
    // to talk to GitHub (which can fail with a helpful 4xx) and — for a
    // user-supplied repo — mints the traject branch there; we do that
    // *before* opening the DB transaction so a network blip or a failed
    // branch-create leaves no half-rolled row behind.
    let target =
        resolve_writable_target(&state, &req, account.id, &headers, &writable_branch).await?;

    // Everything below is database work, and GitHub is not part of that
    // transaction. If any of it fails after the branch was minted, the ref
    // stays behind on the user's repo with no traject pointing at it — and a
    // retry derives a fresh uuid-suffixed name rather than reusing the stray.
    // Deleting someone else's ref from an error path is worse than leaving
    // it, so make it findable instead: one error log with the exact
    // coordinates. Grouping the DB work in a block puts that log on *every*
    // failure path (begin, each insert, commit), not just the first one.
    let persisted: Result<(), (StatusCode, String)> = async {
        let pool = get_pool_msg(&state)?;
        let mut tx = pool.begin().await.map_err(db_err_msg("begin tx"))?;

        sqlx::query(
            "INSERT INTO trajects (id, name, description, scope, created_by)
             VALUES ($1, $2, $3, $4, $5)",
        )
        .bind(traject_id)
        .bind(name)
        .bind(&req.description)
        .bind(&req.scope)
        .bind(account.id)
        .execute(&mut *tx)
        .await
        .map_err(db_err_msg("insert traject"))?;

        sqlx::query(
            "INSERT INTO traject_members (traject_id, account_id, role)
             VALUES ($1, $2, 'owner')",
        )
        .bind(traject_id)
        .bind(account.id)
        .execute(&mut *tx)
        .await
        .map_err(db_err_msg("insert member"))?;

        // Seed federated read-config from the global registry. The global
        // corpus read guard is dropped before the next await so we don't hold
        // it across the database transaction.
        let seeded: Vec<SeedSource> = {
            let corpus = state.corpus.read().await;
            corpus
                .registry
                .sources()
                .iter()
                // Example rulework stays in the public library, where it is
                // labelled as an example (see `Source::library_only`).
                .filter(|s| !s.library_only)
                .map(SeedSource::from_source)
                .collect()
        };

        for seed in seeded {
            sqlx::query(
                "INSERT INTO traject_corpus_sources
                 (traject_id, source_id, name, source_type,
                  gh_owner, gh_repo, gh_branch, gh_path, gh_ref,
                  local_path, priority, auth_ref, scopes, is_writable_own)
                 VALUES ($1, $2, $3, $4::corpus_source_type,
                         $5, $6, $7, $8, $9,
                         $10, $11, $12, $13, FALSE)",
            )
            .bind(traject_id)
            .bind(&seed.source_id)
            .bind(&seed.name)
            .bind(&seed.source_type)
            .bind(seed.gh_owner)
            .bind(seed.gh_repo)
            .bind(seed.gh_branch)
            .bind(seed.gh_path)
            .bind(seed.gh_ref)
            .bind(seed.local_path)
            .bind(seed.priority as i32)
            .bind(seed.auth_ref)
            .bind(seed.scopes)
            .execute(&mut *tx)
            .await
            .map_err(db_err_msg("seed traject source"))?;
        }

        // Writable-own source: the phase-1 MinBZK default or the user-supplied
        // repo (already validated up-front for push access). `writable_branch` is
        // the name derived at the top of this handler — for a user-supplied repo
        // that branch now exists on GitHub, minted during the preflight. Auth
        // flows through `CORPUS_AUTH_{AUTH_REF_UPPER}_TOKEN` via the `auth_ref`
        // stored on this row.
        let writable_source_id = format!("traject-own-{}", traject_id.simple());
        sqlx::query(
            "INSERT INTO traject_corpus_sources
             (traject_id, source_id, name, source_type,
              gh_owner, gh_repo, gh_branch, gh_base_branch, gh_path,
              priority, auth_ref, is_writable_own)
             VALUES ($1, $2, $3, 'github',
                     $4, $5, $6, $7, $8,
                     0, $9, TRUE)",
        )
        .bind(traject_id)
        .bind(&writable_source_id)
        .bind(&target.display_name)
        .bind(&target.owner)
        .bind(&target.repo)
        .bind(&writable_branch)
        .bind(&target.base_branch)
        .bind(&target.path)
        .bind(&target.auth_ref)
        .execute(&mut *tx)
        .await
        .map_err(db_err_msg("insert writable source"))?;

        tx.commit()
            .await
            .map_err(db_err_msg("commit traject create"))?;
        Ok(())
    }
    .await;
    if let Err(e) = persisted {
        if target.minted_branch {
            tracing::error!(
                traject_id = %traject_id,
                owner = %target.owner,
                repo = %target.repo,
                branch = %writable_branch,
                "orphaned traject branch: it was created on GitHub but persisting \
                 the traject failed, so nothing references it; remove it by hand"
            );
        }
        return Err(e);
    }

    state.trajects.invalidate(traject_id).await;

    let mut summary = TrajectSummary {
        id: traject_id,
        name: name.to_string(),
        description: req.description,
        scope: req.scope,
        status: "bezig".to_string(),
        role: "owner".to_string(),
        traject_ref: None,
    };
    summary.fill_ref();
    Ok((StatusCode::CREATED, Json(summary)))
}

/// Lift a bare-status error into the `(StatusCode, String)` shape used
/// by handlers that also return user-facing messages. An empty body is
/// the right answer for these: 403/500/503 carry nothing the caller can
/// act on beyond the status itself.
fn bare(status: StatusCode) -> (StatusCode, String) {
    (status, String::new())
}

/// PATCH /api/trajects/:id — owner-only update of metadata fields and of
/// the root path (`gh_path`) of the traject's own corpus source.
///
/// Returns `(StatusCode, String)` on failure so the path-validation and
/// central-corpus rejections can explain themselves in the UI; the other
/// failures keep an empty body (see [`bare`]).
pub async fn update(
    State(state): State<AppState>,
    Extension(account): Extension<AccountRecord>,
    Path(id): Path<Uuid>,
    Json(req): Json<UpdateTrajectRequest>,
) -> Result<StatusCode, (StatusCode, String)> {
    let pool = get_pool_msg(&state)?;
    require_owner(pool, id, account.id).await.map_err(bare)?;
    if let Some(ref s) = req.status {
        validate_status(s).map_err(bare)?;
    }
    // Mirror `create`'s non-empty check so a PATCH can't blank-out the
    // name with whitespace.
    if let Some(ref n) = req.name {
        if n.trim().is_empty() {
            return Err(bare(StatusCode::BAD_REQUEST));
        }
    }

    // `Option<Option<String>>`: the outer layer is "was the field sent
    // at all" (absent = leave `gh_path` alone), the inner one is the
    // normalised value — `None` for the repo root, stored as SQL NULL
    // exactly like `create` does, so the row never carries the "" vs
    // NULL ambiguity.
    let repo_path: Option<Option<String>> = req.repo_path.as_deref().map(|raw| {
        let trimmed = raw.trim();
        (!trimmed.is_empty()).then(|| trimmed.to_string())
    });
    if let Some(Some(ref path)) = repo_path {
        // Same boundary guard as `create`: the stored `gh_path` is the
        // base every read/write on the own source is resolved against
        // and is never re-validated downstream, so a traversal here
        // would let the backend read/write outside the traject's repo
        // subtree.
        if !valid_repo_path(path) {
            return Err((StatusCode::BAD_REQUEST, REPO_PATH_INVALID_MSG.to_string()));
        }
    }

    let mut tx = pool
        .begin()
        .await
        .map_err(db_err_msg("begin update traject tx"))?;

    if let Some(ref new_path) = repo_path {
        update_own_repo_path(&mut tx, id, new_path.as_deref()).await?;
    }

    sqlx::query(
        "UPDATE trajects SET
            name        = COALESCE($2, name),
            description = COALESCE($3, description),
            scope       = COALESCE($4, scope),
            status      = COALESCE($5::traject_status, status)
         WHERE id = $1",
    )
    .bind(id)
    .bind(req.name.as_deref().map(str::trim))
    .bind(req.description.as_deref())
    .bind(req.scope.as_deref())
    .bind(req.status.as_deref())
    .execute(&mut *tx)
    .await
    .map_err(db_err_msg("update traject"))?;

    tx.commit()
        .await
        .map_err(db_err_msg("commit update traject tx"))?;

    if let Some(ref new_path) = repo_path {
        // Worth a line in the log: this one field decides what the whole
        // traject sees, so "the library suddenly shows nothing" is a
        // question whose answer is this event and its timestamp.
        tracing::info!(
            traject = %id,
            repo_path = new_path.as_deref().unwrap_or("<repo root>"),
            "traject root path changed"
        );
        // The cached `TrajectCorpus` pins the old `gh_path` in its
        // backends, so without this the sidebar would keep listing the
        // laws under the previous root for a whole TTL window. Dropping
        // the cell makes the next read rebuild against the new path.
        state.trajects.invalidate(id).await;
    }

    Ok(StatusCode::NO_CONTENT)
}

/// Write the new root path onto the traject's writable-own source row.
///
/// Refuses the two cases where there is no path of ours to move:
/// a traject that writes to the central MinBZK corpus (its layout is
/// fixed for every traject that shares it), and a writable-own source
/// that is not a GitHub repo (a local source has no `gh_path`).
async fn update_own_repo_path(
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    traject_id: Uuid,
    new_path: Option<&str>,
) -> Result<(), (StatusCode, String)> {
    let row: Option<(String, Option<String>)> = sqlx::query_as(
        "SELECT source_type::text, auth_ref
         FROM traject_corpus_sources
         WHERE traject_id = $1 AND is_writable_own
         FOR UPDATE",
    )
    .bind(traject_id)
    .fetch_optional(&mut **tx)
    .await
    .map_err(db_err_msg("writable-own source lookup"))?;

    let (source_type, auth_ref) = row.ok_or((
        StatusCode::BAD_REQUEST,
        "dit traject heeft geen eigen bron waarvan het pad te wijzigen is".to_string(),
    ))?;
    refuse_unmovable_own_source(&source_type, auth_ref.as_deref(), "pad")?;

    sqlx::query(
        "UPDATE traject_corpus_sources SET gh_path = $2
         WHERE traject_id = $1 AND is_writable_own",
    )
    .bind(traject_id)
    .bind(new_path)
    .execute(&mut **tx)
    .await
    .map_err(db_err_msg("update traject repo path"))?;

    Ok(())
}

/// The writable-own source row as `move_repo` needs it.
#[derive(sqlx::FromRow)]
struct OwnGithubSource {
    source_type: String,
    auth_ref: Option<String>,
    gh_owner: Option<String>,
    gh_repo: Option<String>,
    gh_branch: Option<String>,
    gh_base_branch: Option<String>,
}

/// PUT /api/trajects/:id/repo — point the traject's own source at another
/// GitHub repository, typically after the repository moved to a new owner.
///
/// Only an `editor-admin` may do this, also without being a member: which
/// repo a traject writes to decides where the platform's tokens go, so it
/// is not a traject owner's call. The new repo
/// gets the same preflight as `create`: its token (named after the new
/// `auth_ref`) must be configured, must have push access, the base branch
/// must exist, and the traject branch is minted there when it is missing.
/// Only then are `gh_owner`, `gh_repo`, `auth_ref` and the display name
/// rewritten; branch, base branch and path stay as they are.
pub async fn move_repo(
    State(state): State<AppState>,
    Extension(account): Extension<AccountRecord>,
    session: Session,
    headers: axum::http::HeaderMap,
    Path(id): Path<Uuid>,
    Json(req): Json<MoveRepoRequest>,
) -> Result<StatusCode, (StatusCode, String)> {
    let pool = get_pool_msg(&state)?;
    require_admin(pool, &session, id).await.map_err(bare)?;

    let owner = req.repo_owner.trim();
    let repo = req.repo_name.trim();
    let auth_ref = validate_repo_coords(owner, repo)?;

    let current: OwnGithubSource = sqlx::query_as(
        "SELECT source_type::text AS source_type, auth_ref, gh_owner, gh_repo,
                gh_branch, gh_base_branch
         FROM traject_corpus_sources
         WHERE traject_id = $1 AND is_writable_own",
    )
    .bind(id)
    .fetch_optional(pool)
    .await
    .map_err(db_err_msg("writable-own source lookup"))?
    .ok_or((
        StatusCode::BAD_REQUEST,
        "dit traject heeft geen eigen bron waarvan de repo te wijzigen is".to_string(),
    ))?;
    refuse_unmovable_own_source(&current.source_type, current.auth_ref.as_deref(), "repo")?;

    if current.gh_owner.as_deref() == Some(owner) && current.gh_repo.as_deref() == Some(repo) {
        return Ok(StatusCode::NO_CONTENT);
    }
    // The CHECK on the table guarantees a branch on every GitHub row.
    let branch = current
        .gh_branch
        .clone()
        .ok_or_else(|| bare(StatusCode::INTERNAL_SERVER_ERROR))?;
    // Same fallback the backend applies when it bootstraps the branch.
    let base_branch = current
        .gh_base_branch
        .clone()
        .unwrap_or_else(|| "main".to_string());

    preflight_repo(
        &state,
        account.id,
        &headers,
        owner,
        repo,
        &base_branch,
        &branch,
        &auth_ref,
        "de repo van het traject is niet gewijzigd",
    )
    .await?;

    // Conditional on the row still being what the preflight was run for:
    // the GitHub calls above happen outside any transaction, so a concurrent
    // move or delete would otherwise be overwritten or reported as done.
    let updated = sqlx::query(
        "UPDATE traject_corpus_sources
         SET gh_owner = $2, gh_repo = $3, auth_ref = $4, name = $5
         WHERE traject_id = $1 AND is_writable_own
           AND gh_owner IS NOT DISTINCT FROM $6
           AND gh_repo IS NOT DISTINCT FROM $7
           AND auth_ref IS NOT DISTINCT FROM $8",
    )
    .bind(id)
    .bind(owner)
    .bind(repo)
    .bind(&auth_ref)
    .bind(format!("{owner}/{repo}"))
    .bind(current.gh_owner.as_deref())
    .bind(current.gh_repo.as_deref())
    .bind(current.auth_ref.as_deref())
    .execute(pool)
    .await
    .map_err(db_err_msg("update traject repo"))?;
    if updated.rows_affected() == 0 {
        return Err((
            StatusCode::CONFLICT,
            "het traject is intussen gewijzigd of verwijderd; laad het opnieuw en \
             probeer het nog eens"
                .to_string(),
        ));
    }

    tracing::info!(
        traject = %id,
        by = %account.id,
        from = %format!(
            "{}/{}",
            current.gh_owner.as_deref().unwrap_or_default(),
            current.gh_repo.as_deref().unwrap_or_default()
        ),
        to = %format!("{owner}/{repo}"),
        "traject repo changed"
    );
    // The cached `TrajectCorpus` holds backends on the old repo.
    state.trajects.invalidate(id).await;

    Ok(StatusCode::NO_CONTENT)
}

/// Refuse to move the own source of a traject that has none of its own to
/// move: one that writes to the central MinBZK corpus (its repo and layout
/// are shared by every traject on it), or one whose own source is not a
/// GitHub repo. `what` names what the caller tried to change ("pad",
/// "repo") so the message says it.
fn refuse_unmovable_own_source(
    source_type: &str,
    auth_ref: Option<&str>,
    what: &str,
) -> Result<(), (StatusCode, String)> {
    // `auth_ref` is the marker, not owner/repo: it decides which token
    // the writes run over, and `create` rejects any user-supplied repo
    // that derives to the central ref — so this value appears on
    // exactly the trajects that write to the central corpus.
    if auth_ref == Some(CENTRAL_WRITABLE_AUTH_REF) {
        return Err((
            StatusCode::BAD_REQUEST,
            format!(
                "dit traject schrijft naar het centrale corpus ({CENTRAL_WRITABLE_NAME}); \
                 daar liggen repo en pad vast ('{CENTRAL_WRITABLE_PATH}'). Maak een traject \
                 op een eigen repo om een ander {what} te gebruiken."
            ),
        ));
    }
    if source_type != "github" {
        return Err((
            StatusCode::BAD_REQUEST,
            format!(
                "de eigen bron van dit traject is geen GitHub-repo, dus er is geen {what} te \
                 wijzigen"
            ),
        ));
    }
    Ok(())
}

/// DELETE /api/trajects/:id — owner-only hard delete.
///
/// FK cascades on `traject_members` and `traject_corpus_sources` clean
/// up the dependent rows. The cached `TrajectCorpus` is invalidated so
/// any in-flight reads rebuild against an empty source set and surface
/// `NotFound`. The upstream branch on the writable source is **not**
/// touched — that's a manual cleanup decision and there's no way to
/// know whether the user still wants the in-flight edits preserved
/// elsewhere.
///
/// Open review tasks are a different story: `tasks.traject_id` is
/// `ON DELETE SET NULL`, so the delete would only cut the link and leave
/// an unresolvable task behind — it keeps showing up in the account-wide
/// task list and its `payload.traject_ref` points at a traject that no
/// longer exists, so "review" lands on the traject picker. They are
/// therefore dismissed in the same transaction as the delete (before it,
/// since afterwards there is no way to tell which tasks belonged here),
/// and the result blobs of the jobs involved are cleaned up along the
/// same "only when no open task is left on that job" path that `resolve`
/// uses.
pub async fn delete(
    State(state): State<AppState>,
    Extension(account): Extension<AccountRecord>,
    Path(id): Path<Uuid>,
) -> Result<StatusCode, StatusCode> {
    let pool = get_pool(&state)?;
    require_owner(pool, id, account.id).await?;

    let mut tx = pool
        .begin()
        .await
        .map_err(db_err("begin delete traject tx"))?;

    let job_ids = tasks::dismiss_open_tasks_for_traject(&mut *tx, id)
        .await
        .map_err(db_err("dismiss open tasks of deleted traject"))?;

    let affected = sqlx::query("DELETE FROM trajects WHERE id = $1")
        .bind(id)
        .execute(&mut *tx)
        .await
        .map_err(db_err("delete traject"))?
        .rows_affected();

    if affected == 0 {
        // Dropping the tx rolls the dismissals back with it — nothing was
        // deleted, so nothing should have been closed either.
        return Err(StatusCode::NOT_FOUND);
    }

    for job_id in job_ids {
        tasks::delete_blobs_for_finished_job(&mut *tx, job_id)
            .await
            .map_err(db_err("cleanup job blobs of deleted traject"))?;
    }

    tx.commit()
        .await
        .map_err(db_err("commit delete traject tx"))?;

    state.trajects.invalidate(id).await;
    Ok(StatusCode::NO_CONTENT)
}

/// POST /api/trajects/:id/members — invite by email.
///
/// If the email already maps to an account, a `traject_members` row is
/// created (or updated, idempotently) and the response status is
/// `"active"`. If not, a `traject_invites` row is created and the
/// response status is `"pending"`; that row is promoted to a real
/// membership the next time someone with the matching email claim hits
/// any authenticated endpoint (see `accounts::ensure_account`).
pub async fn add_member(
    State(state): State<AppState>,
    Extension(account): Extension<AccountRecord>,
    Path(id): Path<Uuid>,
    Json(req): Json<AddMemberRequest>,
) -> Result<Json<AddMemberResponse>, StatusCode> {
    let pool = get_pool(&state)?;
    require_owner(pool, id, account.id).await?;
    validate_role(&req.role)?;
    let email = normalize_email(&req.email).ok_or(StatusCode::BAD_REQUEST)?;

    // `accounts.email` keeps the IdP-supplied casing, so the lookup
    // lowercases on the DB side to match our normalised key. The
    // functional unique index `idx_accounts_email_lower` (migration
    // 0016) makes this index-only and case-insensitively unique, so
    // `fetch_optional` is sound by construction.
    let target: Option<(Uuid,)> = sqlx::query_as("SELECT id FROM accounts WHERE lower(email) = $1")
        .bind(&email)
        .fetch_optional(pool)
        .await
        .map_err(db_err("lookup account"))?;

    if let Some((target_id,)) = target {
        // Known account → write traject_members. Guard the "would demote
        // the last owner" case atomically in the same statement so it
        // can't race with a concurrent leave/remove.
        //
        // The WHERE evaluates to true when any of:
        //   * the new role is `owner` (promotion/no-op, never reduces count)
        //   * the target isn't currently an owner (no demotion happening)
        //   * another owner exists for this traject (demotion is safe)
        // When all three are false (target is the sole owner being
        // demoted to contributor) the SELECT returns no row, the INSERT
        // runs zero times, no conflict triggers, and `rows_affected` is
        // 0 → CONFLICT.
        //
        // Transaction: the INSERT and the stale-invite cleanup must
        // commit together. If the cleanup failed mid-flight, GET would
        // briefly return the user in both `members` and `pending_invites`
        // until `promote_pending_invites` next ran.
        let mut tx = pool.begin().await.map_err(db_err("begin add_member tx"))?;
        let affected = sqlx::query(
            "INSERT INTO traject_members (traject_id, account_id, role)
             SELECT $1, $2, $3::traject_role
             WHERE
                 $3::traject_role = 'owner'
                 OR NOT EXISTS (
                     SELECT 1 FROM traject_members tm
                     WHERE tm.traject_id = $1
                       AND tm.account_id = $2
                       AND tm.role = 'owner'
                 )
                 OR EXISTS (
                     SELECT 1 FROM traject_members tm2
                     WHERE tm2.traject_id = $1
                       AND tm2.role = 'owner'
                       AND tm2.account_id <> $2
                 )
             ON CONFLICT (traject_id, account_id) DO UPDATE
                 SET role = EXCLUDED.role",
        )
        .bind(id)
        .bind(target_id)
        .bind(&req.role)
        .execute(&mut *tx)
        .await
        .map_err(db_err("add member"))?
        .rows_affected();

        if affected == 0 {
            return Err(StatusCode::CONFLICT);
        }
        sqlx::query("DELETE FROM traject_invites WHERE traject_id = $1 AND email = $2")
            .bind(id)
            .bind(&email)
            .execute(&mut *tx)
            .await
            .map_err(db_err("clean up stale invite"))?;
        tx.commit().await.map_err(db_err("commit add_member tx"))?;
        return Ok(Json(AddMemberResponse {
            status: "active",
            email,
        }));
    }

    // Unknown account → park the invite. Re-inviting the same email
    // with a different role just updates the role (last write wins).
    sqlx::query(
        "INSERT INTO traject_invites (traject_id, email, role, invited_by)
         VALUES ($1, $2, $3::traject_role, $4)
         ON CONFLICT (traject_id, email) DO UPDATE
             SET role = EXCLUDED.role,
                 invited_by = EXCLUDED.invited_by,
                 invited_at = now()",
    )
    .bind(id)
    .bind(&email)
    .bind(&req.role)
    .bind(account.id)
    .execute(pool)
    .await
    .map_err(db_err("add invite"))?;

    Ok(Json(AddMemberResponse {
        status: "pending",
        email,
    }))
}

/// DELETE /api/trajects/:id/invites/:email — owner-only removal of a
/// pending invite. Returns 404 when no invite exists for the (traject,
/// email) pair, so the operation is idempotent against repeated
/// cancellations.
pub async fn remove_invite(
    State(state): State<AppState>,
    Extension(account): Extension<AccountRecord>,
    Path((id, email)): Path<(Uuid, String)>,
) -> Result<StatusCode, StatusCode> {
    let pool = get_pool(&state)?;
    require_owner(pool, id, account.id).await?;
    let email = normalize_email(&email).ok_or(StatusCode::BAD_REQUEST)?;

    let affected = sqlx::query("DELETE FROM traject_invites WHERE traject_id = $1 AND email = $2")
        .bind(id)
        .bind(&email)
        .execute(pool)
        .await
        .map_err(db_err("remove invite"))?
        .rows_affected();

    if affected == 0 {
        return Err(StatusCode::NOT_FOUND);
    }
    Ok(StatusCode::NO_CONTENT)
}

/// PATCH /api/trajects/:id/members/:account_id — change a member's role.
pub async fn update_member(
    State(state): State<AppState>,
    Extension(account): Extension<AccountRecord>,
    Path((id, account_id)): Path<(Uuid, Uuid)>,
    Json(req): Json<UpdateMemberRequest>,
) -> Result<StatusCode, StatusCode> {
    let pool = get_pool(&state)?;
    require_owner(pool, id, account.id).await?;
    validate_role(&req.role)?;

    // Atomic guard against demoting the last owner. The UPDATE only
    // fires when at least one of these holds:
    //   * the new role is `owner` (no demote)
    //   * the row's current role isn't `owner` (no demote)
    //   * another owner exists for this traject (demote is safe)
    // Otherwise the row stays untouched, `rows_affected` is 0, and we
    // disambiguate "row missing" (NOT_FOUND) from "guard blocked"
    // (CONFLICT) via a follow-up read on the cold path.
    let affected = sqlx::query(
        "UPDATE traject_members SET role = $3::traject_role
         WHERE traject_id = $1 AND account_id = $2
           AND (
               $3::traject_role = 'owner'
               OR role <> 'owner'
               OR EXISTS (
                   SELECT 1 FROM traject_members tm2
                   WHERE tm2.traject_id = $1
                     AND tm2.role = 'owner'
                     AND tm2.account_id <> $2
               )
           )",
    )
    .bind(id)
    .bind(account_id)
    .bind(&req.role)
    .execute(pool)
    .await
    .map_err(db_err("update member"))?
    .rows_affected();

    if affected == 0 {
        return distinguish_member_missing_or_conflict(pool, id, account_id).await;
    }
    Ok(StatusCode::NO_CONTENT)
}

/// DELETE /api/trajects/:id/members/:account_id — remove a member.
pub async fn remove_member(
    State(state): State<AppState>,
    Extension(account): Extension<AccountRecord>,
    Path((id, account_id)): Path<(Uuid, Uuid)>,
) -> Result<StatusCode, StatusCode> {
    let pool = get_pool(&state)?;
    require_owner(pool, id, account.id).await?;

    // Atomic guard: DELETE only succeeds when the target isn't the sole
    // owner. The EXISTS condition is part of the same statement so
    // two concurrent removes can't both pass the check and then both
    // commit a delete.
    let affected = sqlx::query(
        "DELETE FROM traject_members
         WHERE traject_id = $1 AND account_id = $2
           AND (
               role <> 'owner'
               OR EXISTS (
                   SELECT 1 FROM traject_members tm2
                   WHERE tm2.traject_id = $1
                     AND tm2.role = 'owner'
                     AND tm2.account_id <> $2
               )
           )",
    )
    .bind(id)
    .bind(account_id)
    .execute(pool)
    .await
    .map_err(db_err("remove member"))?
    .rows_affected();

    if affected == 0 {
        return distinguish_member_missing_or_conflict(pool, id, account_id).await;
    }
    Ok(StatusCode::NO_CONTENT)
}

/// POST /api/trajects/:id/leave — caller removes themselves from the
/// traject.
///
/// A `contributor` can always leave. An `owner` cannot leave when they are
/// the last owner — they must hand over the role or delete the
/// traject. The next write request the caller makes against this
/// traject's URL will 403 on the membership re-check in
/// `require_traject_corpus_from_ref`.
pub async fn leave(
    State(state): State<AppState>,
    Extension(account): Extension<AccountRecord>,
    Path(id): Path<Uuid>,
) -> Result<StatusCode, StatusCode> {
    let pool = get_pool(&state)?;
    require_membership(pool, id, account.id).await?;

    // Atomic guard, same shape as remove_member: the DELETE only runs
    // when the caller is a contributor or another owner remains. Two
    // owners calling `leave` concurrently can't both pass a separate
    // count then both delete — the second one's DELETE re-evaluates the
    // EXISTS under Postgres' READ COMMITTED snapshot semantics and ends
    // up matching zero rows.
    let affected = sqlx::query(
        "DELETE FROM traject_members
         WHERE traject_id = $1 AND account_id = $2
           AND (
               role <> 'owner'
               OR EXISTS (
                   SELECT 1 FROM traject_members tm2
                   WHERE tm2.traject_id = $1
                     AND tm2.role = 'owner'
                     AND tm2.account_id <> $2
               )
           )",
    )
    .bind(id)
    .bind(account.id)
    .execute(pool)
    .await
    .map_err(db_err("leave traject"))?
    .rows_affected();

    if affected == 0 {
        // require_membership passed above, so the most likely cause is
        // the guard. The narrow window where the row was concurrently
        // removed between the membership check and the DELETE would
        // also yield 0 — distinguish with one more read.
        return distinguish_member_missing_or_conflict(pool, id, account.id).await;
    }

    Ok(StatusCode::NO_CONTENT)
}

/// Disambiguate "row not found" (404) from "guard blocked the write"
/// (409) after a DELETE/UPDATE on `traject_members` came back with zero
/// rows affected. The atomic guards embedded in the write statements
/// can't tell these apart by themselves; this lookup runs on the cold
/// path only.
async fn distinguish_member_missing_or_conflict(
    pool: &PgPool,
    traject_id: Uuid,
    account_id: Uuid,
) -> Result<StatusCode, StatusCode> {
    if member_role(pool, traject_id, account_id)
        .await
        .map_err(db_err("post-write membership lookup"))?
        .is_none()
    {
        Err(StatusCode::NOT_FOUND)
    } else {
        Err(StatusCode::CONFLICT)
    }
}

/// Flattened snapshot of a [`regelrecht_corpus::models::Source`] used to
/// seed `traject_corpus_sources` rows without holding the corpus read
/// guard across the database transaction.
struct SeedSource {
    source_id: String,
    name: String,
    source_type: String,
    gh_owner: Option<String>,
    gh_repo: Option<String>,
    gh_branch: Option<String>,
    gh_path: Option<String>,
    gh_ref: Option<String>,
    local_path: Option<String>,
    priority: u32,
    auth_ref: Option<String>,
    scopes: serde_json::Value,
}

impl SeedSource {
    fn from_source(s: &regelrecht_corpus::models::Source) -> Self {
        let (source_type, gh_owner, gh_repo, gh_branch, gh_path, gh_ref, local_path) =
            match &s.source_type {
                regelrecht_corpus::models::SourceType::GitHub { github } => (
                    "github".to_string(),
                    Some(github.owner.clone()),
                    Some(github.repo.clone()),
                    Some(github.branch.clone()),
                    github.path.clone(),
                    github.git_ref.clone(),
                    None,
                ),
                regelrecht_corpus::models::SourceType::Local { local } => (
                    "local".to_string(),
                    None,
                    None,
                    None,
                    None,
                    None,
                    Some(local.path.to_string_lossy().to_string()),
                ),
            };
        let scopes = serde_json::to_value(&s.scopes).unwrap_or(serde_json::json!([]));
        SeedSource {
            source_id: s.id.clone(),
            name: s.name.clone(),
            source_type,
            gh_owner,
            gh_repo,
            gh_branch,
            gh_path,
            gh_ref,
            local_path,
            priority: s.priority,
            auth_ref: s.auth_ref.clone(),
            scopes,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn slugify_strips_specials() {
        assert_eq!(slugify("Hello World!"), "hello-world");
        assert_eq!(slugify("---hello---"), "hello");
        assert_eq!(slugify("Wet op de Zorgtoeslag"), "wet-op-de-zorgtoeslag");
    }

    #[test]
    fn slugify_caps_length() {
        let s = slugify(&"a".repeat(100));
        assert!(s.len() <= 32);
    }

    #[test]
    fn slugify_empty_falls_back() {
        assert_eq!(slugify(""), "traject");
        assert_eq!(slugify("---"), "traject");
    }

    #[test]
    fn branch_name_format() {
        let id = Uuid::nil();
        let branch = derive_branch_name("Tarief", id);
        assert!(branch.starts_with("traject/tarief-"));
        assert_eq!(branch.len(), "traject/tarief-".len() + 8);
    }

    #[test]
    fn auth_ref_lowercases_and_collapses_separators() {
        // Owner + repo combined with `/`, all separators collapse to a
        // single dash. Operators read this back as the env-var stem
        // `CORPUS_AUTH_<UPPER>_TOKEN`.
        assert_eq!(derive_auth_ref("Acme", "Secret-Repo"), "acme-secret-repo");
        assert_eq!(
            derive_auth_ref("MinBZK", "regelrecht-corpus"),
            "minbzk-regelrecht-corpus"
        );
        assert_eq!(derive_auth_ref("a.b", "c_d"), "a-b-c-d");
    }

    #[test]
    fn auth_ref_stable_under_already_normalised_input() {
        // An already-normalised owner/repo (lowercase + dash-only) must
        // come out byte-identical — operators read this back from the
        // DB row to know the matching env-var name; any drift between
        // create-time and read-time would mean "broken token resolution
        // after a rename".
        assert_eq!(derive_auth_ref("acme", "secret-repo"), "acme-secret-repo");
        assert_eq!(
            derive_auth_ref("minbzk", "regelrecht-corpus"),
            "minbzk-regelrecht-corpus"
        );
    }

    #[test]
    fn auth_ref_trims_leading_and_trailing_dashes() {
        // Garbage in/out: only-special-chars resolves to "" rather than
        // a string of pure dashes. The caller should reject this before
        // it reaches the INSERT.
        assert_eq!(derive_auth_ref("...", "..."), "");
        assert_eq!(derive_auth_ref(".foo.", ".bar."), "foo-bar");
    }

    #[test]
    fn valid_repo_path_accepts_empty_and_simple_subdirs() {
        assert!(valid_repo_path(""));
        assert!(valid_repo_path("regulation/nl"));
        assert!(valid_repo_path("a"));
        assert!(valid_repo_path("a/b/c"));
        assert!(valid_repo_path("dir.with.dots/and_underscores"));
    }

    #[test]
    fn valid_repo_path_rejects_traversal_and_absolute() {
        // The whole point of this check: the base gh_path is never
        // re-validated by GitBackend::resolve, so any traversal or
        // absolute path that lands in the DB row will be honoured by
        // the backend on subsequent reads/writes. Reject at the API
        // boundary.
        assert!(!valid_repo_path("../etc"));
        assert!(!valid_repo_path("/etc"));
        assert!(!valid_repo_path("regulation/../../etc"));
        assert!(!valid_repo_path("./regulation"));
        assert!(!valid_repo_path(".."));
        assert!(!valid_repo_path("regulation/with space"));
        assert!(!valid_repo_path("regulation/with!bang"));
    }

    #[test]
    fn valid_repo_segment_rejects_leading_dot_and_dots_only() {
        // GitHub disallows segments that are literal `.` or `..` and
        // names starting with a leading `.`. The previous validator
        // permitted these because they're inside the `[A-Za-z0-9._-]`
        // character class; this test pins the explicit rejection.
        assert!(!valid_repo_segment("."));
        assert!(!valid_repo_segment(".."));
        assert!(!valid_repo_segment(".hidden"));
        assert!(!valid_repo_segment(".github"));
        // Dots in the middle of a segment remain valid (e.g. version
        // suffixes like `repo.v2`).
        assert!(valid_repo_segment("a.b"));
        assert!(valid_repo_segment("regelrecht-corpus"));
    }

    #[test]
    fn valid_branch_name_accepts_common_branches() {
        // Real branch names that flow through `resolve_writable_target`
        // every day: must not be rejected by the boundary guard.
        assert!(valid_branch_name("main"));
        assert!(valid_branch_name("develop"));
        assert!(valid_branch_name("feature/foo"));
        assert!(valid_branch_name("release-1.0"));
        assert!(valid_branch_name("user/feature/sub-branch"));
    }

    #[test]
    fn valid_branch_name_rejects_url_injection_and_refname_illegals() {
        // `?` would let `main?spoof=1` pass the trim+non-empty check
        // because GitHub strips the query before validating `main`;
        // refuse it at the boundary so the persisted refname is clean.
        assert!(!valid_branch_name("main?spoof=1"));
        // Trailing/embedded whitespace and control chars.
        assert!(!valid_branch_name("main "));
        assert!(!valid_branch_name("ma in"));
        assert!(!valid_branch_name("main\n"));
        assert!(!valid_branch_name("main\t"));
        // Refname-illegal characters from `git-check-ref-format`.
        assert!(!valid_branch_name("feat~1"));
        assert!(!valid_branch_name("feat^2"));
        assert!(!valid_branch_name("a:b"));
        assert!(!valid_branch_name("a*b"));
        assert!(!valid_branch_name("a[b"));
        assert!(!valid_branch_name("a\\b"));
        assert!(!valid_branch_name("feat@{1}"));
        // `#` is legal in git refnames but is the URL fragment delimiter,
        // so `main#spoof` would 200-pass the pre-flight (matches `main`)
        // and produce a non-existent refname downstream. Refuse it.
        assert!(!valid_branch_name("main#spoof"));
        assert!(!valid_branch_name("#"));
        // `..` (dot-dot), leading `-`, leading/trailing `/`, double `/`.
        assert!(!valid_branch_name(".."));
        assert!(!valid_branch_name("a..b"));
        assert!(!valid_branch_name("-x"));
        assert!(!valid_branch_name("/x"));
        assert!(!valid_branch_name("x/"));
        assert!(!valid_branch_name("feat//bar"));
        // Empty + oversized.
        assert!(!valid_branch_name(""));
        let oversized: String = "a".repeat(201);
        assert!(!valid_branch_name(&oversized));
    }

    #[test]
    fn valid_branch_name_rejects_git_specific_corner_cases() {
        // git-check-ref-format rejects refnames that end with `.`, the
        // bare `@`, or any component ending in `.lock`. Mirror those
        // here so the boundary check matches the refname grammar.
        assert!(!valid_branch_name("main."));
        assert!(!valid_branch_name("@"));
        assert!(!valid_branch_name("main.lock"));
        assert!(!valid_branch_name("feat/foo.lock"));
        // No path component may start with `.`. GitHub would accept
        // these refs but git would refuse them downstream at push time.
        assert!(!valid_branch_name(".hidden"));
        assert!(!valid_branch_name("feature/.hidden"));
        assert!(!valid_branch_name(".github"));
        // Sanity: dots / "lock" inside a name (not as a trailing
        // component / suffix) stay accepted.
        assert!(valid_branch_name("main.foo"));
        assert!(valid_branch_name("locked-down"));
    }

    #[test]
    fn minbzk_central_collides_with_reserved_auth_ref() {
        // Documents the collision the `resolve_writable_target` guard
        // exists to prevent: a user-supplied owner=MinBZK, repo=central
        // derives byte-for-byte to the hardcoded CENTRAL_WRITABLE_AUTH_REF.
        // Without the guard, that traject would silently get the central
        // token routed to a user-chosen repo. The guard rejects the
        // request before INSERT; this test pins the precondition so a
        // future rename of either constant or `derive_auth_ref` can't
        // silently break the guard's premise.
        assert_eq!(
            derive_auth_ref("MinBZK", "central"),
            CENTRAL_WRITABLE_AUTH_REF
        );
        // Sanity check: an unrelated repo doesn't collide.
        assert_ne!(
            derive_auth_ref("MinBZK", "regelrecht-corpus"),
            CENTRAL_WRITABLE_AUTH_REF
        );
    }
}
