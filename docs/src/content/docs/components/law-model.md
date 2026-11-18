---
title: "Law Model"
description: "The Rust crate that defines the law-YAML document types once, for every crate that reads or writes the law format, and the suite that holds it to the JSON schema."
---

The law model is the Rust representation of one version of a rulework: the structs and enums a document under `corpus/regulation/` deserializes into. It is defined once, in its own crate, so the engine, the corpus library and the workers all share one set of types instead of each deriving its own.

## Overview

- **Language**: Rust
- **Location**: `packages/law-model/` (crate `regelrecht-law-model`)
- **Type**: Library crate, no binary
- **Dependencies**: serde, serde_json, `rust_decimal` and the workspace's `regelrecht-shared`; nothing from the engine or any runtime

## What it does

The hand-authored JSON schema under `schema/` is the specification of the law format. This crate is one implementation of that specification, and neither is generated from the other. The schema is what a third party reads to build its own engine; the model is what the Rust code in this repository compiles against.

The crate holds document types and allocation-free accessors, plus the stored form of a `Value` and the rule two stored values are compared by (`stored.rs`, below). It does no YAML loading, applies no security limits and evaluates no law. Those belong to the engine: `packages/engine/src/article.rs` adds the `LawLoad` trait, which reads a law from YAML under the size and array limits and checks the schema version, and re-exports the model's types at their historical `regelrecht_engine::article` paths so older imports still compile.

Besides the engine, the model is used directly in a few places:

- The corpus library, the TUI and the editor API call `parse_law_header` to read a law's `$id`, name and dates.
- The enrich step in the pipeline parses a law into `ArticleBasedLaw` to count its articles and the ones that carry `machine_readable`.
- The harvester's writer tests parse what the writer produced back into the model.
- The cell (`packages/cel`) keeps and compares the values in its chronicle through `stored`, and the engine's `EQUALS` on lists and records uses the same comparison.

## Architecture

| Module | Purpose |
|--------|---------|
| `model.rs` | The document structure: `ArticleBasedLaw`, `Article`, `MachineReadable`, `Execution`, `Input`, `Output`, `Source`, `OpenTerm`, `ImplementsDeclaration`, `HookDeclaration`, `OverrideDeclaration`, `Marking`, and the rest of the types the schema defines |
| `value.rs` | `Value`, the runtime value type; `Operation`, the enum of operation names; `ParameterType`; and `RegulatoryLayer`, re-exported from `regelrecht-shared` |
| `stored.rs` | `to_stored` and `from_stored`, which write a `Value` to JSON and read it back with every decimal digit kept; `equal_as_written`, the rule for comparing two such values |
| `header.rs` | `parse_law_header`, a line-based scan for the top-level fields and an approximate article count |

`Value` separates two kinds of nothing, following [RFC-036](/rfcs/rfc-036). `Value::Null` is an absence the data is authoritative about: the register says there is no partner. `Value::Unknown` is a fact nobody has supplied yet, and it carries the names of the missing facts so a decision process can ask for exactly those.

The ordinary JSON of a `Value` (its `Serialize` and its `From` conversions) loses the difference between an `Int` and a `Decimal`. A `Decimal` is written as a float, so a fraction of 28 digits comes back with fewer, and a whole float such as `5.0` is read back as `Int(5)`. The API, the WASM build and the frontend read that JSON, so it stays as it is. Whatever is kept to be read back goes through `stored.rs` instead. `to_stored` writes a decimal as `{"__decimal": "5.0"}`, keeping its digits and scale. `from_stored` reads that object back as a `Decimal`. It refuses an object that holds the key in any other shape, and `to_stored` refuses a record that holds the key itself. `equal_as_written` is the rule the engine's `EQUALS` applies to two lists or two records, and the cell applies it to two stored values. Decimals are equal by value whatever their scale, and an integer against a decimal of the same value is an error.

`parse_law_header` exists because some callers need a law's metadata from text that is not valid YAML: an editor draft mid-edit, or a file an enrichment step is rewriting. Full deserialization would reject those. It matches header keys only at column 0, so an indented `$id:` inside an article is never taken for the law's id. The article count is a heuristic (it counts `- number:` entries at any depth) and is meant for display, not for logic.

### Conformance to the schema

Nothing about a separate crate stops the model from drifting from the schema, so a test suite checks both directions. Run it with `just conformance`; `just test` includes it. It lives in the engine's test directory (`packages/engine/tests/conformance.rs`) because it needs the engine's `validate` feature for the JSON schema validator.

The suite checks that the model accepts nothing the schema rejects, and that it parses everything the schema accepts without losing data on a round trip. It does that in three tiers: a differential over every corpus law, synthetic valid and invalid fixtures that each isolate one construct, and a coverage check that derives the list of schema properties from the schema itself, so a new property cannot land without a fixture or a written exemption.

The model is currently more permissive than the schema in documented ways. It has no `deny_unknown_fields` and uses untagged enums, so some invalid documents still parse. Those cases are listed in `KNOWN_GAPS` in the test file; an undocumented gap fails the suite, and so does a listed gap the model has since closed.

`packages/law-model/tests/schema_shapes.rs` adds regression tests for individual shapes where the model used to be narrower than the schema, such as `requires`, which the schema types as a list of objects and the model once read as a list of strings.

## The shared crate underneath

`packages/shared/` (crate `regelrecht-shared`) sits one level lower and holds the few definitions that every crate in the workspace needs to agree on. It is small enough that it has no page of its own:

- `RegulatoryLayer`, the enum of regulatory layer types (`GRONDWET`, `WET`, `AMVB` and so on), which the law model re-exports.
- `CURRENT_SCHEMA_VERSION` and `SCHEMA_URL`, the `$schema` value that the harvester and the pipeline stamp into every version they write. A test asserts the constant matches the `schema/latest` symlink.
- `dates::today()` (feature `dates`), the current calendar date in the Europe/Amsterdam timezone, so a container running in UTC and a laptop in another timezone pick the same law version.
- `telemetry::init_subscriber` (feature `telemetry`), the `tracing` setup the service binaries share.

Both features are optional so that the law model, and the corpus library through it, stay free of `chrono` and `tracing-subscriber`.

## Further reading

- [Conformance](/reference/conformance) - what the schema-to-model suite proves, next to the manifest coverage checks
- [Schema Reference](/reference/schema) - the specification the model implements
- [Execution Engine](./engine) - loads laws into this model and executes them
- [RFC-036](/rfcs/rfc-036) - the distinction between `Null` and `Unknown`
