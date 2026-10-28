---
title: "Editor API"
description: "The Rust backend behind the law editor: corpus reads, traject-scoped writes, tasks, and proxies to the pipeline and harvester-admin services."
---

The editor API is the HTTP server behind the law editor. It serves the compiled frontend, exposes the corpus over REST, and runs every write the editor makes. Writes always go to a traject: an editing session with its own members and its own corpus repository branch.

## Overview

- **Language**: Rust (Axum)
- **Location**: `packages/editor-api/`
- **Port**: 8000 (fixed in `serve()` in `main.rs`)
- **Database**: PostgreSQL, shared with the pipeline. Sessions, accounts, trajects, favorites, notes, tasks and feature flags live there. Migrations come from the pipeline crate (`regelrecht_pipeline::ensure_schema`).

## Authorization model

Every route sits in one router group, and each group carries one role gate. The roles are Keycloak realm roles: `editor-reader`, `editor-writer`, `editor-admin`, plus `harvester-reader` for the harvester proxy. Keycloak composite roles make a writer also a reader. See [Auth and roles](/auth-and-roles/) for the role model.

| Gate | What it checks |
|------|----------------|
| Public | Nothing. Reads the global corpus only. |
| Reader | Session carries `editor-reader`. |
| Writer | Session carries `editor-writer`. |
| Admin | Session carries `editor-admin`. |
| + account | The request also resolves an `accounts` row (`account_middleware`). Without a database this answers 503. |
| + membership | The handler checks traject membership on every request. Some actions are owner-only. |

A missing session answers 401, a missing role 403. When OIDC is not configured, every role gate passes: that mode exists for local development only, and the server logs a warning at startup. In that mode there is no database pool, so every route that needs an account answers 503, and the GitHub OAuth routes are not mounted at all.

The login itself has a separate gate: `OIDC_REQUIRED_ROLE` is the minimum role needed to complete a login at all.

## Endpoints

Derived from the router in `packages/editor-api/src/main.rs` (and `github_oauth.rs` for `/auth/github/*`). That file is the authority; this list is a map of it.

### Service and session

| Method | Path | Gate |
|--------|------|------|
| GET | `/health` | Public |
| GET | `/auth/login`, `/auth/callback`, `/auth/logout`, `/auth/status` | Public (OIDC flow from `regelrecht-auth`) |
| GET | `/api/feature-flags` | Public |
| PUT | `/api/feature-flags/{key}` | Admin |
| GET | `/api/document-upload-formats` | Public |

Any other path falls through to the single-page app in `STATIC_DIR`.

### Global corpus (read-only)

All public. These serve the corpus as loaded at startup, without any traject overlay.

| Method | Path |
|--------|------|
| GET | `/api/sources` |
| GET | `/api/corpus/laws` |
| GET | `/api/corpus/laws/{law_id}` |
| GET | `/api/corpus/laws/{law_id}/versions` |
| GET | `/api/corpus/laws/{law_id}/outputs` |
| GET | `/api/corpus/laws/{law_id}/implementors` |
| GET | `/api/corpus/laws/{law_id}/scenarios` |
| GET | `/api/corpus/laws/{law_id}/scenarios/{filename}` |
| GET | `/api/corpus/laws/{law_id}/annotations` |
| POST | `/api/corpus/reload` (Admin) |

