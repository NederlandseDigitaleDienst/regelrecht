# Law Generate - Usage Examples

All examples below pass `just validate` against the latest schema.

## Example 1. Simple Constant (direct value assignment)

**Legal Text:**
```
De standaardpremie bedraagt € 2.112 per jaar.
```

**machine_readable:**
```yaml
machine_readable:
  execution:
    output:
      - name: standaardpremie
        type: amount
        type_spec:
          unit: eurocent
    actions:
      - output: standaardpremie
        value: 211200  # €2.112 in eurocent
```

---

## Example 2. Eligibility Check (AND with comparisons)

**Legal Text:**
```
Een persoon heeft recht op zorgtoeslag indien hij:
a. de leeftijd van 18 jaar heeft bereikt;
b. verzekerd is ingevolge de Zorgverzekeringswet;
c. in Nederland woont.
```

**machine_readable:**
```yaml
machine_readable:
  execution:
    produces:
      legal_character: BESCHIKKING
      decision_type: TOEKENNING
    parameters:
      - name: bsn
        type: string
        required: true
        description: Burgerservicenummer
    input:
      - name: leeftijd
        type: number
        source:
          regulation: wet_basisregistratie_personen
          output: leeftijd
          parameters:
            bsn: $bsn
      - name: is_verzekerd
        type: boolean
        source:
          regulation: zorgverzekeringswet
          output: is_verzekerd
          parameters:
            bsn: $bsn
      - name: woont_in_nederland
        type: boolean
        source:
          regulation: wet_basisregistratie_personen
          output: woont_in_nederland
          parameters:
            bsn: $bsn
    output:
      - name: heeft_recht_op_zorgtoeslag
        type: boolean
        description: Geeft aan of de persoon recht heeft op zorgtoeslag
    actions:
      - output: heeft_recht_op_zorgtoeslag
        value:
          operation: AND
          conditions:
            - operation: GREATER_THAN_OR_EQUAL
              subject: $leeftijd
              value: 18
            - operation: EQUALS
              subject: $is_verzekerd
              value: true
            - operation: EQUALS
              subject: $woont_in_nederland
              value: true
```

**Key points:**
- `source` uses `regulation` + `output` (NOT `url`)
- `AND` uses `conditions` array
- Comparisons use `subject` (must be `$variable`) + `value`
- Action uses `value:` pattern (not top-level `operation:`)
- The bindings here are illustrative. Before you copy one, read the target law in
  the corpus and check that it really produces an output by that name

---

## Example 3. Internal Reference Between Articles

**Article 2** references article 3's output:
```yaml
# Article 2
machine_readable:
  execution:
    parameters:
      - name: bsn
        type: string
        required: true
    input:
      - name: vermogen_onder_grens
        type: boolean
        source:
          output: vermogen_onder_grens  # Internal reference (no regulation field)
    output:
      - name: heeft_recht
        type: boolean
    actions:
      - output: heeft_recht
        value:
          operation: EQUALS
          subject: $vermogen_onder_grens
          value: true
```

**Article 3** provides the referenced output:
```yaml
# Article 3
machine_readable:
  definitions:
    VERMOGENSGRENS_ALLEENSTAAND:
      value: 15485900  # €154.859 in eurocent
    VERMOGENSGRENS_GEHUWD:
      value: 18687500  # €186.875 in eurocent
  execution:
    parameters:
      - name: bsn
        type: string
        required: true
    input:
      - name: vermogen
        type: amount
        source:
          description: "Vermogen van de belanghebbende (extern)"
        type_spec:
          unit: eurocent
      - name: heeft_partner
        type: boolean
        source:
          regulation: algemene_wet_inkomensafhankelijke_regelingen
          output: heeft_toeslagpartner
          parameters:
            bsn: $bsn
    output:
      - name: vermogen_onder_grens
        type: boolean
        description: Is vermogen onder de toepasselijke grens?
    actions:
      - output: toepasselijke_grens
        value:
          operation: IF
          cases:
            - when:
                operation: EQUALS
                subject: $heeft_partner
                value: true
              then: $VERMOGENSGRENS_GEHUWD
          default: $VERMOGENSGRENS_ALLEENSTAAND
      - output: vermogen_onder_grens
        value:
          operation: LESS_THAN_OR_EQUAL
          subject: $vermogen
          value: $toepasselijke_grens
```

