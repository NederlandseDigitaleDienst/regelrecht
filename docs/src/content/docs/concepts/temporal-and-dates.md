---
title: "Temporal Validity and Dates"
description: "How the engine selects the law version in force on a date, how it reports references to expired laws, and how dates are compared and subtracted."
---

A calculation always happens *as of* a date. The rules and amounts in force on 15 June 2024 are not the ones in force on 15 June 2025, and a faithful answer uses the version that applied on the day in question. Two mechanisms make this work: version selection by date, and date operations inside the rules.

## Which version is in force

A law version declares `valid_from`, and optionally `valid_to`: the first and last calendar days on which it is in force, both inclusive. The engine selects the version where `valid_from <= reference_date <= valid_to`, where the reference date is the calculation date supplied by the caller.

`valid_to` lets a law expire without a successor. A version with `valid_to: 2024-12-31` resolves on its last day:

```gherkin
Scenario: A law still resolves on its last day in force (inclusive bound)
  Given the calculation date is "2024-12-31"
  When I evaluate "normbedrag" of "test_einddatum"
  Then the execution succeeds
  Then output "normbedrag" equals 500
```

and the day after, it is gone. Selection does **not** fall through to an older version once the in-force one has ended: an expired law is expired, and its predecessor does not take over. A reference to a law that has ended fails with the concrete dates:

```gherkin
Scenario: A law no longer resolves after its end date
  Given the calculation date is "2025-06-01"
  When I evaluate "normbedrag" of "test_einddatum"
  Then the execution fails with "No version of law 'test_einddatum' in force on 2025-06-01; last in force until 2024-12-31"
```

The same applies across a cross-law reference: a law that reads an ended law reports which law ended and when, and does not compute on rules that are no longer valid. The selection outcome is one of in force, not yet in force, or ended on a date (`SelectionReason` in `packages/engine/src/resolver.rs`), and both `valid_from` and `valid_to` are recorded in the [Execution Receipt](./execution-provenance) so the choice is reproducible. The scenarios above come from `bdd/conformance/einddatum.feature`.

## Comparing and subtracting dates

Deadlines and durations need arithmetic on dates. Two routes cover it.

**Comparison operators dispatch on operand type.** `GREATER_THAN`, `LESS_THAN`, `LESS_THAN_OR_EQUAL` and the rest compare numbers when both operands are numeric, and compare chronologically when both are ISO 8601 dates. So `$indieningsdatum <= $peildatum` works directly, with no detour through `AGE`. `EQUALS` gains a date fallback for the mixed case (a date string against the `{iso, year, month, day}` object form of `referencedate`).

**A date an operation computes is a date value.** `DATE`, `DATE_ADD` and `START_OF` return a calendar day in the years 0001 to 9999, and the `iso` field of `$referencedate` is one too ([RFC-055](/rfcs/rfc-055)). It is written as its text, `2025-03-15`, wherever a value leaves the engine, so the API, traces and stored values read as before. A date that arrives through a parameter or a data source is still a string. Beside a date, such a string is read as a day: `EQUALS` and the orderings compare it as a date, and a string that is no date there is an error. A date and a string do not `ADD`, and a date is not a condition. The date operations take a date in any of those forms and refuse anything else: a number is no date, and a string that is no day in the form `YYYY-MM-DD` is an `InvalidDate`. A count they take (a part of `DATE`, the `years`, `months`, `weeks` and `days` of `DATE_ADD`) is a whole number; `months: 1.5` is an error, where it used to add one month. `DATE` of a day that does not exist, such as 30 February, is an error too, and so is a result or a date written as text outside the years 0001 to 9999. Turning a string into a date when it enters a field declared `date` is still being built.

**`DATE_DIFF` measures the span between two dates** with an explicit unit:

```yaml
operation: DATE_DIFF
from: $indieningsdatum
to: $referencedate
in: days        # or: months, years
```

It is signed: positive when `to` is on or after `from`, negative otherwise. For a request filed on 2025-01-01 against a peildatum of 2025-07-01, the span is `181` days; flip the two dates and it is `-181`. Months and years count whole calendar units, reusing the same arithmetic as `AGE` (BW art. 1:2), so end-of-month and leap-year cases stay consistent: 31 January to 28 February is one whole month, because January has no 31st counterpart in February.

```gherkin
Scenario: An end-of-month span counts as a whole month
  Given the calculation date is "2025-02-28"
  Given the following parameters:
    | indieningsdatum | 2025-01-31 |
  When I evaluate "doorlooptijd_maanden" of "test_date_operations"
  Then output "doorlooptijd_maanden" equals 1
```

Dates must be in canonical `YYYY-MM-DD` form, zero-padded. `2025-1-1` is rejected; the engine does not guess. These scenarios come from `bdd/conformance/date_operations.feature`, and the related operations `AGE`, `DATE_ADD` and `DATE` are specified in the [Schema Reference](/reference/schema#operations).

## Reading a component, and cutting back to a month

Two more operations, added by [RFC-032](/rfcs/rfc-032), work on a single date.

**`DATE_PART` reads one calendar component out of a date** and returns it as a number:

```yaml
operation: DATE_PART
date: $peildatum
in: year        # or: month, day
```

It is the inverse of `DATE`, so `in` ranges over exactly the components `DATE` takes. The weekday is not one of them and stays `DAY_OF_WEEK`. The unit is singular here where `DATE_DIFF`'s is plural, because this reads a component instead of counting units; `in: months` is an error that names the words it accepts. A month number is only meaningful within one year, so comparing two of them across a year boundary is almost always wrong.

**`START_OF` cuts a date back** to 1 January of its year, or to the first day of its month:

```yaml
operation: START_OF
date: $peildatum
in: month       # or: year
```

Truncation is a named operation and never an implicit property of a date, for the same reason [RFC-024](/rfcs/rfc-024) made rounding explicit: the cut belongs in the law where the law prescribes it.

Together they write "de eerste dag van de kalendermaand volgende op", the figure that runs through the whole of social security law:

```yaml
operation: DATE_ADD
date:
  operation: START_OF
  date: $peildatum
  in: month
months: 1
```

Truncate first, add afterwards. In that order the result is exact for every input date, including the first of the month and including 31 January, because the truncated date is day 1 and the end-of-month clamping of `DATE_ADD` never comes into play. Where the law asks whether something happened *after* the first of the month, test the day number: a change on the first counts in that month itself.

## Further reading

- [Cross-Law References](./cross-law-references) - how an expired reference surfaces in a chain
- [Execution Provenance](./execution-provenance) - validity windows in the receipt
- [RFC-019: Law End Dates](/rfcs/rfc-019) and [RFC-021: Date Comparison](/rfcs/rfc-021) - full specifications
- [Rules as Executed, section 9.4](/research/rules-as-executed#sec:versioning) - the position paper on version management: Staatsblad dates as release tags, and every dependency resolved to the version in force on the relevant date
