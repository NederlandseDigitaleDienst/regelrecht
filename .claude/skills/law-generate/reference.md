# Law Generate - Technical Reference

Based on the latest schema (`schema/latest/schema.json`). Validate with `just validate`.

## Complete Machine-Readable Section Structure

```yaml
machine_readable:
  endpoint: string              # Named endpoint, callable from other regulations
  competent_authority:          # Who has binding authority
    name: "Belastingdienst"
    type: "INSTANCE"            # INSTANCE (default) or CATEGORY
  # OR as internal reference:
  # competent_authority: "#bevoegd_gezag"

  requires:                     # Dependencies (optional)
    - law: "zorgverzekeringswet"
      values: ["is_verzekerd"]
    - article: "11"             # Same-law article reference

  definitions:                  # Constants (optional, arbitrary keys)
    standaardpremie_vast:       # Structured: value, type, and a unit for a quantity (RFC-023)
      value: 211200             # € 2.112 in eurocent
      type: amount              # string | number | boolean | amount | date | array
      type_spec:
        unit: eurocent          # amount: always eurocent
      description: "Description"
    termijn_rapportage:
      value: 4                  # "vier jaar"
      type: number
      type_spec:
        unit: years             # number: ratio | percentage | years | months | weeks | days
    # A bare `simple_key: "simple value"` is valid but carries no type; write
    # the structured form.

  open_terms:                   # The law leaves the content open (optional)
    - id: standaardpremie
      type: amount              # string | number | boolean | amount | date
      required: true
      delegated_to: Onze Minister              # omit when the article names nobody
      delegation_type: MINISTERIELE_REGELING   # omit likewise
      expected_source: Regeling zorgverzekering  # when the text names the regulation
      decided_per_case_by: het college          # when the article appoints nobody
      legal_basis: artikel 4 Wet op de zorgtoeslag

  markings:                     # The format cannot express a construct (optional)
    - about: de week waarin de peildatum valt
      reason: >-                # why it does not fit, in terms of what the format has
        De motor leest jaar, maand en dag uit een datum, en kent geen weeknummer.
      resolution: operation     # operation | model
      resolved_by: Een weeknummer als waarde van `in` bij DATE_PART
      target: []                # values this article therefore does not produce
      legal_text_excerpt: de week waarin de aanvraag is ingediend
      accepted: false

  implements:                   # IoC fulfillment (optional)
    - law: wet_op_de_zorgtoeslag
      article: '4'
      open_term: standaardpremie
      gelet_op: Gelet op artikel 4 van de Wet op de zorgtoeslag

  hooks:                        # Reactive execution (optional, RFC-007)
    - hook_point: pre_actions   # pre_actions | post_actions
      applies_to:
        legal_character: BESCHIKKING  # required
        decision_type: TOEKENNING     # optional
        stage: BESLUIT                # optional (default: BESLUIT)

  overrides:                    # Lex specialis declarations (optional, RFC-007)
    - law: algemene_wet_bestuursrecht   # omit for an article of this same law
      article: '6:7'            # the producing entry's number as this file writes it
      output: termijn_weken
      voids: false              # true: the output does not arise at all
      legal_text_excerpt: In afwijking van artikel 6:7 ...

  execution:
    produces:                   # Legal character (optional)
      legal_character: BESCHIKKING  # BESCHIKKING | TOETS | WAARDEBEPALING |
                                    # BESLUIT_VAN_ALGEMENE_STREKKING | INFORMATIEF
      decision_type: TOEKENNING     # TOEKENNING | AFWIJZING | GOEDKEURING |
                                    # GEEN_BESLUIT | ALGEMEEN_VERBINDEND_VOORSCHRIFT |
                                    # BELEIDSREGEL | VOORBEREIDINGSBESLUIT |
                                    # ANDERE_HANDELING | AANSLAG
      procedure_id: beschikking     # optional: selects specific AWB procedure variant

    parameters:                 # Caller-provided inputs
      - name: "bsn"
        type: "string"          # string | number | boolean | date
        required: true
        description: "Burgerservicenummer"

    input:                      # Data from external sources
      - name: "toetsingsinkomen"
        type: "amount"          # string | number | boolean | amount | object | array | date
        source:
          regulation: "algemene_wet_inkomensafhankelijke_regelingen"
          output: "toetsingsinkomen"
          parameters:
            bsn: "$bsn"
        type_spec:
          unit: "eurocent"      # eurocent | years | months | weeks | days

    output:                     # What this article produces
      - name: "hoogte_zorgtoeslag"
        type: "amount"
        type_spec:
          unit: "eurocent"
        description: "Hoogte van de zorgtoeslag"

    actions:                    # Computation logic
      - output: "result_name"   # Required: which output to set
        value: <operationValue> # Value assignment (literal, $variable, or operation)
        legal_basis:            # Optional: traceability
          law: "Wet op de zorgtoeslag"
          article: "2"
```