**Key points:**
- Internal reference: `source: { output: "vermogen_onder_grens" }` (no `regulation`)
- `IF` uses `cases`/`default` (NOT `when`/`then`/`else`)
- Intermediate output `toepasselijke_grens` is referenced by later action

---

## Example 4. Complex Nested Calculation

From the actual zorgtoeslag law (simplified):

```yaml
machine_readable:
  definitions:
    percentage_drempelinkomen_alleenstaande:
      value: 0.01896
    percentage_drempelinkomen_partner:
      value: 0.04273
  execution:
    parameters:
      - name: bsn
        type: string
        required: true
    input:
      - name: standaardpremie
        type: amount
        source:
          output: standaardpremie
        type_spec:
          unit: eurocent
      - name: toetsingsinkomen
        type: amount
        source:
          regulation: algemene_wet_inkomensafhankelijke_regelingen
          output: toetsingsinkomen
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
    output:
      - name: hoogte_zorgtoeslag
        type: amount
        type_spec:
          unit: eurocent
    actions:
      - output: hoogte_zorgtoeslag
        value:
          operation: MAX
          values:
            - 0
            - operation: SUBTRACT
              values:
                - operation: IF
                  cases:
                    - when:
                        operation: EQUALS
                        subject: $heeft_toeslagpartner
                        value: true
                      then:
                        operation: MULTIPLY
                        values:
                          - 2
                          - $standaardpremie
                  default: $standaardpremie
                - operation: MULTIPLY
                  values:
                    - operation: IF
                      cases:
                        - when:
                            operation: EQUALS
                            subject: $heeft_toeslagpartner
                            value: true
                          then: $percentage_drempelinkomen_partner
                      default: $percentage_drempelinkomen_alleenstaande
                    - $toetsingsinkomen
```

**Key points:**
- Action uses `value:` wrapper with nested `operation: MAX` + `values: [...]`
- Operations nest deeply: MAX → SUBTRACT → MULTIPLY → IF
- Each nested operation is a full operation object
- No `subject`/`value` on arithmetic, only a `values` array
- IF uses `cases`/`default` everywhere

---

## Example 5. Open Terms, Declaring Law (IoC declaration)

When a law delegates a value to a lower regulation ("bij ministeriële regeling"),
the law declares an `open_term`:

```yaml
# wet_op_de_zorgtoeslag article 4
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
        value: $standaardpremie   # Resolved by engine via implements_index
```

**Key points:**
- `open_terms` declares what the law expects from the regulations that fill it
- `$standaardpremie` references the open term as a variable
- The engine resolves it by finding the regulation that `implements` this term
- `delegation_type` constrains which regulatory layer may fill it
- `required: true` means execution fails if no implementing regulation is found

---

## Example 6. Open Terms, Implementing Regulation (IoC implementation)

The implementing regulation registers as filling the open term:

```yaml
# regeling_standaardpremie article 1
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
      - name: berekeningsjaar
        type: number
    actions:
      - output: standaardpremie
        value: 211200
      - output: berekeningsjaar
        value: 2025
```

**Key points:**
- `implements` links back to the law, article, and open_term id
- `gelet_op` provides legal traceability ("Considering article X of law Y")
- The output name must match the open term `id` so the engine can resolve it
- Priority is resolved via lex superior (regulatory layer) and lex posterior (date)

---

## Example 7. A Marking Next to a Full Model

Article 1 of the Wet op de zorgtoeslag, the begrippenlijst. Its onderdelen are as
translatable as this corpus gets: "Onze Minister" is a literal, "zorgverzekering"
and "premie" are definitions by reference, "verzekerde" is three cross-law
categories with a leeftijdsgrens and one exception, "drempelinkomen" is a sum
over the minimumloon. Every one of them is modelled below.

