---
title: "Development Environment"
description: "How the local stack runs: PostgreSQL in Docker, the backends natively, and the frontends on Vite with hot reload."
---

## Architecture

`just dev` runs PostgreSQL in Docker and the application services natively.
It starts only what the chosen app needs:

| `just dev …` | Browser | Backend | PostgreSQL |
|---|---|---|---|
| `all` (default) | editor `:7300`, lawmaking `:7500` | editor-api `:8000`, admin API `:8001` | yes |
| `editor` | editor `:7300` | editor-api `:8000` | yes |
| `admin` | none (API only) | admin API `:8000` | yes |
| `lawmaking` | lawmaking `:7500` | none | no |

The browser only talks to Vite. Vite serves the frontend with hot module
replacement and proxies `/api`, `/auth` and `/health` to editor-api. The admin
API has no UI of its own: its dashboard is the editor's Corpusinwinning
section, which reaches it through editor-api's `/api/harvest-admin/*` proxy.
That section only works in `all`, where editor-api's `HARVEST_ADMIN_URL` points
at the local admin API; with `editor` alone those screens answer 503.

## One-Time Setup (build speed)

[Getting Started](./getting-started) lists the prerequisites. Then, once per
machine after cloning:

```bash
just dev-setup
```

It installs `sccache`, plus the [mold](https://github.com/rui314/mold) linker
on x86_64 Linux (through apt, dnf or Homebrew, whichever it finds), and points every git
worktree at a single shared cargo `target-dir` so a new worktree reuses the
already-built dependency graph instead of cold-building from scratch. That
setting lands in a gitignored `.cargo/config.toml` at the root of the main
checkout.

When the repo is on a slow mount (9p/NFS/SMB, e.g. a WSL2 or Docker-Desktop
dev container backed by a Windows drive), it relocates that target dir to fast
local storage under `~/.cache/regelrecht/`, which is usually the biggest
build-time win. `sccache` is installed but left off locally (it disables
incremental compilation, which hurts the hot-reload loop); CI uses both.

Sharing that target dir has a cost when two worktrees build at once. Cargo
locks a target dir exclusively for the length of a build, so the second one
waits: `just validate` measured 2 seconds alone and 38 seconds next to a
45-second `just lint` in another worktree. Sharing still wins by a wide margin
when one build runs at a time (a first `just build-check` in a fresh worktree
took 1 second shared and 170 seconds with its own target dir). A worktree about
to run long builds can step out of the queue with `just target-isolated`, and
`just target-shared` puts it back. The measurements are at the top of
`script/target-dir.sh`.

sccache does not give you both. It hashes the working directory, so two
worktrees on different paths share no Rust compilation at all: a cold
`just build-check` with a warm cache gave 480 misses and 0 hits.

mold is the configured linker on x86_64 Linux (`packages/.cargo/config.toml`),
so builds there fail to link without it. On that platform `just dev` refuses to
start a Rust service when mold is missing. On macOS and aarch64 Linux cargo uses the default linker, and
neither `just dev-setup` nor the dev recipes ask for mold.

## Starting the Dev Stack

```bash
just dev             # everything (default)
just dev editor      # just the editor
just dev admin       # just the harvester-admin API, e.g. for pipeline work
just dev lawmaking   # just the lawmaking UI (no backend)
just dev-down        # stop it
```

The recipe checks prerequisites (docker, plus cargo and node where the app
needs them, and mold on x86_64 Linux), starts PostgreSQL and waits for it,
builds the engine WASM for the editor, installs frontend dependencies when they
are missing, and starts the services in the background. `just dev-frontend` is
an alias, from when the recipe had that name.

Notes:

- **Backends run once** via `cargo run`, not `cargo watch`; Vite keeps HMR for
  the frontend. After a backend change, stop and start the stack. Restarts after
  the first build are near-instant because the Rust artifacts are reused (see
  [One-Time Setup](#one-time-setup-build-speed)). `cargo watch` is not used
  because its recursive watch hangs on a 9p-mounted worktree.
- **The editor uses real SSO** against the central Keycloak, so `all` and
  `editor` need `.env.sso-local` (copy `.env.sso-local.example` and fill in the
  values, see [Auth and roles](/auth-and-roles/)). The default ports `7300` and `7500` are the redirect URIs already registered
  on the `regelrecht-local` Keycloak client. Override them with `EDITOR_PORT` /
  `LAWMAKING_PORT`.
- In a dev container where the native backend can't reach Postgres on
  `localhost`, set `DB_HOST=host.docker.internal` in `.env` for the admin API;
  editor-api takes that host from `DATABASE_URL` in `.env.sso-local`.
- Prometheus and Grafana are not part of `just dev`. See
  [Grafana](/components/grafana/#running-locally) to run them.

## Stopping

```bash
just dev-down
```

## Logs

```bash
tail -f .dev-admin.log           # Admin (harvester) API log
tail -f .dev-editor-api.log      # editor-api log
tail -f .dev-editor.log          # Editor Vite log
tail -f .dev-lawmaking.log       # Lawmaking Vite log
just dev-logs                    # PostgreSQL log
```

## Database Access

```bash
just dev-psql
```

## Full Docker Stack

For running everything in Docker without hot reload:

```bash
just local          # Start
just local-down     # Stop
just local-logs     # Logs
just local-psql     # Database access
```

## Environment Variables

Create a `.env` file in the project root:

```bash
# Optional overrides
POSTGRES_PORT=5433
DB_HOST=localhost
EDITOR_PORT=7300
LAWMAKING_PORT=7500
RUST_LOG=info
```

### Logging

Six binaries read these variables: editor-api, admin, poc-portal, and the three
pipeline binaries (harvest worker, enrich worker, pipeline API). The harvester CLI builds
its own subscriber and reads only `RUST_LOG`.

| Variable | Values | Default | Effect |
|---|---|---|---|
| `RUST_LOG` | `tracing` filter | `info` (harvester CLI: `warn`) | Which events are emitted |
| `LOG_FORMAT` | `text` (`plain`), `json` | `text` | Output format |
| `LOG_SPAN_EVENTS` | `none`, `close`, `new`, `active`, `full` | per service: `close` for editor-api, `none` elsewhere | Per-span timing lines |

`LOG_FORMAT=json` writes one JSON object per event. The event's own fields are
flattened to the top level; the enclosing spans are added as nested `span` and
`spans` keys, so a log backend can search per field. Set it per deployment in
ZAD; locally the text lines read better, so leave the variable unset. An
unrecognized value falls back to text and warns on stderr, so a typo never
silences logging.

## Architecture Explorer

`just arch-explore` builds and starts a local explorer of the codebase on port 7180 (override with `ARCH_EXPLORE_PORT`). It renders a model of the Rust workspace and the Vue frontends, from crate down to method and from app down to component, with the dependencies between them. The model comes from `packages/arch-extract/`, a developer tool that is not deployed. It is generated from the working tree on demand and never committed, so it cannot go stale; `just arch-generate` writes it to disk for inspection. `packages/arch-extract/README.md` explains how the edges are resolved and what the explorer misses.

## Pre-commit Hooks

Install [pre-commit](https://pre-commit.com/) (for example with
`uv tool install pre-commit`), then register the hooks in your clone:

```bash
pre-commit install --hook-type pre-commit --hook-type commit-msg
```

The `commit-msg` type matters. The Conventional Commits check on the commit
message runs at that stage, and a plain `pre-commit install` registers only the
`pre-commit` stage, so that check would never run locally.

On commit the hooks run, each only when a matching file changed:

- Trailing whitespace, end-of-file, merge-conflict and large-file checks
- YAML linting (yamllint, config in `.yamllint`)
- Rust formatting (`just format`) and clippy (`just lint`)
- Schema validation of ruleworks (`just validate`): the corpus, the chronolexografie PoC and the fixtures of the cell's BDD suite
- Licence information (`reuse lint`): every file needs a licence, set in
  `REUSE.toml` with the licence texts in `LICENSES/`. Code and text get the
  default by file type. An image, a font or a file type the list does not name
  fails the hook until it has its own entry with its rightsholder
- The test suites of the CI scripts and merge gates under `script/`, when that
  script or its workflow changed

`.pre-commit-config.yaml` has the full list. What to do when a hook fails is on
[Contributing](/operations/contributing#pre-commit-hooks).
