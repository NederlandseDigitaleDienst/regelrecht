---
title: "Open Points in Chronolexography"
description: "The choices in the chronolexography implementation that are still open, and the parts that need investigation or a large change."
---

What is open in [chronolexography](/concepts/chronolexography), one row per point. RFCs link here instead of keeping their own list, and a closed point loses its row.

A point is listed when a choice is still to be made, when it needs investigation first, or when it needs a large refactor or a breaking change to the schema. Missing functionality that only needs building is not listed; it gets built when a case needs it. Neither is a limitation of the proof of concept that does not affect how chronolexography works.

Elsewhere:

- the Awb procedure in the law format: [Applications, Decisions and Objections](/concepts/administrative-procedures#open-points);
- the [open questions of RFC-022](/rfcs/rfc-022#open-questions), still with its authors: cell roles, security-context binding, update cadence, and whether Blauwe Knop and FSC are one mechanism.

| Point | Why open | RFC |
|---|---|---|
| Should `REDUCTIE`, `source.decisions`, `source.facts` and `combine: ONLY` leave the law format for a profile of their own? They serve a cell, not the law. Today only `cel.yaml` says which answers a cell gives. A profile would also decide whether picking a record needs operations of its own, such as the latest one or the first that matches (`LATEST`, `FIND`), beside `combine: ONLY`. | Choice; breaking schema change | [048](/rfcs/rfc-048) |
| A cell that receives a copy of another cell's decision keeps the decision value as it came in. It cannot check that value against the other cell's chronicle, and trusts it as a court trusts the documents a party submits. Should a receiving cell be able to check a copy at its source? | Choice; investigation | [053](/rfcs/rfc-053) |
| A chronolexogram has one date, `recorded_on`. The cell takes it as the day of receipt (Awb 4:13, 6:9) and as the day a decision is made known (Awb 6:8). A record another cell makes late, or dates back, changes what a replay sees. Does a chronolexogram need a second date, for the moment that counts in law? | Choice; breaking change to the chronicle | [046](/rfcs/rfc-046), [048](/rfcs/rfc-048) |
| A replay assumes every source knows what it knew on a given day. A source that holds only its current state needs a facade that stores each value with its moment. How that works for real registers needs investigation. | Investigation | [046](/rfcs/rfc-046) |
| Who fixes the name of an answer? For synthesis several cells give one answer under one name, and a binding asks for that name. Who sets it, and how it is versioned, is open; the position paper names Consumer Driven Contracts as a research question. | Choice; investigation | [048](/rfcs/rfc-048) |
| A reduction may read no chronicle itself and only ask, inside it, for another answer of its cell. Should such a reduction be policy of its holder instead? | Choice | [048](/rfcs/rfc-048) |
| The engine asks the cells a term names wherever a binding runs, where RFC-052 asked them only for a term reached through `source.regulation`. Should competence route there too, or only where another law reads the term? | Choice | [048](/rfcs/rfc-048) |
| The engine builds a binding's parameters before it asks. A parameter the binding cannot fill fails the decision as soon as there is a cell to ask, even one that turns out not to give the answer. Should a missing parameter make the binding silent instead? | Choice | [048](/rfcs/rfc-048) |
| Which instance does the law mean? It names a category, and the engine asks every cell of that category. The cell does not pass its identity to the law, so a policy scoped by `gemeente_code` is not found and competence by place of residence cannot be checked. See also [#1029](https://github.com/NederlandseDigitaleDienst/regelrecht/issues/1029). | Choice | [048](/rfcs/rfc-048) |
| A term that reads a fact names the fact's competent authorities a second time, and the two lists can drift apart. Deriving the term's list from the fact needs the engine to know which fact a term reads. Should it? | Choice | [051](/rfcs/rfc-051) |
| The cell takes a decision's inputs only from the deciding article. A judgment that belongs in another regulation must sit on the deciding article and be passed on as a parameter, so the rulework departs from the law. | Large refactor | [046](/rfcs/rfc-046) |
| Policy wins over a finding, so the deciding authority cannot depart from a policy that fills a term in a single case, as Awb 4:84 lets it. How should such a departure be recorded? | Choice | [045](/rfcs/rfc-045) |
| The engine sees a cell as a black box, so `Cell::explain_answer` replays the decision and runs the reduction again, with tracing. If an answerer could return a trace with its answer, the engine would nest it under the answer node. The whole chain (law, policy binding, reduction, the decisions it counted) would then be one trace, and `explain_answer` could go. | Choice; small engine API change | [048](/rfcs/rfc-048) |
| The [version history](/reference/schema#version-history) says what each version adds to the schema, but not what changed with it in the engine and the cell, which a rulework author also notices. Nothing checks that the highest version has a row. Should each version get release notes split into schema and behavior, with a CI check, filled in back to v0.8.0? | Choice | none |
| A reduction filters rows with a `FOREACH` per question. A filter on a source (a `where` on an input, or a lambda passed to a reduction it calls) would let a reduction ask for the rows it needs, and the reduction of the Awb would not have to return every row that holds. For a later change. | Choice | [048](/rfcs/rfc-048) |
| A reduction, or the policy it calls, that picks one record with `combine: ONLY` fails when two records qualify. A correction recorded on the same day then blocks the decision, and the chronicle cannot undo it. Should a later record replace an earlier one? | Choice | [048](/rfcs/rfc-048) |

## Next steps

In order of priority, as of 24 October 2026:

1. A policy that fills an open term is final. `policy_first` and the fallback to a finding go, so one rule decides where a term's value comes from; a departure in a single case is the Awb 4:84 point. Working out what is asked of whom moves out of the cell. In progress; it changes [RFC-048](/rfcs/rfc-048) §2.
2. One trace for a decision and the cell's answer (`Cell::explain_answer`, in the table above).
3. Release notes for the schema, beside the [version history](/reference/schema#version-history).
4. Awb 4:1 as the start of an application, with 4:5 ([open points of the Awb procedure](/concepts/administrative-procedures#open-points)).
5. The motivation requirements of Awb 3:46 to 3:50, and 3:45 ([open points of the Awb procedure](/concepts/administrative-procedures#open-points)), together with the Awb 4:84 point.
6. Building out the NAPP case.

Later: several instances of one category, and a profile of its own for what serves a cell.