The sentence above them is not translatable at all. "In deze wet **en de daarop
berustende bepalingen**" gives the definitions a reach past this document: they
govern the wording of every regulation resting on this law, whether or not that
regulation refers to them. The format has the opposite direction only, a `source`
binding written by the document that reads. So the entry carries the model and
the marking, and the marking takes nothing away from the model.

```yaml
machine_readable:
  endpoint: is_verzekerde
  markings:
    - about: het bereik van de begripsbepalingen over de daarop berustende bepalingen
      reason: >-
        De begrippen van dit artikel beheersen ook het woordgebruik van elke regeling
        die op deze wet berust, zonder dat die regeling ernaar verwijst. Het model kent
        alleen de omgekeerde richting: een document leest een ander document via een
        source-binding die het zelf schrijft. Een bereik dat geldt zonder dat de andere
        kant het aanroept, heeft geen veld en geen bewerking.
      resolution: model
      resolved_by: >-
        Een vorm waarin de begripsbepalingen van een wet in bereik komen bij het
        uitvoeren van een regeling die op die wet berust, afgeleid uit de legal_basis
        die de regeling zelf declareert.
      target: []
      legal_text_excerpt: In deze wet en de daarop berustende bepalingen wordt, tenzij anders is geregeld, verstaan onder
      accepted: false
  definitions:
    zorgverzekering:
      value: de schadeverzekering, bedoeld in artikel 1, onder d, van de Zorgverzekeringswet
      description: >-
        Onderdeel b. Begripsbepaling door verwijzing: waar deze wet "zorgverzekering"
        gebruikt, geldt het begrip van de Zorgverzekeringswet. De bepaling stelt zelf
        geen waarde vast.
  execution:
    parameters:
      - name: bsn
        type: string
        required: true
      - name: peildatum
        type: date
        required: true
      - name: geboortedatum
        type: date
        required: true
    input:
      - name: is_verzekerde_zorgverzekeringswet
        type: boolean
        description: 'Onderdeel c, eerste categorie: de persoon, bedoeld in artikel 1, onder f'
        source:
          regulation: zorgverzekeringswet
          output: is_verzekerde
          parameters:
            bsn: $bsn
      - name: zijn_rechten_uit_zorgverzekering_opgeschort
        type: boolean
        description: 'Onderdeel c, uitzondering: de verzekerde, bedoeld in artikel 24, eerste of derde lid'
        source:
          regulation: zorgverzekeringswet
          output: rechten_uit_zorgverzekering_opgeschort
          parameters:
            bsn: $bsn
    output:
      - name: onze_minister
        type: string
      - name: is_verzekerde
        type: boolean
    actions:
      - output: onze_minister
        value: Onze Minister van Volksgezondheid, Welzijn en Sport
        legal_basis:
          law: Wet op de zorgtoeslag
          article: '1'
          paragraph: '1'
          explanation: >-
            Onderdeel a wijst Onze Minister aan. De aanhef laat toe dat een andere regeling
            dit begrip opzijzet ("tenzij anders is geregeld"); die verdringing wordt door
            die regeling zelf als override verklaard en niet hier.
      - output: achttiende_verjaardag
        value:
          operation: DATE_ADD
          date: $geboortedatum
          years: 18
      - output: eerste_dag_kalendermaand_verzekerde
        value:
          operation: DATE_ADD
          date:
            operation: START_OF
            date: $achttiende_verjaardag
            in: month
          months: 1
      - output: is_verzekerde
        value:
          operation: AND
          conditions:
            - operation: EQUALS
              subject: $is_verzekerde_zorgverzekeringswet
              value: true
            - operation: GREATER_THAN_OR_EQUAL
              subject: $peildatum
              value: $eerste_dag_kalendermaand_verzekerde
            - operation: NOT
              value: $zijn_rechten_uit_zorgverzekering_opgeschort
```

