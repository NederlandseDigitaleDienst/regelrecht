---
title: "Inversion of Control"
description: "How a law leaves a value to another regulation, by delegation or co-government, and how that regulation declares that it fills it."
---

A *wet* (formal law) often leaves specifics to someone else. The Healthcare Allowance Act delegates setting the standard premium to the minister, whose ministerial regulation sits below the act. The Participation Act leaves the reduction of social assistance to municipal ordinances, but that is co-government (*medebewind*): the act calls on the municipality to execute it through its own ordinance. The municipality is not administratively subordinate to the state, but its ordinance is still bound by the act and yields to it (Gemeentewet art. 121-122).

In RegelRecht, both relationships are modeled through the same two constructs: the law declares an **open term** (a value it needs but does not define), and the regulation that fills it declares that it **implements** that term.

## The legal pattern

A Dutch ministerial regulation typically opens with a preamble: *"Gelet op artikel 4 van de Wet op de zorgtoeslag"* ("In consideration of article 4 of the Healthcare Allowance Act"). The preamble records which article the regulation rests on; the power itself comes from that article of the law, not from the preamble. Consolidated texts on wetten.overheid.nl show only the preamble of the original regulation, so a later amendment on a new basis does not show there.

RegelRecht mirrors the direction. The law does not need to know which regulation fills its open term. The implementing regulation registers itself.

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

Two optional fields say more about who fills the term, and both are properties of the law rather than of the corpus:

- **`expected_source`** names the regulation the article itself points at, such as `Regeling zorgverzekering`, with a BWB id when the text carries one. Whether that regulation is currently in the corpus is a separate question, because that changes without the law changing.
- **`decided_per_case_by`** names the authority that fills the norm in the individual case while no general specification exists, with the article making it competent. That answer needs a motivation under Awb 3:46 and forms part of the *besluit* rather than a ground for it, which is what separates a discretionary power from a value that is merely missing.

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

The `gelet_op` field matches the real legal preamble text. The `implements` block tells the engine: "I fill in the `standaardpremie` open term from Zorgtoeslagwet article 4."

### The engine connects them at load time

When the engine loads all ruleworks, it builds an index of all `implements` declarations. When it encounters an `open_term` during execution, it looks up the index, finds the implementing regulation, and executes it to get the value.

## Municipal ordinances (co-government)

The Participation Act leaves the reduction of social assistance to municipal ordinances, in co-government. The schema records this with `delegated_to` and `delegation_type`, the same fields it uses for delegation, although legally it is not delegation. Each municipality can set different reduction percentages. The engine uses `gemeente_code` in the execution parameters to select the right municipal ordinance.

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

## An implementation that reads the law it fills in

An implementation can state its value in terms of the law it fills in. A ministerial regulation indexing an amount is the common case: *"het in artikel 4 genoemde bedrag wordt met ingang van 1 januari verhoogd met 3,2 procent"*, where the law leaves the amount open with a `default`. The implementing article then reads an output of that law, which depends on the very term it fills. That read is the law without this filling: the next implementation, or the `default` when there is none. The filling then applies once, whether the law or the implementing regulation is asked.

## Policy that fills an open term with a cell's answer

A cell's reduction ([RFC-048](/rfcs/rfc-048)) reads the cell's own decisions back and gives a named answer. It is not a regulation and fills no open term. Policy of the authority that decides does that: an article fills the term with `implements` and takes the answer as an input (`source.reduction`, schema v0.8.9). Art. 13a of the Leerplichtwet 1969 leaves `verlof_al_verleend_in_schooljaar` to the head per case (`decided_per_case_by`), and the head's policy binds it:

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

The engine does not run the reduction. A caller can pass an answerer with the execution (`OpenTermAnswerer`, in a `Call`), and the engine asks it for the answer wherever the binding runs, at any depth. That includes an article called anew, as Awb 7:11 does on an objection. During a decision the cell is that answerer: it runs the reduction its catalogue names with the parameters the binding passes, and returns the value with where it came from. The engine records that provenance under `open_terms` (`filled_by: {answer: ...}`, with the binding's article) and as one node per answer in the trace. Why the reduction answered as it did is the cell's to explain (`Cell::explain_answer`); it is not in the trace of the decision. A `REDUCTIE` is never in the engine's implements index, so loading one changes nothing by itself.

The order of precedence is the one for every open term: an implementation, such as the binding, then the finding of the authority for the case, then the `default`. A binding without an answer is silent, as an implementation that gives null is. A term decided per case that none of these fills is an error; on a term with neither a default nor `decided_per_case_by`, a null from the binding is the value. Outside a cell there is no answerer, so a scenario gives the head's finding.

## Conflict resolution

When multiple regulations implement the same open term, the engine resolves conflicts using two rules from legal theory:

1. **Lex superior** (higher regulatory layer wins): a *wet* takes precedence over a *ministeriële regeling*, and an ordinance yields to the act it executes
2. **Lex posterior** (newer wins): between two regulations at the same level, the one with the later `valid_from` date takes precedence

Temporal filtering ensures the right version applies: only regulations where `valid_from <= calculation_date` are considered.

## Comparison with cross-law references

Cross-law references and IoC both let laws use values from other laws, but they serve different purposes:

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