At startup the global corpus loads every local source, the laws listed in `favorites.json` from the GitHub sources, and every law of a source marked `load_all`. The other laws of each GitHub source are indexed by path from one tree listing, so search finds them; their bodies are fetched from GitHub when someone opens one, and kept in a cache of at most 1024 bodies that a reload empties. Until then such a law has no name and never counts as enriched, so search matches it on its `$id` only. Anyone can open a law without an account, and each uncached body costs a call on the service token that trajecten also read with, so the global scope fetches at most 1000 bodies an hour (`GLOBAL_FETCHES_PER_HOUR`); past that an uncached law answers 503 until the hour is over. A GitHub source that fails to list is reported as `index_error` on `/api/sources`. A reload fetches again only the bodies that were loaded up front, plus any law it is asked for, and lists the sources again. The current version of a law is the one in force today among the versions loaded, across sources (see [Federated Corpus](/concepts/federated-corpus#how-it-works)).

`/api/corpus/laws` (and its traject variant) filters with `q` (search: every word of the query must occur in the law's id or name, in any order, ignoring case and accents; a query without letters or digits finds nothing, and only its first 200 characters and 10 distinct words count), `ids` (exact ids), `source` (one source), `enriched=true` (only laws whose current version is enriched) and `loaded=true` (only laws whose body is loaded, the ones that carry a name; the editor asks for these to show display names). A search returns the best matches first, within each source in priority order: a law whose id or name is the query, then one that starts with it (as characters, so *wetboek* counts for *wet*), then one where every word sits at the start or end of a word (a Dutch compound such as *zorgverzekeringswet* ends in its head), then any other match. Laws whose body is loaded go before the rest of the same rank, then the shorter id. The total before the cap is in `X-Total-Count`. Each entry carries `enriched`, and, when the current version is not enriched but another version is, `enriched_version` with that version's date. A law from a source marked `enriched: false` never counts as enriched. A law from a GitHub source carries `source_url`, the folder of its current version on GitHub at the ref the source reads (inside a traject, the traject's branch for its own repository).

### Trajects

Reader + account for reads, Writer + account for changes. Membership is checked in the handler.

| Method | Path | Notes |
|--------|------|-------|
| GET | `/api/trajects` | Trajects you are a member of |
| POST | `/api/trajects` | Creator becomes owner |
| GET | `/api/trajects/{id}` | |
| PATCH, DELETE | `/api/trajects/{id}` | Owner only |
| POST | `/api/trajects/{id}/leave` | |
| POST | `/api/trajects/{id}/members` | Owner only; adds a member or an invite |
| PATCH, DELETE | `/api/trajects/{id}/members/{account_id}` | Owner only |
| DELETE | `/api/trajects/{id}/invites/{email}` | Owner only |
| GET | `/api/trajects/{traject_ref}/integrity` | Integrity report over the traject's corpus repo |
| GET | `/api/trajects/{traject_ref}/sources` | |
| GET | `/api/trajects/{traject_ref}/favorites` | |
| PUT, DELETE | `/api/trajects/{traject_ref}/favorites/{law_id}` | |

Member roles are `owner` and `contributor`.

### Traject corpus

The traject-scoped view of the corpus: the global corpus with the traject's own changes on top. Reads are Reader + account, writes Writer + account, and all of them check membership.

| Method | Path | Gate |
|--------|------|------|
| GET | `/api/trajects/{traject_ref}/corpus/laws` | Reader |
| POST | `/api/trajects/{traject_ref}/corpus/laws` | Writer; create a new law (body cap 5 MiB) |
| POST | `/api/trajects/{traject_ref}/corpus/laws/upload` | Writer; upload a law document for conversion (25 MiB) |
| GET | `/api/trajects/{traject_ref}/corpus/changed-laws` | Reader |
| GET, PUT | `/api/trajects/{traject_ref}/corpus/laws/{law_id}` | Reader / Writer (5 MiB) |
| GET | `.../corpus/laws/{law_id}/versions` | Reader |
| GET | `.../corpus/laws/{law_id}/outputs` | Reader |
| GET | `.../corpus/laws/{law_id}/implementors` | Reader |
| GET | `.../corpus/laws/{law_id}/scenarios` | Reader |
| GET, PUT, DELETE | `.../corpus/laws/{law_id}/scenarios/{filename}` | Reader / Writer (1 MiB) |
| GET, PUT | `.../corpus/laws/{law_id}/annotations` | Reader / Writer (1 MiB) |
| POST | `.../corpus/laws/{law_id}/promote` | Writer; copy a law from the central corpus into the traject repo |
| POST | `.../corpus/laws/{law_id}/enrich` | Writer; request an enrichment task |
| POST | `/api/trajects/{traject_ref}/corpus/harvest` | Writer; harvest a law into the traject via a task |
| GET | `/api/trajects/{traject_ref}/corpus/documents` | Reader |
| POST | `.../corpus/documents/upload` | Writer; PDF or Word, converted to markdown asynchronously (25 MiB) |
| GET | `.../corpus/documents/jobs` | Reader; running conversions |
| DELETE | `.../corpus/documents/jobs/{job_id}` | Reader; any member may cancel a conversion |
| GET, PUT, DELETE | `.../corpus/documents/{*doc_path}` | Reader / Writer (1 MiB) |

`...` stands for `/api/trajects/{traject_ref}`.

### Tasks

Review tasks produced by asynchronous jobs, scoped to the account that requested them. Reader + account for reads, Writer + account for resolving.

| Method | Path |
|--------|------|
| GET | `/api/tasks` |
| GET | `/api/tasks/{task_id}` |
| POST | `/api/tasks/{task_id}/resolve` |
| GET | `/api/tasks/jobs/{job_id}` |
| POST | `/api/tasks/jobs/{job_id}/apply` |

### Personal data

| Method | Path | Gate |
|--------|------|------|
| GET | `/api/favorites` | Reader |
| PUT, DELETE | `/api/favorites/{law_id}` | Reader |
| GET | `/api/user/settings` | Reader |
| PUT | `/api/user/settings/{key}` | Writer (4 KiB body) |
| GET | `/api/user/notes/{law_id}` | Reader + account |
| POST | `/api/user/notes/{law_id}` | Writer + account (128 KiB) |
| PUT, DELETE | `/api/user/notes/{law_id}/{note_id}` | Writer + account |

Personal notes stay in Postgres and never reach a git repository.

### Harvesting and the harvester-admin proxy

| Method | Path | Gate | Forwards to |
|--------|------|------|-------------|
| GET | `/api/harvest/status` | Public | pipeline-api `/harvest/status` |
| GET | `/api/harvest/search` | Reader | pipeline-api `/harvest/search` |
| POST | `/api/harvest` | Writer | pipeline-api `/harvest` |
| POST | `/api/harvest/batch` | Writer | pipeline-api `/harvest/batch` |
| any | `/api/harvest-admin/{*rest}` | `harvester-reader` | harvester-admin `/api/{rest}` |

The pipeline proxy strips `/api` and forwards only `content-type`. The search endpoint is behind a login because it makes outbound requests to zoekservice.overheid.nl.

The harvester-admin proxy is how the editor's Corpusinwinning section reaches the [harvester-admin API](./admin); the browser never calls that API directly. It also forwards the session cookie. The two services share the Postgres session store, so harvester-admin resolves the same session and enforces its own `harvester-writer` and `harvester-admin` gates. The `harvester-reader` gate here is a first filter only.

Both proxies answer 503 when their upstream URL cannot be resolved.

### GitHub account link

Only mounted when OIDC is enabled. Writer + account, except the relay.

| Method | Path | Notes |
|--------|------|-------|
| GET | `/auth/github/login` | Start the OAuth flow |
| GET | `/auth/github/callback` | |
| GET | `/auth/github/status` | Whether a token is linked, and whether one is required |
| POST | `/auth/github/disconnect` | |
| GET | `/auth/github/relay` | Public. Forwards the OAuth callback from the fixed callback host to the originating deployment, limited to `GITHUB_OAUTH_ALLOWED_ORIGIN_SUFFIXES` |

When `GITHUB_OAUTH_*` is not configured the handlers answer 501. The `github.user_oauth` feature flag decides whether traject writes require the user's own token.

## Configuration

### Service

From `main.rs`, `config.rs` and `packages/shared/src/telemetry.rs`.

| Variable | Default | Purpose |
|----------|---------|---------|
| `STATIC_DIR` | `static` | Compiled frontend to serve |
| `DATABASE_URL` | none | Postgres connection. Required when OIDC is on; `DATABASE_SERVER_FULL` is accepted as fallback. Without OIDC no pool is opened. |
| `HOSTNAME` | set by the platform | Pod hostname. When it names a ZAD deployment, the proxy targets are derived from it and win over the two variables below. |
| `PIPELINE_API_URL` | none | Pipeline-api base URL when `HOSTNAME` does not yield one (`http://<deployment>-pipelineapi:8000`) |
| `HARVEST_ADMIN_URL` | none | Harvester-admin base URL when `HOSTNAME` does not yield one (`http://<deployment>-harvester-admin:8000`) |
| `TASK_ENRICH_PROVIDER` | `claude` | LLM provider for enrichment tasks requested from a traject. Must be one of the pipeline's providers (`opencode`, `claude`); any other value stops startup. |
| `RUST_LOG` | `info` | Log filter |
| `LOG_FORMAT` | text | `json` for one JSON object per log line |
| `LOG_SPAN_EVENTS` | on for this service | Span-close timing lines; `none` turns them off |

### Login (OIDC)

From `packages/auth/src/config.rs`, shared with harvester-admin. Leaving `OIDC_CLIENT_ID` unset disables login entirely.

| Variable | Default | Purpose |
|----------|---------|---------|
| `OIDC_CLIENT_ID` | none | Enables OIDC when set |
| `OIDC_CLIENT_SECRET` | none | Required once `OIDC_CLIENT_ID` is set |
| `OIDC_DISCOVERY_URL` | none | Issuer discovery URL; takes priority over the Keycloak pair |
| `KEYCLOAK_BASE_URL`, `KEYCLOAK_REALM` | none | Build the issuer as `<base>/realms/<realm>` when there is no discovery URL |
| `OIDC_REQUIRED_ROLE` | `allowed-user` | Minimum realm role to log in. Set it to `editor-reader`; the default only exists for old deployments and logs a warning. |
| `BASE_URL` | derived from request headers | Public origin used for redirect URLs. An `http://localhost` value also drops the `Secure` flag from the session cookie. |

### Corpus

From `init_corpus` in `main.rs`, `packages/corpus/src/auth.rs` and, for `GITHUB_API_BASE`, `packages/github/src/client.rs`.

| Variable | Default | Purpose |
|----------|---------|---------|
| `CORPUS_REGISTRY_PATH` | `corpus-registry.yaml` | Registry manifest with the corpus sources |
| `CORPUS_REGISTRY_LOCAL_PATH` | `corpus-registry.local.yaml` | Local overrides on the manifest |
| `CORPUS_AUTH_FILE` | `corpus-auth.yaml` | Per-source tokens, file-backed |
| `CORPUS_AUTH_<KEY>_TOKEN` | none | Per-source token; checked before the auth file |
| `CORPUS_GIT_TOKEN` | none | Shared fallback token for manifest sources. Never used for a repository a user chose. |
| `GITHUB_API_BASE` | `https://api.github.com` | GitHub API base; a test seam, and usable for GitHub Enterprise |

`CORPUS_REPO_URL` and `CORPUS_BRANCH` are not read by this service. They configure the corpus checkout of the pipeline workers; see [Pipeline](./pipeline).

### GitHub account link

From `github_oauth.rs`. The first three are all-or-nothing: set all of them or none, or startup fails.

| Variable | Default | Purpose |
|----------|---------|---------|
| `GITHUB_OAUTH_CLIENT_ID` | none | OAuth app client id |
| `GITHUB_OAUTH_CLIENT_SECRET` | none | OAuth app secret |
| `GITHUB_TOKEN_ENC_KEY` | none | Base64 key that encrypts the stored user token |
| `GITHUB_OAUTH_SCOPES` | `repo read:org` | Requested scopes |
| `GITHUB_USER_TOKEN_REQUIRED` | `false` | `1` or `true` forces a personal token for traject writes, regardless of the feature flag |
| `GITHUB_OAUTH_CALLBACK_BASE` | none | Fixed callback host for relay mode (preview deployments) |
| `GITHUB_OAUTH_ALLOWED_ORIGIN_SUFFIXES` | none | Comma-separated host suffixes the relay may forward to. Required when `GITHUB_OAUTH_CALLBACK_BASE` is set. |

## Running locally

```bash
just editor-api       # cargo run -p regelrecht-editor-api, no login
just editor-sso       # same, with a real Keycloak login from .env.sso-local
```

`STATIC_DIR` defaults to `static`, relative to `packages/`. The `just` recipes that run this API locally (`editor-api`, `editor-sso`, `dev`) set `CORPUS_REGISTRY_PATH` to the repository's `corpus-registry.yaml` and `STATIC_DIR` to `frontend/public`, where `favorites.json` lives, so a local run loads the same sources and favorites as the image. For frontend work, `just dev editor` starts this API with a Vite dev server in front of it, logged in against the central Keycloak (it needs `.env.sso-local`). `just dev all` adds a local harvester-admin on port 8001 and points `HARVEST_ADMIN_URL` at it; without it the Corpusinwinning screens answer 503. See [Editor](./frontend).

## Further reading

- [Editor](./frontend): the frontend this API backs
- [Harvester Admin](./admin): the API behind the `/api/harvest-admin` proxy
- [Corpus Library](./corpus): how the corpus is loaded
- [Auth and roles](/auth-and-roles/): the role model