**Key points:**
- The marking and the model answer different words in the same entry. Neither is
  the price of the other, and an entry that carries only one of the two is
  incomplete in whichever direction it left out.
- `target: []` because the marking blocks nothing. Every output this entry
  declares is computed; what is missing is the reach of the words, not a value.
- `reason` names the shape that comes closest, the `source` binding, and says
  where it falls short. Without that a reader cannot tell whether the wish in
  `resolved_by` is earned.
- "Tenzij anders is geregeld" gets no marking. A displacement is declared by the
  regulation that departs, so `overrides` covers it, and the clause is recorded
  in the `legal_basis.explanation` of the definitions it qualifies.
- A reference to a law that has not been harvested is still a binding. Whether
  the Zorgverzekeringswet is in the corpus is a state of the corpus and does not
  belong in this file.

---

## Example 8. IF with Multiple Cases (replaces SWITCH)

```yaml
actions:
  - output: normbedrag
    value:
      operation: IF
      cases:
        - when:
            operation: AND
            conditions:
              - operation: GREATER_THAN_OR_EQUAL
                subject: $leeftijd
                value: 21
              - operation: EQUALS
                subject: $is_alleenstaande
                value: true
          then: 109171
        - when:
            operation: AND
            conditions:
              - operation: GREATER_THAN_OR_EQUAL
                subject: $leeftijd
                value: 21
              - operation: EQUALS
                subject: $is_alleenstaande
                value: false
          then: 155958
      default: 0
```

**Key points:**
- Use `IF` with `cases`/`default` for multi-branch conditionals (SWITCH was removed in v0.5.0)
- Cases are evaluated in order; the first matching case wins
- Each case has `when` (condition) and `then` (result value)
- `default` is the fallback if no case matches

---

## Example 9. AGE Operation

From Wet open overheid, calculating how old a document is:

```yaml
actions:
  - output: informatie_leeftijd_jaren
    value:
      operation: AGE
      date_of_birth: $informatie_datum
      reference_date: $peildatum
  - output: verzwaarde_motiveringsplicht
    value:
      operation: GREATER_THAN_OR_EQUAL
      subject: $informatie_leeftijd_jaren
      value: 5
```

**Key points:**
- AGE calculates complete years between two dates
- Works for any "age" calculation, not just people (document age, policy age, etc.)
- Uses `date_of_birth` and `reference_date` (not subject/value)
- `$peildatum` must be declared as a parameter, it is NOT a built-in

---

## Example 10. DATE_ADD Operation (deadline calculations)

From AWB article 6:8, calculating appeal deadlines:

```yaml
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
        description: Datum waarop het besluit is bekendgemaakt
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
      # "met ingang van de dag na die waarop het besluit is bekendgemaakt"
      - output: termijn_startdatum
        value:
          operation: DATE_ADD
          date: $bekendmaking_datum
          days: 1
      # einddatum = bekendmaking + termijn in weken
      - output: termijn_einddatum
        value:
          operation: DATE_ADD
          date: $bekendmaking_datum
          weeks: $termijn_weken
```

**Key points:**
- DATE_ADD supports `years`, `months`, `weeks`, `days` (all optional)
- Multiple duration components can be combined in one operation
- Values can be literals or $variable references
- Combined with hooks, this fires automatically for all BESCHIKKING decisions

---

## Example 11. Hooks Pattern (AWB cross-cutting concern)

From AWB article 3:46, the motivation requirement:

```yaml
machine_readable:
  hooks:
    - hook_point: pre_actions
      applies_to:
        legal_character: BESCHIKKING
        stage: BESLUIT
  execution:
    output:
      - name: motivering_vereist
        type: boolean
    actions:
      - output: motivering_vereist
        value: true
```

**Key points:**
- `pre_actions` fires before the target article's actions execute
- `post_actions` fires after the target article's actions
- `applies_to.legal_character` is required
- `applies_to.stage` is optional (defaults to BESLUIT)
- This creates a cross-cutting obligation that applies to ALL beschikkingen

