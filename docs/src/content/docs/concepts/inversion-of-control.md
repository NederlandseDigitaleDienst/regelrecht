---
title: "Inversion of Control"
description: "How a law leaves a value to another regulation, by delegation or co-government, and how that regulation declares that it fills it."
---

A *wet* (formal law) often leaves the details to someone else. The Healthcare Allowance Act delegates setting the standard premium to the minister. The minister does so in a ministerial regulation, which ranks below the act.

The Participation Act leaves the reduction of social assistance to municipal ordinances. That is co-government (*medebewind*): the act calls on the municipality to carry it out through an ordinance of its own. The municipality is not administratively subordinate to the state. Its ordinance is still bound by the act and yields to it (Gemeentewet art. 121-122).

RegelRecht models both relationships with the same two constructs. The law declares an **open term**: a value it needs but does not define. The regulation that fills it declares that it **implements** that term.

## The legal pattern

A Dutch ministerial regulation typically opens with a preamble: *"Gelet op artikel 4 van de Wet op de zorgtoeslag"* ("In consideration of article 4 of the Healthcare Allowance Act"). The preamble records which article the regulation rests on. The power itself comes from that article of the law, not from the preamble. Consolidated texts on wetten.overheid.nl show only the preamble of the original regulation. A later amendment on a new legal basis therefore does not show there.

RegelRecht follows the same direction. The law does not need to know which regulation fills its open term. The implementing regulation registers itself.

## How it works

### The law declares an open term

```yaml
# Zorgtoeslagwet, article 4
machine_readable:
  open_terms:
    - id: standaardpremie
      type: amount
      required: true
      delegated_to: minister
      delegation_type: MINISTERIELE_REGELING
      legal_basis: artikel 4 Wet op de zorgtoeslag
      default:
        actions:
          - output: standaardpremie
            value: 211200
```

This says: "I need a value called `standaardpremie`. The minister should set it via a ministerial regulation. If nobody has set it, use 211200 (EUR 2,112.00)."

Two optional fields say more about who fills the term. Both describe the law, not the state of the corpus:

- **`expected_source`** names the regulation that the article itself points at, such as `Regeling zorgverzekering`, with a BWB id when the text gives one. Whether that regulation is in the corpus right now is a separate question, because the corpus changes while the law stays the same.
- **`decided_per_case_by`** marks a norm that an authority fills in for each individual case, as long as no general rule exists. Its text says in words who that authority is, and which article makes it competent. That finding needs a motivation under Awb 3:46. The finding is part of the *besluit* itself, not a ground for it. That is what separates a discretionary power from a value that is simply missing. The engine does not read the authority from the words in this field. The term's own `competent_authority` names the authority, or else the authority that carries out the article judges ([Competent Authority](/concepts/competent-authority#who-gives-a-finding)).

### The implementing regulation fills it

```yaml
# Regeling standaardpremie, article 1
# regulatory_layer: MINISTERIELE_REGELING
# valid_from: 2025-01-01
machine_readable:
  implements:
    - law: wet_op_de_zorgtoeslag
      article: '4'
      open_term: standaardpremie
      gelet_op: Gelet op artikel 4 van de Wet op de zorgtoeslag
  execution:
    output:
      - name: standaardpremie
        type: amount
        type_spec:
          unit: eurocent
    actions:
      - output: standaardpremie
        value: 211200
```

The `gelet_op` field holds the text of the real preamble. The `implements` block tells the engine: "I fill in the `standaardpremie` open term from Zorgtoeslagwet article 4."

### The engine connects them at load time

When the engine loads the ruleworks, it builds an index of all `implements` declarations. During execution, when it meets an `open_term`, it looks the term up in the index, finds the implementing regulation and executes it to get the value.

## Municipal ordinances (co-government)

The Participation Act leaves the reduction of social assistance to municipal ordinances, in co-government. The schema records this with `delegated_to` and `delegation_type`. Those are the fields it uses for delegation, although in law co-government is not delegation. Each municipality can set its own reduction percentages. The engine uses `gemeente_code` in the execution parameters to pick the right municipal ordinance.

### The national law declares open terms with defaults

```yaml
# Participatiewet, article 8
machine_readable:
  open_terms:
    - id: verlaging_percentage
      type: number
      required: true
      delegated_to: gemeenteraad
      delegation_type: GEMEENTELIJKE_VERORDENING
      legal_basis: artikel 8 lid 1 onderdeel a Participatiewet
      default:
        actions:
          - output: verlaging_percentage
            value: 0
```

The default of 0 follows the legal logic: article 18(2) says "reduces in accordance with the ordinance." No ordinance means no reduction.

### A municipality implements the open term

```yaml
# Afstemmingsverordening Participatiewet Diemen, article 9
# regulatory_layer: GEMEENTELIJKE_VERORDENING
# gemeente_code: GM0384
machine_readable:
  implements:
    - law: participatiewet
      article: '8'
      open_term: verlaging_percentage
      gelet_op: Gelet op artikel 8, eerste lid, onderdeel a van de Participatiewet
```

