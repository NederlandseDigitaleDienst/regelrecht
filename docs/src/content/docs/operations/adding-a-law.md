---
title: "Adding a Law"
description: "A step-by-step walkthrough from downloading a law's text to running tests against it."
---

Adding a new law to the corpus takes six steps, from downloading the legal text to running tests against it.

## Step 1: Find the law

Every Dutch national law has a BWB ID (format: `BWBR` + 7 digits). Find it on [wetten.overheid.nl](https://wetten.overheid.nl).

For example, the Zorgtoeslagwet is `BWBR0018451`.

## Step 2: Harvest the legal text

Use the harvester to download and convert the law from BWB XML to YAML. There is no installed binary to call; run it through cargo, from the repository root:

```bash
cargo run --manifest-path packages/Cargo.toml -p regelrecht-harvester -- \
    download BWBR0018451 --date 2025-01-01 --output corpus/regulation/nl
```

Leave out `--date` for the version in force today. Pass `--output` explicitly: without it the harvester quietly creates `regulation/nl/` under the directory you run from, while an explicit `--output` has to exist already. The command-line interface sits behind the crate's `cli` feature, which is on by default, so no feature flag is needed. A CVDR identifier works the same way for a municipal regulation.

This produces a YAML file with the law's text but no `machine_readable` sections, at `corpus/regulation/nl/{layer}/{slug}/{date}.yaml`. A file already at that path is replaced without a warning, `machine_readable` sections included. The Wet op de zorgtoeslag in the example is in the corpus already, so run the command with your own identifier, and check `git status` after harvesting a law that may already be there. The `law-download` skill in `.claude/skills/` wraps this step for a coding agent.

## Step 3: Add machine-readable logic

Each article that contains executable logic needs a `machine_readable` section. This can be done:

- **Manually** - write the `machine_readable` YAML by hand following the [law format](/concepts/law-format)
- **With a coding agent** - the repository carries skills for this in `.claude/skills/`. `law-interpret` runs the whole sequence: `law-mvt-research` looks up the Memorie van Toelichting and turns its worked examples into scenarios, `law-generate` writes the `machine_readable` sections and runs validation and the scenarios until they pass, and `law-reverse-validate` checks that every element of the result traces back to the legal text. Each can also run on its own. `law-wetsgeschiedenis` answers, per article and lid, which parliamentary paper a line of the law came from and what the papers say about it, with the paper to cite and a check against the official text. Claude Code loads them by name; other agents can read the `SKILL.md` files as instructions.
- **Via the pipeline** - trigger an enrichment job in the editor's Corpusinwinning section, which uses an LLM to generate candidate interpretations

Whoever or whatever wrote the logic, validate it (step 4) and test it against the MvT examples (step 5).

## Step 4: Validate against the schema

```bash
# Validate a specific file
just validate corpus/regulation/nl/wet/your_law/2025-01-01.yaml

# Validate all files
just validate
```

The validator rejects files with an unknown or missing `$schema` version. Make sure the `$schema` URL uses a tag-based ref (`refs/tags/schema-vX.Y.Z`) and points to a released schema version. CI checks only the version in the URL, not that the ref is a tag, so a branch-based URL slips through.

The tag is what makes that URL a promise rather than a hope: it pins the schema your law validated against, and it cannot move afterwards. A schema version is tagged automatically when it lands on `main`, and CI blocks a version that has no tag, so the address a version cites always resolves.

Fix any schema errors before proceeding.

## Step 5: Write BDD test scenarios

Derive test scenarios from the Memorie van Toelichting (MvT), the explanatory memorandum that accompanies the law. The MvT contains worked examples of how the legislature intended the law to be applied.

A law's scenarios live next to the law, in a `scenarios/` directory beside the YAML file: `corpus/regulation/nl/wet/your_law/scenarios/eligibility.feature`. That is bucket A, the law-validation bucket, described in [Testing](/guide/testing).

The step vocabulary is not free text. `bdd/grammar.yaml` is the single source of truth, and the bindings are generated from it, so a step that is not in that file does not exist. A minimal scenario:

```gherkin
Feature: Wet op de zorgtoeslag

  Scenario: MvT example, single person, output present
    Given the calculation date is "2025-01-01"
    Given parameter "bsn" is "999993653"
    When I evaluate "hoogte_zorgtoeslag" of "wet_op_de_zorgtoeslag"
    Then the execution succeeds
    Then output "hoogte_zorgtoeslag" equals 157731
```

Laws that need source data (BRP, Belastingdienst, and the like) provide it with a data-table step keyed on the identifier the law looks up:

```gherkin
    Given the following "personal_data" data with key "bsn":
      | bsn       | geboortedatum | verblijfsadres |
      | 999993653 | 2005-01-01    | Amsterdam      |
```

See `corpus/regulation/nl/wet/wet_op_de_zorgtoeslag/scenarios/eligibility.feature` for a complete, data-driven example.

Run your law's scenarios. `BDD_BUCKET=corpus` limits the run to bucket A, the scenarios next to the laws, and skips the engine-conformance suite:

```bash
BDD_BUCKET=corpus just bdd
```

To see how the engine reached a result, run the same scenarios with traces:

```bash
BDD_BUCKET=corpus just bdd-trace
```

This writes one box-drawing trace per successful evaluation to `trace_output/` in the repository root, named `<sequence>_<law>_<output>_<date>.txt`, so `ls trace_output | grep your_law` finds yours. A trace shows each input and where it came from (parameter, data-source table, another law, a definition), every operation with its result, and each output. The data tables in your scenario appear there as `DATA_SOURCE` lines, which makes a trace the quickest way to check that a scenario feeds the law what you meant it to.

**CI does not run these scenarios.** It runs bucket B, the engine-conformance suite, and blocks on it, but bucket A stays out on purpose: a failure there means a law changed or a scenario went stale, and a human has to decide which. (The **BDD demo** job does run bucket A, but over `corpus/demo/`, not over `corpus/regulation/`.) So a scenario that fails on your branch will not stop the merge, and nobody else will see it fail. Run bucket A yourself before opening the PR, and again after any change to a law your law reads from.

## Step 6: Open a pull request

Commit the new rulework and its scenarios, and open a PR. CI validates the law against the schema and runs the engine tests and the engine-conformance BDD suite, but not your law's own scenarios (see step 5): their result is whatever you saw locally, so say in the PR description that they pass. Add the `deploy:preview` label to the PR if reviewers should be able to try the law in a running editor.

End the PR body with a `Werkpakket:` line, which a required check enforces, and add a `Wet:` line naming the law's `$id`:

```
Werkpakket: referentie-casus-i
Wet: wet_op_de_zorgtoeslag
```

See [Contributing](/operations/contributing) for what both lines mean and how the slug is checked.

## Further reading

- [Law Format](/concepts/law-format) - how to structure the YAML
- [Testing](/guide/testing) - more on writing and running tests
- [Validation Methodology](/concepts/methodology) - the execution-first validation approach
