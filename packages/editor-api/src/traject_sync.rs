//! Trajectloader: open a corpus that is worked on outside the editor (by hand
//! or by an agent, in a git clone) as a traject, and keep the two in step.
//!
//! The model is two branches and plain merges, never a shared branch tip:
//!
//! - the **working branch** (`gh_base_branch`) belongs to whoever works in the
//!   clone; the editor never writes to it;
//! - the **traject branch** (`gh_branch`) belongs to the editor, which commits
//!   one file per save on top of it through the Contents API.
//!
//! Each side therefore only ever fast-forwards its own branch. Bringing the
//! traject up to date is a server-side merge of the working branch into the
//! traject branch ([`sync_base`]); handing the editor's work back is a pull
//! request from traject to working branch ([`propose_to_base`]), or a local
//! `git merge origin/<traject-branch>` in the clone. A conflict is a clean 409
//! that leaves both branches as they were.
//!
//! [`preflight`] is the "load" step in front of traject creation: before a
//! traject is pointed at a repo, it reports what the editor will find there —
//! how many law files, which of them won't open, which laws also come from
//! another source, and whether the notes sidecars sit where the editor looks.

use std::collections::{BTreeMap, BTreeSet};
use std::sync::Arc;

use axum::extract::{Path, Query, State};
use axum::http::StatusCode;
use axum::{Extension, Json};
use regelrecht_corpus::models::SourceType;
use regelrecht_github::{GithubError, MergeOutcome};
use serde::{Deserialize, Serialize};
use tower_sessions::Session;

use crate::accounts::AccountRecord;
use crate::corpus_handlers::require_traject_corpus_from_ref;
use crate::credentials::TrajectCredentials;
use crate::state::AppState;
use crate::trajects::{
    CENTRAL_WRITABLE_AUTH_REF, CENTRAL_WRITABLE_OWNER, CENTRAL_WRITABLE_REPO, derive_auth_ref,
    repo_access_error_to_status, valid_branch_name, valid_repo_path, valid_repo_segment,
};

/// Law files whose content the preflight fetches and checks. Beyond this the
/// report says how many went unchecked rather than fanning out further.
const MAX_CHECKED_FILES: usize = 300;
/// Parallel content fetches during the preflight.
const FETCH_CONCURRENCY: usize = 8;

type ApiError = (StatusCode, String);

// ---------------------------------------------------------------------------
// Pure helpers (unit-tested below)
// ---------------------------------------------------------------------------

/// What the preflight found in a repo tree, before looking at any content.
#[derive(Debug, Default, PartialEq, Eq)]
pub(crate) struct TreeSurvey {
    /// Repo path of every law-shaped YAML file under the source root:
    /// `{root}/{layer...}/{dir}/{date}.yaml`. The law's identity is its
    /// `$id`, not the directory name (`example_body/leidraad_x/` holds
    /// `$id: leidraad_x_example_body`), so only the content can tell.
    pub law_files: Vec<String>,
    /// Notes sidecars where the editor reads them: `{root}/annotations/…`.
    pub annotations_under_root: usize,
    /// Notes sidecars at the repo root, when the source root is a sub-path.
    pub annotations_at_repo_root: usize,
}

/// Classify repo paths the way the traject index will see them. Mirrors the
/// selection in `regelrecht_corpus::github` (`.yaml` under the source root, at
/// least `{layer}/{law_id}/{file}`, the `annotations/` subtree excluded), and
/// additionally skips dot-files, which `just validate` skips too.
pub(crate) fn survey_tree(paths: &[String], repo_path: Option<&str>) -> TreeSurvey {
    let root = repo_path.unwrap_or("").trim_matches('/');
    let prefix = if root.is_empty() {
        String::new()
    } else {
        format!("{root}/")
    };
    let mut out = TreeSurvey::default();
    for path in paths {
        if !path.ends_with(".yaml") {
            continue;
        }
        if !root.is_empty() && path.starts_with("annotations/") {
            out.annotations_at_repo_root += 1;
        }
        let Some(rel) = path.strip_prefix(&prefix) else {
            continue;
        };
        let parts: Vec<&str> = rel.split('/').collect();
        if parts.first() == Some(&"annotations") {
            out.annotations_under_root += 1;
            continue;
        }
        if parts.len() < 3 || parts.iter().any(|p| p.starts_with('.')) {
            continue;
        }
        out.law_files.push(path.clone());
    }
    out.law_files.sort();
    out
}

