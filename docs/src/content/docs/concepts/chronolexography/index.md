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

## Recording and deciding

- `Cell::record_submission` runs the article that establishes the submission. It runs that article at the stage named after the kind of submission, so the hooks on the submission run too. A required field that is missing goes under `missing`, because an incomplete application is still an application (Awb 4:5). The moment the law counts from is the day the cell records the submission. A submission may refer to a decision (`refers_to`), as an objection does. The cell does not store what the article and its hooks give. `Cell::submission_outputs` computes it again from the fields, under the law in force on `recorded_on`. A decision on the submission computes it the same way, for example whether an objection came in time (Awb 6:9).
- `Cell::decide` takes the id of a submission in the chronicle that asks for this decision. It then runs the later stages. A finding is what the authority itself decides on an open norm in a single case. Findings are passed separately from the parameters. Some outputs have the type `decision`, such as a new decision taken on an objection. Each of those that is not null becomes a chronolexogram of its own, recorded in the same step.
- `Cell::record_executogram` records a fact completely or not at all. The fact refers to no other entry. To change a fact, the cell records a new executogram. The old one stays.

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
      source:
        decisions: wet_voorbeeld#3      # one row per decision under art. 3
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

- Each cell decides for itself how it reduces. The engine only asks it for an answer by name. The cell in `packages/cel` uses what the engine offers: rulework of its own kind (`REDUCTIE`) in the cell's own directory. The directory says whose reduction it is, so the reduction names no `competent_authority`. A reduction reads decisions or facts (`source.decisions`, `source.facts`). For decisions it names an article, or a parameter that names one. It may call policy for a norm.
- A reduction may also call a reduction of no cell, in `cells/reductions/`. Such a reduction has no chronicle of its own, and no cell gives it as an answer. It reads the chronicle of the cell that runs the reduction that calls it. The reduction of the Awb is one: given an article, it gives the decisions under that article that still hold after objection and appeal.
- A reduction never calls a reduction of another cell, because that reduction would read the wrong chronicle. The corpus loader of the PoC refuses such a call, and its tests check the whole corpus for it.
- The engine asks the cell for the answer wherever the binding runs. The cell then runs the reduction that its catalogue names for that answer. Because the binding is an article of policy, its value becomes the value of the term, even when that value is null. A finding of the authority cannot replace it ([Inversion of Control](/concepts/inversion-of-control#policy-that-fills-an-open-term-with-a-cells-answer)). Sometimes the cell has no answer and the authority should judge the case itself. The policy then says so with an open term of its own (`decided_per_case_by`).
- The loader refuses `source.decisions` and `source.facts` outside a `REDUCTIE`. It refuses `source.reduction` outside such a binding.

### Playing the chronicle forward (experimental)

Schema v0.8.14 adds a second way to reduce. A reduction with a `projection` declares a state, a few named properties each with a type, and says per article what a record under it does with that state. A property starts from the value of its type: false for a boolean, 0 for a number or an amount, null for a string or a date. The engine plays the chronicle forward, record by record, and the reduction's `execution` answers from the last state as `$state.<property>`. The loader refuses an answer that reads a property the state does not declare, or the state as a whole, and an article without a projection that reads `$state`.

A step names the article its records rest on, such as `test_chronicle_effects#3`. Where the article gives more than one kind of record, such as an application and the decision on it, a third segment names the kind: `test_chronicle_effects#1#BESCHIKKING`. A step on an article that requires a fact applies to the recorded instances of that fact.

What a record does with the state is law, so a step does not compute it. It is written like an article: its `input` takes outputs of a law through `source.regulation`, with parameters from a field of the record (`$record.<field>`) or a property of the state (`$state.<property>`), and its `actions`, written as the actions of an article, give properties of the state their values: `output` names a property and `value` is a bare reference to an input of the step, a field of the record or a property of the state. A property no action names keeps its value, and one step gives a property one action at most. In the parking-card test, article 7 of the test law says that a grant gives the applicant a card and raises the number of grants by one, and article 8 says that a card handed in is gone:

```yaml
steps:
  - record: test_chronicle_effects#1#BESCHIKKING
    input:
      - name: in_bezit_na_besluit
        type: boolean
        source:
          regulation: test_chronicle_effects
          output: in_bezit_na_besluit
          parameters:
            in_bezit: $state.in_bezit
            aantal_verleend: $state.aantal_verleend
            kaart_verleend: $record.kaart_verleend
      # aantal_verleend_na_besluit in the same way
    actions:
      - output: in_bezit
        value: $in_bezit_na_besluit
      - output: aantal_verleend
        value: $aantal_verleend_na_besluit
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

The engine calls the law in the version in force on the day the record was recorded, not on the calculation date, the same rule as for effects. The article it calls reads only what the step passes: an article with an `input`, with or without a `source`, or with open terms cannot be called, so a step stays a function of the record and the state before it. A step takes no other source than `source.regulation`, and its actions take no operation and no literal. Each parameter must be one the called article takes, of the type of the field or property it comes from, each input must have the type of the output it takes, and each value an action gives must fit its property. The loader checks the shape of the step; the engine checks the called law when it plays the projection, because the reduction and the law are separate regelwerken that load in any order. In the trace, the call hangs under the node of the record, like any law call, and that node shows every property the step set, before and after.

What one decision does with another stays in the law. The article declares it in `produces[].effects`: `revokes`, optionally only `when` a boolean outcome of the decision is true, such as a well-founded objection. A decision in the place of another revokes it and counts as a record of its own. When a decision that counts revokes an earlier one, the engine plays the chronicle again from the start without the revoked decision. A decision that is itself revoked revokes nothing any more, so what it alone revoked counts again. The engine records which record set each property last and every record it applied or skipped, and shows them in the trace.

A chronicle holds the records of many people. With `key`, a projection plays the records of one of them: the records that hold the value asked for under the key's name, as a field or an output, such as an applicant, and every record that refers to one of those, through `refers_to` or a field an effect names. An application holds the applicant and so plays as well, without a step. An objection and the decision on it usually carry no applicant and join through the grant they refer to. A record that refers in but holds another key of its own belongs to someone else. It still counts for its effects: a decision that moves a parking permit to a new licence plate revokes the permit for the old plate too, because the law says once what the decision does, whichever key the question is about. Its step is not applied, and the trace shows it as counting for its effects only. The same holds for a record without a key that refers only to such records, such as the revocation of that move, which brings the permit back for the old plate. Records never join through what they themselves refer to, so the partition stays the records reachable from the key. An effect on a record outside the partition is ignored.

A policy reaches a projection the normal way. When an article binds an open term to the cell's answer (`source.reduction`), the engine asks the cell, and the cell plays its chronicle through the reduction's projection, with the parameters the binding passes, such as the key. The engine refuses a reduction with a projection on every path that bypasses the chronicle: asked directly, from another rulework, at a stage, or with a `state` the caller passes. The tests are in `packages/cel/bdd`; one of them has a fictional policy take the cell's answer, and its outcome flips when a well-founded objection revokes the grant.

## Documents between cells

Each actor keeps its own chronicle. The law sends documents from one actor to another. With an appeal, the court receives a copy of the contested decision together with the notice of appeal (Awb 6:5 lid 2). After the judgment, the parties receive a copy of it (Awb 8:79 lid 1). A cell takes in another cell's document as a copy. It never reads the other cell's chronicle.

- `Cell::afschrift` makes a copy of one of the cell's own decisions. The copy holds the chronolexogram, the decision value from a replay in that cell, and `about`: what the submission behind the decision had received. When a field of type `decision` holds an id, the copy holds the value of that decision instead, nested in the same way. Only the cell that took the decision makes a copy of it.
- `record_submission` with `Reference::Received(&copy)` records a submission against a decision of another cell. The chronolexogram then holds `received: {from, id}` instead of `refers_to`. Its field of type `decision` holds the decision value itself, because this cell cannot replay the other cell's decision.
- `Cell::record_receipt` records that the cell received a copy. It does so as an executogram of the fact that the law requires for it. The receipt is the only executogram that refers to a decision: the cell's own decision that the copy is about.
- `Cell::decide_after` decides on a submission again. It needs a later chronolexogram, such as a receipt, that refers to the last decision on that submission. The new decision holds that chronolexogram as `after`. This rule names no article. Whether the earlier decision still counts is up to whoever reads the chronicle.

The cell refuses four things here:

- a copy of something that is not a decision;
- a copy of its own decision, offered back to it;
- a receipt for a decision that the copy is not about, or a receipt dated before the decision in the copy;
- a decision taken again without such a later chronolexogram.

When it opens a chronicle, the cell checks `after` and every receipt in the same way. A reduction reads receipts like any other recorded fact. The row of a receipt also gives the decision it is about, under `refers_to`. [RFC-053](/rfcs/rfc-053) gives the design, for the appeal to the administrative court.

## Answers of other cells

A decision can take other cells along, such as the Chamber of Commerce for the business parking permit. Suppose a binding fills an open term that has its own `competent_authority`. The engine then asks the cell in the call that the term names: the deciding cell, or a cell the decision took along ([Competent Authority](/concepts/competent-authority#who-answers-an-open-term)). If the term names no competent authority, the engine asks the cell of the authority that holds the policy. A cell that does not have the answer in its catalogue stays silent.

One question goes to one cell. If the call holds more than one cell that the term names, the decision fails. The engine never merges answers. Which cell of a category should answer is an [open question](/concepts/chronolexography/open-questions).

Synthesis is what the asking article does with those answers. It takes answers from different cells, each to its own question, and makes one outcome of them. The business parking permit, for instance, follows from what the Chamber of Commerce records about the company and from the municipality's own record of permits. Each cell reduces only its own chronicle.

## Explaining a decision

The cell stores no reasons. It explains a decision by taking it again.

- `Cell::explain` takes the decision again on its original day, with the law and the chronicle as they were then. It fails if the outcome differs.
- `Cell::findings` reads from that replay which findings counted.
- `Cell::explain_answer` runs a reduction again to show how it reached its answer. In the trace of the decision, the answer is a single step.

## Try it

- `cargo test --manifest-path packages/cel/Cargo.toml` runs the tests of the cell.
- The [PoC portal](/components/poc-portal#chronolexografie) plays the cases in a browser.
