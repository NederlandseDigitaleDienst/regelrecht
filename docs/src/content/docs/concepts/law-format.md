---
title: "Law Format"
description: "How a rulework is put together, walked through on one real article of the Wet op de zorgtoeslag."
---

Laws in RegelRecht are stored as YAML files that conform to the [law schema](/reference/schema). Each file holds one regulation as it reads at one point in time: the published text, article by article, and next to an article's text the logic that executes it. The files of one regulation together are its rulework (Dutch: *regelwerk*), and each file is a version of it.

This page explains how such a file is put together and why it looks the way it does, using one real article as the thread. It does not list fields. The [Schema Reference](/reference/schema) does that, generated from the released schema, so what you read there cannot fall behind the contract. Where this page mentions a field, it links to the entry that defines it.

## File organization

A file's place in the corpus says what kind of instrument it is and when its text took effect:

```
corpus/regulation/nl/
├── wet/                              # Formal laws (wetten)
│   ├── wet_op_de_zorgtoeslag/
│   │   └── 2025-01-01.yaml
│   ├── participatiewet/
│   │   └── 2022-03-15.yaml
│   └── burgerlijk_wetboek_boek_5/
│       └── 2024-01-01.yaml
├── ministeriele_regeling/            # Ministerial regulations
│   └── regeling_standaardpremie/
│       ├── 2024-01-01.yaml
│       └── 2025-01-01.yaml
└── gemeentelijke_verordening/        # Municipal ordinances
    ├── amsterdam/
    │   └── apv_erfgrens/
    │       └── 2024-01-01.yaml
    └── diemen/
        └── afstemmingsverordening_participatiewet/
            └── 2015-01-01.yaml
```

The directory name is the law's `$id`, and the file name is the date the version took effect. A law that changes gets a new file beside the old one rather than an edit to it, because a decision taken in 2024 has to be recomputable against the 2024 text. [Temporal Validity and Dates](/concepts/temporal-and-dates#which-version-is-in-force) explains how the engine picks the version in force on a given date.

## A worked example

The walkthrough follows article 3 of the Wet op de zorgtoeslag, from `corpus/regulation/nl/wet/wet_op_de_zorgtoeslag/2025-01-01.yaml`. The article is short, yet it needs a value from another law before it can decide anything, which is where most of the format's machinery comes from. In plain terms, nobody is entitled to zorgtoeslag if their assets at the start of the year exceed € 141.896, or € 179.429 for someone with a partner.

### The header

A file opens with the metadata that identifies the regulation:

```yaml
$schema: https://raw.githubusercontent.com/NederlandseDigitaleDienst/regelrecht/refs/tags/schema-vX.Y.Z/schema/vX.Y.Z/schema.json
$id: wet_op_de_zorgtoeslag
regulatory_layer: WET
publication_date: '2024-12-20'
valid_from: '2025-01-01'
bwb_id: BWBR0018451
url: https://wetten.overheid.nl/BWBR0018451/2025-01-01
name: '#wet_naam'
competent_authority: '#bevoegd_gezag'
```