## Procedures (top-level)

Procedures define AWB lifecycle stages for administrative decisions. Declared at the
**top level** of the YAML file (same level as `articles`), typically in the AWB itself.

```yaml
procedure:
  - id: beschikking
    default: true               # Default procedure for this legal_character
    applies_to:
      legal_character: BESCHIKKING
    stages:
      - name: AANVRAAG
        description: Belanghebbende dient aanvraag in (AWB 4:1)
        requires:
          - name: aanvraag_datum
            type: date
      - name: BEHANDELING
        description: Bestuursorgaan onderzoekt de aanvraag (AWB 3:2)
        requires:
          - name: beslistermijn_start
            type: date
      - name: BESLUIT
        description: Bestuursorgaan neemt besluit (AWB 1:3)
        requires:
          - name: besluit_datum
            type: date
      - name: BEKENDMAKING
        description: Besluit wordt bekendgemaakt (AWB 3:41)
        requires:
          - name: bekendmaking_datum
            type: date
      - name: BEZWAAR
        description: Bezwaarperiode (AWB 6:4 e.v.)
```

## Markings and Open Terms

A **marking** says the format cannot express a construct. That is a language gap,
resolved by building an operation (`resolution: operation`, e.g. a weeknummer as a
value of `in` on `DATE_PART`) or by changing the format (`resolution: model`, e.g.
quantification over persons, a rule about a set rather than a value, a legal
fiction).

An **open term** says the law leaves the content open and another regulation or
implementing policy fills it. The language expresses it fine; the content sits
elsewhere. `delegated_to` and `delegation_type` say who may fill it and with what
kind of regulation, and `expected_source` names the filling regulation where the
text itself names it. `delegation_type` takes a value from the same closed list
as the document's own `regulatory_layer` (see Regulatory Layers below): the layer
the article requires of the implementation, and an implementation at another
layer does not fill the term. Where the article names nobody ("redelijkerwijs", "in
bijzondere gevallen"), those fields stay absent and `decided_per_case_by` names
the authority that fills the term in the individual case. One of the three is
always present; a check reports a term that names nobody.

A value another law produces is neither. It is an `input` with a `source`.

None of the three excludes the others inside one article. A marking answers one
construct and says nothing about the rest, so an article whose remaining words
are fully modelled still carries the marking those words do not cover.

Required on a marking: `about`, `reason`, `resolution`, `resolved_by`, `target`,
`legal_text_excerpt`. Required on an open term: `id`, `type`.

The three prose fields say different things and none of them substitutes for
another. `about` is the construct in the words of the article. `reason` is why
it does not fit, stated in terms of what the format does have: name the shape or
the operation that comes closest and say where it falls short. `resolved_by` is
the change that would close the gap, concrete enough to become work. The change
follows from the reading; the reading cannot be recovered from the change, which
is why the diagnosis has a field of its own. A reason that restates `about`, or
that is `resolved_by` said twice, or that says no more than "this does not fit",
is reported by a check.

`target` names the values in this article that cannot be produced because of the
marking. An empty list asserts the article stays executable, which is the normal
case. A name in the list is a value this article's model declares (input,
parameter, output, definition, open term) and is absent from its actions:
computing or calculating with a value you declared blocked is a contradiction,
and a check reports both, as it reports a name the model declares nowhere.

