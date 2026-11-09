---
title: "Applications, Decisions and Objections"
description: "How the law format models an Awb procedure: what an article establishes, the hooks on an application, the stages of a decision, an objection that takes a decision again, and the appeal to the court."
---

A specific law says that someone may apply for something. The procedure around that application comes from the Algemene wet bestuursrecht (Awb), from what the application must contain up to the objection against the decision. The rulework of the specific law does not have to name the Awb. The Awb articles attach themselves to every application through hooks.

[RFC-045](/rfcs/rfc-045) gives the design and its reasons. [Chronolexography](/concepts/chronolexography) shows how a cell, the organization that keeps a chronicle of its legal facts, records the procedure.

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
- The decision lists its `outputs`. `derived` holds values that follow from the decision but are not part of it, such as the end date of a permit.
- [Law Format](/concepts/law-format#what-the-article-produces) gives the rules for the list. An article that produces only one thing keeps the single-object form.

## The Awb hooks onto every application

```yaml
# Awb 4:2
hooks:
  - hook_point: pre_actions
    applies_to:
      submission: AANVRAAG
```

- Every application gets the requirements of Awb 4:2 (name, address, date, signature) and the decision period of Awb 4:13.
- `established_by` limits a hook to the application of one article.
- Awb 4:13 marks the day of receipt as the moment the period counts from (`produces.moment`).

[Hooks and Reactive Execution](/concepts/hooks-and-reactive-execution) covers hooks in general.

## Stages

A procedure has stages ([RFC-008](/rfcs/rfc-008)). When the engine runs an article at one stage, it computes only that stage:

| Stage | What runs |
|---|---|
| AANVRAAG, BEZWAAR, BEROEP | The hooks on that kind of submission: Awb 4:2 and 4:13 on an application, 6:5 and 6:9 on an objection and an appeal, and 7:10 on an objection. The decision computes no output yet |
| BESLUIT | The decision, with Awb 3:46 and 6:7 |
| BEKENDMAKING | Awb 6:8: the decision is made known and the objection period starts |
| UITSPRAAK, TOEZENDING | In the procedure `beroep`: the judgment on an appeal, and sending a copy of it to the parties (Awb 8:79) |

Without a procedure, as in a scenario or the editor, every hook fires at once.

## What the decision needs

The law says what the decision needs. The process supplies it from three places:

- the application, by field name;
- policy, through `implements` ([Inversion of Control](/concepts/inversion-of-control)). A policy may use the answer that a cell gives from its own records ([Chronolexography](/concepts/chronolexography#answering-from-the-chronicle));
- the authority, for a term with `decided_per_case_by`. The authority gives a finding: what it decides itself on an open norm in a single case. The finding is keyed by the term's id.

When a policy fills an open term, the term gets the policy's value. The engine does not ask the authority for a finding. If the policy wants the authority to judge a case itself, the policy says so with an open term of its own ([Inversion of Control](/concepts/inversion-of-control#policy-that-fills-an-open-term-with-a-cells-answer)). [Cross-Law References](/concepts/cross-law-references#calling-the-decision-of-an-article) says when the engine refuses a finding.

## Objection

An objection is a second submission. Awb 7:11 establishes it and also decides on it. The objection holds the contested decision. To decide on the objection, Awb 7:11 takes the contested decision again:

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

This call runs on the day of the decision on the objection. It uses the fields and findings of the contested decision, plus any new findings of the authority. [Cross-Law References](/concepts/cross-law-references#calling-the-decision-of-an-article) describes the decision value.

| Situation | Result |
|---|---|
| Late (Awb 6:9) | Not admissible; no reconsideration |
| Against a decision on objection (Awb 7:1 lid 1 onder a) | Not admissible. The engine sees this because the contested decision's `taken_on` is 7:11 |
| Admissible, and the outputs are the same | Unfounded; the decision stands |
| Admissible, and an output differs | The decision is revoked; `nieuw_besluit` holds the new decision |

Only `outputs` count. A `derived` end date that shifts because the day is later does not make the objection founded.

## Appeal

Against the decision on the objection, an interested party (*belanghebbende*) can go to court (Awb 8:1). Awb 8:70 establishes the appeal and decides on it, as 7:11 does for the objection.

The judgment has the kind `UITSPRAAK`. A court is not an administrative body (Awb 1:1 lid 2 onder c), so what it decides is not a *besluit*. The appeal follows a procedure of its own, `beroep`, with the stages BEROEP, UITSPRAAK and TOEZENDING.

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

- Awb 6:5 and 6:9 hook onto the appeal just as onto the objection. Their text speaks of the "bezwaar- of beroepschrift", and their output names follow that text: `termijn_einddatum`, `bezwaar_of_beroepschrift_tijdig_ingediend`. On a decision on objection, `termijn_einddatum` is the last day for lodging the appeal.
- An appeal is admissible when all of these hold:
  - it came in on time;
  - the contested decision was taken on objection (`taken_on` is 7:11, Awb 7:1 lid 1);
  - the court finds the appellant an interested party (8:1).
- Whether the appeal is well founded is for the court to judge. The court finds whether the decision breaches a rule of law, and which rule (8:77 lid 2). These are open terms with `decided_per_case_by`, and the court gives them as findings.
- When the appeal is well founded, 8:70 reads 8:72. The court annuls the decision. Its legal effects go with it, unless the court keeps them in force (lid 3 onder a).
- The court may also decide the case itself (lid 3 onder b). 8:72 then takes the contested decision again with the court's findings, as 7:11 does. 8:70 gives the result as `uitspraak_in_de_plaats`.
- If the court neither keeps the legal effects nor decides the case itself, it orders a new decision, within a period if it sets one (lid 4).

| Situation | Result |
|---|---|
| Late (Awb 6:9) | Not admissible; the court judges nothing |
| Against a decision on an application, without objection (Awb 7:1 lid 1) | Not admissible |
| No rule breached | Unfounded; the decision stands |
| A rule breached, legal effects not kept | Well founded: the decision is annulled, with an order for a new decision |
| A rule breached, legal effects kept | Well founded: the decision is annulled, and no new decision is needed |
| A rule breached, and the court decides the case itself | Well founded: the decision is annulled, and the judgment holds the decision in its place. There is no order |

[RFC-053](/rfcs/rfc-053) gives the design. `packages/poc-chronolexografie/corpus/regulation/nl/wet/algemene_wet_bestuursrecht/scenarios/beroep.feature` plays these cases.

## Open questions

A question is listed only when a choice is still to be made, when it needs investigation first, or when it needs a large refactor or a breaking change to the schema. [Open Questions in Chronolexography](/concepts/chronolexography/open-questions) gives the rule.

| Question | Why open |
|---|---|
| Stages are an Awb detail built into the format. The schema calls `procedure` Awb-defined, and the `stage` of a hook defaults to `BESLUIT`. The engine uses `BESLUIT` and the submission kinds (`AANVRAAG`, `BEZWAAR`, `BEROEP`) as stages. The cell runs two pairs of stages by name: `BESLUIT` and `BEKENDMAKING`, or `UITSPRAAK` and `TOEZENDING` ([RFC-053](/rfcs/rfc-053)). Should stages stay, as a free field whose values come from the law that defines the procedure? A run without a stage, such as a scenario, counts as `BESLUIT`. A procedure without that stage, such as `beroep` (UITSPRAAK, TOEZENDING), then fires no stage hooks. No such hooks exist yet. | Choice; breaking schema change |
| `decision_type` is fixed per article, while one article can both grant and refuse. The value says nothing about the outcome, and no hook in the corpus filters on it. [RFC-022](/rfcs/rfc-022) proposes an open vocabulary. Should the field go, be derived from the outcome, or stay as a free field? | Choice; breaking schema change |
| Awb 8:72 lid 4 says the court "kan" (may) order a new decision. The model always orders one when the court annuls, does not keep the legal effects and does not decide the case itself. It does not model the court's instructions. Should the order be a finding of the court instead? | Choice |
| A decision on objection can revoke a decision and take a new one in its place. A party can appeal against that new decision. When the court annuls it, the administrative body cannot yet decide on the objection again. Deciding again (`Cell::decide_after`) needs a receipt that refers to the last decision on the objection itself. Should a receipt for the new decision count too? | Choice |
| The court can decide the case itself and grant a parking card. The municipality's reduction then counts that card as an earlier card. But the reduction is silent on whether the card is still valid, so the college judges that. The row of the decision the court set in place holds `kaart_geldig_tot` as null, because only the college derives that date when it decides. Should the court's decision value carry the derived data too? | Choice |
| An appeal against the judgment (*hoger beroep*, Awb 8:104 and 8:105) is recorded as text only. How the judgment of a higher court would reach the cells of the lower court and the administrative body is not designed yet. | Investigation |
| Which article 7:11 calls is known only at run time. So the dependency graph, the editor and the type checker do not see that link. | Large refactor |
| Of the motivation requirements in Awb 3:46 to 3:50, only 3:46 is modeled, as a flag that the decision needs a motivation. These are not modeled: 3:47 (state the motivation and its statutory basis when the decision is made known), 3:48 (when the motivation may be omitted), 3:49 and 3:50 (referring to an advice and departing from one), and 3:45 (the notice of legal remedies). The engine already knows much of a motivation: the legal basis, who filled each open term, and the trace. Should that go into the decision record? The answer bears on the Awb 4:84 question in [Open Questions in Chronolexography](/concepts/chronolexography/open-questions), because a departure from policy must be motivated. | Choice |
| Each substantive article establishes its own application (`produces: AANVRAAG`). Awb 4:1 says that an application for a *beschikking* goes to the competent body. So 4:1 could establish every such application once, and the substantive article would only say what is decided. The Awb would then say in one place what an application is. Awb 4:5 (not processing an incomplete application) would hook onto that one article. | Choice; breaking schema change |
