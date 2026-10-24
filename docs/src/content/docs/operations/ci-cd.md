---
title: "CI/CD Pipeline"
description: "What runs on every push and pull request, and how only the checks relevant to changed files run."
---

Continuous integration runs on every push to `main` and every pull request via `.github/workflows/ci.yml`. Only checks relevant to changed files run, so CI stays fast.

## What CI checks

### Code quality (on Rust/YAML changes)

- **Formatting** - `just format` (rustfmt check)
- **Linting** - `just lint` (clippy)
- **YAML validation** - yamllint + schema validation on corpus files
- **Pre-commit hooks** - trailing whitespace, end-of-file, merge conflicts,
  and licence information for every file (`reuse lint`, from `REUSE.toml`)

### Tests (on Rust changes)

CI runs the Rust tests as a two-leg matrix, each leg a single cargo
invocation. The `unit` leg runs `just test-no-docker`: every crate in the
workspace except `regelrecht-pipeline`, `regelrecht-editor-api` and
`regelrecht-admin`, which are excluded whole. The `db` leg runs `just test-db`,
all tests of exactly those three crates, against a PostgreSQL service container that the
job starts next to itself (`TEST_POSTGRES_URL`). A new crate lands in the `unit`
leg without anyone having to add it to a list.

Locally, `just test` runs both halves in one go and starts its own PostgreSQL
through testcontainers, so it needs Docker. `just test-no-docker` is the same
coverage minus those three crates. `just check` runs the full `test`.

The BDD suite (`just bdd`, cucumber-rs with Gherkin scenarios) covers two
buckets and is **not** part of `just test`; the target carries `test = false` so
it only runs when called by name. `BDD_BUCKET` picks the bucket: `all` (the
default, what `just bdd` runs), `corpus` or `conformance`.

### BDD conformance and demo (on relevant changes)

The **BDD conformance** job runs bucket B (`bdd/conformance/*.feature` against
the synthetic `test_*` laws) as `BDD_BUCKET=conformance cargo test --test bdd`.
It hangs on the `Test` gate, so it blocks a merge. That bucket proves the engine
speaks the whole feature language and depends on nothing outside the repo.

The **BDD demo** job runs bucket A over `corpus/demo/regulation`, with
`REGULATION_PATH` pointing at it. It runs as a matrix of two legs, each on its
own runner. The `forward` leg first validates every law in that corpus against
the schema. It then runs the scenarios with the actions in file order, followed
by the Awb lifecycle test over the same corpus. The `reversed` leg runs the same
scenarios with the actions of every article in reverse order
(`BDD_REVERSE_ACTIONS=1`). The order in the file must never change an outcome.
Locally, `just validate-demo` and `just bdd-demo` do what the `forward` leg
does. `just bdd-reversed` first runs both buckets over `corpus/regulation` in
reverse, then the `reversed` leg. The demo corpus and its synthetic persona data
live entirely in the repository, so this job also hangs on the `Test` gate and
blocks a merge when either leg fails.

Bucket A over the live corpus (`corpus/regulation/**/scenarios/*.feature`) stays out of CI. It asserts
what the live laws currently produce, so a failure there means a law changed or a
scenario went stale; a human decides what that is worth. Run it locally with
`BDD_BUCKET=corpus`.

### Walkthrough replay (on demo changes)

The **Rondleiding speelt terug** job checks that the recorded walkthrough
(`/rondleiding`, see [the demo page](/components/demo)) still replays in the demo
as it is now. The walkthrough is the presenter's voice plus a log of what the
presenter clicked, typed and scrolled; a change to the demo can leave one of those actions
with nothing to act on (a renamed button, a moved panel, a law that is gone), and
the walkthrough would then show something other than what the voice says.

The job downloads the browser engine from the **WASM Build** job, starts the
demo's dev server, fetches only the voice from the walkthrough's release, and
replays every chapter in a headless Chromium in two window sizes. Dutch plays in
both sizes, every other language only in the first. CI spreads these runs over
three parallel jobs with `--shard K/3`. An action whose element is not found
fails the job, with an annotation naming the chapter; an action that only finds
its element with the numbers in its description masked is reported as a
warning, since it may now point at something else. It runs when `frontend-demo/`,
`corpus/demo/`, the engine or the schema changes, hangs on the `Test` gate, and
is the command you run locally, `just walkthrough verify`, with `--shard` added.
A failure is fixed in the demo, or by recording that chapter again.

### The browser jobs run in the Playwright image

