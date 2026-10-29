---
title: "Glossary"
description: "Dutch legal and technical terms used across RegelRecht, with English translations."
---

Dutch legal terminology used throughout RegelRecht, with English translations.

## Legal Hierarchy

These are the values of `regulatory_layer`, the schema field naming the kind of legal instrument a file is. The layer fixes which identifier the file must carry, and it ranks candidates when several regulations implement the same [open term](#regelrecht-specific-terms).

Note that *verordening* names four different things in this list. An EU-verordening is directly applicable across the Union and outranks national law; a gemeentelijke verordening is local. The shared word is not a shared concept.

| Value | Dutch | English | Description |
|-------|-------|---------|-------------|
| `GRONDWET` | Grondwet | Constitution | Supreme law of the Netherlands |
| `WET` | Wet | Act of Parliament | Formal legislation passed by the Staten-Generaal |
| `AMVB` | Algemene Maatregel van Bestuur (AMvB) | Order in Council | General administrative order by the Crown, on the basis of a wet |
| `KONINKLIJK_BESLUIT` | Koninklijk besluit (KB) | Royal decree | A decision of the Crown, used among other things to set entry into force |
| `MINISTERIELE_REGELING` | Ministeriële regeling | Ministerial regulation | Regulation issued by a minister under a delegation |
| `BELEIDSREGEL` | Beleidsregel | Policy rule | A rule on how a body uses a power it already has (Awb art. 1:3 lid 4). Binds the body, not the citizen directly |
| `EU_VERORDENING` | EU-verordening | EU Regulation | Directly applicable in every member state, without transposition |
| `EU_RICHTLIJN` | EU-richtlijn | EU Directive | Binding as to the result, transposed into national law by the member state |
| `VERDRAG` | Verdrag | Treaty | International agreement binding on the Netherlands |
| `UITVOERINGSBELEID` | Uitvoeringsbeleid | Implementation policy | Published practice of an executing body, below the level of a beleidsregel. Since schema v0.8.9 it may bind an open term to the named answer of a cell (`source.reduction`); it then names its holder in `competent_authority` ([RFC-048](/rfcs/rfc-048)) |
| `GEMEENTELIJKE_VERORDENING` | Gemeentelijke verordening | Municipal ordinance | Local regulation by a municipality |
| `PROVINCIALE_VERORDENING` | Provinciale verordening | Provincial ordinance | Regional regulation by a province |
| `WATERSCHAPS_VERORDENING` | Waterschapsverordening | Water authority ordinance | Regulation by a waterschap, the regional water authority |
| `REDUCTIE` | Reductie | Reduction | No regulation: a cell's reduction over its own chronicle (RFC-022 §4), in the cell's directory (`corpus/cells/<cell>/reductions/`); it names no `competent_authority` (schema v0.8.12). It alone reads a chronicle: decisions (`source.decisions`) and the facts the cell recorded (`source.facts`, [RFC-051](/rfcs/rfc-051)). Each gives one row per record, with its `basis` and `id`. Since schema v0.8.11 a parameter may name the article whose decisions it reads. Its cell gives it as a named answer (`cel.yaml`), which policy binds to an open term. A reduction of no cell (`corpus/cells/reductions/`), such as that of the Awb, is called by other reductions and reads from the chronicle of the cell that runs them. It does not act on law (no `implements` since schema v0.8.9, no `hooks`, `overrides`, `execution.produces` or `open_terms`) ([RFC-048](/rfcs/rfc-048)) |

## Administrative Law (Bestuursrecht)

| Dutch | English | Description |
|-------|---------|-------------|
| **Algemene wet bestuursrecht (Awb)** | General Administrative Law Act | Framework law for government decision-making |
| **Besluit** | Decision | A written decision of an administrative body constituting a public-law act (Awb art. 1:3 lid 1) |
| **Beschikking** | Individual decision | A besluit that is not of general scope (Awb art. 1:3 lid 2). The scope, not the number of people affected, is what distinguishes it |
| **Aanvraag** | Application | A request for a decision from a government body |
| **Bekendmaking** | Notification | Making a decision known to the addressee (Awb art. 3:41). The bezwaar period runs from this moment, not from the decision itself |
| **Bezwaar** | Objection | First recourse, filed with the body that decided |
| **Bezwaartermijn** | Objection period | Six weeks from bekendmaking (Awb art. 6:7 and 6:8) |
| **Beroep** | Appeal | Court appeal against a decision on bezwaar (Awb art. 8:1 with 7:1) |
| **Uitspraak** | Judgment | What the administrative court decides on a beroep (Awb art. 8:70). No besluit: a court is not a bestuursorgaan (Awb art. 1:1 lid 2 onder c) |
| **Belanghebbende** | Interested party | A person whose interest is directly affected by a besluit (Awb art. 1:2) |
| **Motivering** | Statement of reasons | The obligation to state the grounds a decision rests on (Awb art. 3:46) |

### Legal character

The values of `legal_character`, the schema field saying what kind of thing an output is.

| Value | Description |
|-------|-------------|
| `BESCHIKKING` | A besluit not of general scope (Awb art. 1:3 lid 2) |
| `BESLUIT_VAN_ALGEMENE_STREKKING` | A besluit that is of general scope, addressed to an open group |
| `RECHTSPOSITIE` | A legal position arising by operation of law, without a besluit and without an aanvraag (voting rights, majority, nationality) |
| `TOETS` | An assessment against a norm that is not itself a decision |
| `WAARDEBEPALING` | A determination of a value that other rules then use |
| `INFORMATIEF` | Informative output, carrying no legal consequence of its own |
| `UITSPRAAK` | The judgment of an administrative court on an appeal (Awb art. 8:70), which is not a besluit (schema v0.8.10) |

### Decision type

The values of `decision_type`, saying what kind of decision an output is within its legal character.

| Value | Description |
|-------|-------------|
| `TOEKENNING` | Granting what was applied for |
| `AFWIJZING` | Refusing what was applied for |
| `GOEDKEURING` | Approval of an act by another body |
| `AANSLAG` | A tax assessment |
| `ALGEMEEN_VERBINDEND_VOORSCHRIFT` | A generally binding rule |
| `BELEIDSREGEL` | A policy rule on the use of an existing power (Awb art. 1:3 lid 4) |
| `VOORBEREIDINGSBESLUIT` | A preparatory decision taken ahead of a further one |
| `ANDERE_HANDELING` | Another act that is not one of the above |
| `GEEN_BESLUIT` | Not a decision at all; the article establishes a fact |

## Government Bodies

| Dutch | English | Description |
|-------|---------|-------------|
| **Bestuursorgaan** | Administrative body | Any government body making decisions |
| **Staten-Generaal** | Parliament | Dutch legislature (Eerste + Tweede Kamer) |
| **Tweede Kamer** | House of Representatives | Lower house of Parliament |
| **Eerste Kamer** | Senate | Upper house of Parliament |
| **Gemeente** | Municipality | Local government |
| **Belastingdienst** | Tax Authority | National tax administration |
| **SVB** | Social Insurance Bank | Executes social insurance laws |
| **UWV** | Employee Insurance Agency | Executes employee insurance laws |
| **Dienst Toeslagen** | Benefits Administration | Executes the income-dependent benefit schemes, zorgtoeslag among them |
| **Provincie** | Province | Regional government, one layer above the municipality |
| **Waterschap** | Water authority | Regional body for water management, with its own regulatory power |

## Legislative Process

| Dutch | English | Description |
|-------|---------|-------------|
| **Memorie van Toelichting (MvT)** | Explanatory Memorandum | Legislative intent document accompanying a bill |
| **Wetsvoorstel** | Bill | Proposed legislation before Parliament |
| **Staatsblad** | Official Gazette | Where national legislation is formally published |
| **Gemeenteblad** | Municipal Gazette | Where municipal ordinances are formally published |
| **Provincieblad** | Provincial Gazette | Where provincial ordinances are formally published |
| **Waterschapsblad** | Water Authority Gazette | Where waterschap ordinances are formally published |
| **Inwerkingtreding** | Entry into force | Date a law becomes effective |

## Temporal Concepts

| Dutch | English | Description |
|-------|---------|-------------|
| **Geldigheid** | Validity | Period during which a law is in force |
| **Peildatum** | Reference date | Date at which conditions are evaluated |
| **Terugwerkende kracht** | Retroactive effect | Law applies to situations before its enactment |

## RegelRecht-Specific Terms

| Term | Dutch | Description |
|------|-------|-------------|
| **Rulework** | Regelwerk | One regulation as recorded in the [law format](/concepts/law-format): its legal text with the machine-executable interpretation beside it, identified by its `$id`. The whole rulework is written in the format; the legal text in it is a verbatim quotation that the format carries and does not interpret. Each dated YAML file is a version of the rulework, so a regulation that was amended once has one rulework with two versions. It need not be a statute: a ministerial regulation or a municipal ordinance gets a rulework too. The schema and the language are not a rulework. Neither is the legal source: *wet* and *regeling* keep naming what the legislature laid down. Several together form a *stelsel van regelwerken*. Older documents and the paper call it a law YAML, an executable specification or an encoding ([RFC-044](/rfcs/rfc-044)). See [Ruleworks](/concepts/ruleworks) |
| **Corpus** | Corpus | The git-versioned collection of ruleworks. A [federated corpus](/concepts/federated-corpus) spans more than one repository |
| **Open term** | Open term | The schema construct (`open_terms`) for a value that an article does not set itself. A regulation fills it, under delegation or in co-government. When the term carries `decided_per_case_by` and no regulation fills it, the competent authority gives a finding in each case. See [Inversion of Control](/concepts/inversion-of-control) |
| **Open norm** | Open norm | A standard the law leaves vague on purpose, such as *redelijkerwijs* or *in bijzondere gevallen*, so that its content is decided case by case. Not the same thing as a delegated value, though the format records it in the same place: as an open term with `decided_per_case_by`. It is not a marking, because the language can express it. See [RFC-031](/rfcs/rfc-031) |
| **Implements** | Gelet op | A regulation declaring which open terms of a law it fills. The schema field carries `gelet_op`, the citation the Dutch instrument opens with in its preamble. The preamble records the basis; the power itself comes from the article of the law |
| **Cross-law reference** | Verwijzing | A law reading an output of another law through a `source` block. See [Cross-Law References](/concepts/cross-law-references) |
| **Parameter** | Parameter | A value the caller supplies with the question, such as the `bsn` of the person it is about. See [Law Format](/concepts/law-format#parameters-inputs-and-outputs) |
| **Input** | Invoer | A value an article needs but does not decide itself. It comes from another law through `source`, or from outside the corpus |
| **Data source** | Gegevensbron | Where an input with `source: {}` comes from: a register, the caller or the person, anything outside the corpus. The empty block says that no law in the corpus computes the value |
| **Nullable** | - | `nullable: true` on a parameter, input or output: absence (`null`) is a legitimate value of that field. Defaults to `false`, and the type checker and the engine hold a law to it. See [Absent and unknown values](/concepts/values#absent-and-unknown-values) |
| **Legal basis** | Grondslag | `legal_basis`: the provision an element cites for itself, such as the article that creates a delegation on an open term, or the lid an action carries out. A trace reports it next to the place the engine actually was ([RFC-039](/rfcs/rfc-039)) |
| **Basis** | - | `basis`: the article a decision was taken under, as `<regulation>#<article>`, in a decision value and in a chronolexogram. Not the same thing as `legal_basis` |
| **Produces** | - | The annotation stating what an article yields in legal terms, its `legal_character` and `decision_type`. Hooks fire on it. See [The produces annotation](/concepts/hooks-and-reactive-execution#the-produces-annotation) |
| **Override** | Lex specialis | `overrides`: a specific provision that replaces a value set by a general one, as article 69 Vreemdelingenwet 2000 sets a four-week objection period in departure from article 6:7 Awb. Declared by the overriding law alone. See [Overrides](/concepts/hooks-and-reactive-execution#overrides-lex-specialis) |
| **Marking** | Markering | A construct the format cannot yet express, flagged on the article. Called `untranslatables` before schema v0.7.0. See [Markings](/concepts/markings) |
| **Void** | Bestaat geen aanspraak | An override stating that an output does not arise at all, rather than being replaced by a value. Not the same as an entitlement of zero, which is still a decision carrying legal remedies. See [Voiding an output](/concepts/hooks-and-reactive-execution#voiding-an-output) |
| **Hook** | Haak | Logic that fires at a stage of a procedure rather than on a direct request. See [Hooks and Reactive Execution](/concepts/hooks-and-reactive-execution) |
| **Trace** | Spoor | The tree showing how each value in an execution was computed, published as a `{trace_version, root}` document whose nodes carry an address and the provision they came from ([RFC-039](/rfcs/rfc-039)). See [Traceability](/concepts/traceability) |
| **Execution Receipt** | Uitvoeringsbewijs | The sealed record of one execution: engine, schema, law version and hash, so the result can be reproduced. See [Execution Provenance](/concepts/execution-provenance) |
| **Traject** | Traject | A working context in the editor, with its own members, roles and branch, optionally backed by its own repository |
| **Chronolexogram** | Chronolexogram | A dated recording of a legal fact in the chronicle of the organization that establishes it: a submission, a decision or an executogram. The term comes from the [Chronolexografie position paper](https://chronolexografie.nl/position-paper/). See [Chronolexography](/concepts/chronolexography) |
| **Chronicle** | Kroniek | The dated list of chronolexograms that a cell keeps. The cell adds entries to it and never changes or removes one ([RFC-046](/rfcs/rfc-046)) |
| **Cell** | Cel | An organization that keeps its own chronicle and answers questions from it, such as a school or a municipality. In RegelRecht a cell is a directory with a `cel.yaml`. That file says who the organization is and which answers it gives. The code in `packages/cel` runs the cell ([RFC-046](/rfcs/rfc-046)) |
| **Executogram** | Executogram | A chronolexogram of a fact that is neither a submission nor a decision, such as the place where a school is established. An article that needs such a fact declares it with `requires`, and that declaration fixes which fields the executogram has ([RFC-051](/rfcs/rfc-051)) |
| **Reduction** | Reductie | A rulework (`regulatory_layer: REDUCTIE`) that reads a cell's chronicle and gives a named answer, such as whether a pupil already had leave this school year. It is the only kind of rulework that may read a chronicle, and it runs in the cell that keeps that chronicle. A reduction fills no open term. The policy of the deciding authority decides which term the answer fills ([RFC-048](/rfcs/rfc-048)) |
| **Finding** | Bevinding | What the authority itself decides on an open norm in a single case, given for an open term with `decided_per_case_by`. When a policy fills the term, the engine refuses a finding on it ([RFC-048](/rfcs/rfc-048) §2). See [Inversion of Control](/concepts/inversion-of-control) |
| **Synthesis** | Synthese | An article that makes one outcome of the answers of different cells to different questions. The parking card is an example. Its decision takes the GGD's answer on the medical examination and the municipality's own answer on an earlier card. Synthesis happens at the consumer, never in a cell ([RFC-048](/rfcs/rfc-048)). The [position paper](https://chronolexografie.nl/position-paper/) calls synthesis the combining of lexostatussen from several cells at the consumer, and leaves open whether those cells answer the same question. RegelRecht asks each question of one cell. How one question would reach several cells is an [open question](/concepts/chronolexography/open-questions) |
| **Bevoegd gezag** | Bevoegd gezag | The body competent to take a decision under a given article. See [Competent Authority](/concepts/competent-authority) |
| **Absent vs. unknown** | Afwezig versus onbekend | A value that does not exist, against a value nobody has. The engine keeps them apart rather than treating both as empty. See [Values](/concepts/values#absent-and-unknown-values) |
| **Collection** | Verzameling | A group of values whose size is not known in advance, such as the medebewoners in a household. `FOREACH` iterates over one, filters it and can combine the results into one value. See [Collections](/concepts/collections) |
| **Unit** | Eenheid | `type_spec.unit`: what an amount is counted in (eurocent, days, a percentage). A label that never changes the value; the engine rejects a calculation that mixes units that do not fit ([RFC-023](/rfcs/rfc-023)). See [Amounts and units](/concepts/values#amounts-and-units) |
| **Type checking** | Typecontrole | The static check that `just validate` and the engine's loader run over a law's expressions, against the types and nullability the law declares. A law that fails it cannot be loaded ([RFC-037](/rfcs/rfc-037)) |
| **Scenario** | Scenario | A case written in Gherkin: the facts, and the outcome a law should give on them. The engine that executes the law executes the scenario. See [Scenarios](/concepts/scenarios) |
| **Execution-first** | - | The validation method in which an interpretation is run against concrete cases, often from the Memorie van Toelichting, and corrected until the outcomes hold, instead of being analyzed in full before anything runs. See [Execution-First Validation](/concepts/methodology) |
| **Reverse validation** | - | The check after generation that every element of a machine-readable article traces back to the legal text. Logic that cannot be grounded in the text is flagged as possibly invented. See [the interpretation loop](/concepts/methodology#one-law-the-interpretation-loop) |
| **Enrichment** | Verrijking | The pipeline stage in which a language model drafts `machine_readable` sections for a harvested law. The program doing it is the enricher, and its output is a draft that automated checks and then people review. See [Pipeline](/components/pipeline) |
| **WASM** | - | WebAssembly. The engine compiled from the same code as the native build, to run in a browser. See [Engine](/components/engine) |
| **Werkpakket** | Werkpakket | A unit of work on the [roadmap](/roadmap). Every pull request names the werkpakket it contributes to |
| **Bucket A / Bucket B** | - | The two BDD suites: law validation against the real corpus, and engine conformance against synthetic laws. See [Testing](/guide/testing) |

## Abbreviations

| Term | Stands for | Description |
|------|------------|-------------|
| **Awb** | Algemene wet bestuursrecht | The general administrative law act |
| **Awir** | Algemene wet inkomensafhankelijke regelingen | The framework act for income-dependent benefit schemes |
| **BWB** | Basis Wettelijke Regelgeving | The national register of legislation, and the source the harvester reads |
| **CVDR** | Centrale Voorziening Decentrale Regelgeving | The register of decentralized (municipal, provincial, water authority) regulation |
| **MvT** | Memorie van Toelichting | The explanatory memorandum accompanying a bill |
| **FSC** | Federated Service Connectivity | The standard for service-to-service connections between government organizations |
| **JAS** | Juridisch Analyseschema | A legal analysis schema used in wetsanalyse |