When executing for a person in Diemen (parameters include `gemeente_code: GM0384`), the engine uses Diemen's percentages. For a municipality without an ordinance, the Participation Act's default applies.

The scope selects among other regulations only. An ordinance can fill an open term of one of its own articles. A hardship clause that sets aside a refusal in the same ordinance does this. Such an article fills the term whatever the scope, because the ordinance that declares the term is already the one being applied.

## An implementation that reads the law it fills in

An implementation can state its value in terms of the law it fills in. The common case is a ministerial regulation that indexes an amount: *"het in artikel 4 genoemde bedrag wordt met ingang van 1 januari verhoogd met 3,2 procent"*. Here the law leaves the amount open, with a `default`.

The implementing article then reads an output of that law, and that output depends on the very term the article fills. The engine answers that read as if this implementation did not exist: it uses the next implementation, or the `default` when there is none. So the increase applies once, whether the caller asks the law or the implementing regulation.

## Policy that fills an open term with a cell's answer

A cell is an organization that records its own decisions and facts, and answers later questions from that record. A reduction ([RFC-048](/rfcs/rfc-048)) is the rulework that reads the cell's earlier decisions and gives an answer under a name. It sits with the cell, in `corpus/cells/<cell>/reductions/`. A reduction is not a regulation, and it fills no open term.

The policy of the deciding authority fills the term. One of its articles fills the term with `implements` and takes the cell's answer as an input (`source.reduction`, schema v0.8.9). Art. 13a of the Leerplichtwet 1969 leaves `verlof_al_verleend_in_schooljaar` to the head of the school, case by case (`decided_per_case_by`). The head's policy binds that term to the answer:

```yaml
# fictief_beleid_vakantieverlof_obs_de_klimop, article 2
# regulatory_layer: UITVOERINGSBELEID
machine_readable:
  implements:
    - law: leerplichtwet_1969
      article: 13a
      open_term: verlof_al_verleend_in_schooljaar
  execution:
    input:
      - name: antwoord_verlof_al_verleend
        nullable: true
        source:
          reduction: verlof_al_verleend_in_schooljaar
          parameters: {bsn_jongere: $bsn_jongere, eerste_verlofdag: $eerste_verlofdag}
```

The engine does not run the reduction itself. A caller can pass an answerer with the execution (`OpenTermAnswerer`, in a `Call`). The engine asks that answerer for the answer wherever the binding runs, at any depth. That includes an article that is called again, as Awb 7:11 does on an objection.

During a decision, the cell is the answerer. It runs the reduction that its catalogue names, with the parameters that the binding passes. It returns the value together with where the value came from. The engine records that origin under `open_terms` (`filled_by: {answer: ...}`, with the binding's article), and as one node per answer in the trace.

The trace of the decision does not show why the reduction gave its answer. The cell explains that separately (`Cell::explain_answer`). A `REDUCTIE` is never in the engine's index of implementations, so loading one changes nothing by itself.

A binding fills the term like any other implementation: the term gets the binding's value. That holds even when the value is null, unless the term has a `default`. Then a null value gives the default ([RFC-036](/rfcs/rfc-036)).

A finding is what the authority itself decides on the term in a single case. A finding never replaces the value of a policy, not even on a term that the law leaves to the authority case by case. If a caller gives a finding for such a term, the engine refuses the execution, even when the case never reads the term.

Sometimes the policy wants the authority to judge a case that the cell does not answer. The policy then declares an open term of its own with `decided_per_case_by`, and uses that term when the answer is null ([RFC-048](/rfcs/rfc-048) §2). A scenario runs without a cell, so no cell gives an answer there. The scenario gives the finding on the policy's own term instead.

## Conflict resolution

When multiple regulations implement the same open term, the engine resolves conflicts using two rules from legal theory:

1. **Lex superior** (higher regulatory layer wins): a *wet* takes precedence over a *ministeriële regeling*, and an ordinance yields to the act it executes
2. **Lex posterior** (newer wins): between two regulations at the same level, the one with the later `valid_from` date takes precedence

The engine only considers regulations whose `valid_from` is on or before the calculation date (`valid_from <= calculation_date`). That way the right version applies.

## Comparison with cross-law references

Cross-law references and inversion of control (IoC) both let a law use values from another law. They serve different purposes:

| | Cross-law reference | Inversion of Control |
|---|---|---|
| Who knows whom? | The referencing law names the target law | The law does not know which regulation fills its open term |
| Direction | Top-down: "give me this value from that law" | Bottom-up: "I fill in this value for that law" |
| Use case | A law needs a specific value from a specific other law | A law leaves a value to whichever regulation fills it, by delegation or in co-government |
| YAML construct | `source: { regulation: ..., output: ... }` | `open_terms` + `implements` |

## Further reading

- [Cross-Law References](./cross-law-references) - the other pattern for inter-law values
- [Hooks and Reactive Execution](./hooks-and-reactive-execution) - yet another pattern: laws that fire automatically
- [Traceability](./traceability) - how an open-term delegation appears in an execution trace
- [RFC-003: Inversion of Control](/rfcs/rfc-003) - the full design specification