**E2E (mocked)**, **Rondleiding speelt terug** and the **Docs accessibility
gate** run inside the
[official Playwright image](https://mcr.microsoft.com/en-us/artifact/mar/playwright),
not directly on the runner. The image ships
Chromium with every system library it needs, so these jobs no longer run
`playwright install --with-deps`. That step spent its time in `apt-get update`
on the runner and varied from ten seconds to more than nine minutes.

The image's apt sources point at the same Azure Ubuntu mirror the runners use.
On a bad day that mirror serves packages at a few kilobytes a second, and
installing a C compiler once took 22 minutes in the merge queue, past the
queue's check timeout. The engine is therefore not built in the image at all.
E2E and the walkthrough replay download it from the **WASM Build** job (see
[below](#wasm-build-on-engine-changes)), so neither needs Rust or a C compiler.
The accessibility gate installs its two packages from pinned `.deb` files, as
described below. No job in `ci.yml` runs `apt-get`, and the `playwright-image`
pre-commit hook fails when one does.

The image is pinned by tag and digest in `ci.yml`. A Playwright release looks
for exactly the browser build it shipped with, so the tag must equal the
Playwright version that both `package-lock.json` and `docs/package-lock.json`
resolve. Dependabot bumps the lockfiles and not the image, so after a
Playwright bump the `playwright-image` pre-commit hook
(`script/playwright-image.test.mjs`) turns the **Pre-commit** check red until
someone moves the tag and the digest along. The digest is the
`Docker-Content-Digest` header of the image index:

```bash
curl -sI -H 'Accept: application/vnd.oci.image.index.v1+json' \
  https://mcr.microsoft.com/v2/playwright/manifests/v<version>-noble \
  | grep -i docker-content-digest
```

Without that `Accept` header the registry answers 404.

A step without a `shell:` runs under `bash -e` on the runner, but under `sh -e`
in a container. The three jobs set `shell: bash` as their default, which also
turns on `pipefail`.

The container runs as root, and Git refuses a checkout owned by another user.
The accessibility gate marks the workspace as a safe directory before its
`git diff`. pa11y starts its Chrome with `--no-sandbox`, which Chrome needs when
it runs as root.

The gate installs two packages the image lacks. Puppeteer needs `unzip` to
unpack pa11y's Chrome. Without the DejaVu fonts, `monospace` resolves to a CJK
font with wider glyphs. Code blocks then overflow, and axe reports them as
scrollable regions without keyboard access.

It installs them without apt, to stay off the mirror described above.
`script/install-a11y-debs.sh` downloads the three `.deb` files apt would pick
(`unzip`, `fonts-dejavu-core` and its dependency `fonts-dejavu-mono`) from
Launchpad, which keeps every published version. It checks each file against a
SHA-256 in the script, stops on a mismatch, and installs them with `dpkg -i`.
The result is the same files as `apt-get install`, fontconfig configuration
included, in about two seconds. A newer Playwright image may need newer
versions; the script says how to look them up.

The preview server that pa11y reads from binds to `127.0.0.1` (`npm run a11y`
in `docs/package.json`), and the gate's page list uses the same address. Left
at `localhost`, the server listened on `::1` in the container while `wait-on`
polled `127.0.0.1`, and the gate timed out before it checked a page.

### WASM build (on engine changes)

The **WASM Build** job runs `just wasm-build` on the runner: a release build of
the engine with the `wasm` feature, `wasm-bindgen --target web`, and the copies
for the demo and the docs. It uploads the result as the `wasm-engine`
artifact, with the same paths a local build writes. E2E and the walkthrough
replay need this job and download the artifact into their checkout, so the
engine they run is the one built here. The job also builds the chronolexografie
poc for wasm32, without uploading it.

It builds whenever the `ci` or the `walkthrough` change group matches (see
[change detection](#change-detection)); `walkthrough` covers the demo, its
corpus, the engine, the schema and the shared frontend code. When it fails, the
two jobs that need it are skipped rather than failed, and the `Test` gate would
read that skip as a pass. The gate therefore waits for **WASM Build** as well,
even though it is a required check of its own.

### Licence and supply-chain audit (always runs)

- **Rust** - `cargo-deny bans licenses sources`: banned crates, licence compliance, and an allowlist of dependency sources
- **Frontend** - `license-checker` over the npm workspace at the repo root, refusing the copyleft licences the project cannot ship

Neither scans for known vulnerabilities. That is `just audit-advisories` (RustSec plus npm advisories), which runs periodically from `security-advisories.yml` rather than on every push.

### Engine release build (on a tag, and monthly)

`release-engine.yml` builds the `evaluate` binary for Linux and both macOS architectures and publishes them as a GitHub release when an `engine-v*` tag is pushed. Between releases it also builds, without publishing, on the first of every month and on a pull request that changes the workflow itself. A route that only runs on a tag says nothing until the next release, so a broken one used to surface only when someone needed it.

### Schema protection (on PRs)

Released schema versions in `schema/v*.*.*` are immutable. CI fails if a PR tries to modify or delete a released schema. `schema/latest` is a symlink to the highest released version (currently `v0.8.11`), not a directory to edit; a schema change goes into a new version directory.

### Provenance checks (on corpus/engine changes)

Among other checks, the `provenance-checks` job verifies that every `$schema` URL in `corpus/regulation`, the reductions included, names a schema version that exists under `schema/`, that every version directory under `schema/` is registered in `packages/engine/src/schema.rs`, and that `schema/latest` points at the highest version (currently `v0.8.11`). It does not check the form of the URL: a tag-based `refs/tags/schema-vX.Y.Z` reference is the convention, but a branch-based URL that names a known version passes. On a pull request that changes engine source it also compares the version in `packages/engine/Cargo.toml` with `main`; an unchanged version only prints a warning. See [RFC-013](/rfcs/rfc-013) for context.

### Docs accessibility gate (on docs changes)

The **Docs accessibility gate** builds the docs site and runs pa11y-ci, with
HTML_CodeSniffer and axe-core, over the pages it produced (`just docs-a11y`). At
about 2.5 seconds a page the full site takes several minutes on one runner, so
the job is a matrix of three shards. Each shard runs on its own runner, in the
Playwright image described above, with its own Chromium.
`scripts/gen-pa11yci.mjs` reads `A11Y_SHARD=K/N` and keeps every N-th page
starting at the K-th. Running the pages in parallel inside one job was tried and
dropped: pa11y-ci then shares one Chromium, and most pages failed on a protocol
error instead of on a finding.

On a pull request and in the merge queue the gate checks only the pages a
change touches, plus the index pages above them. That holds only while every
changed file is a page under `docs/src/content/docs/` or
`docs/src/content/rfcs/`. Any other file in the diff, a layout or a workflow
file for instance, sends the gate over the whole site, because it may change
every page. A push to `main` always checks the whole site.

Every shard builds the site and repeats the source checks in `npm run a11y`.
Only the pa11y run is split. The check that no published URL disappeared runs
in the first shard only.

### Other jobs behind the `Test` check

The required `Test` check is a gate job that fails when any job it depends on fails. Besides the Rust tests, the BDD jobs, the provenance checks and the docs accessibility gate, it waits for:

- **Cross-law integrity** - `script/cross-law-integriteit.py` over the corpus: misplaced, dangling or plain-param source bindings and broken `implements` declarations
- **Frontend tests** - unit tests for the editor, lawmaking, demo, arch-explorer and shared packages, type checks of the shared package, lawmaking, the editor and the demo, bundle builds for the editor, lawmaking and demo, a check that the BDD grammar codegen is in sync, and the design-system slot guard (`just nldd-slots`), which fails the job when the design system is missing instead of skipping
- **Ontwerpsysteem-imports** - checks that the design-system imports match the tags in use
- **E2E (mocked)** - the Playwright suite against mocked backends (`npm run test:e2e -w frontend`, which is what `just test-e2e` runs after building the engine), plus the poc tests that run the engine
- **Rust image build** - builds the `pipeline-api` image to catch Dockerfile breakage
- **Docs source checks** - `npm run check:source` in `docs/`: cited paths, Gherkin steps, prose style, RFC coverage and numbering, the schema version and declared imports, without an Astro build

A skipped job counts as passed; only the `changes` job itself must succeed.

## Change detection

CI uses path filters to determine which checks to run:

| Change group | Triggers on changes to |
|---|---|
| `ci` | `packages/`, `frontend/`, `corpus/regulation/`, `corpus/cells/`, `corpus/demo/`, `bdd/`, `schema/`, `script/`, the PoC trees, and the root build files (`Justfile`, `package.json`, `rust-toolchain.toml`) |
| `docs` | `docs/`, plus the corpus and shared frontend code the landing page reads at build time |
| `docs-source` | `docs/`, `packages/`, `bdd/`, `schema/`, `corpus/regulation/`, `justfile`, `.claude/skills/docs-writing/`, `.github/workflows/ci.yml` |
| `rust-image` | `packages/`, `schema/`, `rust-toolchain.toml`, `.github/workflows/ci.yml` |

The `ci` group includes `frontend/`, so frontend changes also trigger the Rust checks (the editor is shipped as one image built from `frontend/` plus the `editor-api` Rust binary that serves it). Docs-only changes skip the Rust checks and run the docs accessibility gate (`just docs-a11y`), the docs source checks and the design-system import check.

## Merge gates

Passing the checks above is not enough on its own. Four gates weigh in on whether a pull request can merge. Only **Claude review completed** is a required status check on `main`; the other three report and are read by a human.

| Gate | What it requires |
|------|------------------|
| **Werkpakket genoemd** | The PR body ends with a `Werkpakket:` line naming a werkpakket from the roadmap. `geen` is allowed with a reason. See [Contributing](./contributing). |
| **Claude review completed** | The automated review ran to completion for this commit and left no finding marked Critical. There is no override: fix the finding and push again. |
| **Security update approved** | A Dependabot security update needs an approving review from someone with write access, on the commit being merged. Security updates skip the five-day cooldown, so a human looks at them instead. |
| **Mutation Testing (diff)** | Reports on whether the tests pin the behavior the change introduces. |

The full list of required checks on `main` is `Pre-commit`, `WASM Build`, `Protect schema versions`, `Security Audit`, `Test`, `Validate PR title` and `Claude review completed`. That list lives in the branch protection, not in a file; `script/merge-queue-checks.test.mjs` keeps a hand-maintained copy of it (see [the merge queue](#the-merge-queue)).

### The Claude review gate

`claude-code-review.yml` posts its findings as review comments and takes about ten minutes, roughly twice as long as the other required checks. Merging on green used to be possible before the review had said anything. The `review-gate` job (check name **Claude review completed**) closes that window. It waits for the `claude-review` job inside its own workflow run and passes only once that job is `completed` with `success` or `neutral`. When that job will not deliver, the gate can take over a review started by hand instead (see [Starting a review by hand](#starting-a-review-by-hand)).

It looks the job up by run id rather than by head SHA. `ready_for_review` and `reopened` keep the same SHA, and a SHA lookup can then read a leftover check-run from the previous run. It also queries with `filter=all`: the run id survives a "Re-run this job", and `claude-review` is then absent from the newest attempt's job list.

The wait is driven by the job's `status`, never by the number of review comments. Zero comments fits "still running" as well as "the review had nothing to report". The gate does read the comments, but only for what they say, and only after it has established that the review ran to completion. That order is enforced: `assert_no_critical_finding` blocks with an internal-error message if it is reached before `assert_review_ran` has set `review_proven`, and the test suite asserts that the comment endpoints stay untouched on every path where the review is unproven.

#### When the review does not apply

Whether a run reviews at all is decided by the job **Resolve the commit to review** (`target`), from the pull request as the API describes it when the run starts. The event payload is not good enough for that. On PR 1801 a push and "ready for review" came ten seconds apart. The `synchronize` payload still said `draft: true`, so that run skipped the review. The `ready_for_review` payload still carried the commit from before the push. That run reviewed the old commit until the other run cancelled it. `target` skips a run whose commit is no longer the head of the PR; that run's gate reports on the old commit, and the gate on the new head judges the new one. When `target` cannot read the PR it fails rather than skips, so an API error does not pass for a draft.

The gate no longer calls a draft "not applicable" on one reading. The flag has to hold for 30 seconds (`DRAFT_SETTLE_SECONDS`), and a PR that turns ready in that time is judged like any other. Usually the `ready_for_review` run then reviews the same commit, and the push run's gate waits for that review and takes it over (see [When no review is coming](#when-no-review-is-coming)). In the order in which 1801 went wrong, the `ready_for_review` run carries the old commit and nothing reviews the head. The gate then blocks with instructions, where a single reading would have reported a green "not applicable" on an unreviewed commit.

Concurrency sits on the `claude-review` job alone, one group per pull request, and only a run that has something to review joins it. A newer push or a manual run replaces the review in progress; a run that skips cannot cancel anything. There is no concurrency on the workflow as a whole any more, because cancelling a whole run also cancelled its gate.

`claude-review` itself cannot be a required check. It does not run on cross-repo (fork) PRs, which have no secrets and therefore no `CLAUDE_CODE_OAUTH_TOKEN`, and a required check that never reports blocks such a PR forever. `review-gate` always runs and always reports. It derives "not applicable" from the PR as the API describes it (cross-repo, draft, Dependabot) rather than from the conclusion of `claude-review`, so a failed or skipped review can never pass as an inapplicable one.

Dependabot is a deliberate exemption. Those PRs go through `claude-dependabot.yml`, which decides for itself whether to merge; the gate does not verify that it ran.

#### A green job is not proof of a review

`claude-code-action` exits with conclusion `success` without reviewing anything when the workflow file differs from the version on the default branch ("Exiting due to workflow validation skip"). The gate therefore checks two things before it believes a green job.

First, it compares `.github/workflows/claude-code-review.yml` as this run has it with the copy on the default branch, through the contents API. If they differ there is no automatic review to wait for, and the gate blocks straight away, before the wait. That comparison is also what makes the rest trustworthy: identical files mean the workflow that ran is the one from the default branch, not something the PR brought along. A PR that edits this file needs a human review and an admin merge.

The comparison uses `refs/pull/N/merge`, not the head commit. A `pull_request` run executes the workflow file as it stands in the test merge of PR and base, so a branch that has fallen behind still runs the base's copy. Comparing against the head commit would put exactly those branches on red while their review ran fine.

Second, it reads the proof out of the review job. The step **Record that the review ran** carries `if: steps.claude-review.outputs.execution_file != ''`, and that output is only set once the Claude CLI has actually run. On a self-skip the step is `skipped`. The gate reads that step's conclusion from the `steps[]` array of the job it already looked up, so the proof is bound to this run and this attempt, and it exists just as well when the review had no findings. Right after a job completes, the jobs API can still return it without steps; the gate then keeps polling until the steps are there, rather than concluding that the proof step is missing. The gate insists on exactly one step by that name, because two would let an added decoy outvote the real one. Renaming the step means changing `PROOF_STEP` in the gate as well; the test suite asserts that the workflow still carries the step under that name with that `if:`, and the pre-commit hook runs on the workflow file for that reason.

#### A review that posted nothing

The proof step says the Claude CLI ran, not that anything reached the pull request. With a prompt set, the review action runs in its agent mode and posts nothing itself; whether a review appears is up to the model. On 23 October that went wrong on PRs 1778, 1779, 1798 and 1803. The review job was green within a minute, the proof step had run, and no comment by `claude[bot]` existed on the PR, while the gate reported "review afgerond". The cleanup step cannot have removed it: it deletes only what the snapshot recorded before the review started, so never a comment written during the run.

The prompt now says that the final answer goes nowhere and that the review must post a summary comment with `gh pr comment`, also when there is nothing to report. The gate holds it to that. After the proof step it counts the comments `claude[bot]` wrote with the line `<!-- claude-review -->` since the review job started, in the same three places it searches for Critical findings. With none, it blocks and asks you to rerun the whole run, or to set the PR to draft and back. A review without findings is still green, as long as it says so on the PR. The count has the same edge as the Critical check: anything tagged that `claude[bot]` wrote in the window counts, including a comment from a review that was being cancelled.

#### When no review is coming

A skipped job is `completed` and has no steps, ever. The gate used to read "no steps yet" as "just finished, steps still on their way" and waited out the full 35 minutes for them. It now tells the cases apart. The gate gives up on its own run's review when `claude-review` was skipped, failed or was cancelled, or is still missing after `target` has finished. It then looks for another review of the same commit: first another `pull_request` run of this workflow on that commit, then a manual run (next section). With neither, it blocks after two minutes (`NO_REVIEW_GRACE_SECONDS`). The message says what to do: start a review by hand and rerun the gate, or set the PR to draft and back to ready. The long wait stays for the normal case, where a review is queued or running. A failed API call does not count as "no review": the gate cannot tell that apart from a review it failed to see, so it keeps polling until the deadline.

Another `pull_request` run counts only when the PR's base is the default branch and the PR does not change the workflow file: its copy at the head commit must be the same blob as at the merge base. A `pull_request` run executes the workflow file from the test merge of that moment, which the gate cannot read back. When the PR leaves the file alone, every such test merge holds a default-branch version of it, and that is what makes the other run's proof step worth something.

#### Starting a review by hand

A rerun of a `pull_request` run keeps its original payload, and with it the commit it was started for. To review the current head of a PR, start the workflow by hand:

```bash
gh workflow run "Claude Code Review" -R NederlandseDigitaleDienst/regelrecht -f pr=<number>
gh run rerun <run id of the gate> --failed
```

The run executes the workflow file from the default branch. `target` looks up the PR's head commit through the API and refuses, with an error, a PR that is closed, in draft, from a fork, or from Dependabot. Those are the same rules as on a `pull_request` run, taken from the API and not from the input. The review job checks out that commit and restores `.claude/`, `CLAUDE.md` and the other configuration the CLI reads at startup from the default branch, which the review action itself only does on PR events. It also restores `AGENTS.md`, which `CLAUDE.md` imports, and `script/claude-review-comments.sh`, which the next steps execute.

A manual run runs on the default branch, so it has that branch's cache scope and OIDC subject, and the workflow must not execute code from the PR there. What it cannot stop is the review's own Bash tool working in a tree that holds the PR's files; that exposure is the same as on a `pull_request` run, but with the default branch's scope instead of the PR's.

The manual run joins the PR's concurrency group, so it replaces a review in progress and is replaced by the next push.

Such a run has no pull request in its payload, so its checks land on the default branch and its head SHA is a commit there. The gate of the PR therefore looks for it, but only when its own run's review will not deliver. It accepts a review job from a manual run only when all of the following hold:

- GitHub records the run as `workflow_dispatch` of `.github/workflows/claude-code-review.yml`, on the default branch of this repository.
- The workflow file at the run's own head SHA is the same blob as on the default branch now. The code that ran is then the reviewed copy, whatever ref name was used to start it.
- The review job is named `claude-review (PR #<number> @ <head SHA>)`, with this PR's number and the commit under review.

That name is the record of what the run reviewed. It comes from the workflow file alone, and the default-branch copy builds it from what `target` read from the API. Neither the PR, the person who started the run, nor the review itself can set a job name. From there the same proof step and the same Critical check apply as to the gate's own job, with the window measured from that job's `started_at`. Only the newest matching run counts. If its review failed or was cancelled, or its jobs cannot be read, the gate does not fall back on an older one. It keeps looking for two minutes, since a newer manual run may be replacing it. The run title (`Claude Code Review: PR #<number>`) is matched as well, but only to limit the number of API calls; it is set from the input and proves nothing.

#### What the gate trusts

The gate fetches the facts it relies on itself. The workflow that invokes it lives in the pull request, so anything that workflow passes in is written by the author of the change under review. Setting `IS_DRAFT: true` in the env block used to be enough to declare the review inapplicable. Draft status, fork status, author and the workflow file now all come from the API. The env block carries only the coordinates of the run (repository, run id, PR number), and the gate checks that the run and the PR describe the same commit, so pointing it at another PR's run yields a red.

The gate runs the copy of its own script from the base branch, not the one in the PR; otherwise a PR could turn the script into `exit 0` and be green by construction. If that copy cannot be fetched or checked out, the gate blocks rather than falling back to the PR's version.

What none of this reaches is the `review-gate` job itself, which lives in the workflow file the PR brings along. A PR that keeps the job name **Claude review completed** and replaces its steps with `run: true` reports green without running the script, and nothing inside the script can prevent that. Closing that hole takes a rule outside the pull request: a ruleset or CODEOWNERS entry over `.github/workflows/**`, or a gate triggered by `workflow_run` that judges the review run from the outside. Until then the gate protects against mistakes, not against someone determined to route around it.

Every red outcome states what was established and names no cause it has not tested. An earlier version told every clean PR that the review action had skipped itself over a workflow change, on PRs that changed no workflow at all ([issue #1178](https://github.com/NederlandseDigitaleDienst/regelrecht/issues/1178)).

#### Critical findings block

A finished review is not the same as an acceptable one. On 8 August 2026 the review put a Critical finding on PR 1234 at 10:11:31 and the PR merged at 10:11:54, green throughout, because the gate only asked whether a review had happened. It now searches what `claude[bot]` wrote for the exact string `🔴 **Critical**` and turns red on a hit, naming the comments it found it in. Three places count: the sticky comment, the inline comments, and the body of the submitted review. The last one is easy to forget, and in practice it carries findings that appear nowhere else.

Only what this run wrote counts, measured against the review job's `started_at`. An item by `claude[bot]` without a usable timestamp blocks rather than silently falling outside the window; a `PENDING` review is the exception, since it was never submitted. Anything older came from an earlier run. Cleanup removes those comments, but a submitted review cannot be deleted at all, and a cleanup that fails would otherwise keep blocking a finding that has long been fixed. Without a readable `started_at` the gate blocks, because it cannot tell the two apart. One edge the window does not cover: `cancel-in-progress` does not stop a review instantly, so a comment written by the review being cancelled can land inside its successor's window and turn that one red. The next push clears it.

Only Critical blocks. Significant carries "likely" in its own definition and would often be a false positive, and an exception that becomes routine stops holding anything back. There is no override: fix the finding and push again. Since the review rewrites its comment on every run, a finding that is gone is gone from the gate too. An escape hatch (a label, a magic comment, an environment variable) is something an agent operates as easily as a person, while it reads as a human judgement, so there is none; if a case ever arises that needs one, it gets decided then.

The marker lives in `CRITICAL_MARKER` and in the review prompt, and the test suite binds the two: change one without the other and the gate reads past every finding without anyone noticing. The marker is free text written by a language model, so the prompt states that a machine parses it and that it must be written exactly as given, and nowhere else. That cannot be closed completely.

#### Findings cannot be pushed away

Blocking on findings only works if a finding cannot be made to disappear. The review job used to delete every `claude[bot]` comment at the start of its run, so a crash after the deletion took the previous finding with it, and a rerun that happened not to notice it again turned red into green. The order is now reversed: `script/claude-review-comments.sh snapshot` records the ids and texts before the review, and `clean-up` removes them afterwards, under the same `execution_file` condition as the proof step. Cleanup deletes only what is still there with an unchanged `updated_at`. `use_sticky_comment` makes the action reuse the previous comment, so its id is in the snapshot, and deleting by id alone would wipe the review that just ran.

Snapshot and cleanup select comments by the line `<!-- claude-review -->`, which the prompt tells the review to end every comment with. The author alone is not enough: `claude.yml` answers `@claude` in the same thread as the same bot, and that conversation is neither a finding to hand to the next review nor something to delete. Cleanup never fails the step. A failed list call counts as a failed cleanup and is reported in the step summary; failing there would put `claude-review` on `failure` and make the gate report that no usable review exists, for a review that ran fine.

The snapshotted texts go into the prompt as context, with the instruction that they describe an earlier version of the diff and that each one is to be re-checked against the current one: nothing carried over that has been fixed, nothing dropped that still stands. Without that, the gate would reward pushing until the review forgets, which is worse than no gate. On the way in the severity markers are rewritten to `[Critical]` and similar, so a review that quotes an old finding to say it has been fixed does not trip the gate with the quote.

The gate needs `issues: read` on top of `pull-requests: read`, because the summary comment is an issue comment and that endpoint sits behind the issues API.

In short, the gate proves that the review ran to completion for this commit and that it left no Critical finding standing. It says nothing about findings of lower severity, whether the findings hold up, or whether they were addressed.

The logic lives in `script/await-claude-review.sh` and `script/claude-review-comments.sh`. Each has a test suite next to it, using a `gh` stub, that covers every path deciding green versus red, or what is kept and what is thrown away. Both run as pre-commit hooks.

### The security-update gate

Dependabot security updates bypass the five-day `cooldown` in `.github/dependabot.yml`: they arrive the moment the fixed version is published. The check **Security update approved** (`.github/workflows/security-update-gate.yml`) turns green on such a PR only once an engineer with write access has approved it.

It is a check rather than branch protection because branch protection offers no other way to express this. `required_approving_review_count` applies to a branch, not to an author or a label, and ruleset conditions only select refs. Turning it on repository-wide would mean nobody could merge their own PR, because GitHub does not let an author approve their own change. The condition therefore lives in the check, as it does for **Claude review completed**.

No API field says whether a PR is a security update. The gate reads three independent signals: an open Dependabot alert for exactly the package the PR bumps, Dependabot's own line "This update includes a security fix", and a GHSA or CVE identifier in the opening of the PR body, up to the first `<details>` block (after that come quoted changelogs, which may just as well be about another package). One signal is enough. A false positive costs an approval that was not needed; a false negative lets a security patch through unseen, so the gate leans toward the former.

An approval counts only on the commit it was given on. `dismiss_stale_reviews` only works when `required_approving_review_count` is above 0, and here it is 0, so the script does the binding itself: after a rebase or a push nothing is approved any more. Approvals from bots do not count, nor do approvals from someone without write access (an `author_association` other than OWNER, MEMBER or COLLABORATOR).

`claude-dependabot.yml` uses the same script to decide whether a PR is a security update and, if so, no longer merges it itself. The review still runs on it, as input for the engineer who approves. That workflow does not enforce anything; the check does.

There is no minimum age. The age threshold exists because nobody looks at a routine bump, and the approval is exactly what removes that concern. Adding it on top would leave an actively exploited vulnerability waiting five days plus the wait for a human. The publication date of the new version is one of the things the approver looks at, not something the gate decides for them.

The Mattermost notification lives in the same workflow and fires when a security PR is opened and when it is merged, through `MATTERMOST_WEBHOOK_URL`. Dependabot starts that run, and in that case `secrets.*` reads from the Dependabot secret store instead of the Actions one. A webhook URL stored only under Actions would make the notification silently disappear, so the step fails hard on an empty URL.

The logic lives in `script/require-security-approval.sh`, with `script/require-security-approval.test.sh` (a `gh` stub) covering every path that decides green or red; the tests run as a pre-commit hook. The gate runs the copy of the script from the base branch, not the one in the pull request. If the base branch has no copy, that is not a read error: it is the pull request that introduces or removes the gate, so nothing on `main` is protected by this check yet, and the job says so and passes. The same limitation as for the review gate applies: the job itself sits in the workflow file the PR brings along.

## The merge queue

`main` merges through a merge queue. Rather than merging your branch directly, GitHub builds a branch of its own (`gh-readonly-queue/main/...`) holding `main` plus the pull requests waiting in line, and runs the checks there. That is what keeps a set of individually green pull requests from landing as a broken `main`, and it removes the rebase-and-wait cycle that a busy repository otherwise runs on. It also means the required checks have to report on that queue branch, not only on the pull request, and most of what follows comes from that.

### Putting a pull request in the queue

```bash
gh pr merge <nr> -R NederlandseDigitaleDienst/regelrecht --squash
```

gh replies "The merge strategy for main is set by the merge queue". That is a notice, not an error. gh refuses `--delete-branch` while the queue is on, and it is not needed: the repository deletes the head branch itself after the merge.

A command that succeeded does not mean the pull request is in the queue. Ask:

```bash
gh api graphql -f query='{repository(owner:"NederlandseDigitaleDienst",name:"regelrecht"){
  pullRequest(number:<nr>){state mergeQueueEntry{state position}}}}'
```

A pull request that drops out of the queue stays `OPEN`, so waiting for the state to become something other than `OPEN` waits forever after a drop. Follow `mergeQueueEntry` instead: `MERGED` means done, and `OPEN` with `mergeQueueEntry: null` means it dropped out. The reason is in the timeline:

```bash
gh api graphql -f query='{repository(owner:"NederlandseDigitaleDienst",name:"regelrecht"){
  pullRequest(number:<nr>){timelineItems(last:5,itemTypes:[REMOVED_FROM_MERGE_QUEUE_EVENT]){
  nodes{... on RemovedFromMergeQueueEvent{createdAt reason}}}}}}'
```

### When something fails in the queue

A failure in the queue does not show on the pull request. Its checks stay green; the red run belongs to the queue branch. This finds it:

```bash
gh run list -R NederlandseDigitaleDienst/regelrecht --event merge_group --limit 20 \
  --json databaseId,headBranch,workflowName,conclusion \
  --jq '.[] | select(.headBranch | test("pr-<nr>-"))'
```

When a test fails in the queue that the pull request does not touch, while the run on the pull request and the latest run on `main` are green, it is usually a flaky test or a clash with another pull request in the same group. Queue it once more. If it drops out a second time on the same failure it is no longer chance: find the cause instead of queueing it again. A flaky test found this way gets an issue, because every drop costs a full round for everyone behind it.

### Required checks in the queue

The queue has no list of required checks of its own; it uses the list from the branch protection. A check that never reports on the queue branch stays on "Expected" and holds its entry until the status check timeout drops it. That happens quietly, because the message is on the queue branch rather than a red check on the pull request, so it only shows when nothing merges any more. `gh run list --branch 'gh-readonly-queue/main/...'` shows what happened there.

Skipped is not the same as missing. A job skipped by an `if:` reports `skipped`, which GitHub counts as passing. Only a workflow that does not run at all reports nothing. A required job may therefore skip in the queue; its workflow must not drop out.

`Protect schema versions` uses that difference deliberately and skips in the queue. It compares against `origin/main`, and in the queue `HEAD` is not one pull request but the whole group: if one of them touches a released schema version the whole group fails and takes the others with it. The same check already catches that on each pull request.

`Validate PR title` and `Claude review completed` are the other way around: they can only exist on a pull request, since one reads the title and the other reads the review comments, the body and `refs/pull/N/merge`. In the queue both come from `.github/workflows/merge-queue-gates.yml`. What that workflow establishes is that both gates were green at the moment the pull request joined the queue, and nothing more. A finding that arrives after joining no longer stops anything. With "Merge when ready", GitHub joins the queue by itself as soon as the last gate turns green, so no human sits in between at that point.

`script/merge-queue-checks.test.mjs` ties every required check to a job that runs in the queue, with the one deliberate skip listed explicitly. The hook runs on every workflow, because any workflow can carry or lose such a check. Its `REQUIRED_CHECKS` list is maintained by hand, and the two directions are not symmetric: a name too many fails loudly, a name too few is a silent loss of coverage. The branch protection changes with a button rather than a commit, so no review reminds anyone to update that list.

### Branch protection settings

"Require branches to be up to date before merging" is off, and should stay off. It demands that a pull request first be updated to the tip of `main`, which is exactly the manual work the queue takes over. With it on, everyone still rebases by hand, CI runs again afterwards, and the queue only adds an extra run per merge without the benefit. The setting exists in no file, only in the branch protection.

Every merge runs CI twice: once on the queue branch and once on the push to `main` that follows. For a group of several pull requests that gets cheaper per pull request as long as the group passes; if it fails, the work is lost and the rebuilt group builds again. The build concurrency in the queue settings limits that.

Turning the queue off is a single switch in the branch protection. The `merge_group` triggers and `merge-queue-gates.yml` are then never triggered and cost nothing, so no revert is needed. `enforce_admins` is on, so there is no way around a stuck entry: take it out of the queue first, then merge, then switch the queue back on.

## Further reading

- [Contributing](./contributing) - the PR process, including the required trailer
- [Deployment](./deployment) - what happens after CI passes
- [Testing](/guide/testing) - how to run tests locally
