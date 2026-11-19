---
title: "Testing"
description: "Which kinds of test RegelRecht has, where each one lives, the recipe that runs it locally and the CI job that runs it on a pull request."
---

The repository has about twenty kinds of test, and the kind decides who acts on a failure. A red engine test is a bug in the engine. A red scenario next to a law means the law or the scenario changed, and someone has to judge which.

## Which test goes where

The table covers this repository. The CI column names the job; what each job does is on [CI/CD](/operations/ci-cd).

| Kind | Where it lives | Run it locally | In CI |
|------|----------------|----------------|-------|
| Law validation (BDD bucket A) | `corpus/regulation/**/scenarios/*.feature` | `BDD_BUCKET=corpus just bdd` | No, on purpose |
| Engine conformance (BDD bucket B) | `bdd/conformance/*.feature` | `BDD_BUCKET=conformance just bdd` | BDD conformance |
| The cell's BDD suite and its constructs | `packages/cel/bdd/` | `just bdd` | BDD conformance |
| Demo scenarios | `corpus/demo/regulation/**/scenarios/` | `just bdd-demo` | BDD demo |
| Scenarios of the chronolexografie PoC | `packages/poc-chronolexografie/corpus/` | `just bdd-chrono` | BDD demo |
| Scenarios of the other PoCs (JavaScript) | `frontend-poc-*/tests/scenarios.test.js` | `npm test -w frontend-poc-<slug>` | E2E (mocked) |
| Rust unit and integration tests | `packages/*/src/`, `packages/*/tests/` | `just test` | Rust tests |
| Golden fixtures | `packages/engine/tests/golden_tests.rs` | `just test` | Rust tests |
| Trace snapshots | `packages/engine/tests/trace_integration.rs` | `just test` | Rust tests |
| Corpus type check | `packages/engine/tests/typecheck_corpus.rs` | `just test` | Rust tests |
| Schema conformance | `packages/engine/tests/schema_conformance.rs` | `just schema-conformance` | Rust tests |
| Schema validation | the corpora | `just validate` | Pre-commit |
| Script tests | `script/*.test.sh`, `script/*.test.mjs` | `just check` | Pre-commit |
| Frontend unit tests and type checks | `frontend*/src/`, `packages/frontend-shared/` | `just frontend-test` | Frontend tests |
| Editor end to end, mocked | `frontend/e2e/` | `just test-e2e` | E2E (mocked) |
| Walkthrough pipeline | `script/walkthrough/tests/` | `just walkthrough test` | Frontend tests |
| Walkthrough replay | `frontend-demo/scripts/verify-walkthrough.mjs` | `just walkthrough verify` | Rondleiding speelt terug |
| Docs source checks | `docs/scripts/check-*.mjs` | `npm run check:source` in `docs/` | Docs source checks |
| Docs accessibility | `docs/scripts/`, pa11y | `just docs-a11y` | Docs accessibility gate |
| Smoketest of a deployed editor | `frontend/e2e-smoke/` | `just smoke-preview <pr>` | No, it needs a deployment |
| Mutation testing | `packages/engine` | `just mutants-diff` | Mutation Testing (diff) |
| Benchmarks | `packages/engine/benches/` | `just bench` | No |

## BDD scenarios