---

## Example 12. Overrides Pattern (lex specialis)

From Vreemdelingenwet 2000 article 69, overriding the standard AWB appeal deadline:

```yaml
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

**Legal Text:**
```
In afwijking van artikel 6:7 van de Algemene wet bestuursrecht bedraagt
de termijn voor het indienen van een bezwaar- of beroepschrift vier weken.
```

**Key points:**
- `overrides` declares which law, article, and output are being replaced
- The output name must match what is declared in overrides
- The engine substitutes this output wherever the overridden article's output is used
- Pattern: "In afwijking van artikel X van wet Y" signals an override

---

## Example 13. NOT Operation (negation patterns)

```yaml
# Simple negation of a variable
actions:
  - output: openbaarmaking_toegestaan
    value:
      operation: AND
      conditions:
        - operation: NOT
          value: $heeft_absolute_weigeringsgrond
        - operation: NOT
          value: $heeft_relatieve_weigeringsgrond

# Negation of a comparison
actions:
  - output: niet_verzekerd
    value:
      operation: NOT
      value:
        operation: EQUALS
        subject: $is_verzekerd
        value: true

# Negation of a compound condition
actions:
  - output: geen_uitzondering
    value:
      operation: NOT
      value:
        operation: AND
        conditions:
          - $is_milieu_informatie
          - $betreft_emissies
```

**Key points:**
- NOT uses `value:` (not `subject:` or `conditions:`)
- Can negate a `$variable` directly, a comparison, or a compound condition
- Use NOT to express "tenzij", "niet", "geen" in legal text
- Replaces the removed NOT_EQUALS, NOT_IN, NOT_NULL operations:
  - NOT_EQUALS → `NOT` + `EQUALS`
  - NOT_IN → `NOT` + `IN`
  - NOT_NULL → use `NOT` + `EQUALS` with null, or restructure as a positive check

---

## Example 14. MvT Passage to Gherkin Scenario

Shows how to convert a Memorie van Toelichting passage into a BDD scenario.

**MvT passage (from kst-30912-3, Wet op de zorgtoeslag):**
```
Rekenvoorbeeld 1: Alleenstaande met een inkomen van €20.000

De standaardpremie bedraagt €2.112. Het percentage van het drempelinkomen
voor een alleenstaande bedraagt 1,896%. Het percentage normpremie
toetsingsinkomen bedraagt 13,7%.

Normpremie = 1,896% × €20.000 = €379,20
Zorgtoeslag = €2.112 - €379,20 = €1.732,80
```

**Generated Gherkin scenario:**
```gherkin
Feature: Zorgtoeslag, scenarios uit Memorie van Toelichting
  Testscenario's afgeleid uit de Memorie van Toelichting bij de
  Wet op de zorgtoeslag (kst-30912-3).

  # Bron: kst-30912-3
  # URL: https://zoek.officielebekendmakingen.nl/kst-30912-3.html

  Background:
    Given the calculation date is "2025-01-01"

  # === Rekenvoorbeelden uit MvT ===

  Scenario: Alleenstaande met inkomen van 20.000 euro
    # Bron: kst-30912-3, Rekenvoorbeeld 1
    Given the following RVIG "personal_data" data:
      | bsn       | geboortedatum | land_verblijf |
      | 999993653 | 1990-01-01    | NEDERLAND     |
    And the following RVZ "insurance" data:
      | bsn       | is_verzekerd |
      | 999993653 | true         |
    And the following BELASTINGDIENST "box1" data:
      | bsn       | belastbaar_inkomen |
      | 999993653 | 2000000            |
    When the zorgtoeslag is executed for wet_op_de_zorgtoeslag article 2
    Then the hoogte_zorgtoeslag is "173280" eurocent
