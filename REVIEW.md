# Review Guidelines

## Project context

regelrecht makes Dutch law machine-readable and executable. The engine evaluates
YAML-encoded law to produce legally binding decisions (beschikkingen) that directly
affect citizens — benefit entitlements, tax calculations, allowances. Errors are not
just bugs: a wrong operator, misplaced decimal, or broken cross-law reference produces
incorrect legal decisions at scale.

## Always check

### Legal faithfulness (regulation YAML changes)

When `machine_readable` sections are added or changed:

- Does the execution logic faithfully implement what the legal `text` says?
- Are percentages, thresholds, and amounts correct? (e.g., text says "4,273 procent", YAML should have `0.04273`)
- All monetary amounts MUST be in eurocent (integers). Flag any euro values or floats.
- Are conditions complete? If the law says "A AND B AND C", does the logic check all three?
- Are edge cases from the legal text handled (e.g., "tenzij" / "unless" clauses)?
- Do `definitions` values match exact numbers from the legal text?

### Cross-law reference integrity (regulation YAML changes)

- Do `source.regulation` values reference valid law `$id` slugs that exist in `corpus/regulation/`?
- Do `source.output` values match actual `output.name` fields in the referenced law?
- Are `source.parameters` passed correctly (matching the referenced law's parameter names)?
- In annotation sidecars (`corpus/annotations/**`), are `regelrecht://` URIs well-formed:
  `regelrecht://{law_id}`, `regelrecht://{law_id}/{output_name}`, optionally with `#{field}`?
  Regulation YAML itself does not carry these URIs — it binds through `source:`.

### Schema and format compliance (regulation YAML changes)

- Does the YAML structure conform to the schema? `schema/latest` is the current
  version; `packages/engine/src/schema.rs` lists every version the engine loads.
  Review the file against the version it declares in `$schema` — the wrong version
  flags correct constructs and misses real ones.
- Are the schema's required top-level fields present (`regulatory_layer`,
  `publication_date`, `url`, `articles`)? `$schema` is not required by the schema
  itself, but a corpus file without it is rejected downstream
  (`packages/pipeline/src/law_convert.rs`), and RFC-013 requires an immutable tag URL:
  `https://raw.githubusercontent.com/NederlandseDigitaleDienst/regelrecht/refs/tags/schema-vX.Y.Z/schema/vX.Y.Z/schema.json`.
- Are operation names valid? The schema is the list and `just validate` decides.
  Rounding deserves its own look: the schema requires it to be explicit
  (`ROUND`/`CEIL`/`FLOOR` with a `precision`), so an implicitly rounded amount is a finding.
- Are type declarations correct (`string`, `number`, `boolean`, `amount` with `type_spec.unit`)?

### Engine correctness (engine Rust changes)

- No `unwrap()` or `panic!()` on paths reachable during law execution — these crash the engine mid-decision. Use `Result`/`Option` propagation.
- Operation implementations must be mathematically correct (especially integer arithmetic for eurocent amounts — watch for overflow and rounding).
- Cross-law resolution must handle missing laws and circular references gracefully.
- New operations or types must be deterministic — same input must always produce same output.

### BDD scenario correctness (feature file changes)

- Do expected values match what the law actually prescribes?
- Are test data tables realistic and internally consistent?
- Do scenarios cover the important paths from the law (eligibility, calculation, exclusions)?
- Are Given/When/Then steps using existing step definitions?

### Harvester and pipeline (harvester/pipeline/corpus changes)

- Does XML-to-YAML conversion preserve legal text faithfully (no dropped articles, no mangled Unicode)?
- Job queue operations: correct use of `FOR UPDATE SKIP LOCKED`, proper state transitions, retry logic with backoff, no lost jobs on worker crash.
- Law status transitions must be valid. The happy path is
  unknown → queued → harvesting → harvested → enriching → enriched, but
  `LawStatusValue` (`packages/pipeline/src/models.rs`) has eleven values, and the
  five that are missing here are the ones a change tends to drop:
  `harvest_failed`, `harvest_exhausted`, `enrich_failed`, `enrich_exhausted` and
  the terminal `not_harvestable`. A transition that cannot reach a failure state
  is a finding, not a simplification.