Most behavior is tested with Gherkin feature files, which the Rust engine runs through [cucumber-rs](https://github.com/cucumber-rs/cucumber).

### Two buckets, one language

Feature files live in two places, and the difference decides who fixes a failure:

- **Bucket A, law validation**: `corpus/regulation/**/scenarios/*.feature`, run against the real corpus. A failure means a law changed or the scenario went stale, and a human decides which. Scenarios tagged `@wip` are skipped.
- **Bucket B, engine conformance**: `bdd/conformance/*.feature`, tagged by tier, proving an engine speaks the whole language against synthetic `test_*` laws. CI blocks on this bucket.

The cell has a BDD suite of its own in `packages/cel/bdd/`, outside both buckets. Its steps record submissions, decisions and facts in a cell's chronicle, evaluate a reduction that plays the chronicle forward, and check what the engine did. A well-founded objection, for example, makes the engine play the chronicle again without the decision it revokes, while an unfounded one leaves the decision standing. They stay out of `bdd/grammar.yaml`, which is the contract every engine (Rust and JS) executes, because recording a chronicle is the cell's job and only the Rust engine plays one forward; the JS engines do not implement `projection`. `bdd/README.md` explains how that suite reuses the canonical steps and the generator, and why its runner sits in `packages/cel`.

### Trying out a construct

A construct of the law format that is hard to get right gets a directory of its own under `packages/cel/bdd/constructs/`. The first is the decision term of Awb 4:13, a hook on every application. A construct's features load only the ruleworks in its `regulation/` directory. For hooks that matters. A hook fires on every decision or submission its `applies_to` matches, in every loaded rulework, so loading the whole corpus would bring in hooks the construct is not about.

```text
packages/cel/bdd/constructs/beslistermijn/
  regulation/nl/wet/test_general_administrative_act/2026-01-01.yaml
  regulation/nl/wet/test_permit_act/2026-01-01.yaml
  regulation/nl/wet/test_permit_act_implements/2026-01-01.yaml
  regulation/nl/wet/test_permit_act_short_term/2026-01-01.yaml
  scenarios/beslistermijn.feature
```

In the `beslistermijn` construct, a mini-Awb (`test_general_administrative_act`, after Awb 4:2 and 4:13) meets three fictional permit acts. An application is recorded on the day it arrives, which is the moment Awb 4:13 counts from, and the decision follows on a later day. The scenarios check what the application holds, the last day of the term, and whether the decision came on or before it: well within the term, on the last day, one day late, and counted from receipt rather than from the date on the application. Two permit acts set a term of their own, and between them they show both routes to the last day, `decision_deadline`. `test_permit_act_short_term` sets two weeks with an override on `decision_deadline`. `test_permit_act_implements` sets one week by filling the open term `statutory_decision_term` of 4:13 with `implements`. Under the hook on its application, only that act fills it ([RFC-058](/rfcs/rfc-058)). In both cases the fallback of lid 2, `reasonable_decision_term_end`, stays eight weeks.

To add a construct:

1. Make `packages/cel/bdd/constructs/<construct>/` with a `regulation/` and a `scenarios/` directory. The runner fails when either is missing or when `scenarios/` holds no feature.
2. Put the fictional ruleworks in `regulation/`, in the layout of the corpus. Give each an `$id` starting with `test_`, mark the text FICTIEF, and invent the content: no real case, no real person. A general law the construct builds on, such as the Awb, is a mini-version holding only what the scenarios need. The construct then does not change when the corpus does. `just validate` and the pre-commit hook check the ruleworks against the schema.
3. Write the features in `scenarios/`, in English. A feature may use the canonical steps and the cell's own. A decision the cell takes (`the cell decides ...`) sets the result, so `Then output ... equals ...` reads what the decision and the hooks on it gave.
4. A scenario the engine cannot run yet gets `@wip` and a comment above it that names the missing piece and the branch or issue that builds it.

`just bdd` and the CI job "BDD conformance" run the constructs with the rest of the cell's suite; no recipe or workflow changes.

The runner reads the set from where a feature sits, whichever directory `<set>` is. A corpus with a `scenarios/` directory next to its `regulation/` directory would load by the same rule once its features are added to the list in `packages/cel/tests/bdd/main.rs`. Nothing does that yet. The engine's own runner does not look at where a feature sits and always loads the corpus.

### The step vocabulary is generated, not hand-written

`bdd/grammar.yaml` is the single source of truth for the step phrasings. The bindings for every engine are generated from it: Rust through `packages/engine/build.rs`, the editor and demo JavaScript through `bdd/codegen/gen-js.mjs`, which writes `packages/frontend-shared/src/gherkin/grammar.generated.js` (re-exported for the editor by `frontend/src/gherkin/`). Never hand-edit a generated file. Change `grammar.yaml` and run `just bdd-codegen`.

A step that is not in `grammar.yaml` does not exist. A minimal scenario:

```gherkin
Feature: Healthcare allowance

  Scenario: An adult with an active policy is entitled to zorgtoeslag
    Given the calculation date is "2025-01-01"
    Given law "zorgverzekeringswet" is loaded
    Given parameter "bsn" is "999993653"
    When I evaluate "heeft_recht_op_zorgtoeslag" of "wet_op_de_zorgtoeslag"
    Then the execution succeeds
    Then output "heeft_recht_op_zorgtoeslag" is true
    Then output "hoogte_zorgtoeslag" equals 157731
```

Laws that need source data (BRP, Belastingdienst, and the like) provide it with a data-table step keyed on the identifier the law looks up:

```gherkin
    Given the following "personal_data" data with key "bsn":
      | bsn       | geboortedatum | verblijfsadres |
      | 999993653 | 2005-01-01    | Amsterdam      |
```

See `corpus/regulation/nl/wet/wet_op_de_zorgtoeslag/scenarios/eligibility.feature` for a complete, data-driven example.

### Running the scenarios

```bash
just bdd                          # Both buckets, then the cell's own suite
BDD_BUCKET=conformance just bdd   # Engine conformance, then the cell's suite
BDD_BUCKET=corpus just bdd        # Law validation, then the cell's suite
just bdd-demo                     # Bucket A over the demo corpus
just bdd-chrono                   # Bucket A over the corpus of the chronolexografie PoC
just bdd-trace                    # With execution traces in trace_output/
```

`just bdd-chrono` runs bucket A over the corpus of the chronolexografie PoC, which keeps its own copies of the real ruleworks it uses so that work on the PoC leaves `corpus/regulation/` alone. Their scenarios went along with them. Two of those features, `criteria.feature` of the Regeling gehandicaptenparkeerkaart and `regio_en_vakanties.feature` of the Regeling vaststelling schoolvakanties, are still identical to the originals in the corpus, but they test a different version: the PoC's copy of the parking card regulation already makes a choice of its own on article 2. So both stay. The copy runs in CI because its data is invented and a failure is a real error; the original is bucket A and stays out.

The other PoCs with scenarios run them against the engine built to WASM instead, through `tests/scenarios.test.js` in their frontend. They are listed in `BDD_RUNNERS` in `script/orphan-tests.mjs`, which is how the orphan check knows their features are read.

### Deriving scenarios from legislative intent

Test scenarios are derived from the **Memorie van Toelichting** (MvT), the explanatory memorandum that accompanies Dutch legislation. The MvT contains examples and reasoning from the legislature that serve as ground truth for expected behavior.

## Rust tests

The Rust unit and integration tests, across every crate in the workspace:

```bash
just test           # Whole workspace (needs Docker)
just test-no-docker # Everything that needs no external services
just test-db        # Only the container-backed crates
```

A few crates also have a recipe of their own, such as `just pipeline-test` and `just pipeline-integration-test` (the second needs Docker); `just` lists them.

The tests of `packages/cel` and the reduction tests of the engine (`packages/engine/tests/reduction.rs`) share one set of fixtures, `packages/cel/tests/fixtures/corpus/`. A fixture there is YAML made for these tests and holds only what they read: the cells, and fictional rulework and reductions cut down to the articles and outputs a test exercises. Real laws such as the Awb and the Leerplichtwet are not copied in. The tests read them from `corpus/regulation/`.

### Golden fixtures and trace snapshots

`packages/engine/tests/golden_tests.rs` runs the JSON cases in `packages/engine/tests/fixtures/*.json`, each a law as YAML text with parameters and an expected result. [Conformance](/reference/conformance#golden-fixtures) describes their format and where they came from.

`packages/engine/tests/trace_integration.rs` evaluates the zorgtoeslag and compares the rendered trace with `expected_zorgtoeslag_trace.txt` and `expected_standaardpremie_trace.txt`, and checks what each step of the trace carries. It reads a frozen copy of the ruleworks it needs, in `packages/engine/tests/fixtures/trace_corpus/`, and not the corpus. A change to the zorgtoeslag in the corpus therefore does not turn this engine test red; that change is for the bucket A scenarios and a human. Only the ruleworks the evaluation reaches are in the copy, the Awb included because its hooks show in the trace. Adding another one can fire a hook and change the expected trace.

### The corpus type check

`packages/engine/tests/typecheck_corpus.rs` runs the static type check over the corpus, the demo corpus and the corpus of the chronolexografie PoC, and fails on any finding. Unlike the trace test it reads the live corpora on purpose: it guards the corpus, not the engine.

## Schema conformance

```bash
just schema-conformance
```

Checks that the Rust `law-model` accepts exactly what the JSON schema accepts. It is a different suite from engine conformance, BDD bucket B: that one runs scenarios, this one compares the model with the schema. `just test` includes it. What it covers, and what conformance does not cover yet, is on [Conformance](../reference/conformance); adding a fixture is described in `packages/engine/tests/schema_conformance/README.md`.

## Schema validation

Every rulework is validated against the JSON schema. Without arguments, `just validate` checks the corpus (`corpus/regulation`), the ruleworks and reductions of the chronolexografie PoC, and the fixtures of the cell's BDD suite (`packages/cel/bdd/fixtures`: its `regulation/` directory and the reductions of its cell), and the `regulation/` directory of each construct in `packages/cel/bdd/constructs`. It leaves the demo corpus out; `just validate-demo` covers that. The pre-commit hook runs the same recipe on the changed files in all of these directories, the demo corpus included, and CI runs the hooks over the whole tree.

```bash
just validate                     # Validate all
just validate path/to/law.yaml    # Validate a specific file
just validate-annotations         # Validate stand-off notes
just validate-demo                # Validate the demo corpus
```

## Frontend tests

`just frontend-test` runs the unit tests and type checks of the editor, lawmaking, the demo, the shared package and the chronolexografie PoC. `just test-e2e` builds the engine to WASM and drives the editor with Playwright against mocked backends. Where frontend code and its tests go is on [Frontend code](/guide/frontend-code).

`just smoke-preview` is the one test that runs against a deployed editor: a preview by its pull request number, or a URL. It logs in through the real Keycloak and needs the test user's credentials, so CI does not run it. [Deployment](/operations/deployment) covers previews and waking one up first.

## The demo

`just demo-check` checks the demo end to end: the laws, their scenarios, the frontend tests, and the WASM build. Run it before pushing anything demo-related.

The recorded walkthrough has two checks. `just walkthrough test` runs the pytest suite of the pipeline that cuts and subtitles the takes, in `script/walkthrough/tests/`. `just walkthrough verify` replays every chapter headless and names each recorded action that finds nothing on the page, which is the CI job "Rondleiding speelt terug". The route from recording to release is on [Demo](/components/demo#from-recording-to-release).

## Script tests

The scripts in `script/` that decide something in CI, such as a merge gate or the orphan check, have a test suite next to them: `script/<name>.test.sh` or `script/<name>.test.mjs`. A pre-commit hook runs each suite when the script or the workflow it guards changes, and a `just` recipe runs it by hand (`just ci-gate-test`, `just orphan-tests-test`). `just check` runs most of them; CI runs every hook in the Pre-commit job. Change a script that decides green or red together with its suite.

## Docs checks

`npm run check:source` in `docs/` checks the docs without building them: that cited paths exist, that Gherkin steps are in the grammar, the prose style, and the RFC numbering and coverage. `just docs-a11y` builds the site and runs the accessibility gate, pa11y included. The writing rules behind the prose check are in the `docs-writing` skill.

## Mutation testing

`cargo-mutants` checks whether the tests actually pin behavior or merely execute it. CI runs the diff variant as a gate on changed code.

```bash
just mutants       # Full run
just mutants-diff  # Only what this branch changed
```

## Benchmarks

Performance benchmarks using Criterion:

```bash
just bench                       # Run all benchmarks
just bench-save baseline-name    # Save a baseline
just bench-compare baseline-name # Compare against baseline
```

## Everything at once

`just check` runs formatting, lints, a build check, schema and annotation validation, the script test suites, and the full Rust test suite. It needs no npm dependencies. Run it before pushing, and after a frontend change also `just frontend-test`: the frontend unit tests and type checks, as CI runs them. The Playwright suite is `just test-e2e`.

The first three also run on their own, and the pre-commit hooks call them the same way:

```bash
just format       # rustfmt check (cargo fmt --check)
just lint         # clippy over all packages and targets, with the validate and annotation-validation features
just build-check  # cargo check over the whole workspace
```

## Does the new test run?

A test that no CI job starts never fails, so a missing hook or step goes unnoticed for months. An audit of the repository in October 2026 found seventeen. `just orphan-tests` (`script/check-orphan-tests.mjs`) reads what CI runs: the steps in `.github/workflows/`, the pre-commit hooks, and every `Justfile` recipe and npm script those reach. It fails on any of these:

- a `*.test.ts`, `*.test.js` or `*.test.mjs` file that no command names and that no vitest run in CI picks up through its config's `include` and `exclude`
- a `*.test.sh` or `test-*.sh` script that no hook or step runs, directly or through a recipe that one of them calls
- a workspace crate that no `cargo test` in CI covers
- a `.feature` file that no BDD runner in CI reads. The engine reads the directories that `BDD_BUCKET` and `REGULATION_PATH` select per step; the PoC runners are listed in `BDD_RUNNERS` in `script/orphan-tests.mjs`
- a `@wip` tag or an `#[ignore]` without a comment that says why, above it or on the same line (`#[ignore = "..."]` counts too)

The guard skips one directory, `corpus/regulation`, because bucket A over the live laws stays out of CI on purpose. `EXCEPTIONS` in `script/orphan-tests.mjs` records that reason. The guard runs as a pre-commit hook and therefore in the **Pre-commit** check. Its own suite is `just orphan-tests-test`, with a passing and a failing case for each rule.

It reads YAML and shell line by line and leans towards counting a test as run. A recipe with a `case` counts as running every branch, and a file counts as run once it is the command or an argument of an interpreter such as `node` or `bash`. The vitest `include` and `exclude` lists and the cargo flags that narrow a run (`--no-run`, `--lib`, `--test`) decide what a run covers. Beyond those, a test drops out only through a literal `if: false` on its step or job, a leg under a matrix `exclude`, or a hook that CI never runs. Any other condition still counts as a run. So does a workflow that only runs on a schedule, and so does a step with `continue-on-error`. Playwright `*.spec.*` files and the pytest suites are outside its scope. Whether a test that runs also tests something is still a question for review.
