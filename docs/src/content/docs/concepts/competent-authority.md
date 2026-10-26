---
title: "Competent Authority"
description: "How an article records the bevoegd gezag: the authority whose execution produces a binding decision, as a named organization or a category resolved per context."
---

A *beschikking* is only binding when the right body issues it. Income is assessed by the tax inspector (*inspecteur*), but a long-term care indication is the CIZ's to decide, and social assistance is granted by a municipal college. An article records which body that is in `competent_authority`, the *bevoegd gezag*. The engine uses it to tell a binding decision apart from a calculation anyone may run (see [Multi-Org Execution](./multi-org-execution)), and to derive who may authoritatively annotate a law (see [Notes and Annotations](./notes-and-annotations)).

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
- **`CATEGORY`** is a categorical role that resolves to a different concrete body depending on context. "College van burgemeester en wethouders" names a kind of authority; which municipal college is meant depends on the municipality in scope. The distinction tells the engine whether the authority is settled by the article alone or still needs a contextual fact to pin down.

A reference to a computed output, when the authority itself depends on the case:

```yaml
machine_readable:
  competent_authority: '#bevoegd_gezag'
```

The `#` form points at an output computed elsewhere in the same law. The Wet langdurige zorg uses this: which body is competent (`2_1_3_bevoegd_gezag`) is itself a rule, so the article references the output rather than hard-coding a name.

On the article, the field names who decides. It is one authority, in either form.

## Who answers an open term

Since schema v0.8.8 an open term and a required fact (`requires` with `fact`) may carry a `competent_authority` of their own. It takes the named form, or a list of named authorities. On a term it says who may answer it, on a fact who establishes it. An article that establishes several terms can give each its own actors.

BABW art. 49 lets the college issue a parking card under lid 1 and 2, and the CBR under lid 3. Whether an applicant had a card before is something any college or the CBR may answer:

```yaml
open_terms:
  - id: aanvrager_eerder_gehandicaptenparkeerkaart_verstrekt
    type: boolean
    competent_authority:
      - name: college van burgemeester en wethouders
        type: CATEGORY
      - name: CBR
    combine: OR
```

A cell's answer reaches such a term through the policy of the deciding authority. That policy binds the term to the answer (`source.reduction`, [RFC-048](/rfcs/rfc-048)). Wherever that binding runs, the engine asks only the answerers that the term names, and combines their answers with the term's `combine`. A cell that the term does not name is not asked. For a term without `competent_authority`, the engine asks the cell of the policy's holder. The article's own `competent_authority` plays no part in which cell the engine asks.

## Who gives a finding

A finding is the authority's own judgement on an open norm in a single case. An open term with `decided_per_case_by` needs a finding when no regulation fills it. The field also names the authority in words, but the engine reads it only as a marker that the term is judged per case. Who gives the finding follows from competence:

1. The authority that the term itself names in its `competent_authority`.
2. If the term names none: the authority that carries out the article. That is the article's `competent_authority`, or else the holder named at the root of the rulework.

If none of these names an authority, whoever carries out the article gives the finding.

Art. 13a of the Leerplichtwet 1969 says that the head grants the leave, so the head gives the findings on its terms. The fictitious policy of Wilgendaal lets the college judge the four medical criteria when the GGD has given no advice. No law makes the GGD the judge there, and the terms name no authority. So the college gives those findings, as the holder of the policy. A cell gives findings only on the terms it answers, and refuses a finding on a term that another authority answers. [RFC-048](/rfcs/rfc-048) §2 sets out the rule.

## One law, several authorities

A single law can name a different authority per article, because different articles describe different acts. In the Wet langdurige zorg the indication decision is the CIZ's, while later articles assign other steps to the Zorgkantoor (3.3.3, 4.2.4). Each article that grants a power has its own `competent_authority`. An article that establishes a body (Wlz 6.1.1 for the CAK, 7.1.1 for the CIZ) or imposes a duty (the *zorgplicht* of 4.2.1) grants no power, and carries none. The authority is a property of the act described in the article. A law as a whole has no single competent authority. A policy can have one, as the next section shows.

## The holder of a policy

Since schema v0.8.9 the schema describes `competent_authority` at the root of a rulework, for every article that names none of its own. On `UITVOERINGSBELEID` it names whose rulework it is:

```yaml
$id: beleid_vergunningen_voorbeeldstad
regulatory_layer: UITVOERINGSBELEID
competent_authority:
  name: Het college van burgemeester en wethouders van Voorbeeldstad
```

A policy that binds an open term to the answer of a cell must name its holder this way, by name; the loader refuses a `#` reference or a list. A reduction names none: since schema v0.8.12 it sits with its cell (`corpus/cells/<cell>/reductions/`), and a cell answers only from the reductions it keeps.

## Further reading

- [Multi-Org Execution](./multi-org-execution) - how the competent authority decides execute-vs-accept
- [Notes and Annotations](./notes-and-annotations) - how note authority is derived from the competent authority
- [RFC-002: Bevoegdheid](/rfcs/rfc-002) - full specification
- [RFC-051: Facts and Competence](/rfcs/rfc-051) - competence on an open term and a fact
- [RFC-048: Reduction and Synthesis](/rfcs/rfc-048) - binding a cell's answer, and asking the competent cells for it
- [Rules as Executed, section 4.3](/research/rules-as-executed#sec:whochooses) - the position paper on which authority chooses and publishes an encoding
- [Rules as Executed, section 4.6](/research/rules-as-executed#sec:overrides) - the position paper on overrides and discretion
