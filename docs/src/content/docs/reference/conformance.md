---
title: "Conformance"
description: "What conformance enforces today (manifest coverage, schema-to-model conformance, golden fixtures and the BDD conformance bucket) and the language-agnostic suite RFC-014 designs, whose per-level JSON test cases are not yet written."
---

The schema is the specification; the Rust engine is one implementation of it. Nothing stops another organization from building its own engine, and for a government decision system that independence is the point. A conformance suite is how a second implementation would prove it produces the right answers, without depending on the RegelRecht codebase.

That cross-implementation suite is **designed but not built**. This page is therefore in two halves, in order of how real they are:

1. [What is enforced today](#what-is-enforced-today) - the checks that run: manifest operation coverage, a suite binding the Rust model to the schema contract, golden JSON fixtures, and the BDD conformance bucket.
2. [The intended suite](#the-intended-suite) - the cross-implementation test format the manifests are structured for. The manifests are checked in; the per-level JSON test cases and a runner for them are not written yet. The full design is [RFC-014](/rfcs/rfc-014).

If you came here for test cases to run against your own engine, the closest thing today is the BDD conformance bucket: plain Gherkin over synthetic laws, in a vocabulary defined outside any engine. The golden fixtures are JSON too, but their format is read by one Rust test and was never specified as a contract.

## What is enforced today

The `conformance/` directory holds one `manifest.json` per schema version. Each declares a set of conformance levels and, per level, the operations that level is responsible for. Which operation joined which level in which version is in the manifests themselves; diff two of them to see it.

What runs in CI is **operation coverage of the manifests themselves**, nothing more. `packages/engine/tests/conformance_coverage.rs` checks their `operations` lists against the engine's own operation list in three integration tests:

- every operation the engine supports appears in some level, checked against the newest manifest only,
- no level lists an operation the engine does not have, checked against every manifest,
- no operation lands in two levels, checked against every manifest.

So a new operation cannot be added to the engine without being classified into exactly one conformance level; CI fails otherwise, and those three properties keep holding as the engine grows. But it tests the *manifest*, not any law execution: it never runs a regulation, never checks an output. The cross-implementation guarantee a conformance suite is meant to provide does **not** hold today.

### Schema to law-model conformance

A second suite, run with `just conformance`, proves something different and narrower: that the Rust `law-model` conforms to the hand-authored JSON schema. The schema is the canonical, language-agnostic contract; the model is one implementation of it, and neither is generated from the other. The suite checks both directions, that the model is no more permissive than the schema and no more restrictive, in three tiers: a differential over every corpus law, synthetic valid and invalid fixtures per construct, and a coverage check deriving the fixture set from the schema itself so an unexercised property has to carry a reasoned exemption.

It is the structural twin of the BDD conformance bucket: that one proves an engine speaks the whole language behaviorally, this one proves the model accepts exactly the whole language structurally. The model is currently more permissive than the schema in a few documented ways, listed in `KNOWN_GAPS`; reconciling them is tracked separately. Details are in `packages/engine/tests/conformance/README.md`.

This still says nothing about a *second* engine. It binds one implementation to the contract, which is what makes the contract worth writing against.

### Golden fixtures

`packages/engine/tests/fixtures/*.json` holds golden test cases, grouped by theme (basic, arithmetic, comparison, conditional, logical, nested and aggregate operations, action execution, cross-law references, error cases, and a few real regulations). Each case carries a law as YAML text (or several, for a cross-law case), the law and output to evaluate, the parameters, the calculation date and the expected result: success or failure, the article number, and the output values. `packages/engine/tests/golden_tests.rs` runs every case in the files it lists through `LawExecutionService` and fails on any mismatch; a new fixture file has to be added to that list before it runs. It runs with the other engine tests in `just test`.

The fixtures were generated from the earlier Python implementation, which is why a case may carry a `generator_error` (such a case is skipped) and why the format has a `version` field that nothing checks yet. Numbers are compared as floating point with a tolerance of 1e-9, because JSON gives the expected side no more precision than that. They are the nearest thing in the repository to RFC-014's JSON cases, but they are not those cases: they sit under the engine's tests rather than under `conformance/`, are not grouped by conformance level, and follow a format only this Rust test reads.

### The BDD conformance bucket

Bucket B of the BDD suite, `bdd/conformance/*.feature`, is behavioral conformance: scenarios over the synthetic `test_*` laws in the corpus, each feature tagged with the tiers it needs (`@tier:core`, `notes`, `untranslatable`, `provenance`; untagged means core). Its vocabulary is `bdd/grammar.yaml`, from which each engine's step bindings are generated, so any engine that implements the actions can run the same files. CI runs it against the Rust engine and blocks on it (the **BDD conformance** job); locally it is `BDD_BUCKET=conformance just bdd`.

The JavaScript side shares the grammar but not the run. The editor and the demo execute scenarios in the browser against the WASM build, through bindings generated into `packages/frontend-shared/src/gherkin/grammar.generated.js`, and they implement only the `core` tier: actions from the other tiers throw instead of passing silently. No JavaScript runner executes `bdd/conformance/` in CI. What CI does check on that side is narrower: `frontend/src/gherkin/steps.test.ts` asserts that the editor types step values the way `bdd/conformance/value_typing.feature` requires. [Testing](/guide/testing) describes both buckets.

## The intended suite

The manifests are shaped for a test format that does not exist yet. Documented here so the structure already in the repo is legible, and so the gap is explicit.

Tests are designed to be plain JSON, not Rust, so any engine can consume them. Each case would give a regulation, parameters, and the expected outputs; an engine passes by producing those outputs. Trace assertions would be optional, so an engine that does not emit traces can still demonstrate correctness on everything else.

Tests live under `conformance/v<schema>/`, one directory per schema version, because what counts as correct can change between schema versions. A case written against schema v0.5.4 belongs under `conformance/v0.5.4/`.

The manifest groups work into conformance levels, from a minimal core outward:

| Level | Covers |
|-------|--------|
| `core` | Arithmetic, comparison, logical, conditional, and collection operations (`IN`, `LIST`, `FOREACH`), plus variable resolution |
| `cross_law` | Resolving a `source` reference into another law |
| `ioc` | Open terms filled by `implements` regulations |
| `temporal` | Date operations: `AGE`, `DATE_ADD`, `DATE`, `DAY_OF_WEEK`, `DATE_DIFF`, `DATE_PART`, `START_OF` |
| `advanced` | Hooks, overrides, markings (formerly untranslatables), data sources, and Awb procedures |

Once the cases exist, an engine could claim a level by passing every test in it, which gives a precise vocabulary for partial support: an engine might be core-and-cross-law conformant without yet handling the advanced level. The `test_files` entries in the manifests are the planned filenames; those files are not written yet, and there is no runner that executes them against an engine. The golden fixtures above could seed them, once their format is specified and they are sorted by level.

Writing the JSON test corpus and a runner that executes the cases against an arbitrary implementation is the remaining work. [RFC-014](/rfcs/rfc-014) is the full design and tracks that status.

## Further reading

- [Execution Provenance](../concepts/execution-provenance) - the receipt format a conformant engine produces
- [Schema](./schema) - the versioned specification under test
- [RFC-014: Engine Conformance Test Suite](/rfcs/rfc-014) - full specification and current status
- [Rules as Executed, section 9.1](/research/rules-as-executed#sec:engines) - the position paper on multiple engines and semantic equivalence
