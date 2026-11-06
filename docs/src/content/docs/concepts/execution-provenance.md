---
title: "Execution Provenance"
description: "How a result can be reproduced later by pinning the regulation, schema, and engine version, what an Execution Receipt records today, and what is still planned."
---

Government agencies must be able to reproduce a specific decision months or years later, with the exact same result. Dutch administrative law requires this (Awb Art. 3:46, the AERIUS rulings). The EU AI Act makes logging mandatory for high-risk AI systems, for those under Annex III from 2 December 2027 (Regulation (EU) 2026/1744); whether a deterministic engine that executes adopted rules is an AI system under the Act at all is doubtful ([Rules as Executed, section 7.3](/research/rules-as-executed#sec:european)).

Determinism within a single execution is necessary but not sufficient. Reproducibility requires pinning three things: the version of the rulework, the schema version it conforms to, and the engine version that executed it.

## The Execution Receipt

An **Execution Receipt** is an output envelope that contains everything needed to reproduce a result. It is opt-in: an ordinary evaluation returns only the result, and a receipt is built on request, by passing `--receipt` to the `evaluate` CLI or by calling `build_receipt_with_outputs()` on the Rust `LawExecutionService` ([Engine](/components/engine#execution-receipt) has both). The WASM build, which the editor and the demo run on, has no receipt call yet.

This is the receipt the CLI prints for article 4 of the Wet op de zorgtoeslag, which reads the standaardpremie from the Regeling standaardpremie (hashes shortened):

```json
{
  "provenance": {
    "engine": "regelrecht",
    "engine_version": "0.3.0",
    "schema_version": "v0.5.8",
    "regulation_id": "wet_op_de_zorgtoeslag",
    "regulation_valid_from": "2025-01-01",
    "regulation_hash": "sha256:8b47c444..."
  },
  "engine_config": {
    "connectivity": "solo",
    "legal_status": "simulation",
    "untranslatable_mode": "warn"
  },
  "scope": {
    "sources": [],
    "loaded_regulations": [
      { "id": "regeling_standaardpremie", "valid_from": "2025-01-01", "hash": "sha256:e3d58d64..." },
      { "id": "wet_op_de_zorgtoeslag", "valid_from": "2025-01-01", "hash": "sha256:8b47c444..." }
    ]
  },
  "execution": {
    "calculation_date": "2025-01-01",
    "parameters": {}
  },
  "results": {
    "requested_outputs": ["standaardpremie"],
    "outputs": { "standaardpremie": 211200 },
    "output_provenance": {
      "standaardpremie": { "type": "Direct", "law_id": "wet_op_de_zorgtoeslag", "article": "4" }
    }
  },
  "accepted_values": [],
  "timestamp": "2026-09-24T17:16:10.551863+00:00"
}
```

The receipt records which engine version and schema produced the result, which regulations were loaded (with content hashes), the input parameters, and the outputs with their provenance. `results.trace` is filled only when the result it wraps was computed with tracing on; the CLI evaluates without a trace, so its receipts carry none. `results.delegation_refusals` lists any regulation that offered to fill an open term it was not delegated. `results.open_terms` lists each open term that a finding or a regulation filled, and which of the two it was.

## Schema and engine versioning

The schema defines the regulation format. The engine interprets and executes regulations that conform to the schema. These are versioned independently:

- **Schema versions** are immutable directories under `schema/` (one directory per version, such as `schema/v0.7.0/schema.json`), each with a git tag `schema-vX.Y.Z`. A published version is never modified, and `schema/latest` is a symlink to the current one.
- **Engine versions** are the `version` of the `regelrecht-engine` crate in `packages/engine/Cargo.toml`, which is what `engine_version` in a receipt reports. Its `supported-schemas` metadata, mirrored in `SUPPORTED_SCHEMAS` in `config.rs`, declares which schema versions that engine accepts.

The engine version is not yet tied to a release. The design in [RFC-013](/rfcs/rfc-013) is that every engine version is a GitHub Release with a matching tag, so a receipt's `engine_version` points at code anyone can check out. Today it does not: releases are built by `release-engine.yml` only when someone pushes an `engine-v*` tag, and the crate version has moved past the newest such tag, so a receipt's `engine_version` can name a version that no tag marks. Compare `version` in `packages/engine/Cargo.toml` with `git tag -l 'engine-v*'` to see where it stands. Until a tag is pushed for every version, the `regulation_hash` and the loaded-regulation hashes pin the law, but only the repository history pins the engine.

This distinction matters because third-party organizations may build their own engine implementations. The schema is the specification; the engine is one implementation of it.

## Cross-organization reproducibility (planned)

When a decision depends on values from other organizations (the exchange [RFC-009](/rfcs/rfc-009) proposes), the receipt is meant to capture the provenance of each accepted value, so that reproducing the decision uses the value as it was accepted rather than re-calling the other organization. That organization may be running a different engine version by then, and a *beschikking* stands once issued: the accepted value at the time is a legal fact.

None of this is built. `accepted_values` is present in every receipt and always empty, because the multi-organization exchange it would record ([RFC-009](/rfcs/rfc-009)) is not implemented. The intended shape of an entry, from RFC-013:

```json
{
  "accepted_values": [
    {
      "output": "toetsingsinkomen",
      "value": 3200000,
      "authority": "inspecteur",
      "engine_version": "0.5.1",
      "regulation_hash": "sha256:d4e5f6...",
      "signed": true
    }
  ]
}
```

In the same way, `engine_config.connectivity` is always `solo` and `legal_status` always `simulation` today.

## What this enables

A citizen can request their trace and see which rules applied. An auditor re-runs the computation and gets the same number. A court reconstructs the reasoning step by step. When a bug surfaces, every affected decision can be found by querying receipts for the engine version and regulation hash. Section 4.5 of the position paper, [The Recipient's Check](/research/rules-as-executed#sec:traceaccess), spells out what this needs in practice: the attested trace has to reach the addressee together with the decision, without a procedure of their own.

## Further reading

- [Hooks and Reactive Execution](./hooks-and-reactive-execution) - Awb procedure hooks
- [Competent Authority](./competent-authority#a-decision-or-a-calculation) - cross-organization value exchange, and what is built of it
- [RFC-013: Execution Provenance](/rfcs/rfc-013) - full specification
- [RFC-014: Engine Conformance](/rfcs/rfc-014) - conformance test suite
- [Rules as Executed, sections 4.4 and 4.5](/research/rules-as-executed#sec:attestation) - the position paper's constitutional case for attestation and the recipient's re-execution check