Do not record whether the filling regulation is currently in the corpus. That is
a state of the corpus, not a property of the law (RFC-031); the resolve step and
the work queue track it (RFC-026).

## Operation Types

### Arithmetic Operations, with a `values` array
```yaml
operation: ADD              # ADD | SUBTRACT | MULTIPLY | DIVIDE | MIN | MAX
values:
  - $operand_1              # Each item is an operationValue
  - $operand_2              # (literal, $variable, or nested operation)
```

### Logical Operations, with a `conditions` array
```yaml
operation: AND              # AND | OR
conditions:
  - operation: EQUALS
    subject: $a
    value: true
  - operation: GREATER_THAN
    subject: $b
    value: 0
```

### NOT, negation, with `value`
```yaml
operation: NOT
value:                      # operationValue (literal, $var, or operation)
  operation: EQUALS
  subject: $is_verzekerd
  value: true
```

Can also negate compound conditions or simple variables:
```yaml
# Negate a compound ("tenzij zowel A als B") to NOT(A AND B)
operation: NOT
value:
  operation: AND
  conditions:
    - operation: EQUALS
      subject: $a
      value: true
    - operation: EQUALS
      subject: $b
      value: true

# Negate a variable directly, NOT($flag)
operation: NOT
value: $heeft_relatieve_weigeringsgrond
```

### Comparison Operations, with `subject` + `value`
```yaml
operation: EQUALS           # EQUALS | GREATER_THAN | LESS_THAN
                            # GREATER_THAN_OR_EQUAL | LESS_THAN_OR_EQUAL
subject: $variable          # MUST be a $variable reference
value: 18                   # operationValue (literal, $var, or operation)
```

### Conditional IF, with a `cases` array + `default`
```yaml
operation: IF
cases:
  - when:                   # Condition (operationValue evaluating to boolean)
      operation: EQUALS
      subject: $has_partner
      value: true
    then: $partner_amount   # Value when condition is true (operationValue)
  - when:
      operation: EQUALS
      subject: $categorie
      value: "B"
    then: 75000
default: $single_amount     # Value if no case matches (operationValue, optional)
```

Cases are evaluated in order; the first matching case wins.

### IN, membership test, with `subject` + `value` or `values`
```yaml
# With inline list:
operation: IN
subject: $status
values: ["ACTIEF", "GEPAUZEERD"]

# With single reference (e.g., a LIST output):
operation: IN
subject: $status
value: $allowed_statuses
```

### LIST, construct an array
```yaml
operation: LIST
items:
  - $item_1
  - $item_2
  - "literal_value"
```

### FOREACH — iterate over a collection (RFC-016, schema v0.5.7+)
Counting, which is what article 22a Participatiewet does:

```yaml
operation: FOREACH
collection: $medebewoners          # expression evaluating to an array
as: medebewoner                    # names the element; without `as` the element is `item` and its fields are also bound under their own names
body: 1                            # evaluated once per element
combine: ADD                       # optional: ADD | OR | AND | MIN | MAX
```

Use it when the law counts or totals over a group whose size is not fixed, so
the legal test stays in the law instead of in the data source. Without `combine`
the result is an array.

`filter` skips elements, and it belongs there only when the law restricts which
elements take part. Check which noun the restriction attaches to before writing
one: article 22a counts kostendelende medebewoners without a filter, because
both age limits of 21 in that sentence govern the belanghebbende and the
echtgenoot. A filter must also carry every condition the law states, not the one
that is easiest to express, and any per-element arithmetic the law applies
belongs in `body`.

`as` is bound only inside `filter` and `body`. With `as`, read a field as
`$medebewoner.leeftijd`: a bare `$leeftijd` is the variable outside the loop,
not the field (RFC-049). An explicit `as: item` counts as `as`: read its fields
as `$item.<field>`. In a nested FOREACH the inner
`collection` can read the outer binding; the inner `body` cannot. Empty
collections give `0` for ADD, `false` for OR, `true` for AND, and `null` for
MIN/MAX. See [Collections](../../../docs/src/content/docs/concepts/collections.md).

