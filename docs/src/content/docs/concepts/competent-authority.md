---
title: "Competent Authority"
description: "How an article records the bevoegd gezag: the authority whose execution produces a binding decision, as a named organization or a category resolved per context."
---

A *beschikking* is only binding when the right body issues it. The tax inspector (*inspecteur*) assesses income. The CIZ decides on a long-term care indication. A municipal college grants social assistance. An article records which body is competent in `competent_authority`, the *bevoegd gezag*.

The engine uses this field to decide which cell answers an open term (below). The note resolver derives from it whether a note on the article is authoritative (see [Notes and Annotations](./notes-and-annotations)).

## The two forms

`competent_authority` sits in an article's `machine_readable` section and takes one of two shapes.

A named authority, when the body is fixed:

```yaml
machine_readable:
  competent_authority:
    name: Dienst Toeslagen
    type: INSTANCE
```

`type` is either `INSTANCE` or `CATEGORY`, defaulting to `INSTANCE`:

- **`INSTANCE`** is one specific organization, named outright. Belastingdienst, CIZ, Minister van Volksgezondheid, Welzijn en Sport.
- **`CATEGORY`** is a kind of authority. Which body is meant depends on the context. "College van burgemeester en wethouders" names a kind of authority, and which municipal college is meant depends on the municipality in scope.

With `INSTANCE` the article alone settles the authority. With `CATEGORY` the engine still needs a fact from the context to find the body.

A reference to a computed output, when the authority itself depends on the case:

```yaml
machine_readable:
  competent_authority: '#bevoegd_gezag'
```

The `#` form points at an output that the same law computes elsewhere. The Wet langdurige zorg uses this. In that law, which body is competent (`2_1_3_bevoegd_gezag`) is itself a rule. So the article refers to that output instead of naming a body.

On an article, the field names who decides. That is always one authority, in either form.

## Who answers an open term

Since schema v0.8.8, an open term and a required fact (`requires` with `fact`) can each have a `competent_authority` of their own. It takes the named form, or a list of named authorities. On an open term it says who may answer the term. On a fact it says who establishes the fact. An article with several open terms can name different authorities for each.

BABW art. 49 lets the college issue a disabled parking card under lid 1 and 2, and lets the CBR issue one under lid 3. So any college, or the CBR, may answer whether an applicant had a card before:

```yaml
open_terms:
  - id: aanvrager_eerder_gehandicaptenparkeerkaart_verstrekt
    type: boolean
    competent_authority:
      - name: college van burgemeester en wethouders
        type: CATEGORY
      - name: CBR
```

A cell is an organization that keeps its own chronicle and answers questions from it. A cell's answer reaches a term like this one through the policy of the deciding authority. That policy binds the term to the answer (`source.reduction`, [RFC-048](/rfcs/rfc-048)).

Wherever that binding runs, the engine asks the one cell in the call that the term names. It does not ask a cell that the term does not name. If the call holds more than one cell that the term names, the decision fails. The engine merges no answers, and which cell of a category answers is an [open question](/concepts/chronolexography/open-questions). For a term without `competent_authority`, the engine asks the cell of the authority that holds the policy. The article's own `competent_authority` plays no part in which cell the engine asks.

## Who gives a finding

A finding is what the authority itself decides on an open norm in a single case. An open term with `decided_per_case_by` needs a finding when no regulation fills it. The field also names the authority in words. The engine reads it only as a marker that the term is judged case by case. Who gives the finding follows from competence:

1. The authority that the term names in its own `competent_authority`.
2. If the term names none, the authority that carries out the article. That is the article's `competent_authority`, or else the holder named at the root of the rulework.

If neither names an authority, whoever carries out the article gives the finding.

Art. 13a of the Leerplichtwet 1969 says that the head of the school grants the leave. So the head gives the findings on the open terms of art. 13a.

The fictitious policy of Wilgendaal lets the college judge four medical criteria when the GGD has given no advice. No law makes the GGD the judge of those criteria, and the terms name no authority. So the college gives those findings, as the holder of the policy.

A cell gives findings only on the terms it answers. It refuses a finding on a term that another authority answers. [RFC-048](/rfcs/rfc-048) §2 sets out the rule.

## One law, several authorities

A single law can name a different authority per article, because different articles describe different acts. In the Wet langdurige zorg the CIZ takes the indication decision, while later articles give other steps to the Zorgkantoor (3.3.3, 4.2.4). Each article that grants a power has its own `competent_authority`.

Some articles grant no power. An article that establishes a body (Wlz 6.1.1 for the CAK, 7.1.1 for the CIZ) or imposes a duty (the *zorgplicht* of 4.2.1) carries no competent authority of its own. The authority belongs to the act that the article describes. A law as a whole has no single competent authority. A policy can have one, as the next section shows.

## The holder of a policy

Since schema v0.8.9, a rulework can have `competent_authority` at its root. It applies to every article that names no authority of its own. On a policy (`UITVOERINGSBELEID`) it names the holder: the authority whose rulework it is.

```yaml
$id: beleid_vergunningen_voorbeeldstad
regulatory_layer: UITVOERINGSBELEID
competent_authority:
  name: Het college van burgemeester en wethouders van Voorbeeldstad
```

A policy that binds an open term to a cell's answer must name its holder this way, by name. The loader refuses a `#` reference or a list there. A reduction names no holder. Since schema v0.8.12 it sits with its cell (`corpus/cells/<cell>/reductions/`), and a cell answers only from the reductions it keeps.

## A decision or a calculation

Dutch administrative law separates a *beschikking*, a binding act only the competent authority can issue, from a *berekening*, a computation anyone may perform. When one law reads the income another law establishes, it crosses from one organization to another, and the question is whether to compute that value or to accept the determination the competent body made.

The engine does not make that choice. It computes every value locally, whoever is competent for it, and its outputs are unsigned. [RFC-009](/rfcs/rfc-009) proposes four modes on two axes, solo or federated and simulation or authoritative, in which an engine would identify as one organization, call the engines of others over FSC (Federated Service Connectivity) and sign what it decides. That RFC is proposed and none of it is built. Today, a value of another organization reaches a decision in one way: a cell that the open term names answers it from its own chronicle ([Chronolexography](/concepts/chronolexography#answers-of-other-cells)). The receipt has an `accepted_values` field for the other route, and it is always empty ([Execution Provenance](./execution-provenance)).

## Further reading

- [Notes and Annotations](./notes-and-annotations) - how note authority is derived from the competent authority
- [RFC-002: Bevoegdheid](/rfcs/rfc-002) - full specification
- [RFC-009: Multi-Organization Execution](/rfcs/rfc-009) - the proposed execution modes across organizations
- [RFC-051: Facts and Competence](/rfcs/rfc-051) - competence on an open term and a fact
- [RFC-048: Reduction and Synthesis](/rfcs/rfc-048) - binding a cell's answer, and asking the competent cell for it
- [Rules as Executed, section 4.3](/research/rules-as-executed#sec:whochooses) - the position paper on which authority chooses and publishes an encoding
- [Rules as Executed, section 4.6](/research/rules-as-executed#sec:overrides) - the position paper on overrides and discretion
- [Rules as Executed, sections 4.11 and 4.12](/research/rules-as-executed#sec:crossorg) - the position paper on executing across organizations and relying on another body's decision