- Are BWB API interactions robust (retries, error handling for network failures)?
- Git corpus operations: handle merge conflicts, network failures, and concurrent pushes.
- Database migrations must be backwards-compatible and idempotent.

### Admin and frontend (admin/frontend changes)

- OIDC authentication: no session fixation, proper token validation, secure cookie settings.
- SQL injection: all queries must use parameterized statements, sort columns must be allowlisted.
- No XSS vectors in rendered content (law text may contain special characters).
- API pagination: no unbounded queries that could OOM on large datasets.
- A new `any` or non-null `!` without a comment saying why it is safe.
- A `vi.mock` path that no longer names the module it is meant to mock: the mock
  silently stops applying and the test runs against the real module.

### Tests (any change that adds or edits a test)

A test that cannot fail is worse than no test: it reports coverage that is not
there. Each point below is a pattern that has slipped through review before.

- Does the test call the real code? A function, rule or response builder that is
  copied into the test file and then tested proves only that the copy works. The
  production code can drift without the test noticing. Flag it (Significant) and
  ask for the logic to be exported or extracted so the test can import it.
- Can every assertion fail? Look for `is_err()` without checking which error,
  `assert_ne!(status, OK)` where one specific status is expected,
  `toBeTruthy()` or `toBeDefined()` on a value that is always an object,
  `expect(true)`, and asserts on a mock that the test itself filled. Each of these
  passes for the wrong reason as easily as for the right one.
- Does the name promise what the asserts check? A test or scenario called "rejects
  a late appeal" that only checks the response is not empty is a finding (Minor),
  and so is a comment describing a check the code no longer makes.
- Does the BDD scenario depend on what it claims to test? If the outcome follows
  from an input that the regulation hard-codes (a `definitions` value or an
  output that is always `true`), the scenario passes whatever the case data says.
  Flag the scenario, and flag the law if the hard-coded value is not in the text.
- Is there a test for the other side? A new rule or condition with only a
  happy-path test leaves the condition itself untested: delete it and every test
  still passes. Ask for at least one case where the condition decides the outcome.
- Will the new test actually run? A new `*.test.ts`, `*.test.sh` or `features/`
  directory runs only if a CI job picks it up; check `.github/workflows/ci.yml`
  and the `Justfile`. A Rust test behind `#[ignore]`, or a scenario tagged `@wip`,
  needs a comment saying why and what would let it run again.
- Is the setup copied? When `tests/common/` (Rust) or `src/test-utils/`
  (frontend) already offers a helper for a temp directory, a corpus loader or a
  fetch stub, a fresh copy in the new test is a finding (Minor).

### Any code change

- Dutch identifiers, comments or test names in new code (Minor), unless they are
  legal terms or names of concepts without a clear English equivalent.

## Severity scale

- **Critical** — wrong legal outcome, data loss, runtime crash, security vulnerability
- **Significant** — likely bug, broken reference, missing edge case, lost jobs
- **Minor** — code quality, style, non-blocking improvement

The merge gate `Claude review completed` searches the review for the exact string
`🔴 **Critical**` and blocks the merge when it finds one. Reserve that marker for
findings that really are critical, write it exactly like that, and do not use it
anywhere you are not reporting one.

## Skip

- `grammar.generated.js` (in `packages/frontend-shared/src/gherkin/` and the PoCs)
  — code-generated from `bdd/grammar.yaml`. Review the grammar, not the output;
  CI fails on a stale copy.
- `nldd-components.js` — generated from the `nldd-*` tags in use; the
  design-system import check fails on a stale copy.
- Lock files (`Cargo.lock`, `package-lock.json`) unless dependencies were intentionally changed
- Formatting-only changes caught by `cargo fmt` or pre-commit hooks
