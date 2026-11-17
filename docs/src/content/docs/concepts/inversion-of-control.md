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
- **`decided_per_case_by`** marks a norm that an authority fills in the individual case while no general specification exists. Its text says in words who that authority is, and which article makes it competent. That judgement needs a motivation under Awb 3:46. It forms part of the *besluit* rather than a ground for it, and that separates a discretionary power from a value that is merely missing. The engine does not read the authority from those words: the term's own `competent_authority` names it, or else it is the authority that carries out the article ([Competent Authority](/concepts/competent-authority#who-gives-a-finding)).

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

The scope selects among other regulations, with two exceptions. First, an ordinance can fill an open term of one of its own articles. A hardship clause that sets aside a refusal in the same ordinance does this. Such an article fills the term whatever the scope, because the ordinance that declares the term is already the one being applied.

Second, a regulation with the same scope fields as the declaring one, set to the same values, fills its terms whatever the scope as well. Further rules that the college lays down under an ordinance of its own municipality are the case in point. Two national regulations do not count as sharing a territory; for them the scope plays no part anyway.

### The regulation of a place

A `gemeente_code` among the parameters works when the caller knows which municipality a case belongs to. For a rule about a place the caller knows something else: where the activity is. Since schema v0.8.23 an open term can name that place with `location`, and say with `territory` where the territory of an authority is read. The place then picks the regulation.

```yaml
# A law that leaves a ban to the omgevingsplan
machine_readable:
  open_terms:
    - id: verboden_in_omgevingsplan
      type: boolean
      required: true
      delegated_to: gemeenteraad
      delegation_type: GEMEENTELIJKE_VERORDENING
      location: $locatie
      territory:
        regulation: kadasterwet
        output: grens_van_gemeente
      combine: OR
  execution:
    parameters:
      - name: locatie
        type: shape
        required: true
```

`location` is a `$`-reference to a parameter or an input of type `shape` in the article that declares the term. A regulation with a `gemeente_code`, `provincie_code` or `waterschap_code` fills the term only when that shape overlaps the territory of its code. The test is the one `OVERLAPS` applies: the interiors meet, so a location on the boundary of a territory lies in neither neighbor. A code among the parameters plays no part for such a term. A regulation without a territory, the declaring regulation itself and one of the declarer's own territory fill the term as described above.

`territory` names the output of a law that gives the territory of an authority. The boundary of a municipality is a fact a register holds: article 48 of the Kadasterwet puts "de grens van een provincie of gemeente" on the national cadastral map. A register enters the corpus through the law that establishes it. An article of that law reads the register as an input with `source: {}` and gives the value as its output, the way the Wet basisregistratie personen gives what the BRP holds (see [Cross-Law References](./cross-law-references)). The territory is such an output:

```yaml
# The law of the register, article with the boundary of a municipality
machine_readable:
  execution:
    parameters:
      - name: gemeente_code
        type: string
        required: true
    input:
      - name: grens_in_register
        type: shape
        source: {}
    output:
      - name: grens_van_gemeente
        type: shape
    actions:
      - output: grens_van_gemeente
        value: $grens_in_register
```

For each candidate regulation that carries a territory code, the engine asks that output on the calculation date with one parameter: the code, under the name of the field that carries it, so `gemeente_code: GM0273` for a municipal plan. It is a call to another law like any other. It stands in the trace, it is checked for cycles, and the register reaches it as a data source bound to that law. A code is asked once per term, however many regulations carry it. `location` and `territory` come together; the schema and the loader refuse one without the other. A regulation that carries more than one territory code is an error for such a term.

A `territory` that names a law that is not in force on the calculation date, or an output that law does not give, is a defect of the term: an error whenever the term is read, whatever a regulation would say. When the named law is loaded the type check finds most of it earlier. The output must exist and be a `shape`, and the article that gives it must declare a parameter named `gemeente_code`, `provincie_code` or `waterschap_code`, since that is what the engine asks it with. A law that is not loaded cannot be checked then, as for `source.regulation`.

The code is passed under the name of its own field and under no other. An article that takes a `provincie_code` therefore cannot say where a regulation with a `gemeente_code` holds: it would answer without the code it was asked with. The type check refuses a regulation that fills such a term when the article behind `territory` declares no parameter for the field the regulation carries, in whatever order the three laws are loaded. Where the check could not run, the territory of that regulation counts at run time as one that cannot be read.

