---
title: "Applications, Decisions and Objections"
description: "How the law format models an Awb procedure: what an article establishes, the hooks on an application, the stages of a decision, an objection that takes a decision again, and the appeal to the court."
---

A specific law says someone may apply. The procedure around it comes from the Algemene wet bestuursrecht (Awb), from the application to the objection. The rulework of the specific law need not name the Awb, because Awb articles hook onto every application.

[RFC-045](/rfcs/rfc-045) gives the design and its reasons. [Chronolexography](/concepts/chronolexography) shows how a cell records the procedure.

## What an article establishes

An article that establishes an application and decides on it lists both under `produces`:

```yaml
produces:
  - type: AANVRAAG
    requests: wet_voorbeeld#3          # the decision this application asks for
    parameters: [bsn_aanvrager, gevraagde_datum]
  - type: BESCHIKKING
    outputs: [vergunning_verleend]
    derived: [vergunning_geldig_tot]
```

- The application lists what it contains and names the decision it asks for (Awb 4:2 lid 1 onder c).
- The decision lists its `outputs`. `derived` holds what follows from it without being part of it.
- [Law Format](/concepts/law-format#what-the-article-produces) gives the rules for the list. An article that produces one thing keeps the object form.

## The Awb hooks onto every application

```yaml
# Awb 4:2
hooks:
  - hook_point: pre_actions
    applies_to:
      submission: AANVRAAG
```

- Every application gets Awb 4:2 (name, address, date, signature) and the decision period of Awb 4:13.
- `established_by` narrows a hook to one article's application.
- Awb 4:13 marks the day of receipt as the moment that counts (`produces.moment`).

[Hooks and Reactive Execution](/concepts/hooks-and-reactive-execution) covers hooks in general.

## Stages

Run at one stage of its procedure ([RFC-008](/rfcs/rfc-008)), an article computes only that stage:

| Stage | What runs |
|---|---|
| AANVRAAG, BEZWAAR, BEROEP | The hooks on the submission of that kind (Awb 4:2 and 4:13 on an application; 6:5 and 6:9 on an objection and an appeal; 7:10 on an objection). No decision output |
| BESLUIT | The decision, with Awb 3:46 and 6:7 |
| BEKENDMAKING | Awb 6:8: the objection period starts |
| UITSPRAAK, TOEZENDING | The judgment on an appeal, and handing over its copy (Awb 8:79); the procedure `beroep` |

Without a procedure (a scenario, the editor) every hook fires at once.

## What the decision needs

The law says what is needed. The process supplies it from:

- the application, by field name;
- policy, through `implements` ([Inversion of Control](/concepts/inversion-of-control)), which may take the answer of a cell ([Chronolexography](/concepts/chronolexography#answering-from-the-chronicle));
- the authority, for a term with `decided_per_case_by`, as a finding keyed by the term's id.

Which of these wins on an open term follows the [order of precedence](/concepts/inversion-of-control#policy-that-fills-an-open-term-with-a-cells-answer). [Cross-Law References](/concepts/cross-law-references#calling-the-decision-of-an-article) says when a finding is refused.

## Objection

An objection is a second submission. Awb 7:11 establishes it and decides on it. The objection holds the contested decision. The decision on the objection calls that decision anew:

```yaml
# Awb 7:11
produces:
  - type: BEZWAAR
    requests: algemene_wet_bestuursrecht#7:11
    parameters: [bestreden_besluit]
  - type: BESCHIKKING
    outputs: [bezwaar_ontvankelijk, heroverweging_geeft_aanleiding,
              bestreden_besluit_herroepen, nieuw_besluit]
input:
  - name: heroverweging
    type: decision
    nullable: true
    source:
      article: $bestreden_besluit
```

The call runs on the day of the decision on the objection, with the contested decision's fields and findings plus what the authority finds anew. [Cross-Law References](/concepts/cross-law-references#calling-the-decision-of-an-article) describes the decision value.

| Situation | Result |
|---|---|
| Late (Awb 6:9) | Not admissible; no reconsideration |
| Against a decision on objection (Awb 7:1 lid 1 onder a) | Not admissible: its `taken_on` is 7:11 |
| Admissible, outputs the same | Unfounded, the decision stands |
| Admissible, an output differs | Revoked; `nieuw_besluit` holds the new decision |

Only `outputs` count. A `derived` end date that moves with the day gives no cause.

## Appeal

Against the decision on the objection, a belanghebbende can go to court (Awb 8:1). Awb 8:70 establishes the appeal and decides on it, as 7:11 does for the objection. The judgment is of the kind `UITSPRAAK`: a court is not an administrative body (Awb 1:1 lid 2 onder c), so what it decides is not a besluit. It follows a procedure of its own, `beroep`, with the stages BEROEP, UITSPRAAK and TOEZENDING.

```yaml
# Awb 8:70
produces:
  - type: BEROEP
    requests: algemene_wet_bestuursrecht#8:70
    parameters: [bestreden_besluit]
  - type: UITSPRAAK
    outputs: [beroep_ontvankelijk, beroep_gegrond, uitspraak_strekt_tot,
              geschonden_geoordeelde_rechtsregel, bestreden_besluit_vernietigd,
              rechtsgevolgen_vernietigd, opdracht_nieuw_besluit,
              termijn_nieuw_besluit_einddatum]
```

- Awb 6:5 and 6:9 hook onto the appeal as onto the objection; their text speaks of the "bezwaar- of beroepschrift", and so do their names: `termijn_einddatum`, `bezwaar_of_beroepschrift_tijdig_ingediend`. On a decision on objection, `termijn_einddatum` is the last day for lodging the appeal.
- An appeal is admissible when it came in on time, when the contested decision was taken on objection (`taken_on` is 7:11, Awb 7:1 lid 1), and when the court finds the appellant an interested party (8:1).
- Whether the appeal is well founded is the court's judgement: whether the decision breaches a rule of law, and which (8:77 lid 2). These are open terms with `decided_per_case_by`, given as findings.
- When well founded, 8:70 reads 8:72. The court annuls the decision, and its legal effects go with it unless the court keeps them (lid 3 onder a). It may also decide the case itself (lid 3 onder b): 8:72 then calls the contested decision anew with the court's findings, as 7:11 does, and 8:70 gives the result as `uitspraak_in_de_plaats`. Otherwise it orders a new decision, within a term if it sets one (lid 4).

| Situation | Result |
|---|---|
| Late (Awb 6:9) | Not admissible; the court judges nothing |
| Against a decision on an application, without objection (Awb 7:1 lid 1) | Not admissible |
| No rule breached | Unfounded; the decision stands |
| A rule breached, effects not kept | Well founded: annulled, with an order for a new decision |
| A rule breached, effects kept | Well founded: annulled, no new decision |
| A rule breached, the court decides the case itself | Well founded: annulled, the judgment holds the decision in its place, no order |

[RFC-053](/rfcs/rfc-053) gives the design. `corpus/regulation/nl/wet/algemene_wet_bestuursrecht/scenarios/beroep.feature` plays these cases.

## Open points

Listed only when a choice is still to be made, when it needs investigation first, or when it needs a large refactor or a breaking change to the schema. [Open Points in Chronolexography](/concepts/chronolexography-open-points) gives the rule.

| Point | Why open |
|---|---|
| Stages are an Awb detail built into the format. The schema calls `procedure` Awb-defined, and a hook's `stage` defaults to `BESLUIT`. The engine uses `BESLUIT` and the submission kinds (`AANVRAAG`, `BEZWAAR`, `BEROEP`) as stages, and the cell runs two pairs by name, `BESLUIT` and `BEKENDMAKING` or `UITSPRAAK` and `TOEZENDING` ([RFC-053](/rfcs/rfc-053)). Should stages stay, as a free field that the law defining the procedure fills? A run without a stage, such as a scenario, counts as `BESLUIT`. A procedure without that stage, such as `beroep` (UITSPRAAK, TOEZENDING), then fires no stage hooks; no such hooks exist yet. | Choice; breaking schema change |
| `decision_type` is fixed per article, while one article grants or refuses. The value says nothing about the outcome, and no hook in the corpus filters on it. [RFC-022](/rfcs/rfc-022) proposes an open vocabulary. Drop it, derive it from the outcome, or keep it as a free field? | Choice; breaking schema change |
| Awb 8:72 lid 4 says the court "kan" order a new decision. The model orders one whenever the court annuls and keeps neither the legal effects nor decides the case itself, and it does not model the court's instructions. Should the order be a finding of the court instead? | Choice |
| A party can appeal against the new decision that a decision on objection took in place of a revoked one. When the court annuls that new decision, the administrative body cannot yet decide on the objection again: deciding again (`Cell::decide_after`) needs a receipt that refers to the last decision on the objection itself. Should a receipt for the new decision count too? | Choice |
| When the court decides the case itself and grants a parking card, the reduction of the municipality counts that card as an earlier card. On whether it is still valid the reduction is silent, so the college judges. The row of the decision the court set in place holds `kaart_geldig_tot` as null, because that date is derived only when the college decides. Should the court's decision value carry the derived data too? | Choice |
| An appeal against the judgment (hoger beroep, Awb 8:104 and 8:105) is recorded as text only. How a higher court's judgment would reach the cells of the lower court and the administrative body is not designed yet. | Investigation |
| The edge from 7:11 to the article it calls is known only at run time. The dependency graph, the editor and the type checker do not see it. | Large refactor |
| Of the motivation requirements in Awb 3:46 to 3:50 only 3:46 is modeled, as a flag that the decision needs a motivation. Awb 3:47 (state the motivation and its statutory basis when the decision is made known) and 3:48 (the motivation may be omitted) are not. Neither are 3:49 and 3:50, on referring to an advice and departing from one, nor 3:45, the notice of legal remedies. Much of a motivation is what the engine already knows: the legal basis, who filled each open term, the trace. Should that go into the decision record? The answer bears on the Awb 4:84 point in [Open Points in Chronolexography](/concepts/chronolexography-open-points), since a departure from policy must be motivated. | Choice |
| Each substantive article establishes its own application (`produces: AANVRAAG`). Awb 4:1 has an application for a beschikking submitted to the competent body, so 4:1 could establish every such application once, and the substantive article would only say what is decided. The Awb would then say in one place what an application is, and 4:5 (not processing an incomplete application) would hook onto that one article. | Choice; breaking schema change |
