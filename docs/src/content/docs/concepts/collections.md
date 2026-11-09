---
title: "Collections"
description: "How a law iterates over a group whose size is not known in advance, so that the legal test stays in the law instead of in the data source."
---

Dutch law reasons about groups whose size nobody knows in advance: the medebewoners in a household, the children a benefit is calculated for, the registrations in a register. A law that cannot iterate over such a group has to be handed a pre-counted number instead, and whoever produces that number has to decide who counted before the law gets a say.

`FOREACH` keeps that decision in the law.

## Iterating

```yaml
operation: FOREACH
collection: $medebewoners        # an array
as: medebewoner                  # names the element, defaults to "item"
filter:                          # optional: skip elements where this is false
  operation: AND
  conditions:
    - operation: IN
      subject: $medebewoner.relatie
      values: [EERSTEGRAADS_NEERGAANDE_LIJN, PLEEGKIND]
    - operation: LESS_THAN
      subject: $medebewoner.leeftijd_bij_aanvang_berekeningsjaar
      value: 23
body:                            # evaluated once per surviving element
  operation: MAX
  values:
    - 0
    - operation: SUBTRACT
      values: [$medebewoner.toetsingsinkomen, $vrijstelling]
combine: ADD                     # optional: reduce to a single value
```

That is AWIR article 7 lid 5: the income of a medebewoner who is a first-degree relative in the descending line or a foster child, and who had not turned 23 at the start of the berekeningsjaar, counts only for the part above €4.100. Two conditions and an exemption, in one clause, the way the article states it. Splitting selection, transformation and totalling into separate operations would need intermediate outputs the article never names.

The article is in the corpus and not yet modeled; this is what modeling it would look like. One caveat if anyone does: `relatie` is not a field the BRP hands over. AWIR article 4 lid 1 equates a pleegkind with a relative in the descending line but never defines the term, so what counts as one comes from outside this law. That makes it an open term with its own source, not an enum value to type into a filter. The Participatiewet defines it differently again, in article 3 lid 8, which is why the same word across two laws needs checking before it is treated as one concept.

`collection` is any expression that evaluates to an array. A single value iterates once; `null` is an empty collection.

## The element binding

`as` names the current element, and that name exists only inside `filter` and `body`. It shadows an outer variable of the same name. `FOREACH` is the only operation that introduces a name rather than reading one.

With `as`, a field of an object element is read through the binding, and a bare name is whatever it is outside the loop ([RFC-049](/rfcs/rfc-049)):

```yaml
as: medebewoner
body: $medebewoner.bijdrage    # the element's field
filter:
  operation: EQUALS
  subject: $medebewoner.bsn
  value: $bsn                  # the outer variable, not the field
```

An explicit `as: item` counts as `as`: its fields are read as `$item.<field>`.

Without `as`, the element is `$item`, and the fields of an object element are also bound under their own names: `$bijdrage` reads `$item.bijdrage`, and shadows an outer `$bijdrage`. Existing laws use that form. New ones should name the element.

## Nesting

Each `FOREACH` gets its own scope, and a child scope starts empty. That has one consequence worth knowing before you write a nested one: the inner `collection` is evaluated in the *outer* scope, so it can see the outer binding, and the inner `body` cannot.

```yaml
operation: FOREACH
collection: $huishoudens
as: huishouden
body:
  operation: FOREACH
  collection: $huishouden.leden   # sees $huishouden
  as: lid
  body: $lid.bijdrage             # sees $lid, not $huishouden
  combine: ADD
combine: ADD
```

Routing a value through `collection` is the only way into an inner iteration. A child that inherited its parent's locals would let one iteration leak into the next, which is why it does not.

## Combining

| `combine` | Result | Empty collection |
|---|---|---|
| `ADD` | Sum; concatenates strings and flattens arrays | `0` |
| `OR` | True if any result is truthy | `false` |
| `AND` | True if every result is truthy | `true` |
| `MIN` | Lowest value | `null` |
| `MAX` | Highest value | `null` |
| `ONLY` | The one result, of any type; an error when there are more | `null` |
| omitted | The results as an array | `[]` |

The first five are the aggregations legal text performs: *het totaal van*, *indien ten minste een van*, *indien aan alle*, *het laagste of hoogste van*. `ONLY` (schema v0.8.6) is for a reduction that picks one record, such as the place where a school is established on a day ([RFC-048](/rfcs/rfc-048)). It refuses two rather than take the first, because the order of a collection is nothing a law relies on. `SUBTRACT`, `MULTIPLY` and `DIVIDE` are not offered, because subtracting a list leaves open what it is subtracted from. A law that needs one can omit `combine` and apply the arithmetic to the resulting array.