`$schema` pins the schema version the file was written against, by an immutable git tag, so the rules a file was checked under can never change beneath it. The current version and the URL to copy are on the [Schema Reference](/reference/schema#current-version).

`$id` is the name other laws use to refer to this one. The schema does not require it, and a file without one still loads and runs, but nothing can then point at it. In the corpus every file carries one, equal to its directory name.

`regulatory_layer` decides more than it seems to. It fixes which official identifier the file must carry (a `bwb_id` for a national law, a `gemeente_code` for a municipal ordinance), and it ranks regulations when several of them fill the same delegated term. The rules per layer are in [Identifiers per regulatory layer](/reference/schema#identifiers-per-layer).

`name` and `competent_authority` start with `#`. That is an internal reference: the value is computed by an output of this same law. Article 8 of the Wet op de zorgtoeslag reads "Deze wet wordt aangehaald als: Wet op de zorgtoeslag", so the law's name is itself law text, and the file says where that text is instead of copying it. The schema accepts the same form for `valid_from`, for a law whose commencement is fixed elsewhere, but the engine does not resolve it yet. The loader accepts such a file; version selection then reports that whether the law was in force on the date cannot be determined, and when two regulations on the same layer fill one open term, lex posterior refuses to compare a `#` start date (`packages/engine/src/priority.rs`). A law that has to run needs a concrete date in `valid_from`. The full list of top-level keys, and which are required, is in [The rulework](/reference/schema#law-file). `competent_authority` is not among them: the schema declares it per `machine_readable` section, where it also accepts the `#` form, and at the top of a file it is tolerated but not checked. The article is the right place for it: a law grants authority article by article, not in its preamble or its title.

### Articles

Each entry under `articles` mirrors one article of the published law. It carries the article's `number`, its `text` verbatim, and the `url` of that article on wetten.overheid.nl. The text is not a paraphrase: it is what the logic below it will be checked against, by people and by [reverse validation](/concepts/validation-methodology).

An article may have a `machine_readable` section. Most do not. Article 6 of this law obliges the minister to send parliament a report every four years, which decides nothing about any citizen, and it is carried as text only. [Articles](/reference/schema#articles) in the reference lists the fields.

Article 3 does have one:

```yaml
- number: '3'
  text: >-
    1. In afwijking van artikel 7, derde lid, van de Algemene wet inkomensafhankelijke
    regelingen, bestaat geen aanspraak op een zorgtoeslag indien de rendementsgrondslag
    [...] meer bedraagt dan € 141.896 of, indien de verzekerde het gehele berekeningsjaar
    dezelfde partner heeft, de gezamenlijke rendementsgrondslag [...] meer bedraagt dan
    € 179.429. [...]
  url: https://wetten.overheid.nl/BWBR0018451/2025-01-01#Artikel3
  machine_readable:
    definitions:
      vermogensgrens_verzekerde:
        value: 14189600
        type: amount
        type_spec:
          unit: eurocent
      vermogensgrens_gezamenlijk:
        value: 17942900
        type: amount
        type_spec:
          unit: eurocent
    execution:
      produces:
        legal_character: TOETS
        decision_type: GOEDKEURING
      parameters:
        - name: bsn
          type: string
          required: true
      input:
        - name: vermogen
          type: amount
          source:
            regulation: wet_inkomstenbelasting_2001
            output: rendementsgrondslag
            parameters:
              bsn: $bsn
          type_spec:
            unit: eurocent
        - name: heeft_toeslagpartner
          type: boolean
          source:
            regulation: algemene_wet_inkomensafhankelijke_regelingen
            output: heeft_toeslagpartner
            parameters:
              bsn: $bsn
        - name: bsn_toeslagpartner
          type: string
          nullable: true
          source:
            regulation: algemene_wet_inkomensafhankelijke_regelingen
            output: bsn_toeslagpartner
            parameters:
              bsn: $bsn
        - name: vermogen_toeslagpartner
          type: amount
          nullable: true
          source:
            regulation: wet_inkomstenbelasting_2001
            output: rendementsgrondslag
            parameters:
              bsn: $bsn_toeslagpartner
          type_spec:
            unit: eurocent
        - name: heeft_gehele_berekeningsjaar_dezelfde_partner
          type: boolean
          source: {}
      output:
        - name: vermogen_onder_grens
          type: boolean
      actions:
        - output: vermogen_onder_grens
          value:
            operation: AND
            conditions:
              - operation: LESS_THAN_OR_EQUAL
                subject: $vermogen
                value: $vermogensgrens_verzekerde
              - operation: IF
                cases:
                  - when:
                      operation: AND
                      conditions:
                        - operation: EQUALS
                          subject: $heeft_toeslagpartner
                          value: true
                        - operation: EQUALS
                          subject: $heeft_gehele_berekeningsjaar_dezelfde_partner
                          value: true
                        - operation: NOT
                          value:
                            operation: EQUALS
                            subject: $vermogen_toeslagpartner
                            value: null
                    then:
                      operation: LESS_THAN_OR_EQUAL
                      subject:
                        operation: ADD
                        values:
                          - $vermogen
                          - $vermogen_toeslagpartner
                      value: $vermogensgrens_gezamenlijk
                default: true
```

The rulework in the corpus also puts a `legal_basis` on the action, quoting the lid it carries out ([RFC-039](/rfcs/rfc-039)); it is left out here. The sections below take this apart in reading order. Everything the section can hold is listed under [The machine_readable section](/reference/schema#machine-readable).

### Definitions

`definitions` holds the constants the article states outright: here the two asset limits. The article names them in euros; the file stores them in eurocent, like every other amount in this law, and says so with a unit. The bare form, `vermogensgrens_verzekerde: 14189600`, is valid too, but it leaves the value without a unit, and an execution trace then reports it as a plain number.

A definition belongs to the article whose text states it. If a limit changes by ministerial regulation next year, the change lands in that article's new version and nowhere else.

### Parameters, inputs and outputs

The `execution` block separates three kinds of value by where they come from.

A **parameter** is what the caller supplies: here the `bsn` of the person the question is about. An **input** is a value the article needs but does not decide. Article 3 needs the person's assets, which the Wet inkomstenbelasting 2001 defines as the rendementsgrondslag, so the input names that law and output under `source` and passes the `bsn` along. The engine runs the other law and uses its answer; [Cross-Law References](/concepts/cross-law-references) covers how. An input with `source: {}` comes from outside the corpus altogether, from a register or the person themselves.

The partner takes three more inputs. Who the partner is follows from article 3 of the Awir, which yields the partner's `bsn` or `null` when there is none. The partner's rendementsgrondslag comes from the same provision of the Wet inkomstenbelasting 2001 as the applicant's, called with that `bsn`. Without a partner the `bsn` is `null`, the engine does not run the other law, and `vermogen_toeslagpartner` is `null` as well. Both inputs say so with `nullable: true`, and the action tests for the absence before it adds anything ([RFC-036](/rfcs/rfc-036)). That test has a cost. A partner who is registered as a partner but without a `bsn` passes it as if there were no partner, and the applicant is then tested on their own assets alone; the type checker requires the test, and a law has no way to fail on data it knows to be wrong ([issue #1563](https://github.com/NederlandseDigitaleDienst/regelrecht/issues/1563)). The third input, whether the applicant had the same partner for the whole berekeningsjaar, is a fact about the year that no law in the corpus establishes, so it has `source: {}`.

An **output** is what the article decides and offers to others. Its `name` is public: another law that needs this answer asks for `vermogen_onder_grens` by name, which makes renaming an output a change other laws can see. The reference lists the fields each kind carries under [Fields](/reference/schema#fields).

Values are referred to as `$name`, whether they are parameters, inputs, outputs or definitions. Dot notation reads a property, as in `$referencedate.iso`. A string that starts with `$` is always a reference, never a literal. The `#` form from the header is not: inside an operation, `'#wet_naam'` is the literal text `#wet_naam`.

### Operations

An action computes one output. The logic is a tree of operations, and article 3 follows the sentence of the law. Lid 1 names two grounds joined by "of": the applicant's own assets exceed the first limit, or, for someone with the same partner all year, the joint assets exceed the second. The test passes when neither ground holds, so the action is an `AND` of the first comparison and a conditional that applies the second only to a partner of the whole year. Dienst Toeslagen applies only the joint limit to a couple; the two readings differ when the applicant's own assets are above the first limit and the joint assets below the second. The model follows the letter, and a scenario in the corpus fixes that choice so it can be revisited on purpose.

The schema defines 28 operations: arithmetic, comparison, logic, a conditional, rounding, collections and dates. Each one, with the operands it takes, is in [Operations](/reference/schema#operations) in the reference. They share one shape ([RFC-004](/rfcs/rfc-004)): the operation names what happens, and named operands say what it happens to, so `subject` and `value` in a comparison read in the order the law states them.

The engine also accepts the compat aliases `NOT_EQUALS`, `IS_NULL`, `NOT_NULL` and `NOT_IN`, and `SWITCH` as an alias of `IF`. The schema does not, so a law that uses one parses and then fails `just validate`. Write the positive operation wrapped in `NOT` instead.

The set is short on purpose. An operation earns its place when a real law needs it, not when an engine could plausibly offer it: `ROUND` because a law rounds to whole euros, `DATE_DIFF` because a deadline is measured in days, `FOREACH` because a norm counts medebewoners. What an engine *can* do is close to unbounded, and every operation added on that basis is a promise the schema, the editor, the conformance suite and every other engine have to keep. A law that cannot be expressed is the signal to extend the language; the absence of an operation someone imagined a use for is not. The position paper argues the same point in [section 9.2](/research/rules-as-executed#sec:opset).

### What the article produces

`produces` states what executing the article yields in legal terms. Article 3 is a `TOETS`: it tests a condition, and its outcome feeds the decision article 2 takes. Article 2 declares a `BESCHIKKING`, and that difference matters to the engine. The legal character selects which procedural hooks fire, such as the objection period the Awb attaches to a beschikking, so an article's outcome and the procedure around it both depend on this declaration. [Hooks and Reactive Execution](/concepts/hooks-and-reactive-execution#the-produces-annotation) explains the mechanism, and the [glossary](/reference/glossary#legal-character) defines each value.

An article that yields more than one thing writes `produces` as a list, one item per thing (schema v0.8.1). Each item says what it is with `type`, a kind of submission or a legal character. Leerplichtwet 1969 art. 13a establishes the application for leave and decides on it. The application lists the parameters it contains under `parameters` and names the decision it asks for with `requests`; the decision lists its outputs under `outputs`. [RFC-045](/rfcs/rfc-045) shows it and gives the rules.

#### Derived data

Not every output of a deciding article is part of what it decides. The decision lists those under `derived` (schema v0.8.4). For example, a parking card for the disabled:

```yaml
produces:
  - type: BESCHIKKING
    outputs: [gehandicaptenparkeerkaart_verstrekt]   # what the college decides
    derived: [kaart_geldig_tot]                      # what follows from it (BABW art. 51 lid 1)
```

| | `outputs` | `derived` |
|---|---|---|
| Computed when the decision is taken | yes | yes |
| Recorded with the decision | yes | yes |
| Part of the decision value (`$x.outputs`) | yes | no |

The difference matters when a decision is compared with another, as on objection (Awb 7:11). The validity of the card runs from the day of issue. If the college takes the decision anew on another day, the end date differs, although the college decides the same thing. Only the outputs count.

Every output of an article belongs to at most one item, under `outputs` or `derived`. A submission without `outputs` yields none; a decision without `outputs` yields every output no other item lists. An output that belongs to no item is not part of the decision; another article reads it through `source.regulation` ([RFC-045](/rfcs/rfc-045)).

#### What the loader checks

- One item per `type`, and at most one decision.
- No parameter or output the article does not have, and no output given to two items.
- No output that belongs to no item, unless a decision beside it lists its own outputs. A forgotten or misspelled output fails instead of quietly becoming derived data.
- `derived` names outputs of the article, and never sits on a submission.
- `requests` sits on a submission only. Within the same law it must name an article that takes a decision; one to another law is checked when it is used, since that law may load later.

## Type Specifications

A field's `type` says what kind of value it is; `type_spec.unit` says what an amount is counted in. Units exist because a law mixes euros, eurocent, fractions, percentages and durations, and adding a number of days to an amount of money is a mistake a type system can catch ([RFC-023](/rfcs/rfc-023)).

A unit is a **label, never a computational constraint**: tagging a value never changes it. In particular, a `percentage` is not silently divided by 100; any `… / 100` is an explicit operation written where the value is applied. `ratio` and `percentage` are distinct labels for the same dimension, and the corpus keeps both so that a law saying "1,896" and a law saying "30 procent" are each transcribed as written. The available units are listed under [unit](/reference/schema#unit).

The engine uses the labels to reject combinations that make no sense: adding `eurocent` to `days`, or `euro` to `eurocent`, is a unit mismatch. A dimensionless `ratio` or `percentage` multiplied by an amount keeps the amount's unit. Units are opt-in per law. An unannotated value has unit `unknown` and is never checked, so a law is unaffected until someone annotates it. Once it declares units anywhere, `just validate` also flags `amount` outputs that lack one.

`type_spec` also accepts `precision`, `min` and `max`. The engine parses these as metadata and does not enforce them, so `type_spec.precision` rounds nothing. A law that rounds says so with an operation, described next.

## Rounding and precision

Rounding is an explicit instruction written where the law gives it. The engine never rounds a value implicitly, not even money, and intermediate values keep full precision ([RFC-024](/rfcs/rfc-024)). Three operations cover the ways Dutch law phrases it:

| Operation | Direction | Dutch |
|-----------|-----------|-------|
| `ROUND` | nearest, half-up (ties away from zero) | *rekenkundig afronden* |
| `CEIL` | up (toward +∞) | *naar boven afronden* |
| `FLOOR` | down (toward −∞) | *naar beneden afronden / afkappen* |

Half-up is the default because the Hoge Raad applies it where a law says "afronden" without a direction. Each takes one `value` and a `precision` in the value's own unit: `0` rounds to whole units, and `-2` rounds an amount in eurocent to whole euros.

```yaml
# Round the result of a SUBTRACT to whole eurocent, half-up
- output: tegemoetkoming
  value:
    operation: ROUND
    precision: 0
    value:
      operation: SUBTRACT
      values:
        - $normbedrag
        - $eigen_bijdrage
```

The operand rules are in [Rounding](/reference/schema#rounding) in the reference.

## Absent and unknown values

A register does not hold a value for everyone, and the engine keeps two cases apart ([RFC-036](/rfcs/rfc-036)).

An **absent** value, `null`, is a fact: the register says there is none (no partner, no rent). A field may hold it only when its declaration says `nullable: true`. A law tests it with `EQUALS … null` and may branch on it, but calculating, ordering or deciding on it is an error, because a legal text never treats "geen" as an amount or a verdict without saying so. `just validate` checks this before the law runs ([RFC-037](/rfcs/rfc-037)): a `null` test on a non-nullable field, a `null` reaching a sum without an absence test, or an `IF` without `default` on a non-nullable output is refused.

An **unknown** value is a fact nobody has yet: a `source: {}` input no data source could supply, or an optional parameter the caller did not pass. A law cannot write an unknown. It propagates through every operation (a definite `false` still decides an `AND`, a definite `true` an `OR`) and reaches the output carrying the names of the missing facts, so a portal can ask for them.

In a scenario's data table an empty cell is unknown and the word `null` is absence; the assertions read `is absent` and `is unknown`. This is the reverse of SQL, where `null` stands for the unknown. The RFC explains why the literal was kept for absence.

## Between laws

Article 3 reads from other laws through `source`. The other direction exists as well: a law can leave a value open for another regulation to fill. Article 4 of the same law does that with the standaardpremie, which the minister sets each year:

```yaml
- number: '4'
  machine_readable:
    open_terms:
      - id: standaardpremie
        type: amount
        required: true
        delegated_to: minister
        delegation_type: MINISTERIELE_REGELING
        legal_basis: artikel 4 Wet op de zorgtoeslag
```

The Regeling standaardpremie then declares under `implements` that it fills this term, citing the same article in its `gelet_op`. The law does not name the regulation, and the regulation does not change the law; the engine connects them when it loads the corpus. [Inversion of Control](/concepts/inversion-of-control) covers the pattern, including what happens when more than one regulation fills the same term.

A `machine_readable` section can hold more than a calculation. `hooks` let an article react to a decision another law produces, `overrides` let a special provision set aside a general one, and `markings` record where the logic could not follow the text exactly. Each has its own concept page: [Hooks and Reactive Execution](/concepts/hooks-and-reactive-execution) and [Markings](/concepts/markings).

The examples on this page are written to schema v0.5.x, because the corpus is. Schema v0.7.0 added `markings` (the new name for `untranslatables`), `declares`, `placement` and `voids`, and no corpus law uses any of them yet: the real laws are all on v0.5.x and still carry `untranslatables`, and the one file on v0.7.0 is the synthetic `test_date_operations`, which needs its date operations. Read the [Schema Reference](/reference/schema) for the v0.7.0 fields; do not look for them in the corpus.

## Corpus contents

The corpus spans three regulatory layers: national law (`WET`) makes up most of it, with a few ministerial regulations and municipal by-laws that fill in national law at the local layer. Next to the real laws, `corpus/regulation/nl/wet/` holds synthetic laws in the `test_*` directories. They are not Dutch law: each one isolates a corner of the language (null semantics, scoped sources, collections, date operations) for the engine-conformance BDD bucket, which needs a law that tests one feature rather than a statute that mixes many.

This page gives no counts, because they change with every harvest. The authoritative set is [`corpus/regulation/`](https://github.com/NederlandseDigitaleDienst/regelrecht/tree/main/corpus/regulation) itself. To count files per layer, run `grep -rh '^regulatory_layer:' corpus/regulation/ | sort | uniq -c` from the repository root. That counts versions, not laws: each version of a law is its own file, named after its `valid_from` date, in the law's directory.

## Next steps

- [Schema Reference](/reference/schema) - every field, operation and vocabulary, generated from the schema
- [Testing](/guide/testing) - writing BDD scenarios for laws
- [Engine](/components/engine) - how the engine executes laws
- [Rules as Executed, section 9.2](/research/rules-as-executed#sec:opset) - why the position paper restricts the operation set and demands termination
