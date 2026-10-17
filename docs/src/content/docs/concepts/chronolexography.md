---
title: "Chronolexography"
description: "Recording what follows from a law as dated facts in a chronicle, built up step by step on vakantieverlof (Leerplichtwet 1969, art. 13a)."
---

Chronolexography records each legal fact (an application, a decision, an objection) as a dated entry in the chronicle (*kroniek*) of the organization that establishes it. [RFC-022](/rfcs/rfc-022) describes the model. This page lists what works today, one feature per section.

The running example is vakantieverlof: a parent asks the head of a school for leave outside the school holidays (Leerplichtwet 1969, art. 13a).

## How the parts fit together

| Part | What it is | Where |
|---|---|---|
| Law | Art. 13a (the application, the decision and its test); Awb 4:2, 4:13, 3:46, 6:7 and 6:8 as hooks; Awb 7:11 (the objection and the decision on it) with 6:5, 6:9 and 7:10 | `corpus/regulation/` |
| Policy | Fictitious school policy: the first school day (`implements`) and the bounds of a school year | `corpus/regulation/nl/uitvoeringsbeleid/` |
| Reduction | "Did the head already grant leave this school year?", over the school's own decisions | `corpus/cells/obs_de_klimop/reducties/` |
| Engine | Executes articles: hooks per stage, cross-law sources, open terms, data sources. Stores nothing | `packages/engine` |
| Cell | The school: keeps the chronicle, derives what it records, executes through the engine | `packages/cel` |

**Recording the application** (`Cell::record_submission`):

1. The parent supplies the fields: the child, the leave days, name and address.
2. The cell takes the fields from the law: the parameters of the `AANVRAAG` item and of the Awb hooks on it.
3. The engine runs art. 13a at stage AANVRAAG. Awb 4:2 says whether the application is complete, Awb 4:13 gives the decision period.
4. The cell appends one chronolexogram of kind `AANVRAAG`.

A submission may refer to a decision in the chronicle (`refers_to`, RFC-050). The cell then fills fields from that decision. A field of type `decision` gets the decision's id. Every other field that the decision holds under the same name gets its value, which must fit the field. The submitter supplies neither.

**Taking the decision** (`Cell::decide`):

1. The cell finds the application through `requests: leerplichtwet_1969#13a`.
2. It answers the engine for the whole decision. When an article reads an open term that one of its reductions fills, the engine asks the cell. The cell runs that reduction on its own chronicle, with its own decisions as the rows of each `source: {decisions: ...}` input, and returns the value with its provenance.
3. It gathers each input of the decision by name: from the application, or from the head. The engine resolves what policy fills (`implements`), and asks the cell for what its reduction fills. The head supplies what a hook asks for. Its findings come separately (`findings`), keyed by the id of the open term the law leaves to it (`decided_per_case_by`). Here these are the two judgements under art. 11 onder f and whether leave was already granted this school year. In the cell, the reduction fills that last one. The engine also gets the findings separately from the parameters, so a finding applies to every article in the decision that leaves the term to the head.
4. The engine runs art. 13a at stages BESLUIT and BEKENDMAKING: the test, and Awb 3:46, 6:7 and 6:8.
5. The cell appends a chronolexogram of kind `BESCHIKKING` that refers to the application. The head's findings are among its fields. The chronolexogram does not store which findings the decision used. `Cell::findings` replays the decision and reads that from the engine's report; a finding that a policy or the cell's answer set aside is not among them.

`Cell::explain` takes a recorded decision again with tracing, on its day and with the chronicle as it stood before it, and fails when the outcome differs. In its trace the cell's answer within art. 13a is one node: the value, the reduction and its version, and how far the cell read its chronicle. `Cell::explain_answer` gives the cell's own account of that answer: the reduction run again with tracing, with the decisions it counted and those its filter skipped. That trace is not stored.

