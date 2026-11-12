---
title: "Chronolexography"
description: "How RegelRecht records legal facts as dated entries in the chronicle of the organization that establishes them, and answers later questions from that chronicle."
---

A cell is an organization that keeps a record of the legal facts it establishes. Each fact goes into the cell's own chronicle (*kroniek*) as a dated entry, called a chronolexogram. When a later question comes up, such as "was this permit granted before?", the cell answers it from that chronicle. The vocabulary comes from the [Chronolexografie position paper](https://chronolexografie.nl/position-paper/) and [RFC-022](/rfcs/rfc-022).

This page shows how RegelRecht implements it. The design is in [RFC-046](/rfcs/rfc-046) and [RFC-048](/rfcs/rfc-048). What is still open is in [Open Questions in Chronolexography](/concepts/chronolexography/open-questions).

Recording is an optional layer on top of the law ([RFC-046](/rfcs/rfc-046)). The procedure that a cell follows, from application to appeal, is modeled in the law itself ([Applications, Decisions and Objections](/concepts/administrative-procedures)).

| Part | Role | Where |
|---|---|---|
| Cell | Keeps its chronicle. Records submissions and takes decisions through the engine | `cells/<cell>/` of a corpus, `packages/cel` |
| Reduction | Reads the chronicle and gives an answer under a name | `cells/<cell>/reductions/`, or `cells/reductions/` for one of no cell |
| Policy | Says that an open term of the law takes a cell's answer | `regulation/nl/uitvoeringsbeleid/` |
| Engine | Executes the law and asks one cell per question. Stores nothing | `packages/engine` |

