---
title: "Pipeline"
description: "A PostgreSQL-backed job queue that orchestrates and tracks the law-processing workflow."
---

The pipeline is a PostgreSQL-backed job queue and law status tracking system that orchestrates the law processing workflow.

## Overview

- **Language**: Rust
- **Location**: `packages/pipeline/`
- **Database**: PostgreSQL
- **Key feature**: Reliable concurrent job processing with `FOR UPDATE SKIP LOCKED`

## Architecture

The pipeline coordinates two processing stages: **harvesting** (downloading laws from wetten.nl) and **enrichment** (adding machine-readable logic via LLM).

```mermaid
flowchart LR
    subgraph Pipeline
        Q[Job Queue]
        S[Law Status Tracker]
    end
    subgraph Workers
        HW[Harvest Worker]
        EW[Enrich Worker]
    end
    BWB[BWB / wetten.nl] -->|XML| HW
    HW -->|YAML| Corpus[Corpus Juris]
    HW -->|claim/complete| Q
    EW -->|claim/complete| Q
    Q --> S
    LLM[LLM Provider] -->|machine_readable| EW
    EW -->|enriched YAML| Corpus
```

## Modules

| Module | Purpose |
|--------|---------|
| `job_queue.rs` | Job creation, claiming (`FOR UPDATE SKIP LOCKED`), completion, failure with auto-retry |
| `law_status.rs` | Per-law status tracking through 11 states |
| `harvest.rs` | Harvest execution: download XML from BWB, convert to YAML |
| `harvest_request.rs` | The one entry point for "request a harvest", shared by the pipeline API and the admin API |
| `traject_harvest.rs` | Harvest of a law for one traject, delivered as a review task instead of written to the corpus |
| `enrich.rs` | Enrichment execution: call the LLM to add `machine_readable` sections |
| `enrich_v2/` | The model-free parts of enrichment: checks, capability plan, reference graph, closing pass |
| `document_convert.rs` | Uploaded document (docx, PDF and others) to a markdown werkdocument |
| `law_convert.rs` | Uploaded PDF or Word document to a base rulework, followed by a task-flow enrich |
| `law_migrate.rs` | Lift a version of a rulework to schema v0.7.1 (structurally v0.7.0); a version at v0.8.0 or later is left alone |
| `markings.rs`, `untranslatables.rs` | Persist the markings and untranslatables the enrichment agent reports |
| `tasks.rs` | Personal review tasks that tie a finished job to the account that requested it |
| `feature_flags.rs` | Read and write the shared `feature_flags` table |
| `worker.rs` | Polling loops for the harvest and enrich workers |
| `health.rs` | The small HTTP health endpoint inside each worker |
| `api/` | Handlers of the `pipeline-api` service |
| `models.rs` | Data types: `Job`, `LawEntry`, `JobType`, `JobStatus`, `LawStatusValue`, `Priority` |
| `config.rs` | Configuration from environment variables |
| `db.rs` | Connection pool creation and migration runner |
| `error.rs` | Error types (`PipelineError`) |

## Binaries

