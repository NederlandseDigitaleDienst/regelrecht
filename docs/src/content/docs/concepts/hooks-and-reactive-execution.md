---
title: "Hooks and Reactive Execution"
description: "How cross-cutting laws like the Awb apply to decisions automatically, without being called explicitly."
---

Some laws apply to every government decision without being called explicitly. The General Administrative Law Act (Algemene wet bestuursrecht, Awb) is the main example: whenever any government body issues a formal decision (*beschikking*), Awb rules about reasoning requirements, objection periods, and notification deadlines kick in automatically.

The law that triggers these rules does not know about the Awb. The Awb does not know about that specific law. They are decoupled by design.

## How hooks work

An article can declare a `hooks` block. This tells the engine: "fire this article whenever the conditions in `applies_to` match."

```yaml
# Awb 6:7 - sets the objection period to 6 weeks
machine_readable:
  hooks:
    - hook_point: post_actions
      applies_to:
        legal_character: BESCHIKKING
        stage: BESLUIT
  execution:
    output:
      - name: termijn_weken
        type: number
    actions:
      - output: termijn_weken
        value: 6
```

When the Zorgtoeslagwet calculates a healthcare allowance and produces a *beschikking*, this Awb hook fires and adds `termijn_weken: 6` to the result. The Zorgtoeslagwet does not mention the Awb or objection periods.

## Hook points

Hooks can fire at two moments during article execution:

- **`pre_actions`** - after open term resolution, before the article's own logic runs. Used for prerequisites like the Awb reasoning requirement (3:46): the decision must include a motivation.
- **`post_actions`** - after the article's logic completes. Used for consequences like the objection period (6:7) and notification deadlines (6:8).

## The `produces` annotation

For hooks to fire, the target article must declare what kind of legal product it produces:

```yaml
# Zorgtoeslagwet, article 2
execution:
  produces:
    legal_character: BESCHIKKING
    decision_type: TOEKENNING
```

The engine builds a hook index at load time. When it encounters an article with a `produces` annotation, it checks the index for matching hooks and fires them.

A hook can also apply to an application instead of a decision: `applies_to.submission: AANVRAAG` fires on every article whose `produces` says it establishes one ([RFC-045](/rfcs/rfc-045)). Awb 4:2 and 4:13 work this way. When that article also decides on the application, `produces` lists the two, and a hook matches the item whose `type` it names. A staged run then fires the hooks on the application at the stage named after the kind, AANVRAAG, and the hooks on the decision at their own stages ([RFC-045](/rfcs/rfc-045)).

## The objection-deadline chain

Awb hooks compose into a chain:

1. **Awb 3:46** (pre_actions hook, stage: BESLUIT) - sets `motivering_vereist: true`
2. The target law's article runs and produces the decision
3. **Awb 6:7** (post_actions hook, stage: BESLUIT) - sets `termijn_weken: 6`
4. Later, when the decision is communicated (stage: BEKENDMAKING):
5. **Awb 6:8** (post_actions hook, stage: BEKENDMAKING) - calculates `termijn_startdatum` and `termijn_einddatum` based on the notification date and the 6-week period from Awb 6:7

```yaml
# Awb 6:8 - calculates start and end dates for the objection period
machine_readable:
  hooks:
    - hook_point: post_actions
      applies_to:
        legal_character: BESCHIKKING
        stage: BEKENDMAKING
  execution:
    parameters:
      - name: bekendmaking_datum
        type: date
        required: true
    input:
      - name: termijn_weken
        type: number
        source:
          regulation: algemene_wet_bestuursrecht
          output: termijn_weken
    output:
      - name: termijn_startdatum
        type: date
      - name: termijn_einddatum
        type: date
    actions:
      - output: termijn_startdatum
        value:
          operation: DATE_ADD
          date: $bekendmaking_datum
          days: 1
      - output: termijn_einddatum
        value:
          operation: DATE_ADD
          date: $bekendmaking_datum
          weeks: $termijn_weken
```

## Overrides (lex specialis)

Sometimes a specific law overrides a general Awb rule. The Aliens Act (*Vreemdelingenwet*) article 69 says: *"in afwijking van artikel 6:7 Awb bedraagt de termijn vier weken"* ("departing from Awb article 6:7, the period is four weeks").

This is modeled with `overrides`:

```yaml
# Vreemdelingenwet, article 69
machine_readable:
  overrides:
    - law: algemene_wet_bestuursrecht
      article: '6:7'
      output: termijn_weken
  execution:
    output:
      - name: termijn_weken
        type: number
    actions:
      - output: termijn_weken
        value: 4
```

The Awb does not know it is being overridden. The Vreemdelingenwet unilaterally replaces the value. This only applies when the Vreemdelingenwet is part of the execution chain (a Participatiewet case is not affected by this override).

