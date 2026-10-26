---
title: "Documentation Coverage"
description: "Which accepted or built RFCs and features have prose documentation outside the RFCs, and what is still on the backlog."
---

This page tracks where the prose docs cover the RFCs whose design is accepted or built, so a gap is visible before a reader runs into it. An RFC that is neither accepted nor built is out of scope: its own page under [RFCs](/rfcs/) is the source of truth.

CI keeps the list complete, not correct. `docs/scripts/check-rfc-coverage.mjs` fails when an RFC that is `Accepted`, `Implemented` or `Partially implemented` is missing from this page; whether the page a row points at is any good stays a human judgement.

## Accepted RFCs

| RFC | Topic | Prose coverage |
|-----|-------|----------------|
| RFC-001 | YAML schema design | [Law Format](../concepts/law-format), [Schema](./schema) |
| RFC-002 | Competent authority | [Competent Authority](../concepts/competent-authority) |
| RFC-003 | Inversion of control | [Inversion of Control](../concepts/inversion-of-control) |
| RFC-004 | Uniform operation syntax | [Law Format](../concepts/law-format) |
| RFC-005 | Stand-off notes | [Notes and Annotations](../concepts/notes-and-annotations) |
| RFC-006 | Language choice (Rust) | Backlog: the "why Rust" rationale has no prose page |
| RFC-007 | Cross-law execution | [Cross-Law References](../concepts/cross-law-references), [Hooks and Reactive Execution](../concepts/hooks-and-reactive-execution), [Traceability](../concepts/traceability) |
| RFC-008 | Awb procedures | [Hooks and Reactive Execution](../concepts/hooks-and-reactive-execution), [Applications, Decisions and Objections](../concepts/administrative-procedures) |
| RFC-010 | Federated corpus | [Federated Corpus](../concepts/federated-corpus) |
| RFC-011 | Rules language selection | Backlog: the "why custom YAML" rationale has no prose page |
| RFC-012 | Untranslatables (now `markings`) | [Markings](../concepts/markings) |
| RFC-013 | Execution provenance (partially built: no accepted values, no signing) | [Execution Provenance](../concepts/execution-provenance) |
| RFC-014 | Conformance suite | [Conformance](./conformance) |
| RFC-016 | Collection operations | [Collections](../concepts/collections) |
| RFC-018 | Note infrastructure | [Notes and Annotations](../concepts/notes-and-annotations) |
| RFC-019 | Law end dates | [Temporal Validity and Dates](../concepts/temporal-and-dates) |
| RFC-021 | Date comparison | [Temporal Validity and Dates](../concepts/temporal-and-dates) |
| RFC-039 | Addressable execution traces | [Traceability](../concepts/traceability#the-trace-document), [Engine](../components/engine) |
| RFC-043 | Lazy evaluation | [Engine](../components/engine#how-it-works), [Which outputs come back](../components/engine#which-outputs-come-back) |

RFC-000 (the RFC process) is documented by [rfc-000](/rfcs/rfc-000) itself; the contributing guide links to it.

## Built while still Proposed

A reader meets these constructs in a rulework or a running service whatever the status tag says, so they owe coverage on the same terms as an accepted RFC.

| RFC | Topic | Prose coverage |
|-----|-------|----------------|
| RFC-023 | Quantities (money, percentages, units) | [Law Format](../concepts/law-format), [Schema](./schema) |
| RFC-024 | Precision and rounding | [Law Format](../concepts/law-format), [Temporal Validity and Dates](../concepts/temporal-and-dates) |
| RFC-026 | Enricher work queue (partially built) | Backlog: no prose page |
| RFC-027 | Enrichment a legal expert can check (partially built) | Partly: the gates in [Pipeline](../components/pipeline#enrich-worker), the marking channel in [Markings](../concepts/markings); roles, decision points and the gold set have no page |
| RFC-031 | Markings and open norms | [Markings](../concepts/markings) |
| RFC-032 | Date parts and truncation | [Temporal Validity and Dates](../concepts/temporal-and-dates) |
| RFC-033 | Enrichment build plan | Partly: [Pipeline](../components/pipeline#enrich-worker) explains windows; the ordering by dependency layer is only named in the `ENRICH_WINDOW_MODE` setting |
| RFC-036 | Absent and unknown values | [Law Format](../concepts/law-format), [Collections](../concepts/collections) |
| RFC-037 | Type checking of laws | [Law Format](../concepts/law-format) |
| RFC-038 | Callable is not presentable | Backlog: [Schema](./schema) lists the `RECHTSPOSITIE` value it added; the entry-point rule itself has no page |
| RFC-040 | The schema documents itself | [Schema](./schema) |
| RFC-041 | A void is not scoped like a replacement | [Voiding an output](../concepts/hooks-and-reactive-execution#voiding-an-output) |
| RFC-044 | Rulework (regelwerk) as the name for a regulation in YAML (partially applied) | [Glossary](./glossary#regelrecht-specific-terms) |
| RFC-045 | Submissions and decisions in the law format (`produces` as a list, hooks on a submission, `moment`, stages, the type `decision`, `source.article`, the objection) | [Applications, Decisions and Objections](../concepts/administrative-procedures), [Law Format](../concepts/law-format#what-the-article-produces), [Calling the decision of an article](../concepts/cross-law-references#calling-the-decision-of-an-article), [Schema](./schema) |
| RFC-046 | What a cell records (`packages/cel`, `cel.yaml`, submissions, decisions and executograms, replay) | [Chronolexography](../concepts/chronolexography), [Architecture](../guide/architecture) |
| RFC-047 | Superseded: merged into RFC-045 and RFC-046 | [Applications, Decisions and Objections](../concepts/administrative-procedures) |
| RFC-048 | Reduction and synthesis (`REDUCTIE`, `source.decisions`, `source.facts`, `combine: ONLY`, `answers` in `cel.yaml`, the binding with `source.reduction`, the answerer, the sphere, `FilledBy::Answer`) | [Chronolexography](../concepts/chronolexography#answering-from-the-chronicle), [Competent Authority](../concepts/competent-authority#the-holder-of-a-policy), [Inversion of Control](../concepts/inversion-of-control#policy-that-fills-an-open-term-with-a-cells-answer) |
| RFC-049 | FOREACH with `as` binds only its name | [Collections](../concepts/collections#the-element-binding) |
| RFC-050 | Superseded: merged into RFC-045, RFC-046 and RFC-048 | [Applications, Decisions and Objections](../concepts/administrative-procedures), [Calling the decision of an article](../concepts/cross-law-references#calling-the-decision-of-an-article) |
| RFC-051 | Facts and competence in the law format (a fact in `requires`, `competent_authority` and `combine` on an open term) | [Competent Authority](../concepts/competent-authority#who-answers-an-open-term), [Schema](./schema) |
| RFC-052 | Superseded: merged into RFC-045, RFC-046, RFC-048 and RFC-051 | [Competent Authority](../concepts/competent-authority#who-answers-an-open-term) |
| RFC-053 | The appeal to the administrative court (`BEROEP`, `UITSPRAAK`, Awb 8:70 and 8:72) | [Applications, Decisions and Objections](../concepts/administrative-procedures#appeal), [Schema](./schema) |
| RFC-054 | RegelRijm, a Dutch reading of an action tree (partially built: the reading and the editor pane, not yet the presenter) | [Shared Frontend Package](../components/frontend-shared#regelrijm-regelrijmindex), [Editor](../components/frontend) |

## Backlog

Accepted RFCs whose design rationale is not yet written up as prose:

- **RFC-006 (why Rust)** and **RFC-011 (why custom YAML)** are both ratifications of language choices. A single "Design rationale" page covering both, drawing on the alternatives each RFC weighed, would close them together.

Built features that work but are thin or absent in the docs, roughly in priority order:

- **Editor collaboration**: trajects (create, invite members, roles, session branches) have a full backend and UI but no user-facing guide.
- **Editor views**: the law graph (a sheet with trace stepping, opened from a scenario) and the review tasks under *Taken*, where the output of an asynchronous AI job is checked before it lands (`/api/tasks`), have no user-facing guide.
- **WASM API surface**: the JavaScript bindings (`execute`, `executeWithTrace`, `executeMultiple`, `resolveNote`, `registerDataSource`, …) are an integration point with no reference page.
- **Data sources**: registering tabular external data for execution is implemented but unexplained.
- **TUI screens**, the **`evaluate`/`validate` CLI binaries**, the harvester's **CVDR** source, and the pipeline's **LLM-provider** selection are each implemented and lightly or never documented.

When one of these gets a page, move it up into the table above.