### AGE — calculate age in complete years
### AGE, age in complete years
```yaml
operation: AGE
date_of_birth: $geboortedatum     # Date (operationValue)
reference_date: $peildatum         # Date (operationValue)
```

### DATE_ADD, add a duration to a date
```yaml
operation: DATE_ADD
date: $bekendmaking_datum          # Base date (operationValue)
years: 1                           # optional (operationValue)
months: 3                          # optional (operationValue)
weeks: $termijn_weken              # optional (operationValue)
days: 1                            # optional (operationValue)
```

Applied coarsest-to-finest: years → months → weeks → days.
Month/year additions use the Dutch legal "corresponding numbered day" rule:
the day is clamped to the last day of the target month (e.g., Jan 31 + 1 month = Feb 28).

### DATE, construct a date from components
```yaml
operation: DATE
year: $jaar                        # Year (operationValue)
month: 1                           # Month 1-12 (operationValue)
day: 1                             # Day 1-31 (operationValue)
```

### DAY_OF_WEEK, weekday number
```yaml
operation: DAY_OF_WEEK
date: $datum                       # Date (operationValue)
# Returns 0=Monday, 1=Tuesday, ..., 6=Sunday
```

### DATE_DIFF, signed difference between two dates (RFC-021)
```yaml
operation: DATE_DIFF
from: $aanvraagdatum               # Start date (operationValue)
to: $besluitdatum                  # End date (operationValue)
in: months                         # days | months | years (or a $variable)
```

Positive when `to` is on or after `from`. Months and years count complete
calendar units.

### DATE_PART, read one component out of a date (RFC-032)
```yaml
operation: DATE_PART
date: $peildatum                   # Date (operationValue)
in: year                           # year | month | day, singular, no $variable
```

Returns an integer: the year number, the month number (1-12) or the day of the
month. It is the inverse of `DATE`, so `in` ranges over exactly the components
`DATE` takes. The weekday is not one of them and stays `DAY_OF_WEEK`.

The unit is singular here and plural in `DATE_DIFF`, because this reads a
component where `DATE_DIFF` counts units. `in: months` is an error.

A month number is only meaningful within one year. Comparing two month numbers
across a year boundary is almost always wrong; compare the dates themselves.

### START_OF, truncate a date to the start of its year or month (RFC-032)
```yaml
operation: START_OF
date: $peildatum                   # Date (operationValue)
in: month                          # year | month, singular, no $variable
```

Returns a date: 1 January of the year, or the first day of the month.

"De eerste dag van de kalendermaand volgende op" is one legal figure that runs
through the whole of social security law, and this is how it is written:

```yaml
operation: DATE_ADD
date:
  operation: START_OF
  date: $peildatum
  in: month
months: 1
```

Truncate first, add afterwards. In that order the result is exact for every
input date, including the first of the month and including 31 January, because
the truncated date is day 1 and the day-clamping of `DATE_ADD` never comes into
play. The reverse order lands on the same answer through the clamp, which is a
step a reviewer has to re-check each time. Do not write it that way.

When the law asks whether something happened *after* the first of the month
(Awir art. 49), test the day number rather than the date:
`GREATER_THAN(DATE_PART($d, in: day), 1)`. A change on the first counts in that
month itself.

### ROUND, CEIL and FLOOR, rounding (RFC-024)
```yaml
operation: ROUND                   # ROUND | CEIL | FLOOR
value: $bedrag                     # Operand (operationValue)
precision: -2                      # Required: decimals in the value's own unit
```

`ROUND` is half-up (rekenkundig, the Hoge Raad default), `CEIL` rounds up
("naar boven"), `FLOOR` rounds down ("naar beneden", afkapping). `precision`
counts decimals in the value's **own** unit (RFC-023), so rounding a eurocent
amount to whole euros is `precision: -2` and to whole tens of euros `-3`.

Rounding is never implicit. Model the rounding a law states and round nothing
where it states none.

## Variable References

Pattern: `$name` or `$name.property` (dot notation for nested access)