```

**Key points:**
- Each scenario traces back to a specific MvT passage with `# Bron:` comment
- Monetary inputs are in eurocent (€20.000 = 2000000)
- Expected outputs are ALWAYS in eurocent (€1.732,80 = 173280), never euro with decimals
- When/Then steps use concrete law names (not placeholders like `{law_name}`)
- The scenario uses existing Given/When/Then steps, not new ones
- Do NOT invent scenarios, only use what the legislature provided

---

## Common Mistakes and Fixes

### Mistake 1. Wrong IF syntax (v0.4.0 style)
**Wrong (v0.4.0, removed):**
```yaml
operation: IF
when:
  operation: EQUALS
  subject: $x
  value: true
then: 100
else: 0
```

**Correct (v0.5.1):**
```yaml
operation: IF
cases:
  - when:
      operation: EQUALS
      subject: $x
      value: true
    then: 100
default: 0
```

### Mistake 2. Using SWITCH (removed in v0.5.0)
**Wrong:**
```yaml
operation: SWITCH
cases:
  - when: ...
    then: ...
default: 0
```

**Correct (use IF instead):**
```yaml
operation: IF
cases:
  - when: ...
    then: ...
default: 0
```

### Mistake 3. Using url instead of regulation for source
**Wrong:**
```yaml
source:
  url: "regulation/nl/wet/zorgverzekeringswet#is_verzekerd"
```

**Correct:**
```yaml
source:
  regulation: zorgverzekeringswet
  output: is_verzekerd
  parameters:
    bsn: $bsn
```

### Mistake 4. Using subject/value for arithmetic
**Wrong:**
```yaml
operation: SUBTRACT
subject: $bruto
value: $korting
```

**Correct:**
```yaml
operation: SUBTRACT
values:
  - $bruto
  - $korting
```

### Mistake 5. Using values for logical operations
**Wrong:**
```yaml
operation: AND
values:
  - operation: EQUALS
    subject: $a
    value: true
```

**Correct:**
```yaml
operation: AND
conditions:
  - operation: EQUALS
    subject: $a
    value: true
```

### Mistake 6. Using SUBTRACT_DATE for age (removed in v0.5.1)
**Wrong:**
```yaml
operation: SUBTRACT_DATE
subject: $peildatum
value: $geboortedatum
unit: years
```

**Correct:**
```yaml
operation: AGE
date_of_birth: $geboortedatum
reference_date: $peildatum
```

### Mistake 7. Using CONCAT (removed in v0.5.1)
**Wrong:**
```yaml
operation: CONCAT
values:
  - "Beschikking "
  - $wet_naam
```

**Correct (use ADD for string concatenation):**
```yaml
operation: ADD
values:
  - "Beschikking "
  - $wet_naam
```

### Mistake 8. Using NOT_EQUALS, NOT_IN, NOT_NULL (removed in v0.5.1)
**Wrong:**
```yaml
operation: NOT_EQUALS
subject: $status
value: "ACTIEF"
```

**Correct (use NOT + positive operation):**
```yaml
operation: NOT
value:
  operation: EQUALS
  subject: $status
  value: "ACTIEF"
```

### Mistake 9. Wrong monetary type
**Wrong:**
```yaml
output:
  - name: bedrag
    type: number  # Should be amount for monetary values
```

**Correct:**
```yaml
output:
  - name: bedrag
    type: amount
    type_spec:
      unit: eurocent
```

### Mistake 10. Treating $referencedate as a built-in
**Wrong (not declaring it):**
```yaml
execution:
  actions:
    - output: leeftijd
      value:
        operation: AGE
        date_of_birth: $geboortedatum
        reference_date: $referencedate  # ERROR: undeclared variable
```

**Correct (declare as parameter):**
```yaml
execution:
  parameters:
    - name: referencedate
      type: date
      required: true
      description: Peildatum voor berekening
  actions:
    - output: leeftijd
      value:
        operation: AGE
        date_of_birth: $geboortedatum
        reference_date: $referencedate
```

### Mistake 11. Missing $ prefix on variable
**Wrong:**
```yaml
subject: toetsingsinkomen
```

**Correct:**
```yaml
subject: $toetsingsinkomen
```