Two of the empty cases deserve attention. `MIN` and `MAX` return `null` rather than a number, because there is no lowest value of nothing; the law has to handle that. `AND` returns `true` by vacuous truth, so a law that must not read "no items" as "all conditions met" has to check for emptiness itself.

## Counting

Article 22a Participatiewet computes the kostendelersnorm from *A*, which the text defines as "het aantal kostendelende medebewoners plus de belanghebbende en zijn echtgenoot van 21 jaar of ouder indien hij gehuwd is". Counting is `body: 1` with `combine: ADD`:

```yaml
- output: aantal_kostendelende_medebewoners
  value:
    operation: FOREACH
    collection: $medebewoners
    as: medebewoner
    body: 1
    combine: ADD
```

No filter: this article counts every kostendelende medebewoner. Both limits of 21 in that sentence govern the belanghebbende and the echtgenoot, and they appear as ordinary conditions elsewhere in the article. That reading is easy to get wrong, which is the argument for keeping it here, where a jurist can check it against the text.

Before the corpus could iterate, the same article was modeled with a boolean input named `heeft_kostendelende_medebewoners`. A number became a yes-or-no, and whoever supplied it had to decide who qualified.

Scenarios drive it the same way, with one row per element:

```gherkin
Given parameter "medebewoners" is the collection:
  | leeftijd |
  | 25       |
  | 19       |
  | 34       |
When I evaluate "aantal_kostendelende_medebewoners" of "participatiewet"
Then output "aantal_kostendelende_medebewoners" equals 3
```

The worked article is in `corpus/regulation/nl/wet/participatiewet/2022-03-15.yaml`, with its scenarios next to it.

## When the answer is not known

An error in `filter` or `body` aborts the whole operation. Partial results are never returned: a sum over some of the children is not a legal determination.

A [marked construct](./markings) taints the whole result, wherever it appears. Dropping the tainted elements and combining the rest would produce a number that looks complete and is not.

The same holds for a fact nobody has ([RFC-036](/rfcs/rfc-036)). A `filter` that evaluates to *unknown* makes the result unknown, because the engine cannot tell whether that element belongs in the collection. A `body` that evaluates to unknown does too: the element is definitely in the collection and its contribution is not known, so `ADD`, `MIN` and `MAX` report that rather than a total that is short by an unknown amount, naming the missing facts. `OR` and `AND` settle on a definitive `true` or `false` where one exists, and are unknown otherwise.

An *absent* value (`null`) is different: it is a fact, not a gap. A filter that evaluates to `null`, or a body that is `null` under `ADD`, `MIN`, `MAX`, `AND` or `OR`, is an error, because the law is treating "geen" as a verdict or an amount without saying so. The law has to state what absence means for that element (`EQUALS … null` in the filter, say).

An element whose filter is definitively `false` is simply skipped, and an empty collection returns the identities in the table above. Nothing is missing in either case.

## Bounds and units

A collection longer than 1000 elements is an error, and nesting is bounded by the operation-depth limit of 100. Those two multiply: a FOREACH inside a FOREACH, each over a thousand elements, is a million evaluations with both limits satisfied. Collections come from finite data sources; there are no generators.

The bound is what keeps termination intact. The position paper [Rules as Executed](/research/rules-as-executed#sec:opset) predates this operation: it says the format has no aggregation over collections, because the operation set grows only when encoding real law runs into a limit, and it announces that bounded aggregation, which preserves termination, will be added that way. `FOREACH` is that addition, in schema v0.5.7. The paper is left as written; the [note on its page](/research/rules-as-executed#since-publication) records the change.

Under [quantities](/rfcs/rfc-023) the result carries the unit of `body`: summing eurocents yields eurocents, and `MIN` and `MAX` preserve it the same way. `AND` and `OR` produce a boolean, and without `combine` the result is an array, so neither carries a unit.

Every iteration appears in the [execution trace](./execution-provenance) with its index and the value the body produced, or the element itself where the filter skipped it, under a node reporting how many elements were evaluated and how many were skipped. A total that shows only itself is not an explanation of how it was reached.

## Further reading

- [Schema Reference](/reference/schema#operations) lists the full operation set.
- [RFC-016](/rfcs/rfc-016) records why collection operations look like this, and which alternatives were rejected.
- [Markings](./markings) covers what happens when a construct cannot be modeled at all.
