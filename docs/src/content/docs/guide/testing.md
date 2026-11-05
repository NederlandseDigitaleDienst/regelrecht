---
title: "Testing"
description: "The testing strategies RegelRecht uses, led by Gherkin BDD scenarios run with cucumber-rs."
---

RegelRecht uses several testing strategies.

## BDD Tests (Behavior-Driven Development)

The primary testing approach uses Gherkin feature files executed by [cucumber-rs](https://github.com/cucumber-rs/cucumber).

### Two buckets, one language

Feature files live in two places, and the difference decides who fixes a failure:

- **Bucket A, law validation**: `corpus/regulation/**/scenarios/*.feature`, run against the real corpus. A failure means a law changed or the scenario went stale, and a human decides which. Scenarios tagged `@wip` are skipped.
- **Bucket B, engine conformance**: `bdd/conformance/*.feature`, tagged by tier, proving an engine speaks the whole language against synthetic `test_*` laws. CI blocks on this bucket.

The cell has a BDD suite of its own in `packages/cel/bdd/`, outside both buckets. Its steps record submissions and decisions in a cell's chronicle and evaluate a reduction over the decisions whose consequences still count, for example after a well-founded objection has revoked a decision, while an unfounded one leaves it standing. They stay out of `bdd/grammar.yaml`, which is the contract every engine (Rust and JS) executes, because a chronicle is behavior of the cell alone. `bdd/README.md` explains how that suite reuses the canonical steps and the generator, and why its runner sits in `packages/cel`.

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

### Running BDD Tests

```bash
just bdd                          # Both buckets, then the cell's own suite
BDD_BUCKET=conformance just bdd   # Engine conformance, then the cell's suite
BDD_BUCKET=corpus just bdd        # Law validation, then the cell's suite
just bdd-demo                     # Bucket A over the demo corpus
just bdd-chrono                   # Bucket A over the corpus of the chronolexografie PoC
just bdd-trace                    # With execution traces in trace_output/
```

### Deriving Tests from Legislative Intent

Test scenarios are derived from the **Memorie van Toelichting** (MvT), the explanatory memorandum that accompanies Dutch legislation. The MvT contains examples and reasoning from the legislature that serve as ground truth for expected behavior.

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

## Rust Tests

The Rust unit and integration tests, across every crate in the workspace:

```bash
just test           # Whole workspace (needs Docker)
just test-no-docker # Everything that needs no external services
just test-db        # Only the container-backed crates
```

The tests of `packages/cel` and the reduction tests of the engine (`packages/engine/tests/reduction.rs`) share one set of fixtures, `packages/cel/tests/fixtures/corpus/`. A fixture there is YAML made for these tests and holds only what they read: the cells, and fictional rulework and reductions cut down to the articles and outputs a test exercises. Real laws such as the Awb and the Leerplichtwet are not copied in. The tests read them from `corpus/regulation/`.

## Conformance

```bash
just conformance
```

Checks that the Rust `law-model` accepts exactly what the JSON schema accepts. What it covers, and what conformance does not cover yet, is on [Conformance](../reference/conformance); adding a fixture is described in `packages/engine/tests/conformance/README.md`.

## Mutation Testing

`cargo-mutants` checks whether the tests actually pin behavior or merely execute it. CI runs the diff variant as a gate on changed code.

```bash
just mutants       # Full run
just mutants-diff  # Only what this branch changed
```

## End-to-End

`just test-e2e` builds the engine to WASM and drives the editor with Playwright.

## Schema Validation

Every rulework is validated against the JSON schema:

```bash
just validate                     # Validate all
just validate path/to/law.yaml    # Validate a specific file
just validate-annotations         # Validate stand-off notes
just validate-demo                # Validate the demo corpus
```

## The Demo

`just demo-check` checks the demo end to end: the laws, their scenarios, the frontend tests, and the WASM build. Run it before pushing anything demo-related.

## Pipeline Tests

```bash
just pipeline-test               # Unit tests (no Docker)
just pipeline-integration-test   # Integration tests (requires Docker)
```

## Benchmarks

Performance benchmarks using Criterion:

```bash
just bench                       # Run all benchmarks
just bench-save baseline-name    # Save a baseline
just bench-compare baseline-name # Compare against baseline
```
