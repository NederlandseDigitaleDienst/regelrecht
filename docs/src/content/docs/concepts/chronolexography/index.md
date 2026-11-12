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

- Each cell decides for itself how it reduces. The engine only asks it for an answer by name. The cell in `packages/cel` uses what the engine offers: rulework of its own kind (`REDUCTIE`) in the cell's own directory. The directory says whose reduction it is, so the reduction names no `competent_authority`. Since schema v0.8.19 each article of a reduction with a `machine_readable` takes one of two forms. An aggregate plays the records of the chronicle, decisions and recorded facts alike. A read model reads aggregates (`source.projections`) and may call policy for a norm; see [One aggregate per decision](#one-aggregate-per-decision-experimental). One file may hold both. An article with only text is allowed. Up to schema v0.8.16 a reduction read decisions directly (`source.decisions`), and up to v0.8.18 recorded facts (`source.facts`); the loader refuses each from the version after. A rule that only combines other rulework reads no chronicle and is no reduction. Such a rule belongs in the authority's policy. In the PoC, article 3 of the head's policy decides whether leave falls outside the school holidays.
- A reduction may also call a reduction of no cell, in `cells/reductions/`. Such a reduction has no chronicle of its own, and no cell gives it as an answer. It reads the chronicle of the cell that runs the reduction that calls it. No reduction of the PoC is of this kind any more: which decisions still hold after objection and appeal follows from the effects in its Awb (see below).
- A reduction never calls a reduction of another cell, because that reduction would read the wrong chronicle. The corpus loader of the PoC refuses such a call, and its tests check the whole corpus for it.
- The engine asks the cell for the answer wherever the binding runs. The cell then runs the reduction that its catalogue names for that answer. Because the binding is an article of policy, its value becomes the value of the term, even when that value is null. A finding of the authority cannot replace it ([Inversion of Control](/concepts/inversion-of-control#policy-that-fills-an-open-term-with-a-cells-answer)). Sometimes the cell has no answer and the authority should judge the case itself. The policy then says so with an open term of its own (`decided_per_case_by`).
- The loader refuses `source.projections`, `source.decisions` and `source.facts` outside a `REDUCTIE`. It refuses `source.reduction` outside such a binding.

### Playing the chronicle forward (experimental)

Schema v0.8.14 adds a second way to reduce. A reduction with a `projection` declares a state, a few named properties each with a type, and says per article what a record under it does with that state. A property starts from the value of its type: false for a boolean, 0 for a number or an amount, null for a string or a date. The engine plays the chronicle forward, record by record. Since schema v0.8.17 it plays one aggregate per record of a kind (`per`, see [One aggregate per decision](#one-aggregate-per-decision-experimental)). Another reduction answers from their last states. From v0.8.14 to v0.8.16 a projection played the records of one person (`key`) and its own `execution` answered from the last state as `$state.<property>`. The engine still plays such a file; from v0.8.17 the loader refuses `key`. The loader refuses an article without a projection that reads `$state`.

A step names the article its records rest on, such as `test_chronicle_effects#3`. Where the article gives more than one kind of record, such as an application and the decision on it, a third segment names the kind: `test_chronicle_effects#1#BESCHIKKING`. A step on an article that requires a fact applies to the recorded instances of that fact.

An article often requires more than one fact. Article 4 of the test register `test_chronicle_corrections` requires the rent of a parking place and its termination. `#EXECUTOGRAM` cannot tell those apart, so up to schema v0.8.17 such an article could have no step at all. Since v0.8.18 the third segment may be the name of a fact the article requires, in lower case. The step then applies to the instances of that fact alone, and `$record` reads the fields of that fact. `per` takes the same notation:

```yaml
# test_reductie_plaats_per_huur (fictional), one aggregate per rent
projection:
  per: test_chronicle_corrections#4#plaats_gehuurd
  state:
    - {name: plaatsnummer, type: string}
    - {name: einde_huur, type: date}
  steps:
    - record: test_chronicle_corrections#4#plaats_gehuurd
      actions:
        - {output: plaatsnummer, value: $record.plaatsnummer}
    - record: test_chronicle_corrections#4#huur_opgezegd
      actions:
        - {output: einde_huur, value: $record.einde_huur}
```

The loader refuses a fact name the article does not require, and a fact name in a file on v0.8.17 or earlier. On an article with more than one fact, a step without a third segment or with `#EXECUTOGRAM` is still refused, and the error names the steps to write instead. A step on a fact and a step on `#EXECUTOGRAM` under the same article would both apply to that fact, so the loader refuses the pair. From v0.8.18 the loader holds `per` to the same rules as a step, so `per: test_chronicle_corrections#4` is refused too: every record of both facts would be a root. `#EXECUTOGRAM` and the other kinds work as before.

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

A step passes no parameter of the question. Schema v0.8.14 to v0.8.16 allowed one, written `$<name>`, such as a reference date, and the engine still accepts it in a file under those versions. Since v0.8.17 the loader refuses it. The fold then depends on the chronicle alone. A day the question asks about is compared in an article of the law when the question is answered, as in the suspension example below.

The engine calls the law in the version in force on the day the record was recorded, not on the calculation date, the same rule as for effects. The output a step takes depends only on what the step passes, so a step stays a function of the record and the state before it. That holds per output, not per article. The engine follows the output through the actions of its article. Neither the output nor any output it reads in turn may read an `input`, with or without a `source`, or an open term. Another output of the same article may read either, and the step can still take the pure one. `$referencedate` is the day of the record, so it reads nothing from outside. An article that plays a projection itself cannot be called at all. A step takes no other source than `source.regulation`, and its actions take no operation and no literal.

The step must also fit the laws it names. Its input takes an output that an article gives. It passes every required parameter that output reads and none the article does not declare, each of the type of the field or property it comes from. `$record.<field>` must be a field of the kind of record the step is about, as the law declares it. A fact holds the parameters its `requires` lists. A decision holds the parameters of its article, its outputs and the data derived for it, and a submission what its item under `produces` lists. Since schema v0.8.17 `$record.refers_to` is the id of the record it refers to, such as the decision a copy of a ruling concerns, or null. Since v0.8.19 `$record.id` is the id of the record itself. An aggregate can keep it in its state to remember which record set a property. In a reduction on v0.8.19 it hides a field of the record named `id`, as `$record.refers_to` hides a field of that name from v0.8.17. Under v0.8.18 and earlier `$record.id` reads that field, as before. `$state.<property>` must be a property of the state. The article a step is on must give a record at all.

Schema v0.8.16 added three of these rules. The article must give a record, `$record` reads only the fields of the record's kind, and the step passes every required parameter the output reads. A reduction that declares v0.8.15 or earlier loads without them, and there `$record` reads any parameter or output of the article.

Each input must have the type of the output it takes, and each value an action gives must fit its property. The reduction and the law are separate regelwerken that load in any order. From v0.8.16, when a reduction loads, the loader checks each step against the laws it already holds, in the newest version of each, and refuses the reduction if a step does not fit. A reduction on v0.8.15 or earlier is checked only when it plays. A law never depends on a reduction that reads it, so a law that arrives later always loads. The engine checks the same again when it plays the projection, against the versions in force on the calculation date, and refuses a reduction that no longer fits. In the trace, the call hangs under the node of the record, like any law call, and that node shows every property the step set, before and after.

What one decision does with another stays in the law. The article declares it in `produces[].effects`: `revokes`, optionally only `when` a boolean outcome of the same decision is true, such as a well-founded objection. `when` reads nothing else, not the state, a parameter or another record. A decision on an objection that puts a new decision in the place of the contested one revokes it, and the new decision counts as part of the decision on the objection.

Since schema v0.8.15 a fact can revoke too. A court ruling reaches the cell as a copy, and the cell records the receipt as a fact (RFC-053). The law gives that fact its effect, on the `requires` entry of the fact:

```yaml
# test_chronicle_effects, article 11 (fictional, after Awb 8:72 lid 1 and 8:79)
requires:
  - fact: afschrift_uitspraak_ontvangen
    parameters: [vernietigd]
    effects:
      - revokes: refers_to
        when: vernietigd
execution:
  parameters:
    - {name: vernietigd, type: boolean}
```

A fact revokes the record it refers to (`revokes: refers_to`), so which record a fact is about has one source. The receipt of a copy refers to the decision the copy concerns (RFC-053), and takes from the copy whether the ruling annuls it; `when` reads that boolean field of the fact, which may not be null. Since schema v0.8.17 any fact may refer to an earlier record of the chronicle, through `Cell::record_executogram_referring`. That widens RFC-053, under which only the receipt referred to anything. The record that the appeal court annulled a ruling is such a fact with effects: it refers to the receipt of the ruling and revokes it, so the decision the ruling annulled counts again. Handing in a card is a fact without effects. It refers to the grant, ends the holder's possession and leaves the grant standing, so it stays a step of the reduction.

`revokes` has one meaning: the record does not count, from the start (ex tunc). That covers a decision revoked on objection, a decision the court annuls, and a ruling annulled on further appeal, which brings back what it annulled. A change from a later day on (ex nunc) is no effect. A suspension by the court (Awb 8:81) holds from one day to another, and a withdrawal "met ingang van" a date takes effect on that date. Both are a step of the reduction with a date property, as for any other change of state. The step of the suspension puts its period in the state. Lifting it (Awb 8:87) is a later step whose new end an article of the law gives, the earlier of the end it had and the day of lifting. Another article says whether the card is valid on a given day:

```yaml
# test_reductie_kaart_per_besluit (fictional), one aggregate per grant
steps:
  - record: test_chronicle_effects#13        # a suspension of the grant
    actions:
      - {output: geschorst_vanaf, value: $record.schorsing_vanaf}
      - {output: geschorst_tot, value: $record.schorsing_tot}

# test_reductie_kaart_op_peildatum (fictional), the read model
execution:
  parameters: [{name: aanvrager, type: string}, {name: peildatum, type: date}]
  input:
    - name: kaarten
      type: array
      source:
        projections: test_reductie_kaart_per_besluit#1
        where: {aanvrager: $aanvrager}
    - name: kaart_geldig
      type: boolean
      source:
        regulation: test_chronicle_effects   # article 15 compares
        output: kaart_geldig_op_peildatum
        parameters: {kaarten: $kaarten, peildatum: $peildatum}
```

The reductions only pass data along: the aggregate keeps the dates in its state, and the read model hands them, with the day the question asks about, to the law. Which records count therefore depends only on the records, never on that day. A suspended grant still counts. A suspension that is itself revoked does not count, so its step never runs and its period never enters the state. Deciding which records count and running the steps stay separate.

Which records count follows from the effects of decisions and facts alone: a record counts unless a later record that itself counts revokes it. A revoked record revokes nothing, so what it alone revoked counts again. Because this never depends on the state, the engine decides it first, in one pass from the last record back, and then runs the steps of the records that count, in the order the cell recorded them. That order of work is how the engine does it, not part of the meaning. The engine records which record set each property last and what became of each record: applied, revoked and by which records, or counting for its effects only. The trace shows the first phase as a `COUNTS` node and the steps as a `FOLD` node.

`refers_to` names, on every record, the record this record is about: a submission the decision it contests, a decision the submission it was taken on, a fact the record it concerns. A decision taken on no submission, such as a withdrawal ex officio, refers to the decision it affects; the cell takes one with `Cell::decide_ex_officio`, and the authority supplies every parameter. Since schema v0.8.17 a decision revokes only a record it is about, one its `refers_to` reaches directly or through the records it refers to in turn. The decision on an objection reaches the contested decision through the objection. The article still supplies the value its effect names, here from what the objection holds (`revokes: bestreden_besluit`). The cell refuses to record a decision whose value is not such a record. The engine refuses to play such a chronicle through a projection with `per`, whichever version the decision's law declares. Neither counts a fixed number of steps or knows the procedure, so a decision on an objection (Awb 7:11) and a new decision after the court annulled one (8:72) follow the same rule. There is no path syntax such as `refers_to.refers_to`.

The reductions of the PoC play their chronicle this way. Its Awb gives the decision on an objection the effect `revokes: bestreden_besluit` when the objection succeeds (7:11, second paragraph), and the receipt of a ruling the effect `revokes: refers_to` when the court annulled the decision with its legal effects (8:79 with 8:72): the receipt refers to the decision the copy is about. A ruling that keeps the legal effects (8:72, third paragraph, under a) revokes nothing. When the court decides the case itself (8:72, third paragraph, under b), the receipt holds the decision it puts in place, and a step on the receipt reads it: leave the court grants counts as granted, and a card it grants makes the cell silent on a valid card, because the municipality does not know how long that card holds. That stays so after the card has expired: from then on the college judges at every later application, unless it granted a card itself that is still valid. Which decision on the application the court put in place, the Awb says (8:72 with 7:11), and the copy of the ruling carries it to the receipt. On the receipt it is an `object`, not a `decision`: the receiving cell only reads it and does not take it anew, and the copy lacks the article it was taken on (`taken_on`) that a decision value needs. What a single decision means for an answer, such as a permit that still holds on the day of the question, is in a policy of the authority. That policy is a rulework of its own. A step cannot call the rulework whose article asks the cell, because the engine is still resolving it.

A correction of a recorded fact is a fact of its own. One that corrects a single field has a step that sets that property and leaves the rest; one that says an earlier fact was wrong from the start revokes it, and a new fact takes its place. The tests are in `packages/cel/bdd/features/corrections.feature`.

A policy reaches a projection the normal way. When an article binds an open term to the cell's answer (`source.reduction`), the engine asks the cell, and the cell plays its chronicle through the reduction's projection, with the parameters the binding passes, such as the applicant. The engine refuses a reduction with a projection on every path that bypasses the chronicle: asked directly, from another rulework, at a stage, or with a `state` the caller passes. The tests are in `packages/cel/bdd`; one of them has a fictional policy take the cell's answer, and its outcome flips when a well-founded objection revokes the grant. Others revoke the grant by the receipt of a ruling, bring it back when that ruling is annulled on further appeal, and suspend the card for a while.

### One aggregate per decision (experimental)

A state holds single values. One state per person breaks down once a person holds several decisions, each with its own days: two business parking permits of one company, each with its last valid day, one of them suspended for a while. Since schema v0.8.17 a projection therefore plays one aggregate per record of a kind (`per`). Each such record is the root of an aggregate. The aggregate holds the root and every record that refers to it through `refers_to`, directly or through another record of the aggregate. Another record of the root's kind that refers into the aggregate is an aggregate of its own and counts in this one for its effects only. A decision put in the place of the root on objection is no such record, as described below. A root that does not count applies no step; the other records of its aggregate that count still apply theirs.

```yaml
# reductie_bedrijfsparkeervergunning (PoC), article vergunning, one aggregate per decision
projection:
  per: parkeerverordening_gemeente_wilgendaal#2#BESCHIKKING
  state:
    - {name: besluit_datum, type: date}
    - {name: vergunning_geldig_tot, type: date}
    - {name: uitspraak_hoofdzaak_op, type: date}
  steps:
    - record: parkeerverordening_gemeente_wilgendaal#2#BESCHIKKING
      actions:
        - {output: besluit_datum, value: $record.besluit_datum}
        - {output: vergunning_geldig_tot, value: $record.vergunning_geldig_tot}
    - record: algemene_wet_bestuursrecht#8:79#EXECUTOGRAM
      actions:
        - {output: uitspraak_hoofdzaak_op, value: $record.uitspraak_datum}
```

A projection per record answers no question of its own and has no `execution`. A read model reads its aggregates with `source.projections`: one row per root the chronicle holds on the calculation date, the last state of its aggregate with the root's `id`. With `where`, the reduction reads only the roots whose field or output of a given name equals a parameter of the reading article, such as `kvk_nummer: $kvk_nummer`. Such an article plays nothing itself; it is a read model over the aggregates, and an article of policy says what the rows mean on the day asked about:

```yaml
# reductie_bedrijfsparkeervergunning (PoC), article aantal_geldige_bedrijfsparkeervergunningen, the count per company
input:
  - name: vergunningen
    type: array
    source:
      projections: reductie_bedrijfsparkeervergunning#vergunning
      where: {kvk_nummer: $kvk_nummer}
  - name: schorsingen
    type: array
    source: {projections: reductie_bedrijfsparkeervergunning#schorsing}
  - name: aantal
    type: number
    source:
      regulation: beleid_verleende_bedrijfsparkeervergunningen_gemeente_wilgendaal
      output: aantal_geldige_vergunningen
      parameters: {vergunningen: $vergunningen, schorsingen: $schorsingen, peildatum: $besluit_datum}
```

Since schema v0.8.19 `source.projections` names the article of the aggregate, `<regulation>#<article>`, also when that article is in the reading file itself; up to v0.8.18 it named the reduction alone. The loader refuses a reference to an article that does not exist or plays no aggregate, as soon as it holds the file named. One file may hold several aggregates and several read models. The PoC keeps the reductions of one subject together: `reductie_bedrijfsparkeervergunning` holds the aggregates per decision (`vergunning`) and per suspension (`schorsing`) and the two read models over them, and `reductie_vestiging_in_handelsregister` holds the aggregate per registration (`inschrijving`) beside the read model that reads it.

The articles of the PoC's reductions have names instead of numbers, since a reduction has no published article numbers to follow. An aggregate is named after what it tracks, such as `vergunning`, `schorsing` or `advies`, and a read model after its question or main output, such as `plaats_van_vestiging`, in lowercase snake case.

A decision put in the place of another lands in the aggregate of the decision it replaces, by both routes of the Awb. On objection (Awb 7:11, second paragraph) the authority revokes the contested decision and takes a new one in its place. Legally that is one decision on the objection, and the cell records it as one record: the new decision is the value of its output `nieuw_besluit`, not a record of its own. The decision on the objection refers to the objection, which refers to the contested decision, so it joins that aggregate. A step on it reads the new decision, through a policy of the authority, because a step reads no path into a value. When the court decides the case itself (8:72, third paragraph, under b), the cell records the receipt of the ruling, a fact about the annulled decision. That receipt joins the aggregate of the annulled decision, and a step on it reads the decision the ruling puts in place.

Because the new decision is part of the decision on the objection, it falls with it. When the court annuls that decision with its legal effects (8:72, second paragraph), the receipt revokes it, ex tunc as any revocation. Its revocation of the contested decision goes too, and so does the new decision: the refusal counts again, and the grant in its place is gone, as `a_grant_on_objection_the_court_annulled_no_longer_counts` in `packages/cel/tests/beroep.rs` shows. No rule says that a decision falls with the decision it refers to; the receipt revokes one record, and that record holds both. An appeal or a later decision about the new decision refers to the decision on the objection, which is where beroep lies (8:1 with 7:1). A suspension by the voorzieningenrechter does the same. The aggregate of a permit therefore keeps which of its decisions grants the permit now (`besluit_dat_verleent`). The step on the root sets it to the root's own id, and a decision on an objection that took a new decision in its place sets it to its own id (`$record.id`, schema v0.8.19). A suspension touches the permit only when it is about that decision. A suspension of the revoked decision then no longer touches it, and a later ruling about the revoked decision cannot lift a suspension of the decision on the objection.

A row does not say whether its root counts. A root that does not count applies no step, so a property its step sets keeps its start value. The reduction of the PoC therefore has its step on the root set a property of its own, `verleend`, from the decision; a revoked or refused permit leaves it false, and the policy reads that.

The suspension is an aggregate too, rooted in the copy of the ruling of the voorzieningenrechter (Awb 8:84, fourth paragraph), with the decision it concerns and its days in the state. The copy of a ruling that lifts the measure (8:87) refers to that copy and joins it. A later ruling about the same decision is an aggregate of its own, so it leaves the period of the earlier one as it was, also when the question is asked long after both.

The policy joins the two collections with existing operations; a join has no construct of its own. In a nested `FOREACH` the filter and body of the inner one see the element of the outer one ([RFC-016](/rfcs/rfc-016), note under Scope). Article 1 collects the decisions that a suspension covers on the day. For each suspension an inner `FOREACH` over the permits looks up the decision it concerns and ends the suspension on the day of the ruling in the main case (8:85, second paragraph, under c). Article 2 counts the permits that the decision granted (`verleend`), that are valid on the day and whose id is not `IN` that list. The conformance scenario "An inner FOREACH compares its element with the outer one" (`bdd/conformance/collections.feature`) tests the nesting on its own.

A recorded fact is played the same way, one aggregate per recorded instance, with `per` naming the fact where its article requires more than one (schema v0.8.18, see above). Article 5 of the municipality's policy in the PoC requires two, the advice the GGD gives and the advice the municipality receives, so `per: beleid_gehandicaptenparkeerkaart_gemeente_wilgendaal#5#geneeskundig_advies_ontvangen`. The article stays as it is: real legislation often lays down two acts in one article, so splitting an article to suit the reductions would move the rulework away from the law.

The step on the root copies the fields of the fact into the state under their own names, and a read model over such aggregates gets one row per recorded instance of the fact, holding the state and the root's `id`. That replaces the rows `source.facts` gave before v0.8.19. The Kamer van Koophandel of the PoC keeps one aggregate per registration of a business location, with the corrections that refer to it (`reductie_inschrijving_vestiging`), and its read model picks the company's with `where: {kvk_nummer: $kvk_nummer}`. A fact that says an earlier one was wrong from the start can revoke it with an effect on its `requires` entry (`revokes: refers_to`). The revoked root keeps its row, with the start values of its state, so a read model that counts the rows has to skip it, for example by a property that the step on the root sets. None of the facts of the PoC declares such an effect yet.

The engine plays each aggregate on every question, in the version of the reduction in force on the calculation date. The trace of the reading input holds a `PROJECTION` node per aggregate, with its `COUNTS` and `FOLD`. The cell gives the records of its chronicle for it (`Chronicle::records`); `evaluate_projection` plays a read model on the records it is given.

## Documents between cells

Each actor keeps its own chronicle. The law sends documents from one actor to another. With an appeal, the court receives a copy of the contested decision together with the notice of appeal (Awb 6:5 lid 2). After the judgment, the parties receive a copy of it (Awb 8:79 lid 1). A cell takes in another cell's document as a copy. It never reads the other cell's chronicle.

- `Cell::afschrift` makes a copy of one of the cell's own decisions. The copy holds the chronolexogram, the decision value from a replay in that cell, and `about`: what the submission behind the decision had received. When a field of type `decision` holds an id, the copy holds the value of that decision instead, nested in the same way. Only the cell that took the decision makes a copy of it.
- `record_submission` with `Reference::Received(&copy)` records a submission against a decision of another cell. The chronolexogram then holds `received: {from, id}` instead of `refers_to`. Its field of type `decision` holds the decision value itself, because this cell cannot replay the other cell's decision.
- `Cell::record_receipt` records that the cell received a copy. It does so as an executogram of the fact that the law requires for it. The receipt refers to the cell's own decision that the copy is about. Any other fact may refer to the record it is about too (schema v0.8.17).
- `Cell::decide_after` decides on a submission again. It needs a later chronolexogram, such as a receipt, that refers to the last decision on that submission. The new decision holds that chronolexogram as `after`. This rule names no article. Whether the earlier decision still counts is up to whoever reads the chronicle.

The cell refuses these:

- a copy of something that is not a decision;
- a copy of its own decision, offered back to it;
- a receipt for a decision that the copy is not about, or a receipt dated before the decision in the copy;
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