`decide` returns a list of chronolexograms (`Vec<Gram>`). The decision comes first. After it comes each decision that the deciding article takes in turn: every output of type `decision` that is not null. They land in the chronicle together, or none do.

**Objecting**: the same two steps under Awb 7:11, with `record_submission` and `decide`. The objection refers to the contested decision. The decision on it calls art. 13a anew. On revocation 7:11 gives the new decision, and the cell records that in the same append.

Law and policy say what is needed and how to compute it; they know no chronicle. A reduction is the only place that reads decisions, and it computes through law and policy again. Reduction takes place in the cell, as the [position paper](https://chronolexografie.nl/position-paper/) has it ("Reductie vindt altijd plaats ín de cel waar de betreffende chronolexogrammen zijn vastgelegd"). How the cell and the engine divide that work is in [RFC-048](/rfcs/rfc-048) §3.

## What works

| Feature | Rulework | RFC |
|---|---|---|
| [The application, with the Awb on it](#the-application-with-the-awb-on-it) | Leerplichtwet 1969 art. 13a, Awb 4:2 and 4:13 | [RFC-045](/rfcs/rfc-045) |
| [The application in the chronicle](#the-application-in-the-chronicle) | Leerplichtwet 1969 art. 13a, Awb 4:2 and 4:13 | [RFC-046](/rfcs/rfc-046): chronolexography as a layer |
| [The decision on the application](#the-decision-on-the-application) | Leerplichtwet 1969 art. 13a, Awb 3:46, 6:7 and 6:8, fictitious school policy | [RFC-047](/rfcs/rfc-047) |
| [Once per school year](#once-per-school-year) | Leerplichtwet 1969 art. 13a lid 2, fictitious school policy, the school's reduction | [RFC-048](/rfcs/rfc-048) |
| [The objection](#the-objection) | Awb 6:5, 6:9, 7:10 and 7:11, Leerplichtwet 1969 art. 13a, the school's reduction | [RFC-050](/rfcs/rfc-050) |
| [A second case: the parking card](#a-second-case-the-parking-card) | BABW art. 49 and 51, Regeling gehandicaptenparkeerkaart art. 1, the Awb, fictitious municipal policy, the municipality's reduction | [RFC-045](/rfcs/rfc-045) to [RFC-050](/rfcs/rfc-050) |

## The application, with the Awb on it

Article 13a says it establishes an application (`AANVRAAG`), what the application contains, and which decision it asks for. The Awb hooks onto every application, so submitting one also runs Awb 4:2 and 4:13. The fields of the application are the parameters its item lists plus those of the hooks. One exception: the parameter a hook names in `produces.moment`, here `datum_ontvangst` from Awb 4:13, is the moment of receipt and no field of the application.

```yaml
# Leerplichtwet 1969, art. 13a
produces:
  - type: AANVRAAG
    requests: leerplichtwet_1969#13a
    parameters: [bsn_jongere, eerste_verlofdag, laatste_verlofdag]
  - type: BESCHIKKING
    # ... the decision, below
parameters:
  - name: eerste_verlofdag
    type: date

# Awb 4:13
hooks:
  - hook_point: post_actions
    applies_to:
      submission: AANVRAAG
```

- Awb 4:2 adds what every application contains: a signature, name and address, date, the decision asked for.
- Awb 4:13 adds the last day of the decision period when no law sets one: eight weeks after receipt.
- `produces.moment` on Awb 4:13 marks receipt as the moment that counts.
- `established_by: <regulation>#<article>` limits a hook to the application of one article.
- What the head needs to decide, such as the judgements under art. 11 onder f, is not part of the application. It belongs to the decision.

**Try it:** `just bdd`, which runs `corpus/regulation/nl/wet/leerplichtwet_1969/scenarios/vakantieverlof_aanvraag.feature`

## The application in the chronicle

A cell, here a school, records the application as one chronolexogram in its chronicle. Chronolexography is an optional layer: it lives in `packages/cel`, and the law and the engine do not know it. The law names no fields for the record; the cell derives them from the keys above.

```json
{
  "kind": "AANVRAAG",
  "grondslag": "leerplichtwet_1969#13a",
  "recording_actor": "obs_de_klimop",
  "recorded_on": "2026-10-13",
  "moment": {
    "parameter": "datum_ontvangst",
    "on": "2026-10-13",
    "declared_by": "algemene_wet_bestuursrecht#4:13"
  },
  "fields": { "eerste_verlofdag": "2027-02-01", "adres_aanvrager": null },
  "outputs": { "redelijke_beslistermijn_einddatum": "2026-12-08" }
}
```

- The fields are the parameters the item of art. 13a lists and those of its Awb hooks, minus the moment. The cell refuses an unknown field and a value of the wrong type.
- A field left out is `null` when it can be absent. Otherwise it goes under `missing`, and the application is still recorded (Awb 4:5).
- `moment` holds the day of receipt. The cell fills it with the day of recording; that the two are the same day is the cell's assumption.
- Recording runs the stage AANVRAAG of art. 13a: the Awb hooks on the application run, and their outputs go in `outputs`. The outputs of art. 13a belong to the decision.
- The chronolexogram does not repeat the law. Which article asks a field follows from the law version in force on `recorded_on`.
- A number the engine computes is stored without loss: an integer as a JSON integer, a decimal as `{"__decimal": "0.3333333333333333333333333333"}` with every digit, so it reads back as the same decimal and not as a float or an integer. The cell takes this encoding from the law model (`regelrecht_law_model::stored`, see [law-model](/components/law-model)), so every store keeps and compares a value the same way. A supplied value may not hold that key; a supplied number is stored as the JSON it came in, and read back as the engine reads JSON: a whole number such as `5.0` as the integer `5`. When the cell compares two stored values, for example a decision taken in turn against the decision value it came from, it uses the rule of the engine's `EQUALS` on records. That rule lives in the same module. `5.0` and `5.00` are one value; an integer against a decimal of the same value is an error.

The chronicle refuses a chronolexogram dated before the last one.

**Try it:** `cargo test --manifest-path packages/cel/Cargo.toml`

## The decision on the application

The head of the school decides on the application. Art. 13a is also that decision, a `BESCHIKKING`, and follows the Awb procedure. `produces` lists the application and the decision, each its own item. The application asks for this decision with `requests`, and the decision names its outputs:

```yaml
# Leerplichtwet 1969, art. 13a
produces:
  - type: AANVRAAG
    requests: leerplichtwet_1969#13a
    parameters: [bsn_jongere, eerste_verlofdag, laatste_verlofdag]
  - type: BESCHIKKING
    decision_type: TOEKENNING
    outputs: [verlofdagen, verlof_raakt_eerste_twee_lesweken, verlof_verleend]
```

The cell records the decision as a second chronolexogram that refers to the application.

```json
{
  "kind": "BESCHIKKING",
  "grondslag": "leerplichtwet_1969#13a",
  "refers_to": "019a1b2c-…",
  "recorded_on": "2026-10-20",
  "fields": {
    "eerste_verlofdag": "2027-02-01",
    "eerste_lesdag_schooljaar": "2026-08-31",
    "specifieke_aard_beroep_ouder": true,
    "verlof_valt_buiten_schoolvakanties": true
  },
  "findings": ["specifieke_aard_beroep_ouder", "verlof_valt_buiten_schoolvakanties"],
  "outputs": { "verlofdagen": 5, "verlof_verleend": true, "bezwaartermijn_weken": 6 }
}
```

- Each produced thing has its own stage: the application with Awb 4:2 and 4:13 at AANVRAAG, the decision with Awb 3:46 and 6:7 at BESLUIT, Awb 6:8 at BEKENDMAKING.
- What the decision needs comes from the application first, then from the school's policy (`implements`), then from the head when deciding (`decided_per_case_by`). `findings` lists the fields that hold a finding of the head. The chronolexogram repeats neither the basis nor the source of a field or output: both follow from the law on `recorded_on` and from `refers_to`.
- The test: at most ten school days (Monday to Friday), not in the first two lesweken, and the two facts of art. 11 onder f: a parent's profession of a specific nature, and leave outside the school holidays. The head judges both. A refusal is `verlof_verleend: false`.
- The cell refuses a second decision on one application, and a decision dated before it. It makes the decision known on the day it takes it, so Awb 6:8 counts the objection period from there; that is the cell's assumption.
- Not modeled yet: the proportional share for a young person who is kwalificatieplichtig (lid 2, second sentence).

**Try it:** `just bdd`, which runs `vakantieverlof_besluit.feature`, or `cargo test --manifest-path packages/cel/Cargo.toml`

## Once per school year

The head grants leave "slechts eenmaal ... per schooljaar" (art. 13a lid 2). Art. 13a declares the open term `verlof_al_verleend_in_schooljaar`, which is no field of the application. The head decides it per case (`decided_per_case_by`), and it has no default. How the head reads its own decisions back, the law does not say.

In a cell, a reduction fills the term with `implements`: rulework of the kind `REDUCTIE`, kept with the cell at `corpus/cells/obs_de_klimop/reducties/`. It finds the decisions taken under art. 13a and asks the school's fictitious policy in which school year the asked leave falls.

```yaml
# reductie_verlof_al_verleend_in_schooljaar, article 1
implements:
  - {law: leerplichtwet_1969, article: 13a, open_term: verlof_al_verleend_in_schooljaar}
parameters:
  - {name: bsn_jongere, type: string}
  - {name: eerste_verlofdag, type: date}
input:
  - name: besluiten
    type: array
    source:
      decisions: leerplichtwet_1969#13a
  - name: begin_schooljaar
    type: date
    source:
      regulation: fictief_beleid_schooljaargrenzen_obs_de_klimop
      parameters: {datum: $eerste_verlofdag}
output:
  - name: verlof_al_verleend_in_schooljaar
    type: boolean
```

- Each decision is a row with its fields and outputs, such as `bsn_jongere`, `eerste_verlofdag` and `verlof_verleend`, and its `id`. A refusal does not count, nor a decision revoked on objection (see [the objection](#the-objection)).
- The policy says only that a school year runs from 1 August to 31 July. It reads no decisions.
- The engine asks the cell for the term, and the cell runs its reduction with the parameters it declares, taken from art. 13a by name. The answer wins over a finding of the head, as an implementation would. The engine never runs a reduction by itself. Outside a cell nothing fills the term, so a scenario gives the head's finding; without one the execution fails.
- A cell opens with its directory (`Cell::open`, `Cell::in_memory`) and loads its reductions from `reducties/` there, so it cannot be without them. A term that a reduction in that directory implements is the cell's. On a day on which no reduction in force fills it, the cell refuses the decision. A school without reductions opens with a directory that has none.
- `Cell::answers` lists what the cell can answer: per version of each reduction, the open term it fills, the reduction's `$id` and its `valid_from`. This list is a RegelRecht addition. In the [Chronolexografie position paper](https://chronolexografie.nl/position-paper/) a cell reduces its chronicle on request into a lexostatus, the answer that the cell's facts give. The paper does not have a cell declare which lexostatussen it can answer; that idea comes from [RFC-022](/rfcs/rfc-022) §4.1.
- Only a reduction reads `decisions`. Since schema v0.8.5 it may fill an open term (`implements`); it still has no `hooks`, `overrides`, `execution.produces` or `open_terms`. The corpus loaders leave it out; `just validate` checks it.
- The decision records the answer in `fields`. It is not a finding of the head, so a decision value leaves it out.

**Try it:** `just bdd`, which runs `vakantieverlof_besluit.feature` with the head's finding and `schooljaargrenzen.feature` next to the policy, or `cargo test --manifest-path packages/cel/Cargo.toml`

## The objection

The parent objects to the decision. Awb 7:11 establishes the objection and decides on it, as art. 13a does for the application. The objection names the contested decision. The decision on it reconsiders that decision by calling its article again:

```yaml
# Awb 7:11
produces:
  - type: BEZWAAR
    requests: algemene_wet_bestuursrecht#7:11
    parameters: [bestreden_besluit]
  - type: BESCHIKKING
    outputs: [bezwaar_ontvankelijk, heroverweging_geeft_aanleiding,
              bestreden_besluit_herroepen, nieuw_besluit]
parameters:
  - {name: bestreden_besluit, type: decision}
input:
  - name: heroverweging
    type: decision
    nullable: true
    source:
      article: $bestreden_besluit
```

In a procedure, the article that decides gets the earlier steps as context: the objection and the decision it refers to, with their fields and findings. When 7:11 calls that decision anew (`article: $bestreden_besluit`), the call uses the decision's own fields and findings. New findings come in from the top, and an article in the call must ask for them.

The cell records the objection with `record_submission`, referring to the contested decision. It fills `bestreden_besluit` from that reference and keeps it by its id:

```json
{
  "kind": "BEZWAAR",
  "grondslag": "algemene_wet_bestuursrecht#7:11",
  "refers_to": "019a1b2d-…",
  "recorded_on": "2026-11-01",
  "fields": {
    "bestreden_besluit": "019a1b2d-…",
    "gronden_bezwaar": "verklaring van de werkgever: geen verlof in de schoolvakanties",
    "bezwaartermijn_einddatum": "2026-12-01"
  },
  "outputs": { "bezwaarschrift_tijdig_ingediend": true, "beslistermijn_bezwaar_einddatum": "2027-01-12" }
}
```

- The fields are the contested decision and those of Awb 6:5, plus the end of the objection period, which the cell takes from the contested decision. An objection does not change the application: Awb 7:11 lid 1 reconsiders the decision "op grondslag van het bezwaar", and 6:5 asks only for the grounds. A smaller request is a new application.
- Awb 6:9 makes the receipt the moment that counts and says whether it came in on time. A late objection is recorded. 7:11 then declares it not admissible, and does not read the heroverweging (lid 1). An objection against a decision taken on objection is not admissible either (Awb 7:1 lid 1 onder a). That covers the decision on the objection and the new decision it took. The decision value names the article of the submission a decision was taken on (`taken_on`). For both, that is 7:11.
- The head decides with `decide`. The engine gets the contested decision as a decision value. For an admissible objection, 7:11 calls the decision anew: art. 13a runs with that decision's fields and findings, under the law and policy of that day. `decide` passes what the head finds anew to the engine, separately from the parameters. A new finding wins over the decision's own. A finding that no article asks for, such as one on a late objection, is an error. The engine asks the cell in that call, and the cell answers from the chronicle of that day without the contested decision. When an outcome of the decision item of art. 13a comes out different, the heroverweging gives cause, and 7:11 revokes the decision.
- On revocation, `nieuw_besluit` holds the decision that art. 13a gave. The cell records it as a chronolexogram of its own under art. 13a, with its own period (Awb 6:7 and 6:8; for a decision on objection that is the period for appeal). The decision on the objection holds it by id. A second objection against the revoked decision is reconsidered, but the cell refuses to replace a decision twice. The revoked decision stays in the chronicle. The school's reduction reads the decisions under 7:11 as a second input, and leaves out a grant that one of them revoked.

The cell contains no Awb article. [RFC-050](/rfcs/rfc-050) describes the design and what is still open.

**Try it:** `just bdd`, which runs `corpus/regulation/nl/wet/algemene_wet_bestuursrecht/scenarios/bezwaar.feature`, or `cargo test --manifest-path packages/cel/Cargo.toml`

## A second case: the parking card

The college of a municipality grants a gehandicaptenparkeerkaart on application (Besluit administratieve bepalingen inzake het wegverkeer, BABW, art. 49). The case runs on the same cell without a change to the engine or the cell. Only rulework and tests change.

```yaml
# BABW, art. 49
produces:
  - type: AANVRAAG
    requests: besluit_administratieve_bepalingen_inzake_het_wegverkeer_babw#49
    parameters: [bsn_aanvrager, kaart_als_bestuurder, kaart_als_passagier]
  - type: BESCHIKKING
    decision_type: TOEKENNING
    outputs: [gehandicaptenparkeerkaart_verstrekt, kaart_vermeldt_bestuurder, kaart_vermeldt_passagier]
    derived: [kaart_geldig_tot]
```

What is the same:

- The application gets Awb 4:2 and 4:13, the decision Awb 3:46, 6:7 and 6:8, and the objection Awb 6:5, 6:9, 7:10 and 7:11. No hook names the BABW.
- The college judges the criteria after the medical examination, as the head judges Leerplichtwet 1969 art. 11 onder f. These are four open terms with `decided_per_case_by` on art. 49.
- A reduction of the municipality, `reductie_geldige_gehandicaptenparkeerkaart`, reads the college's own decisions and fills an open term with `implements`. A refusal does not count, and neither does a decision that a decision on objection revoked.
- On objection, 7:11 calls art. 49 anew. A new medical judgement revokes a refusal and grants the card, with a validity of its own.

What differs:

- The criteria sit in another regulation, the Regeling gehandicaptenparkeerkaart (art. 1). Art. 49 reads them through a cross-law `source` and passes the college's judgements as parameters.
- Art. 49 says the college "kan" grant the card, and nothing about an applicant who already holds one. Art. 53 lid 1 onder b even counts on a new card replacing the old. The rule "no card next to a valid one" is the municipality's fictitious policy. The policy fills the open term of art. 49 on how the college uses its power. It also declares an open term of its own, `aanvrager_heeft_geldige_gehandicaptenparkeerkaart`. The college decides it per case, and it has no default. The reduction implements that term, so the engine asks the cell inside the policy. Because the term belongs to the policy and not to art. 49, the cell's answer is no field of the decision. A replay reports it (`Cell::explain`), and `Cell::explain_answer` shows which card the reduction counted.
- The validity comes from the law: five years from the day of issue (BABW art. 51 lid 1). The reduction compares that date in each decision with the day of the new decision, so unlike the school year for vakantieverlof, no policy has to set the period.
- The last day of validity is derived data (`derived`, [RFC-050](/rfcs/rfc-050)). A heroverweging falls on a later day and would give a card that runs longer. That alone gives no cause, so an objection without merit against a grant leaves the grant and its date as they were.

An applicant who fails the criteria is refused before art. 49 reads the college's power, so the engine does not ask the cell. Asked to explain its answer for such a decision (`Cell::explain_answer`), the cell says it was not asked.

Not modeled: competence by place of residence (lid 1) and the CBR (lid 3), the institution (lid 2), the shorter validity of art. 51 lid 2 and 3, and the medical examination itself (Regeling art. 2). The judgements rest on that examination, but no rule checks that it took place or when it may be left out (Regeling art. 2 lid 2).

What the cell cannot do yet:

- The medical judgements belong with the criteria in the Regeling (art. 1, after the examination of art. 2 and 3). The engine applies a finding to every article in the decision. The cell, however, derives the inputs of a decision only from the deciding article, and a decision value carries only that article's findings. At a heroverweging the college would have to give judgements on the Regeling anew. So they sit on art. 49 and go to the Regeling as parameters.
- The cell does not pass its own identity to the law. Competence by place of residence cannot be checked, and a policy scoped by `gemeente_code` is not found, so the municipality's policy is unscoped.

**Try it:** `just bdd`, which runs the scenarios next to the BABW and the Regeling, or `cargo test --manifest-path packages/cel/Cargo.toml --test parkeerkaart`