/// Outcome of [`check_law_file`].
#[derive(Debug, PartialEq, Eq)]
pub(crate) struct LawCheck {
    /// The top-level `$id`, whenever one could be read.
    pub id: Option<String>,
    /// `None` when the file will open; otherwise why it won't.
    pub error: Option<String>,
}

/// Whether one law file will open in the editor and load in the engine:
/// well-formed YAML, a top-level `$id`, and a document the law model
/// deserialises.
///
/// This is the engine's load contract, not full JSON-Schema validation; a file
/// that passes here can still carry schema findings `just validate` reports.
pub(crate) fn check_law_file(content: &str) -> LawCheck {
    if let Err(e) = regelrecht_corpus::source_map::validate_yaml_syntax(content) {
        return LawCheck {
            id: None,
            error: Some(format!("geen geldige YAML: {e}")),
        };
    }
    let Some(id) = regelrecht_law_model::parse_law_header(content).id else {
        return LawCheck {
            id: None,
            error: Some("mist een `$id` op het hoogste niveau".to_string()),
        };
    };
    let error = serde_yaml_ng::from_str::<regelrecht_law_model::ArticleBasedLaw>(content)
        .err()
        .map(|e| format!("past niet op het wetmodel: {e}"));
    LawCheck {
        id: Some(id),
        error,
    }
}

/// Paths changed on both sides since the merge base, sorted.
pub(crate) fn overlap(a: &[String], b: &[String]) -> Vec<String> {
    let a: BTreeSet<&String> = a.iter().collect();
    b.iter()
        .filter(|p| a.contains(p))
        .cloned()
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect()
}

// ---------------------------------------------------------------------------
// Shared plumbing
// ---------------------------------------------------------------------------

fn github_client(state: &AppState) -> Result<regelrecht_github::GithubClient, ApiError> {
    let mut client = regelrecht_github::GithubClient::new().map_err(|e| {
        tracing::error!(error = %e, "failed to build GitHub client");
        (
            StatusCode::SERVICE_UNAVAILABLE,
            "kon de GitHub-client niet initialiseren".to_string(),
        )
    })?;
    // Same test seam as the create preflight: the OAuth config's api_base
    // points the whole client at a mock server in tests.
    if let Some(api_base) = state
        .config
        .github_oauth
        .as_ref()
        .map(|o| o.api_base.as_str())
    {
        client.set_base_url(api_base);
    }
    Ok(client)
}

fn upstream(context: &'static str) -> impl FnOnce(GithubError) -> ApiError {
    move |e| match e {
        GithubError::WriteDenied(msg) => {
            tracing::warn!(error = %msg, context, "GitHub refused a write");
            (
                StatusCode::FORBIDDEN,
                "GitHub weigert deze schrijfactie: het token heeft geen schrijfrechten op deze repo"
                    .to_string(),
            )
        }
        GithubError::Api {
            status: 404,
            message,
        } => {
            tracing::warn!(error = %message, context, "GitHub 404");
            (
                StatusCode::NOT_FOUND,
                format!("branch of repo niet gevonden op GitHub ({context})"),
            )
        }
        GithubError::Api {
            status: 422,
            message,
        } => {
            tracing::warn!(error = %message, context, "GitHub 422");
            (
                StatusCode::CONFLICT,
                format!(
                    "GitHub weigerde de wijziging; de branch is intussen veranderd. Probeer het opnieuw ({context})"
                ),
            )
        }
        other => {
            tracing::warn!(error = %other, context, "GitHub call failed");
            (
                StatusCode::BAD_GATEWAY,
                format!("GitHub-aanroep mislukt ({context})"),
            )
        }
    }
}

/// Coordinates of a traject's writable-own source.
struct OwnRepo {
    owner: String,
    repo: String,
    traject_branch: String,
    base_branch: String,
}

impl OwnRepo {
    fn full(&self) -> String {
        format!("{}/{}", self.owner, self.repo)
    }
}

