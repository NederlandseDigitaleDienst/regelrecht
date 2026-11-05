# bdd/ — canonical BDD feature language

`grammar.yaml` is the single source of truth for regelrecht's law-agnostic
Gherkin vocabulary. Step bindings for every engine are GENERATED from it:

- Rust: `packages/engine/build.rs` → `$OUT_DIR/bdd_generated_steps.rs`
- JS:   `bdd/codegen/gen-js.mjs` → `frontend/src/gherkin/grammar.generated.js`

Never hand-edit a generated file. Change `grammar.yaml`, then run
`just bdd-codegen` (which regenerates both and is checked in CI).

## Why

There used to be two separate Gherkin dialects that drifted apart silently:

- **Rust** — rich but *law-specific* steps (`the bijstandsaanvraag is executed
  for participatiewet article 43`). Every new law needed new step sentences and
  thus new engine/test code.
- **JS (editor)** — generic steps (`I evaluate "x" of "y"`, `parameter "x" is
  "y"`). This drove the scenario builder against the WASM engine.

Because they diverged, a hand-written law-validation scenario such as
`wet_op_de_zorgtoeslag/scenarios/eligibility.feature` only ran in the editor and
never under `just bdd` against the live law. One canonical grammar with
generated bindings makes that drift impossible by construction:

| Before | After |
|---|---|
| Editor scenario doesn't run under `just bdd` | One scenario runs verbatim in editor, `just bdd`, and any future engine |
| New law = new step sentences + engine code | New law = write scenarios only, zero engine code |
| Step definitions hand-maintained 3×, silent drift | One `grammar.yaml`, bindings generated → drift impossible |
| A law change breaks a scenario silently (editor-only) | Bucket A runs against the live law under `just bdd` → breaks visibly, a human decides |

## Two buckets, one language

Both buckets use the same grammar and generated bindings; they differ only in
**where the laws come from** and **what a failure means**.

- **Bucket A — law validation**: `corpus/regulation/**/scenarios/*.feature`.
  Written in the editor, run against the LIVE laws. A failure means a law
  changed or the scenario is stale — a human decides (law bug vs. update the
  scenario). `# NB:` annotations are preserved. Tier `core` only.
- **Bucket B — engine conformance**: `bdd/conformance/*.feature`. Proves an
  engine speaks the whole language (incl. `notes`, `untranslatable`,
  `provenance` tiers) against synthetic `test_*` laws — deterministic, because
  this tests the engine, not a real law.

`just bdd` runs both buckets. `BDD_BUCKET` narrows the run to one of them:
`BDD_BUCKET=conformance` for bucket B, `BDD_BUCKET=corpus` for bucket A.

CI runs bucket B and blocks on it (the **BDD conformance** job, hung on the
`Test` gate). Bucket A stays out of CI: a red scenario there is a question for a
human, not a broken build.

## Grammar format

`grammar.yaml` is a flat list of canonical steps:

```yaml
- id: assert_output_equals
  keyword: then                       # category/docs; Gherkin And/But still work
  tier: core                          # core | notes | untranslatable | provenance
  text: 'output "{output}" equals {value}'
  args:
    - { name: output, type: string }
    - { name: value,  type: value }   # value = bool/int/float/string inference
  action: assert_equals               # semantic action
```

- **Placeholder syntax is engine-neutral**: `"{name}"` is a quoted-string
  capture, bare `{name}` is a numeric capture. Codegen translates each arg
  `type` into a matcher (`string`, `int`, `float`, `value`), plus a `datatable`
  flag for steps that take a trailing table. Quoted-only steps emit a
  Cucumber-expression binding (`{string}`); any numeric arg forces a regex
  binding.
- **`action`** links the step to a semantic action. The action set is small and
  fixed (each engine implements one handler per action):
  - setup: `set_calculation_date`, `load_law`, `set_parameter`,
    `set_data_source`, `set_parameters_table`
  - execute: `evaluate`
  - asserts (core): `assert_succeeds`, `assert_fails`, `assert_fails_with`,
    `assert_equals`, `assert_boolean`, `assert_null`, `assert_contains`
  - tier `provenance`: `evaluate_outputs`, `assert_provenance`,
    `assert_exact_outputs`
  - tier `untranslatable`: `set_untranslatable_mode`, `assert_tainted`
  - tier `notes`: `set_note_selector_*`, `set_note_articles`,
    `set_note_hint_*`, `resolve_note`, `assert_note_resolves`,
    `assert_note_{exact,fuzzy}_match`, `assert_note_{ambiguous,orphaned}`

**New wording = change `grammar.yaml` only.** A new *capability* = new action
(rare) + grammar line + an implementation in every engine that supports the tier.

## Tiers

`core` (all engines), `notes`, `untranslatable`, `provenance`. A feature's
required tiers come from its `@tier:<name>` tags (untagged = `core`). A runner
only runs features whose tiers it supports.

## The cell's own steps

`packages/cel/bdd/` holds a BDD suite of its own, with steps that record
submissions, decisions and facts in the chronicle of a cell, evaluate a
reduction that plays the chronicle forward (`projection`, schema v0.8.14), and
check the steps the engine took: the table of records applied and skipped and
of replays, and which record set a property last. Those steps are not part of
this grammar. This grammar is the contract every engine executes, the Rust
engine and the JS engines alike, while a chronicle is behavior of one
implementation, the cell. Whether the law format should describe chronicles at
all is still an open question.

The cell's steps live in `packages/cel/bdd/grammar.yaml`, in the format of this
file. A cell feature may use both sets, so it can still set the calculation
date and assert an output with the canonical steps. The bindings come from the
same generator: `packages/engine/build_codegen/` reads a base grammar plus an
extension, and `packages/cel/build.rs` includes it. `gen-js.mjs` reads only
this file.

The runner (`packages/cel/tests/bdd/`) reuses the engine runner's world,
dispatch and helpers by `#[path]` and adds its actions in its own
`extension.rs`. It sits in the cell because the engine cannot depend on the
cell, not even for a test: the Docker builds of the WASM module, the editor and
admin drop `cel` from the workspace and do not copy it, and Cargo still has to
resolve a path dev-dependency. `just bdd` and the **BDD conformance** job in CI
run it after the engine's runner.

## Engine runner contract

An engine that "speaks" the language provides:

1. An implementation of the action set, for the tiers it supports.
2. A runner that reads `.feature` files, loads laws (bucket A: from the given
   corpus; bucket B: from the synthetic `test_*` laws), and wires the generated
   bindings to its action implementation.
3. A declared set of supported tiers — it runs only features whose used tiers
   are all supported.