The value has to be a shape. A territory that cannot be read as one (the call fails, the value is no shape, its geometry is not valid) is a defect of that one candidate, and the error names the term, the code and the source. Without `combine` it fails the term, since the candidate that cannot be placed may be the one that wins. With `combine: OR` it counts as an implementation that failed: a regulation in scope that gives true still decides, and the failure is recorded. A territory that is unknown is something else. When the register has no row for a code, the input of the register's law is unknown for lack of that fact, and so is the territory. The regulation is then neither in scope nor out of it, and the engine does not read that as "it does not apply". Without `combine` the term is unknown for lack of what the territory lacks. With `combine: OR` a regulation that is in scope and gives true still makes the term true; otherwise the term is unknown.

A location can reach outside the territories of the regulations in scope: a parcel that holds a corner of the municipal boundary, or one that lies partly in a municipality whose plan does not fill the term. The regulations that were asked say nothing about that part, so their answers do not decide the term by themselves. The engine judges whether the location is covered, that is, whether no part of it lies outside the territories of the regulations in scope taken together. Two territories that share a boundary cover a location across it. A regulation in scope that is not held against a territory, such as a national one, covers any location. The part that is not covered is a place for which no regulation is known, and what holds there is what holds where there is none: the `default` of the term.

With `combine: OR` the default then counts as one more answer beside those of the regulations. A required term without a default fails when no regulation in scope gives true, with an error that says part of the location lies in no territory for which a regulation is known. An optional term without a default adds nothing. Without `combine` the term is refused as ambiguous, because no one regulation holds for the whole location. A location that lies wholly outside every territory is not affected: no regulation is in scope, and the term takes its default or fails as a term without an implementation does. Coverage is judged only when the territory of every candidate was read. An unknown territory still makes the term unknown, and one that cannot be read still fails it.

One territory that covers the location settles the question with the exact relation between the two shapes. For a location across several territories the engine lays their surfaces over the location in one overlay, on a fine grid. Points written the same in two boundaries stay the same, so neighbors that share the points of their common boundary leave no gap. Boundaries that do not share them leave a gap, and a gap is not covered unless it is narrower than the grid of the overlay, about a billionth of the extent of the shapes.

The delegation gate comes first, so the territory of a regulation at a layer the term does not delegate to is never asked. A location that is not known makes the term unknown for lack of it. That holds for a term an authority decides per case as well: a finding does not stand in for the plan of an unknown place.

A term with a `location` rests on what a register held, which does not show in the term's own article. The execution therefore records, for every such term, the location and each candidate regulation that was held against it: its territory code, where the territory was read (`kadasterwet#grens_van_gemeente`), the hash of the text of the territory that was used, and whether the location overlapped it. For an unknown territory the hash and the outcome are absent, and for one that could not be read the record holds the error. The candidates come by territory code and then by regulation, whatever the order the laws were loaded in, and the same order decides which of two unreadable territories is the error that is raised. `location_covered` says whether the regulations in scope cover the location; it is absent when a territory is unknown or could not be read. When the location itself is unknown, the record says what it lacks and lists no territory. The record also lists every implementation that was asked and what it gave, with `combine` and without it, and under `default` what the default gave where it answered for a part that is not covered. A term that fails leaves its record too. That matters where the failure is taken as one answer by a term with `combine: OR` further up: the record is then what remains of the term that failed. It is on the result as `open_terms_asked`, on the receipt and in the output of the `evaluate` binary and of the WASM `execute` family under the same name, and in the trace under the open term, where each territory is a step with the call to the register's law beneath it.

A listing made before the article runs, such as [`decision_terms`](/components/engine) or the hand-out of findings to terms an authority decides per case, cannot always tell the place: the location may be an input, or a territory may be unknown. It then reports the place as not known (`place_not_known` on a listed term) and never as "no regulation here". A finding for such a term is passed on, and the term decides when it is resolved with the location in hand.

### Several articles that fill one term

A plan states its bans article by article: one article forbids building without a permit, another felling a tree. Each of those articles fills the same open term, and so may the plans of two municipalities when a location lies across their boundary. Without `combine`, a term takes one implementation, and two of one layer and date fail on an ambiguous priority.

`combine: OR` on a boolean term asks every implementation that is in force, of the delegated layer and in scope. No priority is applied among them. The term is:

