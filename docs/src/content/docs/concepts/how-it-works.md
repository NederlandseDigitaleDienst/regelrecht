---
title: "How RegelRecht Works"
description: "What RegelRecht is and what it is for, the parts it consists of, and a map of the concept pages that explain each of them."
---

RegelRecht writes Dutch legislation down in a form a computer can execute, and publishes it so that anyone can check how a decision was reached. It is an open-source exploration by the Nederlandse Digitale Dienst. No government agency takes decisions with it today.

This page is the map: what the project tries to do, and which page covers each concept. Which programs do the work is on the [architecture overview](/guide/architecture); the Dutch legal terms and the project's own words are in the [glossary](/reference/glossary).

## The problem

Dutch law is published as text. An agency that has to carry it out translates the rules into software by hand, and each agency writes its own reading of the same law. Nobody can check whether those programs match the text Parliament adopted, a citizen cannot see how a decision about them was made, and there is no way to test the software against the law itself.

## The approach

RegelRecht records each regulation once, as a [rulework](/concepts/ruleworks): the published text, article by article, with the executable interpretation beside each article. One engine runs every rulework, and the same rulework gives the same answer in the editor, in a browser and on a server.

A rulework is an interpretation of the law, not the law. Where the two conflict, the statute prevails and the rulework is corrected; legal validity stays with the published text ([Rules as Executed, section 4.2](/research/rules-as-executed#sec:legalstatus)). What the format adds is that the interpretation is written down where anyone can read it and run it, instead of staying inside an agency's software.

| Principle | What it means |
|-----------|---------------|
| **Close to the text** | A rulework follows the structure of the law, article by article, and each article links to its official text. |
| **No domain knowledge in the engine** | The engine has no holidays, tax rates or special cases of its own. Everything comes from the ruleworks. |
| **Identical execution** | Browser, server, editor: the same inputs give the same result. |
| **Version control as governance** | Git history records how the ruleworks changed. A branch is a proposal, a merge is publication. |
| **Traceability** | Every computed value points back to the article and lid it came from. |
| **Open by default** | Ruleworks, tooling and design decisions are public. |

## An example

Article 2 of the Wet op de zorgtoeslag entitles an insured person to an allowance equal to the difference between the standard premium and the normative premium. In the rulework, the `machine_readable` section of that article states the calculation: derive the normative premium from the income as the Awir defines it, take the standard premium a ministerial regulation sets each year, subtract, and never go below zero.

Ask the engine whether someone qualifies on 1 January 2025 and it loads the zorgtoeslag rulework, follows its references into the Awir, the Zorgverzekeringswet and the ministerial regulation, and returns the amount with a trace naming every article it applied. [Law Format](/concepts/law-format) walks through article 3 of the same law field by field.

## The parts

| Part | What it is | Read more |
|---|---|---|
| Ruleworks and the corpus | One rulework per regulation, in YAML, with a version per date its text took effect. The corpus collects them, and can draw on repositories of other authorities. | [Ruleworks](/concepts/ruleworks), [Corpus](/components/corpus) |
| Harvester and enrichment | The harvester writes a rulework with the legal text from the national registers (BWB, CVDR). Enrichment adds the executable interpretation, with a person checking the result. | [Harvester](/components/harvester), [Pipeline](/components/pipeline) |
| Engine | Executes a rulework for a question, a date and a set of facts, natively or as WebAssembly. It follows references into other ruleworks and returns a trace. | [Engine](/components/engine) |
| Scenarios | Cases in Gherkin that the engine runs against a rulework, written so that a legal expert can read them. | [Scenarios](/concepts/scenarios) |
| Editor | A web application to read, edit, annotate and enrich ruleworks. | [Editor](/components/frontend) |
| Cells | An organization that records what it decides and answers later questions from that record. A proof of concept for now. | [Chronolexography](/concepts/chronolexography) |
| Demo | Runs a corpus of several dozen ruleworks end to end in the browser, on synthetic data. | [Demo](/components/demo) |

## The concepts

**The rulework.** [Ruleworks](/concepts/ruleworks) follows a rulework from harvest to execution. [Law Format](/concepts/law-format) explains how one is put together, and [Values](/concepts/values) what kinds of value it computes with: amounts, percentages, rounding, and the difference between an absent and an unknown value. [Temporal Validity and Dates](/concepts/temporal-and-dates) covers versions and date arithmetic, [Collections](/concepts/collections) groups of unknown size such as a household, and [Markings](/concepts/markings) the places where the logic cannot yet follow the text.

**Between laws.** A law reads values another law establishes ([Cross-Law References](/concepts/cross-law-references)), leaves values open for a lower regulation or a municipality to fill ([Inversion of Control](/concepts/inversion-of-control)), and is joined by general rules such as the Awb that fire on every decision ([Hooks and Reactive Execution](/concepts/hooks-and-reactive-execution)). [Applications, Decisions and Objections](/concepts/administrative-procedures) shows how a procedure from application to objection is modeled. [Competent Authority](/concepts/competent-authority) says which body decides.

**Recording decisions.** [Chronolexography](/concepts/chronolexography) is the cell model: an organization keeps a dated chronicle of what it established and answers from it. What is still open there is listed in [Open Points in Chronolexography](/concepts/chronolexography-open-points).

**Accountability.** [Traceability](/concepts/traceability) shows how to read the trace of an execution, and [Execution Provenance](/concepts/execution-provenance) the receipt that is meant to make a decision reproducible.

**Many authorities.** [Federated Corpus](/concepts/federated-corpus) lets each authority keep its ruleworks in its own repository. [Notes and Annotations](/concepts/notes-and-annotations) attach comments to the legal text without changing it.

**Method.** [Execution-First Validation](/concepts/methodology) is how an interpretation is checked by running it. [Branches of Law](/concepts/branches-of-law) surveys which areas of Dutch law fit rule-based execution.

## What is not built

The engine computes everything itself: exchanging signed results between organizations ([RFC-009](/rfcs/rfc-009)) is proposed and not built. The execution receipt exists but is not yet signed or attested ([RFC-013](/rfcs/rfc-013)). Expert workshops that validate a rulework are designed but have not been held. What legal status a rulework could have, and who would adopt it, is a research question on the [roadmap](/roadmap), together with publication infrastructure.
