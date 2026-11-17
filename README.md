# regelrecht

[![CI](https://github.com/NederlandseDigitaleDienst/regelrecht/actions/workflows/ci.yml/badge.svg)](https://github.com/NederlandseDigitaleDienst/regelrecht/actions/workflows/ci.yml)
[![License: EUPL-1.2](https://img.shields.io/badge/License-EUPL--1.2-blue.svg)](https://opensource.org/licenses/EUPL-1.2)
[![Rust 2024](https://img.shields.io/badge/Rust-2024_edition-orange.svg)](https://doc.rust-lang.org/edition-guide/rust-2024/)
[![Mutation Testing](https://github.com/NederlandseDigitaleDienst/regelrecht/actions/workflows/mutation-testing.yml/badge.svg)](https://github.com/NederlandseDigitaleDienst/regelrecht/actions/workflows/mutation-testing.yml)
[![Docs](https://img.shields.io/badge/docs-regelrecht-green.svg)](https://docs.regelrecht.rijks.app)

Machine-readable Dutch law execution. regelrecht takes legal texts, encodes them as structured YAML, and runs them as deterministic decision logic.

## What does it do

- The engine takes a regulation and a set of inputs, evaluates the decision logic, and returns a result with a full explanation trail
- Laws are tested against real-world scenarios using BDD (Gherkin) tests, many derived from legislative explanatory memoranda. The vocabulary those scenarios speak lives in `bdd/grammar.yaml` and every engine's step bindings are generated from it
- A harvester downloads and tracks Dutch legislation from the official BWB repository
- Regulations can be edited through a web UI with live execution preview

## Components

### Rust packages

| Package | Description |
|---------|-------------|
| [packages/engine/](packages/engine/) | Law execution engine (also compiles to WASM) |
| [packages/law-model/](packages/law-model/) | The Rust representation of the law format, conforming to `schema/` |
| [packages/harvester/](packages/harvester/) | Downloads Dutch legislation from BWB |
| [packages/pipeline/](packages/pipeline/) | PostgreSQL job queue for law processing |
| [packages/admin/](packages/admin/) | Harvester-admin API; its dashboard UI lives in the editor |
| [packages/editor-api/](packages/editor-api/) | Backend API for the law editor |
| [packages/corpus/](packages/corpus/) | Git integration for the regulation corpus |
| [packages/auth/](packages/auth/) | Shared OIDC/SSO authentication |
| [packages/github/](packages/github/) | Shared GitHub REST client |
| [packages/shared/](packages/shared/) | Shared domain types across crates |
| [packages/arch-extract/](packages/arch-extract/) | Derives the architecture model from the code (`just arch-explore`) |
| [packages/tui/](packages/tui/) | Terminal dashboard (Ratatui) |

### Frontends and sites

| Directory | Description |
|-----------|-------------|
| [frontend/](frontend/) | Law editor UI (Vue 3 + Vite) |
| [frontend-lawmaking/](frontend-lawmaking/) | Law-making process visualization (Vue 3 + Vite) |
| [packages/frontend-shared/](packages/frontend-shared/) | Shared frontend primitives (auth, colour scheme, API fetch) |
| [docs/](docs/) | Astro site: landing page + documentation |

### Data and testing

| Directory | Description |
|-----------|-------------|
| [corpus/regulation/](corpus/regulation/) | A working set of ruleworks, some of them synthetic (`test_*`) |
| [schema/](schema/) | Versioned JSON schema for the law format (`schema/latest` points at the current version) |
| [bdd/](bdd/) | The Gherkin vocabulary (`grammar.yaml`) and the engine-conformance suite |
| `corpus/regulation/**/scenarios/` | Law-validation scenarios, next to the law they test |
| [packages/grafana/](packages/grafana/) | Grafana monitoring dashboards (provisioning, not a crate) |

### The full corpus lives elsewhere

`corpus/regulation/` in this repo holds a couple of dozen laws: the ones the
tests and BDD scenarios run against, plus synthetic `test_*` laws for the
conformance suite. The harvested body of Dutch legislation is far too large to
carry here and lives in its own repository:

**[MinBZK/regelrecht-corpus](https://github.com/MinBZK/regelrecht-corpus)** — currently some 22,000 YAML files under `regulation/nl`.

Which branch you want depends on what you are after:

| Branch | What is on it |
|--------|---------------|
| `development` | The working branch, and what `corpus-registry.yaml` points this repo at. Harvested laws land here first |
| `main` | Only moves when `development` is merged into it, so it lags |
| `enrich/*` | Output of the enrichment runs, one branch per enricher (`enrich/claude`, `enrich/opencode`) |

Short-lived `prN` and `editor/*` branches belong to preview deployments and
editor sessions; they are not meant to be read directly.

## Deployed services

| Service | URL |
|---------|-----|
| Editor | https://editor.regelrecht.rijks.app |
| Landing page | https://regelrecht.rijks.app |
| Documentation | https://docs.regelrecht.rijks.app |
| Law-making | https://lawmaking.regelrecht.rijks.app |
| Harvester admin | https://harvester-admin.regelrecht.rijks.app |
| Grafana | https://grafana.regelrecht.rijks.app |

The pipeline API and the harvester and enrich workers deploy alongside these but have no web UI of their own.

A PR gets a preview environment once it carries the `deploy:preview` label; taking the label off, or closing the PR, cleans it up again.

## Getting started

Prerequisites: [Rust](https://rustup.rs/) (`rustup` installs the version pinned in
`rust-toolchain.toml`) and [just](https://github.com/casey/just). The dev stacks
below also need Docker and Node.js, and on x86_64 Linux [mold](https://github.com/rui314/mold).

```bash
just check           # everything CI runs (format, lint, build, validate, tests)
just test            # every Rust test in the workspace
just test-no-docker  # the same, minus the container-backed suites
just bdd             # the BDD suites: law scenarios and engine conformance
```

`just test` and `just check` start PostgreSQL containers for the crates that
need one, so they want a Docker daemon. Without one, `just test-no-docker`
covers everything else. `just bdd` runs separately: it executes against the real
corpus and is deliberately not part of `just test`.

### Faster builds (recommended)

Run `just dev-setup` once. It does three things:

1. **Points every worktree at one shared target dir** — a new worktree reuses
   the already-built dependency graph instead of cold-building ~600 crates.
2. **Puts that target on fast local storage when the repo lives on a slow mount.**
   A Rust build writes tens of thousands of small files; on a 9p/NFS/SMB mount
   (e.g. a WSL2 or Docker-Desktop dev container where the repo is a Windows
   drive) that I/O dominates build time — often more than everything else
   combined. `dev-setup` detects this and relocates the target to
   `~/.cache/regelrecht/` (local disk). This is usually the single biggest win.
3. **Installs [mold](https://github.com/rui314/mold) + `sccache`.** mold (wired
   into `packages/.cargo/config.toml`) speeds up linking and is a hard
   requirement for the dev recipes; `sccache` is installed but left off locally.

`sccache` is disabled locally because it requires `CARGO_INCREMENTAL=0` and so
disables incremental compilation, which slows every rebuild after an edit.
Enable it only for cold or flag-varying builds:

```bash
export RUSTC_WRAPPER=sccache CARGO_INCREMENTAL=0
```

CI uses both mold and sccache (see `.github/workflows/ci.yml`).

### Running a dev stack

```bash
just dev             # editor (7300) + lawmaking (7500), editor-api on 8000, admin API on 8001
just dev editor      # just the editor (editor-api + editor UI + DB)
just dev admin       # just the admin API + DB
just dev lawmaking   # just the lawmaking UI (no backend)
just dev-down        # stop it
```

`just dev` starts only what the chosen app needs: no Grafana, Prometheus or
workers. The editor runs with real SSO against the central Keycloak, so it
needs `.env.sso-local` (copy `.env.sso-local.example`).

Vite ports default to `7300/7500` (overridable via `EDITOR_PORT` /
`LAWMAKING_PORT`). When a native backend can't reach Postgres
on `localhost` (e.g. a WSL2/Docker-Desktop dev container, where Postgres is
published on the Docker host), point it at `host.docker.internal`: for the
admin API set `DB_HOST=host.docker.internal` in `.env`; for the
editor that host comes from `DATABASE_URL` in `.env.sso-local` (the
`.env.sso-local.example` already uses `host.docker.internal`).

See the [docs site](https://docs.regelrecht.rijks.app) for full development instructions.

## License

[EUPL-1.2](LICENSE)