| Binary | Source | What it is |
|--------|--------|------------|
| `regelrecht-harvest-worker` | `src/bin/harvest_worker.rs` | Claims `harvest` and `traject_harvest` jobs |
| `regelrecht-enrich-worker` | `src/bin/enrich_worker.rs` | Claims `enrich`, `document_convert` and `law_convert` jobs, which share the LLM CLI environment and the hourly budget |
| `regelrecht-pipeline-api` | `src/bin/pipeline_api.rs` | Internal HTTP service, see [Pipeline API](#pipeline-api) |
| `law-check` | `src/bin/law_check.rs` | Runs the deterministic enrichment checks over ruleworks, without database, git or model. Exits 1 on schema errors; `--strict` fails on every finding; `--corpus` adds the cross-law binding check |
| `law-source` | `src/bin/law_source.rs` | Compares a rulework's text with the official BWB toestand. Exits 1 when an article drifts, is missing or is fabricated. `--rewrite` replaces the text with the official one and keeps `machine_readable` per article |
| `law-migrate` | `src/bin/law_migrate.rs` | Lifts versions of ruleworks to schema v0.7.1 (the v0.7.0 shape; v0.7.1 changed only descriptions) and validates the result. A version at v0.8.0 or later stays as it is and is validated against its own schema. A required field it cannot fill is reported, never guessed; it writes only with `--write` |
| `enrich-once` | `src/bin/enrich_once.rs` | Runs the real enrichment loop against a directory on disk, without database or git, so a worker change can be tried on one law locally |

Run the four tools from `packages/` with
`cargo run -p regelrecht-pipeline --bin <name> -- <args>`. The usage of each is
in the doc comment at the top of its source file.

## Job Lifecycle

```mermaid
stateDiagram-v2
    [*] --> Pending: create_job
    Pending --> Processing: claim_job (FOR UPDATE SKIP LOCKED)
    Processing --> Completed: complete_job
    Processing --> Pending: fail_job (retries left)
    Processing --> Failed: fail_job (max attempts reached)
    Processing --> Pending: reap_orphaned_jobs (heartbeat stale)
    Processing --> Failed: reap_orphaned_jobs (no retries)
```

Workers claim jobs atomically using PostgreSQL's `FOR UPDATE SKIP LOCKED`, so multiple workers can safely process jobs concurrently without blocking each other.

### Automatic Retries

When a job fails and has attempts remaining (`attempts < max_attempts`), it returns to `Pending` for retry. Default `max_attempts` is 3.

### Orphan Reaping

A worker that holds a job records a heartbeat on it every 30 seconds
(`heartbeat_at`), from the moment it claims the job until it is done. A job in
`Processing` whose last heartbeat is older than the orphan timeout (default 5
minutes) is reset to `Pending` or marked `Failed`. That is how a crashed or
restarted worker is handled.

How long a job has been running does not count: a worker that keeps beating
keeps its job, and the job timeout (`WORKER_JOB_TIMEOUT_SECS`) decides when it
gives up. The heartbeat itself stops 10 minutes after the job timeout, so a
worker that is alive but stuck (a hung `git push`) loses its job to the reaper
instead of holding it until the pod restarts. A database outage longer than
the orphan timeout also costs the jobs in flight, because no heartbeat lands
while the database is down. Before the heartbeat, the
reaper measured from `started_at`, so a run still working past the orphan
timeout was put back to `Pending` and its result refused when it finished.

## Law Status Tracking

Each law in the corpus progresses through processing states:

```mermaid
stateDiagram-v2
    [*] --> Unknown
    Unknown --> Queued: harvest job created
    Queued --> Harvesting: worker claims job
    Harvesting --> Harvested: harvest succeeds
    Harvesting --> HarvestFailed: job retries exhausted
    HarvestFailed --> Harvesting: re-queued (fail count below threshold)
    HarvestFailed --> HarvestExhausted: fail count reaches threshold
    Harvested --> Enriching: enrich job claimed
    Enriching --> Enriched: enrichment succeeds
    Enriching --> EnrichFailed: job retries exhausted
    EnrichFailed --> Enriching: re-queued (fail count below threshold)
    EnrichFailed --> EnrichExhausted: fail count reaches threshold
    Harvesting --> NotHarvestable: no consolidated text
    NotHarvestable --> [*]
```

`NotHarvestable` is the terminal one. A work can have no consolidated text to
harvest because it was withdrawn, is not yet in force, or has only been
announced. The skip reason is uniform, so the status is a single value and the
precise reason and date go into the harvest job's result. The job is completed
rather than failed, so it is never retried; a future law can be re-harvested by
hand once its text appears.

## Pipeline API

Besides the workers, the crate builds one HTTP service, `pipeline-api` (`src/bin/pipeline_api.rs`, image `regelrecht-pipeline-api`). It has no public address and no authentication of its own. The editor API forwards `/api/harvest/*` to it (see `PIPELINE_API_URL` on [Editor API](./editor-api)) and does the auth checks in front of it.

| Route | Purpose |
|-------|---------|
| `POST /harvest` | Create a harvest job for one law |
| `POST /harvest/batch` | Create harvest jobs for several laws |
| `GET /harvest/status` | Job and law status |
| `GET /harvest/search` | Search BWB for a law to harvest |
| `GET /health` | Liveness |

## Harvest Worker

The harvest worker:
1. Polls the queue for pending harvest jobs
2. Downloads law XML from BWB (wetten.nl)
3. Converts XML to YAML via the harvester library
4. Writes YAML to the corpus
5. Creates enrich jobs for each LLM provider, but only when `ENRICH_AUTO_ENQUEUE` is on (it is off by default) and the law is not `enrich_exhausted`
6. Creates follow-up harvest jobs for the laws the harvested text references

Two mechanisms pull in further laws, each with its own limit:

- **References in the text.** Step 6 follows the external references the
  harvester finds in the XML. A follow-up job carries its parent's depth plus
  one, and the chain stops at `MAX_HARVEST_DEPTH` (1000, in `harvest.rs`). That
  constant guards against runaway recursion and is not a tuning knob; the chain
  normally ends because every referenced law is already harvested or queued.
- **Related legislation from enrichment.** After an enrich job, the related
  legislation the agent reports (delegated regelingen, cross-law sources, legal
  bases the harvester misses) is harvested only while the enrich job's depth is
  below `RELATED_HARVEST_MAX_DEPTH` (default 2). This is the limit that bounds
  how far one enrichment can grow the corpus.

## Enrich Worker

The enrich worker:
1. Polls the queue for pending enrich jobs, within the hourly budget (`ENRICH_HOURLY_LIMIT`)
2. Spawns an LLM CLI process to generate `machine_readable` sections
3. Writes progress to the job every 10 seconds (see [Progress and stalls](#progress-and-stalls))
4. Computes coverage score (the stored `law_entries.coverage_score` is the cumulative fraction of articles with `machine_readable`; the per-run delta rides in the job result)
5. Pushes the result to a per-provider branch (`enrich/` followed by the provider name, `opencode` or `claude`) for review

The hourly budget fails closed. Without `ENRICH_HOURLY_LIMIT`, or with `0`, the
worker enriches nothing, so a forgotten variable cannot spend a subscription on
the whole corpus. The cap counts runs for the worker's own `LLM_PROVIDER` per
clock hour (Europe/Amsterdam), and because it is counted in the `jobs` table it
survives a restart.

### Large laws and agent sessions

A large law does not fit in one agent session. The worker splits it into
windows of at most `ENRICH_MAX_ARTICLES_PER_RUN` articles (default 15) and owns
the cursor itself: it lives in `.enrichment.yaml` on the `enrich/{provider}`
branch, each chunk pushes its own result, and the next chunk is queued in the
same transaction that completes the current one. A law of N articles is done in
at most `ceil(N / 15)` successful runs, whatever the model does.

Task-flow enrichments (`deliver: task`) walk the same windows but push nothing.
Each window is its own job with its own review tasks. The next job takes the
current job's result as its input blobs: the proposal, so the next window
builds on what this one enriched, and `.enrichment.yaml`, which carries the
cursor. A window's review tasks cover the articles that window changed. The
last window is wider: its closing reconcile pass and binding check run over the
whole law, so it can also carry tasks for articles from earlier windows.

A new law (`new_law`, from an upload or a harvest into a traject) is created as
a whole, so its windows produce no review task along the way. Only the last
window does, with the complete law as one proposal. If a window fails for good,
the requester gets a failure task and the earlier windows' work is lost; the
law has to be uploaded or harvested again.

When an existing law takes more than one window, a window whose proposal cannot
be compared article by article fails and is retried, the last window included.
It does not fall back to one task for the whole law, because that proposal also
contains the earlier windows' articles, and approving it would silently restore
changes a reviewer had rejected.

Within one window, `ENRICH_SESSION_REUSE` decides how the translation pass and
the feedback rounds of the gates share an agent session: all of them (`window`,
the default), only the schema repair (`repair`), or none (`off`). It applies to
the claude provider and never crosses a window. Token use and cost of every
call land in the job result (`agent_calls`, `usage`), which is how the modes are
compared; `enrich-once --session-reuse` prints the same table locally.

The reasoning behind both, including why a window follows document order rather
than "the next articles without `machine_readable`", is in the doc comments on
`plan_chunk` and `SessionReuse` in `enrich.rs`.

### Progress and stalls

The claude provider runs with `--output-format stream-json`, so the worker
reads each event as it happens: assistant turns, tool calls, a partial event
per token, and the closing `result` with the token use. Every 10 seconds the
worker writes `jobs.progress` with two parts:

- at the top level, the note the agent writes before each step (`phase`,
  `step`, `total_steps`), as before;
- under `worker`, what the worker saw: the current call (`translate` or a
  gate), how long it has run, how long ago the agent last wrote anything, the
  number of turns and tool calls, the last few tool calls (`Edit
  laws/…/law.yaml`), and how many of the law's articles carry
  `machine_readable` so far. The tool summary names the tool and what it works
  on, never the text it writes.

The job detail sheet on the Harvester page shows this while a job runs, and
keeps the last state on a job that stopped. That is the place to look when a
run times out: the error itself also ends with a line on what the agent did
last. When the claude CLI exits with an error, such as an API call that kept
failing, the error also carries the reason the CLI gave in its closing
`result` event. The CLI writes nothing to stderr in that case, so without it
the job would only say `exit status: 1`.

A claude call that writes nothing for `ENRICH_AGENT_IDLE_SECS` (default 600)
is stopped as stalled, and fails like a timeout, with the same retries. With
partial events on, a long answer arrives as a stream of events, so silence
means no token at all. The limit is generous because an API retry after an
overload is silent for a while too. `0` turns the check off. The check leaves
opencode alone, because it writes an event per finished part and can be quiet
for longer, and it leaves the document and law conversions alone, because a
converter run through Bash is silent while it works.

With `ENRICH_WINDOW_CONCURRENCY` above 1 the windows of a job share one view
of the agent: the counts add up, and the stall check fires only once every
window has gone quiet.

### Definitions and units

The `law-generate` skill writes every constant in `definitions` in the
structured form, with a `type` and, for a quantity, a `type_spec.unit`
(RFC-023). Money is `type: amount` with `unit: eurocent`.

The deterministic check `definition-type` reports a definition that holds a
number without a `type`, because nothing then says whether 0.137 is a share or
a factor, or whether 3971900 is cents. Its findings go back to the agent as a
feedback round.

### LLM Providers

`LLM_PROVIDER` selects `opencode` (the default) or `claude`. The binary and model
come from `OPENCODE_PATH` / `OPENCODE_MODEL` or `CLAUDE_PATH` / `CLAUDE_MODEL`,
with `LLM_PATH` and `LLM_MODEL` as the fallback for either.

The LLM subprocess runs with a stripped environment (allowlisted vars only) for security.

The image pins the claude CLI (`packages/pipeline/Dockerfile`, version
2.1.303). The CLI is a native binary: the image installs the platform package
`@anthropic-ai/claude-code-linux-x64-musl` and links its binary as `claude`,
because the wrapper package `@anthropic-ai/claude-code` only puts the binary in
place from a postinstall script, which the install skips.

Without `CLAUDE_MODEL` and `LLM_EFFORT` the CLI chooses for itself, and the
stripped environment keeps its own variables (`ANTHROPIC_MODEL`,
`CLAUDE_CODE_EFFORT_LEVEL`) out. Which model that is depends on the CLI
version and the subscription behind the token; 2.1.303 started without a token
names Opus 5.5. The effort it picks also depends on a server-side setting, and
the CLI reports it nowhere. Set both on the enrich worker to know what runs.
The CLI does report the model, in the `init` event at the start of the
stream-json output. The worker records it: per call in the job result's
`agent_calls`, as `models` on the result, as `model` in `.enrichment.yaml`
(which said `default` when no model was set), and in the progress the
Harvester page shows while a job runs.

## Configuration

### Database and workers

| Variable | Default | Purpose |
|----------|---------|---------|
| `DATABASE_URL` | required | PostgreSQL connection string; `DATABASE_SERVER_FULL` is read as fallback |
| `DATABASE_MAX_CONNECTIONS` | 5 | Connection pool size |
| `REGULATION_REPO_PATH` | `./regulation-repo` | Local checkout the workers write to |
| `REGULATION_OUTPUT_BASE` | `regulation/nl` | Directory inside that checkout where harvested laws land |
| `WORKER_POLL_INTERVAL_SECS` | 5 | Queue poll interval |
| `WORKER_MAX_POLL_INTERVAL_SECS` | 60 | Max backoff interval |
| `WORKER_JOB_TIMEOUT_SECS` | 1200 (20 min) | Job execution timeout |
| `WORKER_ORPHAN_TIMEOUT_SECS` | 300 (5 min) | How long a processing job may go without a heartbeat before it is reaped; at least 120 |
| `WORKER_MAX_CONSECUTIVE_RESOURCE_FAILURES` | 5 | Consecutive fork or out-of-memory failures after which the worker exits, so the platform restarts it with a clean process table |
| `EXHAUSTED_THRESHOLD` | 10 | Consecutive failures after which a law becomes `harvest_exhausted` or `enrich_exhausted` |
| `RELATED_HARVEST_MAX_DEPTH` | 2 | Depth up to which related legislation from enrichment is harvested |
| `HEALTH_PORT` | 8000 | Port of the worker's health endpoint |
| `PORT` | 8000 | Listen port of `pipeline-api` |

The corpus checkout is configured by the [corpus library](./corpus)
(`packages/corpus/src/config.rs`). Without `CORPUS_REPO_URL` the workers run
without one.

| Variable | Default | Purpose |
|----------|---------|---------|
| `CORPUS_REPO_URL` | none | Corpus repository to clone and push to |
| `CORPUS_REPO_PATH` | `/tmp/corpus-repo` | Where the clone lives |
| `CORPUS_BRANCH` | derived | Branch to use; a preview deployment derives its own from `HOSTNAME` / `DEPLOYMENT_NAME` |
| `CORPUS_GIT_TOKEN` | none | Token for the central corpus repository only, never for a traject repository |
| `CORPUS_GIT_AUTHOR_NAME`, `CORPUS_GIT_AUTHOR_EMAIL` | `regelrecht-harvester`, `noreply@minbzk.nl` | Commit author |

### Enrichment

| Variable | Default | Purpose |
|----------|---------|---------|
| `ENRICH_HOURLY_LIMIT` | 0 (paused) | Enrich runs per clock hour for this worker's provider; must be set for the worker to enrich at all |
| `ENRICH_NIGHT_MULTIPLIER` | 1 | Multiplier on the hourly limit between 00:00 and 08:00 (Europe/Amsterdam) |
| `ENRICH_AUTO_ENQUEUE` | off | `true` (or `1`, `yes`, `on`) makes a completed harvest queue enrich jobs; otherwise enrichment is requested explicitly through `POST /api/enrich-jobs` on the [Harvester Admin](./admin) API |
| `LLM_PROVIDER` | `opencode` | `opencode` or `claude` |
| `LLM_PATH`, `LLM_MODEL` | none | Fallback binary and model for either provider |
| `OPENCODE_PATH`, `OPENCODE_MODEL` | `opencode`, provider default | Binary and model for opencode |
| `CLAUDE_PATH`, `CLAUDE_MODEL` | `claude`, provider default | Binary and model for claude |
| `LLM_EFFORT` | none | Reasoning effort passed to the provider (`claude --effort`) |
| `LLM_TIMEOUT_SECS` | 600 (10 min) | Ceiling per agent call |
| `CLAUDE_CODE_OAUTH_TOKEN` | none | Subscription token for the claude provider; several comma-separated tokens are rotated over time |
| `SKILLS_DIR` | `/opt/skills` | Skills baked into the image, linked into the checkout when the directory exists |
| `ENRICH_MAX_ARTICLES_PER_RUN` | 15 | Max articles per enrich run (chunked enrichment); `0` disables chunking |
| `ENRICH_FEEDBACK_ROUNDS` | 1 | Feedback rounds per gate; `2` or `checks=2,marking=3` |
| `ENRICH_SESSION_REUSE` | `window` | Session sharing within one window: `window`, `repair` or `off` |
| `ENRICH_STEPS` | every step | Which steps of the chain to run, e.g. `reconcile` for the closing pass alone |
| `ENRICH_WINDOW_MODE` | `entries` | What a window is: `entries` counts entries, `layers` uses the dependency layers of RFC-033 |
| `ENRICH_WINDOW_CONCURRENCY` | 1 | Windows run side by side, each in its own copy of the checkout and its own agent session |
| `ENRICH_CONTEXT_BRIEF` | on | `0` withholds the context brief the worker writes beside the law |
| `ENRICH_MAX_RSS_MB` | 3500 | Memory ceiling for the agent subprocess |
| `ENRICH_AGENT_IDLE_SECS` | 600 | Stop a claude call that writes nothing for this long; `0` turns it off |
| `CODE_COMMIT` | empty | Commit of the running image, recorded in the enrichment metadata |

`LLM_TIMEOUT_SECS` is a ceiling per agent call, and one run makes several: a
translation pass, a feedback round per gate, the closing pass and the final
schema gate. The worker lowers it when the job budget cannot hold that many,
so raising `LLM_TIMEOUT_SECS` without raising `WORKER_JOB_TIMEOUT_SECS` buys
nothing.

The shares are not equal. The translation pass writes every
`machine_readable` in its window and is capped at three shares of the job
budget, less a 30 s reserve; each feedback round the run may make is capped at
one, counted from `ENRICH_FEEDBACK_ROUNDS`. A lower `LLM_TIMEOUT_SECS` stays as
it is. With `WORKER_JOB_TIMEOUT_SECS=3900` and one round per gate the caps are
1290 s for the translation and 430 s per round, where an even split gave each
call about 550 s. More rounds per gate make every share smaller.

## Database Schema

The pipeline crate owns the migrations (`packages/pipeline/migrations/`) for the
whole platform database, so the editor API's tables (accounts, trajects, notes,
settings, feature flags) are created here too. The two the pipeline itself runs
on:

**`jobs`** - Job queue with retry tracking, priority ordering, and JSONB payload/result/progress columns. Partial index `WHERE status = 'pending'` for efficient claiming.

**`law_entries`** - Per-law status tracking with foreign keys to harvest/enrich jobs and a coverage score (0.0–1.0).

Next to those, `tasks` and `job_blobs` carry review tasks and their payloads,
and `markings` and `untranslatables` hold what enrichment reported.

Migrations run automatically at startup using an advisory lock for coordination.

## Testing

```bash
just pipeline-test               # Unit tests (no Docker)
just pipeline-integration-test   # Integration tests (Docker + testcontainers)
```

Integration tests use `testcontainers` to spin up ephemeral PostgreSQL instances; no local database setup is required.

## Further reading

- [Harvester](./harvester) - the BWB law downloader used by harvest jobs
- [Architecture](/guide/architecture) - where the pipeline fits in the system
