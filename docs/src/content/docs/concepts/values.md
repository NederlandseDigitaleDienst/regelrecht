---
title: "Values"
description: "What a rulework computes with: types and units, explicit rounding, the difference between an absent and an unknown value, and which outputs a portal offers."
---

Every parameter, input, output and definition in a rulework has a type. A law also says what an amount is counted in and when to round it. The engine rounds only where the rulework says so, and checks units where an article declares them. This page covers the values themselves. [Law Format](/concepts/law-format) shows where they sit in an article, and [Temporal Validity and Dates](/concepts/temporal-and-dates) covers dates.

## Types

A field's `type` is one of `string`, `number`, `boolean`, `amount`, `date`, `array`, `object` and `decision`. An `amount` is a quantity whose unit is declared next to it (below). A `decision` is the value of a decision another article took, with its fields, findings and outputs ([Cross-Law References](/concepts/cross-law-references#calling-the-decision-of-an-article)). An open term takes the narrower set `string`, `number`, `boolean`, `amount` and `date`.

Inside the engine a value is a boolean, an integer, an exact decimal, a string, a list or an object (`Value` in `packages/law-model/src/value.rs`). It can also be absent or unknown (below), or mark a construct that could not be translated ([Markings](/concepts/markings)). A date travels as a string and an amount as a number. Arithmetic is decimal, never floating point, so 0.1 plus 0.2 is 0.3. A decimal holds 28 significant digits. A quotient that does not fit, such as 1 divided by 3, is rounded at that precision.

A type is checked before the law runs. `just validate` and every load of a rulework run the type checker of [RFC-037](/rfcs/rfc-037): arithmetic and ordering on numbers, logic on booleans, no equality test between two different known types, and a literal output that matches its declared type. A field whose type the checker cannot know is left alone.

## Amounts and units

`type_spec.unit` says what an amount is counted in: `euro`, `eurocent`, `ratio`, `percentage`, `years`, `months`, `weeks` or `days`. A law mixes euros, eurocent, fractions, percentages and durations, and adding a number of days to an amount of money is a mistake a type system can catch ([RFC-023](/rfcs/rfc-023)).

```yaml
definitions:
  vermogensgrens_verzekerde:
    value: 14189600
    type: amount
    type_spec:
      unit: eurocent
```

A unit is a label and never changes a value. A `percentage` is not divided by 100 behind the scenes; a `… / 100` is an explicit operation where the value is applied. `ratio` (0 to 1) and `percentage` (0 to 100) are separate units. A percentage in the law can be kept as written or stored as a fraction, and the unit says which. The zorgtoeslag rulework stores "1,896%" as the ratio `0.01896`. Adding a ratio to a percentage is refused.

The engine uses the labels to refuse a combination that makes no sense. Adding or comparing needs the same unit, so `eurocent` plus `days` fails and so does `euro` plus `eurocent`. A `ratio` or `percentage` times an amount keeps the amount's unit, and dividing two amounts of one unit gives a `ratio`. `just validate` checks this statically, and the engine checks it again at run time (`packages/engine/src/units.rs`). The static pass reads only `value:` expressions, so an action written as a bare `operation` is caught at run time only.

Units are opt-in per article. A value without a unit is never checked, so a rulework is unaffected until someone annotates it. Once an article declares a unit anywhere, `just validate` also warns about an `amount` output of that article without one.

`type_spec` also takes `precision`, `min` and `max`. The engine reads them and does not enforce them: `precision` rounds nothing, and a value outside `min` or `max` passes. Bounds are tracked in [issue #444](https://github.com/NederlandseDigitaleDienst/regelrecht/issues/444).

## Rounding

The engine never rounds on its own, not even money. Intermediate values keep full precision, and a value is rounded only where the law says so, with an operation ([RFC-024](/rfcs/rfc-024)):

| Operation | Direction | Dutch |
|-----------|-----------|-------|
| `ROUND` | nearest, ties away from zero | *rekenkundig afronden* |
| `CEIL` | up, toward +∞ | *naar boven afronden* |
| `FLOOR` | down, toward −∞ | *naar beneden afronden* |

Ties go away from zero because the Hoge Raad applies that where a law says "afronden" without a direction. Each operation takes one `value` and a `precision` in the value's own unit: `0` rounds to whole units, and `-2` rounds an amount in eurocent to whole euros.

```yaml
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

There is no banker's rounding, and no truncation toward zero for negative values; the RFC defers both until a law needs them. Cutting a date back to the first of its month is a date operation, not rounding ([Temporal Validity and Dates](/concepts/temporal-and-dates)). The operand rules are under [Rounding](/reference/schema#rounding) in the schema reference.

One place still loses digits. The plain JSON form of a result writes a decimal as a floating-point number; the stored form keeps every digit ([Law Model](/components/law-model)).

## Absent and unknown values

A register does not hold a value for everyone, and the engine keeps two cases apart ([RFC-036](/rfcs/rfc-036)).

An **absent** value, `null`, is a fact: the register says there is none, such as no partner or no rent. A field may hold it only when its declaration says `nullable: true`. A law tests it with `EQUALS … null` and may branch on it. Calculating, ordering or deciding on it is an error, because a legal text never treats "geen" as an amount or a verdict without saying so. An `IF` without a matching case or `default` also yields `null`, and so does `MIN` or `MAX` over an empty list.

An **unknown** value is a fact nobody has yet: a `source: {}` input no data source could supply, or an optional parameter the caller did not pass. A law cannot write one. It travels through every operation with the names of the missing facts, so a portal can ask for them. `AND` is false as soon as one operand is false, and `OR` true as soon as one is true, whatever the others are; otherwise the result is unknown. An `IF` takes the first case whose condition holds. An unknown condition before that case makes the whole `IF` unknown.

The type checker enforces the contract before the law runs. A `null` test on a field that is not nullable, a nullable value reaching a sum without a test for absence, or an `IF` without `default` on an output that is not nullable is refused. So is a nullable output from another law feeding an input that is not. The engine evaluates operands left to right, so a guard has to come first: `AND` of a false test and an absent value is false, while the same two in the other order is an error.

In a scenario's data table an empty cell is unknown and the word `null` is absence. The assertions read `is absent`, `is unknown`, and `is unknown for lack of "huur"`. This is the reverse of SQL, where `null` stands for the unknown; RFC-036 explains why the literal was kept for absence. The editor shows an unknown output as *onbekend* with the facts it lacks, an absent one as *geen*, and an output that was never computed as *niet berekend*.

[Collections](/concepts/collections#when-the-answer-is-not-known) covers an unknown inside a `FOREACH`.

## Callable and presentable

Every output is callable: another law can ask for it by name, which is what [cross-law references](/concepts/cross-law-references) rest on. Not every output is something to show a citizen. In the demo corpus, the Penitentiaire beginselenwet runs for nearly everyone the zorgtoeslag is computed for, and still has no place on a portal.

What a portal offers follows from the law ([RFC-038](/rfcs/rfc-038)). An article is an entry point when it produces a `BESCHIKKING`, a decision someone can contest, or a `RECHTSPOSITIE`, a position that arises by operation of law, such as the right to vote. Any other legal character, such as a `TOETS` or `INFORMATIEF`, or no `produces` at all, makes an article callable but not an entry point. The [glossary](/reference/glossary#legal-character) defines each legal character. The demo derives its portals from this (`frontend-demo/src/data/entrypoints.ts`). Whether a portal is for a citizen or a business follows from the key the law computes on: `bsn` for a citizen, `kvk_nummer` or `loonheffingennummer` for a business. The engine is not involved.