### Voiding an output

An override can also say that the output does not arise at all, rather than being replaced by another value:

```yaml
machine_readable:
  overrides:
    - law: algemene_wet_bestuursrecht
      article: '6:7'
      output: termijn_weken
      voids: true
      legal_text_excerpt: "geen bezwaar mogelijk"
```

This distinction is legal, not technical. *"Bestaat geen aanspraak"* is not an entitlement of zero: an entitlement of zero is still a decision, so it carries legal remedies and a ground for recovery, and no entitlement carries neither. Writing a void as `value: 0` would erase that difference. The engine reads the flag and needs no knowledge of administrative law to act on it.

An override of another law's output applies only inside an execution that the overriding law started, as above. An article overriding an output of **its own** law is a different claim: there is no other law to protect from, so it applies whenever that law runs. Without that, the outcome would depend on the route in, and an amount the statute says does not arise would still be handed out when a third law read the output directly. `voids` is what exposed this, but the rule is about same-law versus cross-law and holds for a replacement too. See [RFC-041](/rfcs/rfc-041) for the amendment it makes to the contextual-law rule.

### Reading the value an override departs from

A special rule often states its departure in terms of the general rule. Take an (illustrative) article saying *"in afwijking van artikel 2 van de Wet op de zorgtoeslag wordt de zorgtoeslag met tien procent verhoogd"*. The overriding article reads the very output it replaces:

```yaml
overrides:
  - law: wet_op_de_zorgtoeslag
    article: '2'
    output: hoogte_zorgtoeslag
execution:
  input:
    - name: zorgtoeslag_volgens_wet
      source:
        regulation: wet_op_de_zorgtoeslag
        output: hoogte_zorgtoeslag
  actions:
    - output: hoogte_zorgtoeslag
      value:
        operation: MULTIPLY
        values: [$zorgtoeslag_volgens_wet, 1.1]
```

That read is the general rule's value, computed with this override held back, and the override applies once. The same holds for an article that `implements` an open term of a law and reads that law: it reads the law without its own filling, which is the next implementation or the `default`. A `voids` override is not included: reading a voided output still yields the void.

A replacing override takes effect where the overridden article sets the output, so the article's own actions that read it see the replaced value, like every other reader: a monthly amount derived from `hoogte_zorgtoeslag` in the same article follows the raised amount. The article runs its actions in dependency order, so where an action stands in the file never changes which value it reads; only the assignments of an output that is set more than once keep their order among themselves. An override may read the inputs of the article it overrides, but not its other outputs: the engine refuses that, because which of them were already set would depend on the order of the actions in the file. A name that is both an input and an output of that article is the input to the override, except the replaced output itself, which the override receives as the article computed it.

### How overrides differ from IoC

| | IoC (`open_terms` + `implements`) | Overrides |
|---|---|---|
| General law | Knows it delegates, declares `open_terms` | Does not know, declares nothing |
| Specific law | Declares `implements` | Declares `overrides` |
| Legal text | *"Gelet op artikel..."* | *"In afwijking van artikel..."* |
| Relationship | Bilateral (both sides participate) | Unilateral (only the overrider declares) |

## Administrative procedure stages

A *beschikking* takes shape over time, moving through stages: application, review, decision, notification, objection. Hooks bind to specific stages via `applies_to.stage`.

The Awb defines a procedure lifecycle:

| Stage | Description | Example hook |
|-------|-------------|-------------|
| AANVRAAG | Application filed | |
| BEHANDELING | Under review | |
| BESLUIT | Decision taken | Awb 3:46 (reasoning), 6:7 (objection period) |
| BEKENDMAKING | Decision communicated | Awb 6:8 (deadline calculation) |
| BEZWAAR | Objection period | |

The engine yields between stages, returning accumulated outputs and indicating what inputs are needed for the next stage. The engine itself stays stateless; the procedure state is managed externally.

## Further reading

- [Cross-Law References](./cross-law-references) - how laws reference each other explicitly
- [Inversion of Control](./inversion-of-control) - how a law leaves a value to another regulation
- [Traceability](./traceability) - how hook and override nodes appear in an execution trace
- [Temporal Validity and Dates](./temporal-and-dates) - the date arithmetic behind the objection-deadline chain
- [RFC-007: Cross-Law Execution](/rfcs/rfc-007) - hooks and overrides specification
- [Applications, Decisions and Objections](./administrative-procedures) - an Awb procedure in the law format
- [RFC-008: Bestuursrecht/Awb](/rfcs/rfc-008) - the administrative procedure model
- [Rules as Executed, section 4.9](/research/rules-as-executed#sec:modes) - the position paper's four execution modes, with the hook as the declared trigger of the reactive one
