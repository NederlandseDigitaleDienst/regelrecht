# Schema ↔ law-model conformance suite

The canonical, hand-authored `schema/*/schema.json` is the **public, language-agnostic
contract** for the law-YAML format — what a third-party engine author reads to know what a
valid law is, and what `just validate` enforces. The Rust `law-model` (`packages/law-model/`)
is *one* implementation that must provably **conform** to that contract. This suite proves it.

It is the structural twin of the BDD bucket-B engine-conformance suite (`bdd/conformance/`):
bucket-B proves an engine speaks the whole language *behaviourally*; this proves the model
accepts exactly the whole language *structurally*.

Run it with `just conformance` (it needs the engine's `validate` feature, which pulls in
`jsonschema`). The test lives in `packages/engine/tests/conformance.rs`.

## The contract

For every candidate law document `d`, with schema-valid set `S` and model-parseable set `M`:

- **Soundness** — `d ∈ M ⇒ d ∈ S`. If the model parses it, the schema accepts it
  (the model is not *more permissive* than the spec).
- **Completeness** — `d ∈ S ⇒ d ∈ M`, losslessly. If the schema accepts it, the model parses
  it and a re-serialize round-trips to a schema-valid, value-equal document (the model is not
  *more restrictive*, and does not silently drop data).

## Three tiers

- **Tier A — corpus differential** (`tier_a_corpus_differential`). Walks every
  `corpus/regulation/**/*.yaml` with a recognised `$schema`. Hard assertions (also guaranteed
  by the `just validate` CI gate, made explicit here): the schema accepts it and the model
  parses it. **Reported, non-fatal**: whether the re-serialized model is still schema-valid and
  value-stable — these quantify lossy serialization (the model emits `None` as `null`, etc.).
- **Tier B — synthetic fixtures** (`tier_b_fixtures`), under `valid/` and `invalid/`. Each
  fixture is a single, isolated construct.
  - `valid/`: schema accepts ∧ model parses ∧ re-serialized (null-normalized) still schema-valid
    ∧ the round-trip loses nothing the fixture states. That last one is a hard assertion here,
    where Tier A only reports it: a field the model silently drops passes every other check,
    because the document still parses and every optional field may be absent.
  - `invalid/`: schema **rejects** (asserted). The model verdict is *measured* against `KNOWN_GAPS`.
- **Tier C — schema-property coverage** (`tier_c_schema_property_coverage`). Derives the fixture
  set from the schema instead of from what someone thought to write down: every property the
  latest schema defines must be exercised by some `valid/` fixture, or carry a reasoned entry in
  `UNCOVERED_SCHEMA_KEYS`. Coverage is keyed on `(definition, property)`, not on the position in
  the document, because a definition reused at fifteen places yields fifteen paths that no single
  fixture can reach. Only fixtures count; the corpus does not, because only fixtures carry the
  no-loss assertion and a corpus law can stop using a field at any time.

## Why Tier C exists

Tier B proves things about the fixtures that exist. It says nothing about a schema-valid law the
model refuses or mangles through a field no fixture happens to use, and that is the dangerous
direction: the schema is the contract an author writes against, so no amount of care while
writing a law protects against it. `machine_readable.requires` is a list of objects in the schema
and was `Vec<String>` in the model. A law the schema accepted made the engine fail to load, and
no fixture used `requires`.

Tier C plus the Tier-B no-loss assertion closes that: a new schema property cannot land without
either a fixture proving the model carries it, or an explicit statement that it may be lost.

## `KNOWN_GAPS` (the measurement)

The model has no `#[serde(deny_unknown_fields)]` and uses `#[serde(untagged)]` enums, so it is
currently **more permissive** than the schema. `KNOWN_GAPS` in `conformance.rs` lists the
`invalid/` fixtures the model *accepts* anyway — i.e. the soundness gap. The list is kept honest:
an **undocumented** gap fails the suite, and a **stale** entry (the model now rejects it) also
fails. Resolving these gaps (tighten the model vs. consciously declare it lenient) is a Phase-2
decision driven by this measurement.

## Adding a fixture

1. Drop a `*.yaml` law document into `valid/` or `invalid/`. Give it a `$schema` line pinned to
   the latest schema version and otherwise isolate the *one* construct under test (so a rejection
   is attributable to it — mind conditional `required` rules, e.g. `WET` requires `bwb_id`).
2. Run `just conformance`. For an `invalid/` fixture the model also accepts, add its filename to
   `KNOWN_GAPS` with a one-line note on *why* the model is lenient.

### Fixtures that lend an example to the docs

The examples on the docs page *Schema Release Notes*
(`docs/src/content/docs/reference/schema-release-notes.mdx`) are excerpts of these fixtures,
read when the site is built (`docs/src/lib/fixture-excerpt.ts`). A fixture marks the lines it
lends out with a pair of comments:

```yaml
      # release-note-begin: hook
      hooks:
        - hook_point: pre_actions
      # release-note-end: hook
```

Keep the markers when you edit such a fixture. A region that is missing, doubled or empty fails
the docs build, which is the point: the page cannot quietly show an example this suite no longer
checks. A new schema version adds its fixture with markers around its construct, and its note on
that page renders them; the page's section "Adding a release note" has the steps.

The one-construct-per-fixture rule holds for `invalid/`, where attribution is the whole point. The
`valid/` set also carries a few deliberately dense fixtures (`machine_readable_full.yaml`,
`all_operations.yaml`, `law_metadata.yaml`) that exist to satisfy Tier C; a fixture per property
would mean a hundred near-identical files. The layer identifiers are split one per file, since the
schema's conditional `required` rules make them mutually exclusive.
