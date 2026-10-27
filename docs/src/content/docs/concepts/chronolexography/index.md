---
title: "Chronolexography"
description: "How RegelRecht records legal facts as dated entries in the chronicle of the organization that establishes them, and answers later questions from that chronicle."
---

A cell is an organization that records each legal fact it establishes as a dated entry, a chronolexogram, in its own chronicle (*kroniek*). The cell answers later questions from that chronicle. The vocabulary comes from the [Chronolexografie position paper](https://chronolexografie.nl/position-paper/) and [RFC-022](/rfcs/rfc-022).

This page shows how RegelRecht implements it. The design is in [RFC-046](/rfcs/rfc-046) and [RFC-048](/rfcs/rfc-048); what is still open is in [Open Questions in Chronolexography](/concepts/chronolexography/open-questions).

Recording is an optional layer ([RFC-046](/rfcs/rfc-046)). The procedure a cell plays is modeled in the law itself ([Applications, Decisions and Objections](/concepts/administrative-procedures)).

| Part | Role | Where |
|---|---|---|
| Cell | Keeps its chronicle; records and decides through the engine | `corpus/cells/<cell>/`, `packages/cel` |
| Reduction | Gives a named answer from the chronicle | `corpus/cells/<cell>/reductions/`, or `corpus/cells/reductions/` for one of no cell |
| Policy | Binds an open term of the law to a cell's answer | `corpus/regulation/nl/uitvoeringsbeleid/` |
| Engine | Executes the law and asks one cell per question. Stores nothing | `packages/engine` |

## A cell

A cell is a directory with `cel.yaml` and its reductions, one directory per `$id` under `reductions/`. `Cell::open` opens it with its chronicle on file, `Cell::in_memory` with an empty chronicle.

```yaml
# corpus/cells/<cell>/cel.yaml
name: Het college van burgemeester en wethouders van Voorbeeldstad
category: college van burgemeester en wethouders
answers:
  eerder_vergunning_verleend: reductie_eerder_vergunning_verleend
```

The law names a role ([Competent Authority](/concepts/competent-authority)). The engine matches an `INSTANCE` to `name` and a `CATEGORY` to `category`. `answers` is the cell's catalogue: which answers it gives, and from which of its own reductions. It holds no logic, and nothing else about a cell is configured. A catalogue that names a reduction the cell does not keep is refused when the cell opens.

## What a chronicle holds

| Kind | Recorded by | Fields come from |
|---|---|---|
| Submission (`AANVRAAG`, `BEZWAAR`, `BEROEP`) | `Cell::record_submission` | The article's submission item and the hooks on it |
| Decision (`BESCHIKKING`, `UITSPRAAK`) | `Cell::decide`, `Cell::decide_after` | The deciding article's inputs, findings included |
| Executogram (`EXECUTOGRAM`) | `Cell::record_executogram` | A fact an article `requires` ([RFC-051](/rfcs/rfc-051)) |

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

A chronolexogram repeats nothing the law says. The law in force on `recorded_on` gives each field's sources and basis; a replay shows which fields held a finding. The chronicle is append-only, in date order.

## Recording and deciding

- `Cell::record_submission` runs the establishing article at the stage named after the submission's kind, so the hooks on it run. A missing required field goes under `missing`: an incomplete application is still one (Awb 4:5). The moment the law counts from is the day of recording. A submission may refer to a decision (`refers_to`), as an objection does.
- `Cell::decide` takes the id of a submission in the chronicle that requests this decision, and runs the later stages. Findings travel apart from the parameters. Each non-null output of type `decision`, such as a new decision on objection, becomes its own chronolexogram in the same append.
- `Cell::record_executogram` records a fact whole or not at all. The fact refers to nothing. A change is a new executogram; the old one stays.

The cell's code names no article of any law; everything it checks comes from the law. It refuses:

- any field the law does not give, and a wrong type;
- a `moment` supplied by the submitter;
- a decision on an id that is not a submission requesting it, a second decision on one submission, and replacing a decision twice;
- in a decision, a value from the caller that comes from elsewhere, and a finding of the wrong type on a term of the deciding article;
- a fact the article does not require, or under an article not in force that day;
- a fact with a field named `id` (rows use `id` for the executogram itself);
- a fact whose `competent_authority` does not name the cell.

The engine refuses a finding that no article in the decision asks for, or one of the wrong type.

A decision output of another legal character than the deciding article's own stays a value of the decision: a judgment can hold the decision the court sets in the place of the annulled one, and the court's cell does not record it as a decision of its own.

## Answering from the chronicle

A reduction gives a named answer from what the cell recorded earlier, such as "was this permit granted before?". It is the only rulework that reads a chronicle.

```yaml
# corpus/cells/gemeente_voorbeeldstad/reductions/reductie_eerder_vergunning_verleend/<valid_from>.yaml
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

The law does not ask the cell. The policy of the authority that decides binds the open term to the answer:

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

- How a cell reduces is up to the cell; the engine only asks it for an answer by name. The cell in `packages/cel` uses what the engine offers: rulework of its own kind (`REDUCTIE`) in the cell's directory. The directory names its cell, so the reduction names no `competent_authority`. It reads decisions (`source.decisions`, by an article or by a parameter that names one) or facts (`source.facts`), and may call policy for a norm. It may also call a reduction of no cell, in `corpus/cells/reductions/`. Such a reduction has no chronicle of its own and no cell gives it as an answer; it reads the chronicle of the cell that runs its caller. The reduction of the Awb is one: given an article, it gives the decisions under it that hold after objection and appeal. A reduction never calls a reduction of another cell, because that one would read the wrong chronicle. The corpus loader of the PoC refuses such a call, and its tests hold the whole corpus to it.
- Wherever the binding runs, the engine asks the cell for the answer. The cell runs the reduction that its catalogue names. The binding is an article of policy, so its value becomes the value of the term, even when it is null. The authority's own finding cannot replace it ([Inversion of Control](/concepts/inversion-of-control#policy-that-fills-an-open-term-with-a-cells-answer)). If the cell has no answer and the authority should judge the case, the policy says so with an open term of its own (`decided_per_case_by`).
- The loader refuses `source.decisions` and `source.facts` outside a `REDUCTIE`, and `source.reduction` outside such a binding.

## Documents between cells

Each actor keeps its own chronicle, and the law sends documents between them: the copy of the contested decision with the beroepschrift (Awb 6:5 lid 2), and the copy of the judgment to the parties (Awb 8:79 lid 1). A cell takes in another cell's document as a copy, never by reading that cell's chronicle.

- `Cell::afschrift` makes a copy of one of the cell's own decisions: the chronolexogram, its decision value from a replay in that cell, and `about`, what the submission it was taken on had received. Where a field of type `decision` holds an id, the copy holds that decision's value instead, nested the same way. Only the cell that took the decision makes a copy.
- `record_submission` with `Reference::Received(&copy)` records a submission on a decision of another cell. The chronolexogram holds `received: {from, id}` instead of `refers_to`, and its field of type `decision` holds the decision value itself, because this cell cannot replay it.
- `Cell::record_receipt` records that the cell received a copy, as an executogram of the fact the law requires for it. The receipt is the one executogram that refers to a decision: the cell's own decision that the copy concerns.
- `Cell::decide_after` decides on a submission again after a later chronolexogram that refers to the last decision on it, such as a receipt. The new decision holds that chronolexogram as `after`. The rule names no article; whether the earlier decision still counts is for whoever reads the chronicle.

The cell refuses a copy of what is no decision, a copy of its own decision offered back to it, a receipt for a decision the copy does not concern or dated before it, and a decision taken again without such a later chronolexogram. Opening a chronicle checks `after` and every receipt the same way. A reduction reads the receipts as it reads any recorded fact; the row of a receipt also gives the decision it concerns, under `refers_to`. [RFC-053](/rfcs/rfc-053) gives the design, for the appeal to the administrative court.

## Answers of other cells

A decision may take other cells along, such as the GGD for the parking card or the Chamber of Commerce for the business parking permit. When a binding fills an open term with `competent_authority`, the engine asks the cell of the call that the term names: the deciding cell, or one it took along ([Competent Authority](/concepts/competent-authority#who-answers-an-open-term)). Without competence on the term, it asks the cell of the policy's holder. A cell without that answer in its catalogue is silent.

One question goes to one cell. If the call holds two cells that the term names, the decision fails, and the engine never merges answers. Which single cell of a category should answer is an [open question](/concepts/chronolexography/open-questions).

Synthesis, in the paper's sense, is what the asking article does with those answers. It takes the answers of different cells, each to its own question, and makes one outcome of them: the parking card follows from the GGD's advice and from the municipality's own record of earlier cards. Each cell reduces only its own chronicle.

## Explaining a decision

The cell stores no reasons. It replays.

- `Cell::explain` retakes the decision on its own day, with the law and chronicle of that moment. It fails if the outcome differs.
- `Cell::findings` reads from the replay which findings counted.
- `Cell::explain_answer` reruns a reduction to show its reasoning. In the decision's trace the answer is one step.

## Try it

- `cargo test --manifest-path packages/cel/Cargo.toml` runs the cases in `corpus/cells/`.
- The [PoC portal](/components/poc-portal#chronolexografie) plays them in a browser.