- true when one implementation gives true, whatever the others did;
- otherwise the error of the first failure: a candidate whose territory could not be read, then an implementation that failed, then the default where it was asked for a part of the location that is not covered;
- otherwise an error when the term is required, has no default, and part of its location lies in no territory for which a regulation is known;
- otherwise untranslatable when one of them is ([Markings](/concepts/markings));
- otherwise unknown when one of them is unknown, or when a candidate regulation could not be asked because its territory is unknown, for lack of what they lack;
- otherwise false.

Every implementation is asked, also after one gave true. An implementation that fails does not take the term down with it when another says so: its failure is recorded and not raised. One that gives something other than a boolean or null counts as failed.

An implementation that gives null does not speak about the case and counts as not saying so. That separates two answers a single implementation could not: a plan that was asked and forbids nothing gives false, while a location where no plan applies leaves the term without an implementation. The term then takes its `default`, and a required term without one is an error, as it is without `combine`. The execution records every implementation that gave true as having filled the term, and under `open_terms_asked` what each implementation gave: true, false, silent, unknown with what it lacks, untranslatable, or failed with its error. For a term that is no boolean, asked without `combine`, the answer is the value itself, written as `{"value": "12"}`. A term that comes out false or unknown is explained by that list.

`combine` does not go with `decided_per_case_by`. A term with `location` and without `combine` still takes one regulation, and takes it for the whole location. A location in the territories of two codes of one kind, such as two municipalities, is refused as ambiguous, whatever the dates of the two regulations: two municipalities are not each other's later law. Priority keeps ranking regulations of one code, and a regulation without a code next to them. It also ranks regulations whose codes are of different kinds. A province and a municipality in it always lie over each other, and the higher layer wins there as it does without `location`.

## An implementation that reads the law it fills in

An implementation can state its value in terms of the law it fills in. A ministerial regulation indexing an amount is the common case: *"het in artikel 4 genoemde bedrag wordt met ingang van 1 januari verhoogd met 3,2 procent"*, where the law leaves the amount open with a `default`. The implementing article then reads an output of that law, which depends on the very term it fills. That read is the law without this filling: the next implementation, or the `default` when there is none. The filling then applies once, whether the law or the implementing regulation is asked.

## Policy that fills an open term with a cell's answer

A cell's reduction ([RFC-048](/rfcs/rfc-048)) reads the cell's own decisions back and gives a named answer. It sits with the cell, in `cells/<cell>/reductions/` of the corpus. It is no regulation and fills no open term; policy of the authority that decides does that: an article fills the term with `implements` and takes the answer as an input (`source.reduction`, schema v0.8.9). Art. 13a of the Leerplichtwet 1969 leaves `verlof_al_verleend_in_schooljaar` to the head per case (`decided_per_case_by`), and the head's policy binds it:

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

The engine does not run the reduction. A caller can pass an answerer with the execution (`OpenTermAnswerer`, in a `Call`), and the engine asks it for the answer wherever the binding runs, at any depth. That includes an article called anew, as Awb 7:11 does on an objection. During a decision the cell is that answerer: it runs the reduction its catalogue names with the parameters the binding passes, and returns the value with where it came from. The engine records that provenance under `open_terms` (`filled_by: {answer: ...}`, with the binding's article) and as one node per answer in the trace. Each answer an input asked for and did not get is listed under `unanswered`, beside the answers that were given. When the execution traces, the request says so (`AnswerRequest::trace`), the cell returns the run of the reduction with its answer (`Answer::trace`), and the engine hangs that run under the node of the answer. The cell can also explain one answer on its own (`Cell::explain_answer`). A `REDUCTIE` is never in the engine's implements index, so loading one changes nothing by itself.

A binding fills the term like any other implementation: the term gets the binding's value. That is also true when the value is null, unless the term has a `default`; then the default applies ([RFC-036](/rfcs/rfc-036)). A finding is the authority's own judgement on the term in a single case. It never replaces the value of a policy, not even on a term that the law leaves to the authority per case. If a caller gives a finding for such a term, the engine refuses the execution, even when the case never reads the term. If the policy wants the authority to judge a case that the cell does not answer, it declares an open term of its own with `decided_per_case_by`, and uses that term when the answer is null ([RFC-048](/rfcs/rfc-048) §2). Outside a cell nobody gives answers, so a scenario gives the finding on the policy's own term.

## Conflict resolution

When multiple regulations implement the same open term, and the term has no `combine`, the engine resolves conflicts using two rules from legal theory:

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