The paths are relative to the root of a corpus. The cells and cases in this repository are in the corpus of the [PoC](/components/poc-portal#chronolexografie), `packages/poc-chronolexografie/corpus/`.

## A cell

A cell is a directory with a file `cel.yaml` and its reductions, one directory per `$id` under `reductions/`. `Cell::open` opens the cell with the chronicle stored on disk. `Cell::in_memory` opens it with an empty chronicle.

```yaml
# cells/<cell>/cel.yaml
name: Het college van burgemeester en wethouders van Voorbeeldstad
category: college van burgemeester en wethouders
answers:
  eerder_vergunning_verleend: reductie_eerder_vergunning_verleend
```

The law names a role, not an organization ([Competent Authority](/concepts/competent-authority)). The engine matches a role of type `INSTANCE` to the cell's `name`, and a role of type `CATEGORY` to its `category`.

`answers` is the cell's catalogue. It lists the answers the cell gives, and for each one the reduction of its own that computes it. The catalogue holds no logic. Nothing else about a cell is configured. When the catalogue names a reduction that the cell does not keep, the cell refuses to open.

## What a chronicle holds

A chronicle holds three kinds of entry. An executogram is a fact that an article requires, recorded by the cell that establishes it.

| Kind | Recorded by | Where the fields come from |
|---|---|---|
| Submission (`AANVRAAG`, `BEZWAAR`, `BEROEP`) | `Cell::record_submission` | The submission item of the article, and the hooks on it |
| Decision (`BESCHIKKING`, `UITSPRAAK`) | `Cell::decide`, `Cell::decide_after` | The inputs of the deciding article, including the findings |
| Executogram (`EXECUTOGRAM`) | `Cell::record_executogram` | A fact that an article `requires` ([RFC-051](/rfcs/rfc-051)) |

```json
{
  "id": "019a1b2d-…",
  "kind": "BESCHIKKING",
  "basis": "wet_voorbeeld#3",
  "recording_actor": "gemeente_voorbeeldstad",
  "recorded_on": "2026-10-20",
  "refers_to": "019a1b2c-…",
  "fields": { "bsn_aanvrager": "999993653", "medische_beoordeling": true },
  "outputs": { "vergunning_verleend": true, "bezwaartermijn_einddatum": "2026-12-01" }
}
```

A chronolexogram does not copy anything the law already says. The law in force on `recorded_on` tells where each field came from and on what legal basis. To see which fields were the authority's findings, the cell takes the decision again (a replay). The chronicle only grows: entries are added in date order and never changed.

A cell in `packages/cel` keeps one chronicle. The position paper lets a cell keep several chronicles (chronolexokronieken), and building that is on [the open questions page](/concepts/chronolexography/open-questions).

## Recording and deciding

- `Cell::record_submission` runs the article that establishes the submission. It runs that article at the stage named after the kind of submission, so the hooks on the submission run too. A required field that is missing goes under `missing`, because an incomplete application is still an application (Awb 4:5). The moment the law counts from is the day the cell records the submission. A submission may refer to a decision (`refers_to`), as an objection does. The cell does not store what the article and its hooks give. `Cell::submission_outputs` computes it again from the fields, under the law in force on `recorded_on`. A decision on the submission computes it the same way, for example whether an objection came in time (Awb 6:9).
- `Cell::decide` takes the id of a submission in the chronicle that asks for this decision. It then runs the later stages. A finding is what the authority itself decides on an open norm in a single case. Findings are passed separately from the parameters. Some outputs have the type `decision`, such as a new decision taken on an objection. The cell checks each of those that is not null by taking it again under its own article, and keeps it as the value of that output. It records no chronolexogram for it: revoking a decision and putting a new one in its place are one act, so they are one record. When that record is revoked, the new decision falls with it.
- `Cell::record_executogram` records a fact completely or not at all. `Cell::record_executogram_referring` records one that refers to an earlier entry. To change a fact, the cell records a new executogram. The old one stays.

The code of the cell names no article of any law. Everything it checks comes from the law. The cell refuses:

- a field that the law does not give, and a field of the wrong type;
- a `moment` that the submitter supplies;
- a decision on an id that is not a submission asking for it;
- a second decision on one submission, and a second replacement of the same decision;
- in a decision, a value from the caller that the law takes from somewhere else;
- a finding of the wrong type on a term of the deciding article;
- a fact that the article does not require, or a fact under an article that is not in force that day;
- a fact with a field named `id`, because the rows use `id` for the executogram itself;
- a fact whose `competent_authority` does not name the cell.

The engine refuses a finding that no article in the decision asks for, and a finding of the wrong type.

Sometimes a decision has an output with a different legal character than the deciding article. That output stays a value inside the decision. For example, a judgment can hold the decision that the court sets in place of the annulled one. The court's cell does not record that decision as a decision of its own.

## Answering from the chronicle

A reduction reads what the cell recorded earlier and gives an answer under a name, such as `eerder_vergunning_verleend` ("was this permit granted before?"). It is the only kind of rulework that reads a chronicle.

```yaml
# cells/gemeente_voorbeeldstad/reductions/reductie_eerder_vergunning_verleend/<valid_from>.yaml
regulatory_layer: REDUCTIE
execution:                         # shortened
  parameters: [bsn_aanvrager]
  input:
    - name: besluiten
      type: array
      source:                           # one row per decision under art. 3
        projections: reductie_besluit_voorbeeld#besluit
        where: {bsn_aanvrager: $bsn_aanvrager}
  output:
    - name: eerder_vergunning_verleend
```

The law itself never asks the cell. The deciding authority's policy does. It fills the open term of the law with the cell's answer:

```yaml
# beleid_vergunningen_voorbeeldstad, art. 1 (UITVOERINGSBELEID)
implements:
  - {law: wet_voorbeeld, article: '3', open_term: eerder_vergunning_verleend}
execution:
  input:
    - name: antwoord
      nullable: true
      source:
        reduction: eerder_vergunning_verleend
        parameters: {bsn_aanvrager: $bsn_aanvrager}
```

This policy article is called a binding: it binds the open term to the answer.

- Each cell decides for itself how it reduces. The engine only asks it for an answer by name. The cell in `packages/cel` uses what the engine offers: rulework of its own kind (`REDUCTIE`) in the cell's own directory. The directory says whose reduction it is, so the reduction names no `competent_authority`. Each article of a reduction with a `machine_readable` is an aggregate, which plays the records of the chronicle, or a read model, which reads aggregates (`source.projections`) and may call policy for a norm. One file may hold both. Which forms earlier schema versions allowed is in the [version history](/reference/schema#version-history). A rule that only combines other rulework reads no chronicle and is no reduction; it belongs in the authority's policy.
- A reduction may also call a reduction of no cell, in `cells/reductions/`. Such a reduction has no chronicle of its own, and no cell gives it as an answer. It reads the chronicle of the cell that runs the reduction that calls it.
- A reduction never calls a reduction of another cell, because that reduction would read the wrong chronicle. The corpus loader of the PoC refuses such a call.
- The engine asks the cell for the answer wherever the binding runs. The cell then runs the reduction that its catalogue names for that answer. Because the binding is an article of policy, its value becomes the value of the term, even when that value is null. A finding of the authority cannot replace it ([Inversion of Control](/concepts/inversion-of-control#policy-that-fills-an-open-term-with-a-cells-answer)). Sometimes the cell has no answer and the authority should judge the case itself. The policy then says so with an open term of its own (`decided_per_case_by`).
- The loader refuses `source.projections` outside a `REDUCTIE`, as it does the older `source.decisions` and `source.facts`. It refuses `source.reduction` outside such a binding.

### Playing the chronicle forward (experimental)

A reduction with a `projection` declares a state, a few named properties each with a type, and says per article what a record under it does with that state. A property starts from the value of its type: false for a boolean, 0 for a number or an amount, null for a string or a date. The engine plays the chronicle forward, record by record, one aggregate per record of a kind (`per`, see [One aggregate per decision](#one-aggregate-per-decision-experimental)). A read model answers from their last states.

A step names the article its records rest on, such as `test_chronicle_effects#3`. Where the article gives more than one kind of record, a third segment names the kind (`#BESCHIKKING`). Where it requires more than one fact, the third segment is the name of the fact in lower case (`#plaats_gehuurd`). `per` takes the same notation. The loader refuses a step that could apply to more than one of them. The rules per schema version are in the [version history](/reference/schema#version-history).

What a record does with the state is law, so a step does not compute it. It is written like an article: its `input` takes outputs of a law through `source.regulation`, with parameters from a field of the record (`$record.<field>`) or a property of the state (`$state.<property>`). Its `actions` give properties of the state their values. A value is a bare reference: an input of the step, a field of the record or a property of the state. A property no action names keeps its value. In the parking-card test, article 7 of the test law says that a grant gives the applicant a card, and article 8 says that a card handed in is gone:

```yaml
steps:                                # shortened
  - record: test_chronicle_effects#1#BESCHIKKING
    input:
      - name: in_bezit_na_besluit
        type: boolean
        source:
          regulation: test_chronicle_effects
          output: in_bezit_na_besluit
          parameters:
            in_bezit: $state.in_bezit
            kaart_verleend: $record.kaart_verleend
    actions:
      - output: in_bezit
        value: $in_bezit_na_besluit
  - record: test_chronicle_effects#5
    input:
      - name: in_bezit_na_inlevering
        type: boolean
        source:
          regulation: test_chronicle_effects
          output: in_bezit_na_inlevering
    actions:
      - output: in_bezit
        value: $in_bezit_na_inlevering
```

A step is a function of the record and the state before it. It passes no parameter of the question, so the fold depends on the chronicle alone; a day the question asks about is compared in an article of the law when the question is answered. The engine calls the law in the version in force on the day the record was recorded, not on the calculation date. The output a step takes may read nothing else: no `input` and no open term, directly or through another output it reads. Another output of the same article may read either. `$referencedate` is the day of the record.

When the reduction loads, the loader checks each step against the laws it already holds, and the engine checks again when it plays. In the trace, the call hangs under the node of the record, which shows every property the step set, before and after.

#### Which records count

What one record does with another stays in the law as well. An article declares it in `produces[].effects`, and a fact on its `requires` entry: `revokes`, optionally only `when` a boolean of the same record is true: an output of a decision, a field of a fact. A decision on a well-founded objection revokes the contested decision this way. The receipt of a copy of a court ruling ([RFC-053](/rfcs/rfc-053)) revokes the decision it refers to when the ruling annuls it:

```yaml
# test_chronicle_effects, article 11 (fictional)
requires:
  - fact: afschrift_uitspraak_ontvangen
    parameters: [vernietigd]
    effects:
      - revokes: refers_to
        when: vernietigd
```

`revokes` has one meaning: the record does not count, from the start (ex tunc). That covers a decision revoked on objection, a decision the court annuls, and a ruling annulled on further appeal, which brings back what it annulled. A change from a later day on (ex nunc), such as a suspension or a withdrawal "met ingang van" a date, is no effect. It is a step of the reduction with a date property, as for any other change of state, and the law compares that date with the day asked about.

A record counts unless a later record that itself counts revokes it. A revoked record revokes nothing, so what it alone revoked counts again. Because this never depends on the state, the engine decides it first, in one pass from the last record back. It then runs the steps of the records that count, in the order the cell recorded them. The trace shows the first phase as a `COUNTS` node and the steps as a `FOLD` node.

`refers_to` names, on every record, the record this record is about: a submission the decision it contests, a decision the submission it was taken on, a fact the record it concerns. A decision taken on no submission, such as a withdrawal ex officio, refers to the decision it affects; the cell takes one with `Cell::decide_ex_officio`. A decision revokes only a record its `refers_to` reaches, directly or through the records it refers to in turn: the decision on an objection reaches the contested decision through the objection. The effect still names the record through a parameter of the article, such as `revokes: bestreden_besluit`; `refers_to` only limits which record that may be, and the cell refuses another. The rule counts no fixed number of steps and knows no procedure, so a decision on objection and a new decision after the court annulled one follow it alike.

A correction of a recorded fact is a fact of its own. One that corrects a single field has a step that sets that property and leaves the rest. One that says an earlier fact was wrong from the start revokes it, and a new fact takes its place. The tests are in `packages/cel/bdd/features/corrections.feature`.

A policy reaches a projection the normal way: the binding asks the cell, and the cell plays its chronicle through the reduction's projection with the parameters the binding passes. The engine refuses a reduction with a projection on every path that bypasses the chronicle: asked directly, from another rulework, at a stage, or with a `state` the caller passes. The tests are in `packages/cel/bdd`.

### One aggregate per decision (experimental)

A state holds single values. One state per person breaks down once a person holds several decisions, each with its own days, such as two business parking permits of one company. A projection therefore plays one aggregate per record of a kind (`per`). Each such record is the root of an aggregate. The aggregate holds the root and every record that refers to it through `refers_to`, directly or through another record of the aggregate. Another record of the root's kind that refers into the aggregate is an aggregate of its own and counts in this one for its effects only. A root that does not count applies no step; the other records of its aggregate that count still apply theirs.

```yaml
# reductie_bedrijfsparkeervergunning (PoC), article vergunning
projection:
  per: parkeerverordening_gemeente_wilgendaal#2#BESCHIKKING
  state:
    - {name: besluit_datum, type: date}
    - {name: vergunning_geldig_tot, type: date}
  steps:
    - record: parkeerverordening_gemeente_wilgendaal#2#BESCHIKKING
      actions:
        - {output: besluit_datum, value: $record.besluit_datum}
        - {output: vergunning_geldig_tot, value: $record.vergunning_geldig_tot}
```

A projection per record answers no question of its own and has no `execution`. A read model reads its aggregates with `source.projections`, which names the article of the aggregate (`<regulation>#<article>`). It gets one row per root the chronicle holds on the calculation date: the last state of its aggregate, with the root's `id`. With `where`, it reads only the roots whose field or output of a given name equals a parameter of the reading article, such as `kvk_nummer: $kvk_nummer`. An article of policy says what the rows mean on the day asked about.

A decision put in the place of another lands in the aggregate of the decision it replaces. On objection, revoking the contested decision and taking a new one is one decision, and the cell records it as one record: the new decision is the value of its output `nieuw_besluit`. The decision on the objection refers to the objection, which refers to the contested decision, so it joins that aggregate. Because the new decision is part of that record, it falls with it: when the court annuls the decision on the objection, the contested decision counts again and the new decision in its place is gone (`a_grant_on_objection_the_court_annulled_no_longer_counts` in `packages/cel/tests/beroep.rs`). No rule says that a decision falls with the decision it refers to. The receipt revokes one record, and that record holds both. A step reads the id of its own record as `$record.id`, and the record it refers to as `$record.refers_to`. An aggregate can keep which of its records grants the permit now by storing that id in its state.

A row does not say whether its root counts. A root that does not count applies no step, so a property its step sets keeps its start value. A reduction that needs to know therefore has its step on the root set a property of its own, such as `verleend`, which a revoked root leaves false.

A recorded fact is played the same way, one aggregate per recorded instance, with `per` naming the fact where its article requires more than one. Real legislation often lays down two acts in one article, so the article stays whole rather than being split to suit the reductions. The step on the root copies the fields of the fact into the state, and the read model gets one row per instance. A revoked root keeps its row, with the start values of its state, so a read model that counts rows has to skip it.

The engine plays each aggregate on every question, in the version of the reduction in force on the calculation date. The trace of the reading input holds a `PROJECTION` node per aggregate, with its `COUNTS` and `FOLD`. The cell gives the records of its chronicle for it (`Chronicle::records`); `evaluate_projection` plays a read model on the records it is given.

## Documents between cells

Each actor keeps its own chronicle. The law sends documents from one actor to another. With an appeal, the court receives a copy of the contested decision together with the notice of appeal (Awb 6:5 lid 2). After the judgment, the parties receive a copy of it (Awb 8:79 lid 1). A cell takes in another cell's document as a copy. It never reads the other cell's chronicle.

- `Cell::afschrift` makes a copy of one of the cell's own decisions. The copy holds the chronolexogram, the decision value from a replay in that cell, and `about`: what the submission behind the decision had received. When a field of type `decision` holds an id, the copy holds the value of that decision instead, nested in the same way. Only the cell that took the decision makes a copy of it.
- `record_submission` with `Reference::Received(&copy)` records a submission against a decision of another cell. The chronolexogram then holds `received: {from, id}` instead of `refers_to`. Its field of type `decision` holds the decision value itself, because this cell cannot replay the other cell's decision.
- `Cell::record_receipt` records that the cell received a copy. It does so as an executogram of the fact that the law requires for it. The receipt refers to the cell's own decision that the copy is about.
- `Cell::decide_after` decides on a submission again. It needs a later chronolexogram, such as a receipt, that refers to the last decision on that submission. The new decision holds that chronolexogram as `after`. This rule names no article. Whether the earlier decision still counts is up to whoever reads the chronicle.

The cell refuses these:

- a copy of something that is not a decision;
- a copy of its own decision, offered back to it;
- a receipt for a decision that the copy is not about, or a receipt dated before the decision in the copy;
- a second receipt of the same copy, so a copy sent twice is recorded once;
- a decision taken again without such a later chronolexogram;
- a fact that refers to a record not in the chronicle.

When it opens a chronicle, the cell checks `after` and every receipt in the same way, and that a fact that refers to a record without having received a copy refers to one before it. A reduction reads receipts like any other recorded fact. The row of a receipt also gives the decision it is about, under `refers_to`. [RFC-053](/rfcs/rfc-053) gives the design, for the appeal to the administrative court.

## Answers of other cells

A decision can take other cells along, such as the Chamber of Commerce for the business parking permit. Suppose a binding fills an open term that has its own `competent_authority`. The engine then asks the cell in the call that the term names: the deciding cell, or a cell the decision took along ([Competent Authority](/concepts/competent-authority#who-answers-an-open-term)). If the term names no competent authority, the engine asks the cell of the authority that holds the policy. A cell that does not have the answer in its catalogue stays silent.

One question goes to one cell. If the call holds more than one cell that the term names, the decision fails. The engine never merges answers. Which cell of a category should answer is an [open question](/concepts/chronolexography/open-questions).

Synthesis is what the asking article does with those answers. It takes answers from different cells, each to its own question, and makes one outcome of them. The business parking permit, for instance, follows from what the Chamber of Commerce records about the company and from the municipality's own record of permits. Each cell reduces only its own chronicle.

## Explaining a decision

The cell stores no reasons. It explains a decision by taking it again.

- `Cell::explain` takes the decision again on its original day, with the law and the chronicle as they were then. It fails if the outcome differs.
- `Cell::findings` reads from that replay which findings counted.
- In the trace of the decision, the node of a cell's answer holds the run of the cell's reduction. That run includes the `PROJECTION` node and, under each record that counted, the law calls of its step. The engine asks the cell for this run only in a traced execution.
- `Cell::explain_answer` runs a reduction again on its own, for one answer of a decision.

## Try it

- `cargo test --manifest-path packages/cel/Cargo.toml` runs the tests of the cell.
- The [PoC portal](/components/poc-portal#chronolexografie) plays the cases in a browser.