```yaml
# Parameter reference
subject: $bsn

# Input reference
subject: $toetsingsinkomen

# Definition/constant reference
value: $STANDAARDPREMIE

# Previous action output reference
subject: $intermediate_result
```

## Source Formats (for input fields)

### External Law Reference
```yaml
source:
  regulation: "regeling_standaardpremie"   # Law/regulation $id
  output: "standaardpremie"                # Output field to retrieve
  parameters:                              # Parameters to pass (optional)
    bsn: $bsn
```

### Internal Reference (same law)
```yaml
source:
  output: "vermogen_onder_grens"           # Output from another article
  # No regulation field = same law
```

### Open Terms (IoC, Inversion of Control)

When a law leaves a value to another regulation ("bij ministeriële regeling",
delegation; "bij gemeentelijke verordening", often co-government, where the
municipality executes the act through its own ordinance), use the `open_terms` +
`implements` pattern.
An open term whose article names no filler works the same way, with
`delegated_to` and `delegation_type` left out.

**The law** declares an open term:
```yaml
machine_readable:
  open_terms:
    - id: standaardpremie
      type: amount
      required: true
      delegated_to: Onze Minister
      delegation_type: MINISTERIELE_REGELING
      legal_basis: artikel 4 Wet op de zorgtoeslag
  execution:
    output:
      - name: standaardpremie
        type: amount
        type_spec:
          unit: eurocent
    actions:
      - output: standaardpremie
        value: $standaardpremie   # Engine resolves via implements_index
```

**The implementing regulation** registers as filling it:
```yaml
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

The engine automatically resolves `$standaardpremie` by finding the regulation
that `implements` the open term, using lex superior / lex posterior priority rules.

## Hooks, Reactive Execution

Hooks allow articles to fire automatically when matching lifecycle events occur.
Used by the AWB for cross-cutting requirements (motivation, appeal deadlines).

```yaml
machine_readable:
  hooks:
    - hook_point: pre_actions      # Fires BEFORE the target article's actions
      applies_to:
        legal_character: BESCHIKKING   # Required: match articles producing this
        decision_type: TOEKENNING      # Optional: narrow to decision type
        stage: BESLUIT                 # Optional: lifecycle stage (default: BESLUIT)
    - hook_point: post_actions     # Fires AFTER the target article's actions
      applies_to:
        legal_character: BESCHIKKING
        stage: BEKENDMAKING
  execution:
    # Normal execution section, output, actions, etc.
```

Valid `hook_point` values: `pre_actions`, `post_actions`
Valid `legal_character` values: `BESCHIKKING`, `TOETS`, `WAARDEBEPALING`,
`BESLUIT_VAN_ALGEMENE_STREKKING`, `INFORMATIEF`

## Overrides, Lex Specialis

When a specific law needs to replace an output from a more general law:

```yaml
machine_readable:
  overrides:
    - law: algemene_wet_bestuursrecht   # $id of the law being overridden
      article: '6:7'                     # Article number being overridden
      output: termijn_weken              # Specific output being replaced
  execution:
    output:
      - name: termijn_weken
        type: number
    actions:
      - output: termijn_weken
        value: 4