fn own_repo(traject: &crate::traject_corpus::TrajectCorpus) -> Result<OwnRepo, ApiError> {
    let source = traject
        .corpus
        .registry
        .get_source(&traject.writable_own_source_id)
        .ok_or((
            StatusCode::INTERNAL_SERVER_ERROR,
            "traject heeft geen eigen bron".to_string(),
        ))?;
    let SourceType::GitHub { github } = &source.source_type else {
        return Err((
            StatusCode::CONFLICT,
            "de eigen bron van dit traject is geen GitHub-repo".to_string(),
        ));
    };
    let base_branch = traject.own_base_branch.clone().ok_or((
        StatusCode::CONFLICT,
        "dit traject heeft geen werkbranch (base) vastgelegd".to_string(),
    ))?;
    Ok(OwnRepo {
        owner: github.owner.clone(),
        repo: github.repo.clone(),
        traject_branch: github.branch.clone(),
        base_branch,
    })
}

// ---------------------------------------------------------------------------
// POST /api/trajects/preflight
// ---------------------------------------------------------------------------

#[derive(Debug, Deserialize)]
pub struct PreflightRequest {
    pub repo_owner: String,
    pub repo_name: String,
    /// The working branch to base the traject on. Omitted: the repo's
    /// default branch.
    #[serde(default)]
    pub base_branch: Option<String>,
    #[serde(default)]
    pub repo_path: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct PreflightLaw {
    pub path: String,
    /// The file's `$id`, when it could be read.
    pub law_id: Option<String>,
    /// `None` when the file passed [`check_law_file`]; otherwise why not.
    pub error: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct SeedOverlap {
    pub source_id: String,
    pub source_name: String,
    pub law_ids: Vec<String>,
}

#[derive(Debug, Serialize)]
pub struct Finding {
    /// `blocker` (creating the traject makes no sense), `warning` or `info`.
    pub level: &'static str,
    pub message: String,
}

#[derive(Debug, Serialize)]
pub struct PreflightReport {
    pub repo: String,
    pub is_private: bool,
    pub default_branch: String,
    pub branches: Vec<String>,
    pub base_branch: String,
    pub repo_path: Option<String>,
    pub law_file_count: usize,
    pub law_count: usize,
    pub invalid_count: usize,
    pub unchecked_count: usize,
    pub laws: Vec<PreflightLaw>,
    pub annotations_under_root: usize,
    pub annotations_at_repo_root: usize,
    pub seed_overlap: Vec<SeedOverlap>,
    pub findings: Vec<Finding>,
}

/// POST /api/trajects/preflight — inspect a repo before a traject is pointed
/// at it. Read-only: nothing is created on GitHub or in the database.
pub async fn preflight(
    State(state): State<AppState>,
    Extension(account): Extension<AccountRecord>,
    headers: axum::http::HeaderMap,
    Json(req): Json<PreflightRequest>,
) -> Result<Json<PreflightReport>, ApiError> {
    let owner = req.repo_owner.trim();
    let repo = req.repo_name.trim();
    if !valid_repo_segment(owner) || !valid_repo_segment(repo) {
        return Err((
            StatusCode::BAD_REQUEST,
            "repo_owner / repo_name mogen alleen letters, cijfers, en de tekens '-', '_' en '.' bevatten"
                .to_string(),
        ));
    }
    let repo_path = req
        .repo_path
        .as_deref()
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(str::to_string);
    if let Some(p) = &repo_path
        && !valid_repo_path(p)
    {
        return Err((
            StatusCode::BAD_REQUEST,
            "repo_path moet een relatief pad zijn zonder '..' segmenten".to_string(),
        ));
    }
    let requested_base = req
        .base_branch
        .as_deref()
        .map(str::trim)
        .filter(|s| !s.is_empty());
    if let Some(b) = requested_base
        && !valid_branch_name(b)
    {
        return Err((
            StatusCode::BAD_REQUEST,
            "base_branch bevat tekens die niet zijn toegestaan in een git branch-naam".to_string(),
        ));
    }
    let auth_ref = derive_auth_ref(owner, repo);
    if auth_ref.is_empty() || auth_ref == CENTRAL_WRITABLE_AUTH_REF {
        return Err((
            StatusCode::BAD_REQUEST,
            format!("repo \"{owner}/{repo}\" kan niet als traject-repo worden gebruikt"),
        ));
    }

    // Same token the create handler will preflight and the traject will
    // write with: the per-repo service token, else the user's own link.
    let token = TrajectCredentials::new(&state, account.id, &headers)
        .for_new_repo_preflight(&auth_ref)
        .await?;
    let client = github_client(&state)?;
    let info = client
        .check_repo(owner, repo, &token)
        .await
        .map_err(|e| repo_access_error_to_status(&e, owner, repo, requested_base.unwrap_or("")))?;
    let full = format!("{owner}/{repo}");
    let branches = client
        .list_branches(&full, Some(&token))
        .await
        .map_err(upstream("branches ophalen"))?;
    let base_branch = requested_base
        .map(str::to_string)
        .unwrap_or_else(|| info.default_branch.clone());

    let mut findings = Vec::new();
    if info.is_private {
        findings.push(Finding {
            level: "info",
            message: "Privé-repo: alleen wie toegang heeft tot de repo én lid is van het traject ziet de inhoud.".to_string(),
        });
    }
    if !branches.iter().any(|b| b == &base_branch) {
        findings.push(Finding {
            level: "blocker",
            message: format!("Branch '{base_branch}' bestaat niet op {full}."),
        });
        return Ok(Json(PreflightReport {
            repo: full,
            is_private: info.is_private,
            default_branch: info.default_branch,
            branches,
            base_branch,
            repo_path,
            law_file_count: 0,
            law_count: 0,
            invalid_count: 0,
            unchecked_count: 0,
            laws: Vec::new(),
            annotations_under_root: 0,
            annotations_at_repo_root: 0,
            seed_overlap: Vec::new(),
            findings,
        }));
    }

    let tree = client
        .list_tree_files(&full, &base_branch, Some(&token))
        .await
        .map_err(upstream("bestandslijst ophalen"))?
        // A 304 can only come from an earlier identical call on this fresh
        // client, which there wasn't; treat it as empty to be safe.
        .unwrap_or_default();
    let paths: Vec<String> = tree.into_iter().map(|e| e.path).collect();
    let survey = survey_tree(&paths, repo_path.as_deref());

    // Fetch and check the law files, bounded in count and in parallelism.
    let to_check: Vec<String> = survey
        .law_files
        .iter()
        .take(MAX_CHECKED_FILES)
        .cloned()
        .collect();
    let unchecked_count = survey.law_files.len() - to_check.len();
    let client = Arc::new(client);
    let token = Arc::new(token);
    let semaphore = Arc::new(tokio::sync::Semaphore::new(FETCH_CONCURRENCY));
    let mut set = tokio::task::JoinSet::new();
    for path in to_check {
        let client = client.clone();
        let token = token.clone();
        let semaphore = semaphore.clone();
        let full = full.clone();
        let base_branch = base_branch.clone();
        set.spawn(async move {
            let _permit = semaphore.acquire_owned().await;
            let check = match client
                .fetch_file_raw(&full, &base_branch, &path, Some(token.as_str()))
                .await
            {
                Ok(content) => check_law_file(&content),
                Err(e) => LawCheck {
                    id: None,
                    error: Some(format!("kon het bestand niet ophalen: {e}")),
                },
            };
            PreflightLaw {
                path,
                law_id: check.id,
                error: check.error,
            }
        });
    }
    let mut laws = Vec::new();
    while let Some(res) = set.join_next().await {
        match res {
            Ok(law) => laws.push(law),
            Err(e) => tracing::error!(error = %e, "preflight check task failed"),
        }
    }
    laws.sort_by(|a, b| a.path.cmp(&b.path));
    let invalid_count = laws.iter().filter(|l| l.error.is_some()).count();
    let law_ids: BTreeSet<String> = laws.iter().filter_map(|l| l.law_id.clone()).collect();

    // Laws that the global corpus already serves from another source. In the
    // traject the repo wins them (see `effective_priorities`); the report
    // says so, because a stale local mirror of the same repo shows up here.
    let seed_overlap = {
        let corpus = state.corpus.read().await;
        let mut by_source: BTreeMap<String, (String, Vec<String>)> = BTreeMap::new();
        for id in &law_ids {
            if let Some(law) = corpus.source_map.get_law(id) {
                by_source
                    .entry(law.source_id.clone())
                    .or_insert_with(|| (law.source_name.clone(), Vec::new()))
                    .1
                    .push(id.clone());
            }
        }
        by_source
            .into_iter()
            .map(|(source_id, (source_name, law_ids))| SeedOverlap {
                source_id,
                source_name,
                law_ids,
            })
            .collect::<Vec<_>>()
    };

    if survey.law_files.is_empty() {
        findings.push(Finding {
            level: "blocker",
            message: format!(
                "Geen regelwerken gevonden onder '{}'. Klopt het pad binnen de repo?",
                repo_path.as_deref().unwrap_or("<repo-root>")
            ),
        });
    }
    if invalid_count > 0 {
        findings.push(Finding {
            level: "warning",
            message: format!(
                "{invalid_count} versie(s) van regelwerken openen niet in de editor; zie de lijst hieronder."
            ),
        });
    }
    if unchecked_count > 0 {
        findings.push(Finding {
            level: "info",
            message: format!(
                "{unchecked_count} versie(s) niet gecontroleerd (maximum is {MAX_CHECKED_FILES})."
            ),
        });
    }
    if survey.annotations_at_repo_root > 0 && survey.annotations_under_root == 0 {
        findings.push(Finding {
            level: "warning",
            message: format!(
                "Er staan {} notitiebestand(en) in 'annotations/' op de repo-root, maar de editor \
                 zoekt ze onder '{}/annotations/'. Verplaats ze, of ze blijven in de editor onzichtbaar.",
                survey.annotations_at_repo_root,
                repo_path.as_deref().unwrap_or("")
            ),
        });
    }
    for overlap in &seed_overlap {
        findings.push(Finding {
            level: "info",
            message: format!(
                "{} regelwerk(en) staan ook in bron '{}'. In dit traject gaat de repo voor.",
                overlap.law_ids.len(),
                overlap.source_name
            ),
        });
    }

    Ok(Json(PreflightReport {
        repo: full,
        is_private: info.is_private,
        default_branch: info.default_branch,
        branches,
        base_branch,
        repo_path,
        law_file_count: survey.law_files.len(),
        law_count: law_ids.len(),
        invalid_count,
        unchecked_count,
        laws,
        annotations_under_root: survey.annotations_under_root,
        annotations_at_repo_root: survey.annotations_at_repo_root,
        seed_overlap,
        findings,
    }))
}

// ---------------------------------------------------------------------------
// GET /api/trajects/{traject_ref}/base-status
// ---------------------------------------------------------------------------

#[derive(Debug, Serialize)]
pub struct BaseStatus {
    pub repo: String,
    pub base_branch: String,
    pub traject_branch: String,
    /// Commits on the working branch the traject does not have yet.
    pub behind_by: u32,
    /// Commits on the traject branch the working branch does not have yet.
    pub ahead_by: u32,
    /// Files the traject changed since the two diverged.
    pub traject_files: Vec<String>,
    /// Files the working branch changed since the two diverged.
    pub base_files: Vec<String>,
    /// Files changed on both sides: where a merge can conflict.
    pub overlap: Vec<String>,
}

async fn compute_base_status(
    client: &regelrecht_github::GithubClient,
    own: &OwnRepo,
    token: Option<&str>,
) -> Result<BaseStatus, ApiError> {
    let full = own.full();
    let traject_side = client
        .compare_status(&full, &own.base_branch, &own.traject_branch, token)
        .await
        .map_err(upstream("traject vergelijken met werkbranch"))?;
    let base_side = client
        .compare_status(&full, &own.traject_branch, &own.base_branch, token)
        .await
        .map_err(upstream("werkbranch vergelijken met traject"))?;
    let (Some(traject_side), Some(base_side)) = (traject_side, base_side) else {
        return Err((
            StatusCode::NOT_FOUND,
            format!(
                "branch '{}' of '{}' bestaat niet (meer) op {full}",
                own.traject_branch, own.base_branch
            ),
        ));
    };
    Ok(BaseStatus {
        repo: full,
        base_branch: own.base_branch.clone(),
        traject_branch: own.traject_branch.clone(),
        behind_by: traject_side.behind_by,
        ahead_by: traject_side.ahead_by,
        overlap: overlap(&traject_side.files, &base_side.files),
        traject_files: traject_side.files,
        base_files: base_side.files,
    })
}

/// What the token is for: reads follow `TrajectCredentials::for_read`, writes
/// `for_write` (which also runs the writability gate).
#[derive(Clone, Copy)]
enum Access {
    Read,
    Write,
}

/// The token to call GitHub with for the traject's own repo, resolved the
/// way the traject's own reads and saves resolve it: the source's configured
/// token first (strict for a user-chosen repo, exactly as the backend was
/// built), otherwise the acting user's linked token where the deployment
/// requires one. `Ok(None)` means: no credential, call unauthenticated.
///
/// Deliberately not `for_new_repo_preflight`: that resolver is for a repo no
/// traject points at yet, and its strict lookup plus user fallback would send
/// members of a central-corpus traject into the GitHub connect flow.
async fn own_token(
    state: &AppState,
    account: &AccountRecord,
    headers: &axum::http::HeaderMap,
    traject: &crate::traject_corpus::TrajectCorpus,
    access: Access,
) -> Result<Option<String>, ApiError> {
    let source = traject
        .corpus
        .registry
        .get_source(&traject.writable_own_source_id)
        .ok_or((
            StatusCode::INTERNAL_SERVER_ERROR,
            "traject heeft geen eigen bron".to_string(),
        ))?;
    let github = state.corpus.read().await.github.clone();
    let configured = github
        .credentials()
        .resolve_source(source)
        .map(regelrecht_corpus::auth::TokenDecision::into_token)
        .map_err(|e| {
            tracing::error!(error = %e, "auth lookup failed for traject own source");
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                "auth lookup failed".to_string(),
            )
        })?;
    let entry = traject
        .corpus
        .backends
        .get(&traject.writable_own_source_id)
        .ok_or((
            StatusCode::INTERNAL_SERVER_ERROR,
            "backend van de eigen bron ontbreekt".to_string(),
        ))?;
    let backend = entry.backend.lock().await;
    let creds = TrajectCredentials::new(state, account.id, headers);
    let user = match access {
        Access::Read => creds.for_read(&**backend, entry.writable).await?,
        Access::Write => creds
            .for_write(&**backend, entry.writable)
            .await?
            .read_token()
            .map(str::to_string),
    };
    // `for_read`/`for_write` hand out a user token only where the backend has
    // no credential of its own, so at most one of the two is set.
    Ok(user.or(configured))
}

fn is_central(own: &OwnRepo) -> bool {
    own.owner == CENTRAL_WRITABLE_OWNER && own.repo == CENTRAL_WRITABLE_REPO
}

/// GET /api/trajects/{traject_ref}/base-status — how far the traject branch
/// and its working branch have drifted apart.
///
/// Called unprompted by the library banner, so it never asks for a GitHub
/// link: a missing link (428) becomes a quiet 409, as does the central
/// corpus, whose `development` branch moves for reasons unrelated to any one
/// traject.
pub async fn base_status(
    State(state): State<AppState>,
    Extension(account): Extension<AccountRecord>,
    session: Session,
    Path(traject_ref): Path<String>,
    headers: axum::http::HeaderMap,
) -> Result<Json<BaseStatus>, ApiError> {
    let traject = require_traject_corpus_from_ref(&state, &session, &traject_ref).await?;
    let own = own_repo(&traject)?;
    if is_central(&own) {
        return Err((
            StatusCode::CONFLICT,
            "niet van toepassing op het centrale corpus".to_string(),
        ));
    }
    let token = match own_token(&state, &account, &headers, &traject, Access::Read).await {
        Err((StatusCode::PRECONDITION_REQUIRED, _)) => {
            return Err((
                StatusCode::CONFLICT,
                "koppel je GitHub-account om de werkbranch te kunnen vergelijken".to_string(),
            ));
        }
        other => other?,
    };
    let client = github_client(&state)?;
    Ok(Json(
        compute_base_status(&client, &own, token.as_deref()).await?,
    ))
}

// ---------------------------------------------------------------------------
// POST /api/trajects/{traject_ref}/sync-base
// ---------------------------------------------------------------------------

#[derive(Debug, Default, Deserialize)]
pub struct SyncQuery {
    /// Set by the editor's background loop. A missing GitHub link then
    /// answers 409 instead of 428, so an unprompted call never starts the
    /// connect flow; a click on "Bijwerken" still does.
    #[serde(default)]
    pub auto: bool,
}

#[derive(Debug, Serialize)]
pub struct SyncResult {
    /// `fast_forward` (the traject had no commits of its own and now points
    /// at the working branch), `merged` (a merge commit was added to the
    /// traject branch) or `up_to_date` (nothing to do).
    pub outcome: &'static str,
    pub sha: Option<String>,
}

/// POST /api/trajects/{traject_ref}/sync-base — bring the working branch into
/// the traject branch on GitHub: a fast-forward when the traject has no
/// commits of its own, otherwise a merge. A conflict is a 409 naming the
/// files changed on both sides; both branches are then exactly as they were,
/// and the merge has to be done in a clone.
pub async fn sync_base(
    State(state): State<AppState>,
    Extension(account): Extension<AccountRecord>,
    session: Session,
    Path(traject_ref): Path<String>,
    Query(query): Query<SyncQuery>,
    headers: axum::http::HeaderMap,
) -> Result<Json<SyncResult>, ApiError> {
    let traject = require_traject_corpus_from_ref(&state, &session, &traject_ref).await?;
    let own = own_repo(&traject)?;
    let token = match own_token(&state, &account, &headers, &traject, Access::Write).await {
        Err((StatusCode::PRECONDITION_REQUIRED, _)) if query.auto => {
            return Err((
                StatusCode::CONFLICT,
                "koppel je GitHub-account om automatisch bij te werken".to_string(),
            ));
        }
        other => other?,
    };
    let token = token.as_deref();
    let client = github_client(&state)?;
    let full = own.full();
    let entry = traject
        .corpus
        .backends
        .get(&traject.writable_own_source_id)
        .ok_or((
            StatusCode::INTERNAL_SERVER_ERROR,
            "backend van de eigen bron ontbreekt".to_string(),
        ))?;

    // Hold the writable-own backend's lock from the divergence check through
    // the ref update: saves in this process take the same lock, so none lands
    // in between. (Saves in another replica can still interleave; GitHub
    // refuses a fast-forward that is no longer one, and serialises merges, so
    // the worst case is a 422/409 the user retries.)
    let outcome = {
        let _guard = entry.backend.lock().await;
        let status = compute_base_status(&client, &own, token).await?;
        if status.behind_by == 0 {
            Ok(("up_to_date", None))
        } else if status.ahead_by == 0 {
            client
                .fast_forward_branch(&full, &own.traject_branch, &own.base_branch, token)
                .await
                .map(|sha| ("fast_forward", Some(sha)))
        } else {
            client
                .merge_branch(
                    &full,
                    &own.traject_branch,
                    &own.base_branch,
                    &format!(
                        "Werkbranch '{}' bijgewerkt in traject '{}'",
                        own.base_branch, own.traject_branch
                    ),
                    token,
                )
                .await
                .map(|m| match m {
                    MergeOutcome::UpToDate => ("up_to_date", None),
                    MergeOutcome::Merged { sha } => ("merged", Some(sha)),
                })
        }
    };

    match outcome {
        Ok((kind, sha)) => {
            if sha.is_some() {
                // The branch content moved under the snapshot: rebuild on the
                // next request instead of serving pre-merge bodies for a TTL.
                state.trajects.invalidate(traject.traject_id).await;
                tracing::info!(
                    traject = %traject.traject_id,
                    repo = %full,
                    base = %own.base_branch,
                    branch = %own.traject_branch,
                    outcome = kind,
                    "brought working branch into traject branch"
                );
            }
            Ok(Json(SyncResult { outcome: kind, sha }))
        }
        Err(GithubError::Conflict(msg)) => {
            tracing::info!(traject = %traject.traject_id, error = %msg, "sync-base conflict");
            let files = compute_base_status(&client, &own, token)
                .await
                .map(|s| s.overlap)
                .unwrap_or_default();
            let list = if files.is_empty() {
                String::new()
            } else {
                format!(" Aan beide kanten gewijzigd: {}.", files.join(", "))
            };
            Err((
                StatusCode::CONFLICT,
                format!(
                    "De werkbranch '{}' en het traject wijzigen dezelfde regels; GitHub kan ze niet \
                     automatisch samenvoegen.{list} Voeg ze samen in een clone met \
                     `git fetch origin && git checkout {} && git merge origin/{}`, push de \
                     werkbranch, en probeer het daarna opnieuw.",
                    own.base_branch, own.base_branch, own.traject_branch
                ),
            ))
        }
        Err(e) => Err(upstream("werkbranch samenvoegen")(e)),
    }
}

// ---------------------------------------------------------------------------
// POST /api/trajects/{traject_ref}/propose
// ---------------------------------------------------------------------------

#[derive(Debug, Serialize)]
pub struct ProposeResult {
    pub number: u64,
    pub url: String,
}

/// POST /api/trajects/{traject_ref}/propose — open (or refresh) the pull
/// request from the traject branch to its working branch, so the editor's
/// commits flow back to whoever works in the clone.
pub async fn propose_to_base(
    State(state): State<AppState>,
    Extension(account): Extension<AccountRecord>,
    session: Session,
    Path(traject_ref): Path<String>,
    headers: axum::http::HeaderMap,
) -> Result<Json<ProposeResult>, ApiError> {
    let traject = require_traject_corpus_from_ref(&state, &session, &traject_ref).await?;
    let own = own_repo(&traject)?;
    let token = own_token(&state, &account, &headers, &traject, Access::Write)
        .await?
        .ok_or((
            StatusCode::SERVICE_UNAVAILABLE,
            "er is geen GitHub-token beschikbaar om een pull request te openen".to_string(),
        ))?;
    let client = github_client(&state)?;
    let pr = client
        .ensure_pr(
            &own.owner,
            &own.repo,
            &own.traject_branch,
            &own.base_branch,
            &format!("Wijzigingen uit traject '{}'", own.traject_branch),
            &format!(
                "Wijzigingen die in de regelrecht-editor zijn gemaakt in traject-branch \
                 `{}`.\n\nMergen brengt ze naar werkbranch `{}`. Lokaal kan dat ook met \
                 `git merge origin/{}`.",
                own.traject_branch, own.base_branch, own.traject_branch
            ),
            &token,
        )
        .await
        .map_err(upstream("pull request openen"))?;
    Ok(Json(ProposeResult {
        number: pr.number,
        url: pr.html_url,
    }))
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;

    fn s(v: &[&str]) -> Vec<String> {
        v.iter().map(|x| x.to_string()).collect()
    }

    #[test]
    fn survey_selects_law_files_under_root_and_counts_annotations() {
        let paths = s(&[
            "regulation/nl/wet/wet_a/2025-01-01.yaml",
            "regulation/nl/uitvoeringsbeleid/org/beleid_b/2026-07-01.yaml",
            "regulation/nl/wet/wet_a/scenarios/basis.feature",
            "regulation/nl/wet/wet_a/.enrichment.yaml",
            "regulation/nl/annotations/wet_a/annotations.yaml",
            "annotations/wet_a/annotations.yaml",
            "docs/notes.yaml",
            "regulation/nl/losse.yaml",
        ]);
        let survey = survey_tree(&paths, Some("regulation/nl"));
        assert_eq!(
            survey.law_files,
            s(&[
                "regulation/nl/uitvoeringsbeleid/org/beleid_b/2026-07-01.yaml",
                "regulation/nl/wet/wet_a/2025-01-01.yaml",
            ])
        );
        assert_eq!(survey.annotations_under_root, 1);
        assert_eq!(survey.annotations_at_repo_root, 1);
    }

    #[test]
    fn survey_at_repo_root_skips_annotations_as_laws() {
        let paths = s(&[
            "wet/wet_a/2025-01-01.yaml",
            "annotations/wet_a/annotations.yaml",
        ]);
        let survey = survey_tree(&paths, None);
        assert_eq!(survey.law_files.len(), 1);
        assert_eq!(survey.annotations_under_root, 1);
        assert_eq!(survey.annotations_at_repo_root, 0);
    }

    #[test]
    fn check_accepts_a_minimal_law_whose_id_differs_from_its_directory() {
        let yaml = "$id: wet_a_org\nregulatory_layer: WET\npublication_date: '2025-01-01'\nvalid_from: '2025-01-01'\nname: Wet A\narticles:\n  - number: '1'\n    text: Tekst.\n    url: https://example.org/wet_a#1\n";
        assert_eq!(
            check_law_file(yaml),
            LawCheck {
                id: Some("wet_a_org".to_string()),
                error: None
            }
        );
    }

    #[test]
    fn check_rejects_bad_yaml_and_missing_id() {
        let bad = check_law_file("articles: [\n");
        assert!(bad.id.is_none());
        assert!(bad.error.unwrap().contains("geen geldige YAML"));
        let no_id = check_law_file("name: x\n");
        assert!(no_id.error.unwrap().contains("mist een `$id`"));
    }

    #[test]
    fn check_keeps_the_id_of_a_document_the_model_cannot_load() {
        let c = check_law_file("$id: wet_a\narticles: 12\n");
        assert_eq!(c.id.as_deref(), Some("wet_a"));
        assert!(c.error.unwrap().contains("wetmodel"));
    }

    #[test]
    fn overlap_is_sorted_intersection() {
        assert_eq!(
            overlap(&s(&["b", "a", "c"]), &s(&["c", "a", "a", "d"])),
            s(&["a", "c"])
        );
    }
}