```

The engine uses the override output instead of the original when the overriding
law is in scope (lex specialis principle).

## Regulatory Layers

```yaml
regulatory_layer: WET  # One of:
# GRONDWET | WET | AMVB | KONINKLIJK_BESLUIT | MINISTERIELE_REGELING |
# BELEIDSREGEL | EU_VERORDENING | EU_RICHTLIJN | VERDRAG |
# UITVOERINGSBELEID | GEMEENTELIJKE_VERORDENING | PROVINCIALE_VERORDENING
```

## Eurocent Conversion

In Dutch notation `.` is the thousands separator and `,` is the decimal
separator, the opposite of English. `€1.234,56` is one thousand two hundred
thirty-four euro and fifty-six cents, so `123456` eurocent. `€2.112` is two
thousand one hundred twelve euro, so `211200` eurocent, not `2112`.

Every monetary value in every file is `type: amount` with
`type_spec: { unit: eurocent }`. A file that uses `unit: euro` is broken even
when it is consistent with itself, because `unit` is a label and never a
conversion (RFC-023) and the engine will not notice the factor of a hundred at a
law boundary.

## Common Legal Phrases → Operations

| Dutch Legal Phrase | Operation Pattern |
|-------------------|------------------|
| "heeft bereikt de leeftijd van X jaar" | `AGE` + `GREATER_THAN_OR_EQUAL`, value: X |
| "ten minste X" | `GREATER_THAN_OR_EQUAL`, value: X |
| "niet meer dan X" | `LESS_THAN_OR_EQUAL`, value: X |
| "minder dan X" | `LESS_THAN`, value: X |
| "meer dan X" | `GREATER_THAN`, value: X |
| "gelijk aan X" | `EQUALS`, value: X |
| "vermenigvuldigd met" | `MULTIPLY`, values: [...] |
| "gedeeld door" | `DIVIDE`, values: [...] |
| "vermeerderd met" | `ADD`, values: [...] |
| "verminderd met" | `SUBTRACT`, values: [...] |
| "indien ... en ..." | `AND`, conditions: [...] |
| "indien ... of ..." | `OR`, conditions: [...] |
| "tenzij" / "niet" | `NOT`, value: ... |
| "ingevolge" | Cross-law reference via source.regulation |
| "bedoeld in artikel X" | Internal reference via source.output |
| "binnen X weken na" | `DATE_ADD`, date: ..., weeks: X |
| "het aantal maanden tussen X en Y" | `DATE_DIFF`, from: X, to: Y, in: months |
| "het kalenderjaar waarin" / "het berekeningsjaar" | `DATE_PART`, date: ..., in: year |
| "de eerste dag van de kalendermaand volgende op" | `DATE_ADD` over `START_OF`, in: month, months: 1 |
| "na de eerste dag van de maand" | `GREATER_THAN` on `DATE_PART`, in: day, value: 1 |
| "afgerond op hele euro's" | `ROUND` with `precision: -2` on a eurocent value |
| "voor zover" | A bound on an amount: `MAX`/`MIN` around the qualifying part. A boolean here loses the partial case, and always in the citizen's disfavour |
| "dan wel" (two measures side by side) | `OR` over two comparisons, each with its own parameter. One parameter whose description covers both is not a model of the disjunction |
| "in afwijking van artikel X" | `overrides` declaration |
| "bij ministeriële regeling" | `open_terms` + `implements` IoC pattern |

Do not work from a stock list of input names and sources. Every binding goes to
an output that the target law in this corpus really produces, which you find by
reading that law. Writing down a plausible name instead is how hallucinated
dependencies get in.

## Debugging Tips

1. `just validate <file>` catches schema violations with exact paths
2. Action patterns: `value:` for assignments and operations; `operation:` with
   `values:` only for arithmetic
3. `IF` takes `cases`/`default`, never `when`/`then`/`else`
4. Arithmetic takes a `values` array, never `subject`/`value`
5. Logical operations take a `conditions` array, never `values`
6. Comparison takes `subject` (a `$variable`) plus `value`
7. `NOT` takes `value`, never `conditions` or `subject`
8. `source` takes `regulation` and `output`, never `url`
9. Monetary fields are `type: amount` with `type_spec: { unit: eurocent }`
10. `AGE` takes `date_of_birth` and `reference_date`
11. `DATE_ADD` takes `date` plus optional `years`/`months`/`weeks`/`days`
12. `ROUND`, `CEIL` and `FLOOR` require `precision`
13. `DATE_PART` and `START_OF` take `date` plus a singular `in`; `DATE_DIFF`
    takes a plural one
14. `$referencedate` is not a built-in and must be declared as a parameter

## External Resources

- **Schema**: `schema/latest/schema.json`, the arbiter
- **Worked patterns**: `examples.md` in this directory
- **Engine source**: `packages/engine/src/`
- **Validation binary**: `packages/engine/src/bin/validate.rs`

The corpus files under `corpus/regulation/nl/` are on an older schema version and
predate several of the rules in `SKILL.md`. Read one to see how an operation
behaves in practice; do not read one as a model of how complete a law file should
be.
