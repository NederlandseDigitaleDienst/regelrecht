@tier:core
Feature: Operations are defined per type — RFC-055
  Every operation has a closed list of signatures: the operand types it accepts
  and the type it returns. Operands that match no signature are an error, never
  a quiet conversion. A count is not a verdict, a number and a text do not add,
  and two values of different types are not "unequal": the question is refused.

  The operands are fields of a record, of which no declaration says the type,
  so each scenario decides what reaches the engine and the engine has to refuse
  what does not fit. Each operation sits in an article of its own.

  A scenario that shows an operation accepting a row of the table carries the
  tag `@signature:<operation>:<row>`. The engine's test suite fails when a row
  has no such scenario, so the table cannot grow without one.

  Background:
    Given the calculation date is "2025-01-01"
    Given parameter "bsn" is "999993653"

  # ---------------------------------------------------------------------------
  # ADD: numbers add, strings join, arrays join. Nothing else.
  # ---------------------------------------------------------------------------

  @signature:ADD:number
  Scenario: ADD of two numbers is their sum
    Given the following "register" data with key "bsn" for law "test_signaturen":
      | bsn       | rij              |
      | 999993653 | {"a": 2, "b": 3} |
    When I evaluate "som" of "test_signaturen"
    Then the execution succeeds
    Then output "som" equals 5

  @signature:ADD:string
  Scenario: ADD of two strings joins them
    Given the following "register" data with key "bsn" for law "test_signaturen":
      | bsn       | rij                        |
      | 999993653 | {"a": "af", "b": "spraak"} |
    When I evaluate "tekst" of "test_signaturen"
    Then the execution succeeds
    Then output "tekst" equals "afspraak"

  @signature:ADD:array
  Scenario: ADD of two arrays joins them in order
    Given the following "register" data with key "bsn" for law "test_signaturen":
      | bsn       | rij                                                        |
      | 999993653 | {"a": ["x", "y"], "b": ["z"], "verwacht": ["x", "y", "z"]} |
    When I evaluate "lijst_klopt" of "test_signaturen"
    Then the execution succeeds
    Then output "lijst_klopt" is true
  Scenario: ADD of two arrays keeps their order
    Given the following "register" data with key "bsn" for law "test_signaturen":
      | bsn       | rij                                                        |
      | 999993653 | {"a": ["x", "y"], "b": ["z"], "verwacht": ["z", "x", "y"]} |
    When I evaluate "lijst_klopt" of "test_signaturen"
    Then the execution succeeds
    Then output "lijst_klopt" is false

  Scenario: ADD of a number and a string is refused
    Given the following "register" data with key "bsn" for law "test_signaturen":
      | bsn       | rij                |
      | 999993653 | {"a": 2, "b": "3"} |
    When I evaluate "som" of "test_signaturen"
    Then the execution fails with "ADD is not defined for (number, string)"

  Scenario: ADD of a boolean and a number is refused
    Given the following "register" data with key "bsn" for law "test_signaturen":
      | bsn       | rij                 |
      | 999993653 | {"a": true, "b": 1} |
    When I evaluate "som" of "test_signaturen"
    Then the execution fails with "ADD is not defined for (boolean, number)"

  Scenario: ADD of a string and an array is refused
    Given the following "register" data with key "bsn" for law "test_signaturen":
      | bsn       | rij                    |
      | 999993653 | {"a": "x", "b": ["y"]} |
    When I evaluate "lijst" of "test_signaturen"
    Then the execution fails with "ADD is not defined for (string, array)"

  # ---------------------------------------------------------------------------
  # SUBTRACT, MULTIPLY, DIVIDE, MIN, MAX: numbers only.
  # ---------------------------------------------------------------------------

  @signature:SUBTRACT:number @signature:MULTIPLY:number @signature:DIVIDE:number @signature:MAX:number @signature:MIN:number
  Scenario: The other arithmetic takes numbers
    Given the following "register" data with key "bsn" for law "test_signaturen":
      | bsn       | rij               |
      | 999993653 | {"a": 10, "b": 4} |
    When I evaluate outputs "verschil, product, quotient, grootste, kleinste" of "test_signaturen"
    Then the execution succeeds
    Then output "verschil" equals 6
    Then output "product" equals 40
    Then output "quotient" equals 2.5
    Then output "grootste" equals 10
    Then output "kleinste" equals 4

  Scenario: SUBTRACT of two strings is refused
    Given the following "register" data with key "bsn" for law "test_signaturen":
      | bsn       | rij                   |
      | 999993653 | {"a": "10", "b": "4"} |
    When I evaluate "verschil" of "test_signaturen"
    Then the execution fails with "SUBTRACT is not defined for (string, string)"

  Scenario: MULTIPLY of a number and a string is refused
    Given the following "register" data with key "bsn" for law "test_signaturen":
      | bsn       | rij                 |
      | 999993653 | {"a": 10, "b": "4"} |
    When I evaluate "product" of "test_signaturen"
    Then the execution fails with "MULTIPLY is not defined for (number, string)"

  Scenario: DIVIDE of a number and a boolean is refused
    Given the following "register" data with key "bsn" for law "test_signaturen":
      | bsn       | rij                  |
      | 999993653 | {"a": 10, "b": true} |
    When I evaluate "quotient" of "test_signaturen"
    Then the execution fails with "DIVIDE is not defined for (number, boolean)"

  Scenario: Dividing by zero is an error
    Given the following "register" data with key "bsn" for law "test_signaturen":
      | bsn       | rij               |
      | 999993653 | {"a": 10, "b": 0} |
    When I evaluate "quotient" of "test_signaturen"
    Then the execution fails with "Division by zero"

  Scenario: MAX of a number and a string is refused
    Given the following "register" data with key "bsn" for law "test_signaturen":
      | bsn       | rij                 |
      | 999993653 | {"a": 10, "b": "4"} |
    When I evaluate "grootste" of "test_signaturen"
    Then the execution fails with "MAX is not defined for (number, string)"

  Scenario: MIN of two strings is refused
    Given the following "register" data with key "bsn" for law "test_signaturen":
      | bsn       | rij                  |
      | 999993653 | {"a": "a", "b": "b"} |
    When I evaluate "kleinste" of "test_signaturen"
    Then the execution fails with "MIN is not defined for (string, string)"

  # ---------------------------------------------------------------------------
  # No operation rounds: a division is exact.
  # ---------------------------------------------------------------------------

  Scenario: A third times three is one
    Given the following "register" data with key "bsn" for law "test_signaturen":
      | bsn       | rij              |
      | 999993653 | {"a": 1, "b": 3} |
    When I evaluate "terug" of "test_signaturen"
    Then the execution succeeds
    Then output "terug" equals 1

  # ---------------------------------------------------------------------------
  # ROUND: a number. A tie goes away from zero.
  # ---------------------------------------------------------------------------

  @signature:ROUND:number
  Scenario: ROUND rounds a tie away from zero
    Given the following "register" data with key "bsn" for law "test_signaturen":
      | bsn       | rij        |
      | 999993653 | {"a": 2.5} |
    When I evaluate "afgerond" of "test_signaturen"
    Then the execution succeeds
    Then output "afgerond" equals 3

  Scenario: ROUND rounds a negative tie away from zero
    Given the following "register" data with key "bsn" for law "test_signaturen":
      | bsn       | rij         |
      | 999993653 | {"a": -2.5} |
    When I evaluate "afgerond" of "test_signaturen"
    Then the execution succeeds
    Then output "afgerond" equals -3

  @signature:CEIL:number @signature:FLOOR:number
  Scenario: CEIL rounds up and FLOOR rounds down, also below zero
    Given the following "register" data with key "bsn" for law "test_signaturen":
      | bsn       | rij         |
      | 999993653 | {"a": -2.5} |
    When I evaluate outputs "naar_boven, naar_beneden" of "test_signaturen"
    Then the execution succeeds
    Then output "naar_boven" equals -2
    Then output "naar_beneden" equals -3

  Scenario: CEIL of a boolean is refused
    Given the following "register" data with key "bsn" for law "test_signaturen":
      | bsn       | rij         |
      | 999993653 | {"a": true} |
    When I evaluate "naar_boven" of "test_signaturen"
    Then the execution fails with "CEIL is not defined for (boolean)"

  Scenario: FLOOR of a string is refused
    Given the following "register" data with key "bsn" for law "test_signaturen":
      | bsn       | rij          |
      | 999993653 | {"a": "2.5"} |
    When I evaluate "naar_beneden" of "test_signaturen"
    Then the execution fails with "FLOOR is not defined for (string)"

  Scenario: ROUND of a string is refused
    Given the following "register" data with key "bsn" for law "test_signaturen":
      | bsn       | rij          |
      | 999993653 | {"a": "2.5"} |
    When I evaluate "afgerond" of "test_signaturen"
    Then the execution fails with "ROUND is not defined for (string)"

  # ---------------------------------------------------------------------------
  # EQUALS: two values of one type. An absence is tested, never compared.
  # ---------------------------------------------------------------------------

  @signature:EQUALS:alike
  Scenario: EQUALS of two equal numbers is true
    Given the following "register" data with key "bsn" for law "test_signaturen":
      | bsn       | rij              |
      | 999993653 | {"a": 5, "b": 5} |
    When I evaluate "gelijk" of "test_signaturen"
    Then the execution succeeds
    Then output "gelijk" is true

  Scenario: EQUALS of a whole number and the same number with a fraction part is true
    Given the following "register" data with key "bsn" for law "test_signaturen":
      | bsn       | rij                |
      | 999993653 | {"a": 5, "b": 5.0} |
    When I evaluate "gelijk" of "test_signaturen"
    Then the execution succeeds
    Then output "gelijk" is true

  Scenario: EQUALS of two different strings is false
    Given the following "register" data with key "bsn" for law "test_signaturen":
      | bsn       | rij                    |
      | 999993653 | {"a": "ja", "b": "nee"} |
    When I evaluate "gelijk" of "test_signaturen"
    Then the execution succeeds
    Then output "gelijk" is false

  Scenario: EQUALS of two booleans compares them
    Given the following "register" data with key "bsn" for law "test_signaturen":
      | bsn       | rij                     |
      | 999993653 | {"a": true, "b": true} |
    When I evaluate "gelijk" of "test_signaturen"
    Then the execution succeeds
    Then output "gelijk" is true

  Scenario: EQUALS of a number and a string is refused, not false
    Given the following "register" data with key "bsn" for law "test_signaturen":
      | bsn       | rij                |
      | 999993653 | {"a": 5, "b": "5"} |
    When I evaluate "gelijk" of "test_signaturen"
    Then the execution fails with "EQUALS is not defined for (number, string)"

  Scenario: EQUALS of a boolean and a number is refused
    Given the following "register" data with key "bsn" for law "test_signaturen":
      | bsn       | rij                 |
      | 999993653 | {"a": true, "b": 1} |
    When I evaluate "gelijk" of "test_signaturen"
    Then the execution fails with "EQUALS is not defined for (boolean, number)"

  Scenario: EQUALS of an absence and a value is false
    Given the following "register" data with key "bsn" for law "test_signaturen":
      | bsn       | rij                 |
      | 999993653 | {"a": null, "b": 5} |
    When I evaluate "gelijk" of "test_signaturen"
    Then the execution succeeds
    Then output "gelijk" is false

  Scenario: EQUALS of two absences is true
    Given the following "register" data with key "bsn" for law "test_signaturen":
      | bsn       | rij                    |
      | 999993653 | {"a": null, "b": null} |
    When I evaluate "gelijk" of "test_signaturen"
    Then the execution succeeds
    Then output "gelijk" is true

  # ---------------------------------------------------------------------------
  # Orderings: two numbers, or two dates.
  # ---------------------------------------------------------------------------

  @signature:GREATER_THAN:number-number @signature:LESS_THAN:number-number @signature:GREATER_THAN_OR_EQUAL:number-number @signature:LESS_THAN_OR_EQUAL:number-number
  Scenario: An ordering compares two numbers
    Given the following "register" data with key "bsn" for law "test_signaturen":
      | bsn       | rij              |
      | 999993653 | {"a": 7, "b": 5} |
    When I evaluate outputs "groter, kleiner_of_gelijk, kleiner, groter_of_gelijk" of "test_signaturen"
    Then the execution succeeds
    Then output "groter" is true
    Then output "kleiner_of_gelijk" is false
    Then output "kleiner" is false
    Then output "groter_of_gelijk" is true

  @signature:GREATER_THAN:date-date @signature:LESS_THAN:date-date @signature:GREATER_THAN_OR_EQUAL:date-date @signature:LESS_THAN_OR_EQUAL:date-date
  Scenario: An ordering compares two dates, the later one being greater
    Given the following "register" data with key "bsn" for law "test_signaturen":
      | bsn       | rij                                    |
      | 999993653 | {"a": "2025-06-01", "b": "2025-01-31"} |
    When I evaluate outputs "groter, kleiner_of_gelijk, kleiner, groter_of_gelijk" of "test_signaturen"
    Then the execution succeeds
    Then output "groter" is true
    Then output "kleiner_of_gelijk" is false
    Then output "kleiner" is false
    Then output "groter_of_gelijk" is true

  Scenario: An ordering of two equal numbers
    Given the following "register" data with key "bsn" for law "test_signaturen":
      | bsn       | rij              |
      | 999993653 | {"a": 5, "b": 5} |
    When I evaluate outputs "groter, kleiner_of_gelijk, kleiner, groter_of_gelijk" of "test_signaturen"
    Then the execution succeeds
    Then output "groter" is false
    Then output "kleiner_of_gelijk" is true
    Then output "kleiner" is false
    Then output "groter_of_gelijk" is true

  Scenario: An ordering of two strings that are no dates is refused
    Given the following "register" data with key "bsn" for law "test_signaturen":
      | bsn       | rij                          |
      | 999993653 | {"a": "appel", "b": "peer"} |
    When I evaluate "groter" of "test_signaturen"
    Then the execution fails with "GREATER_THAN is not defined for (string, string)"

  Scenario: An ordering of a number and a boolean is refused
    Given the following "register" data with key "bsn" for law "test_signaturen":
      | bsn       | rij                 |
      | 999993653 | {"a": 7, "b": true} |
    When I evaluate "groter" of "test_signaturen"
    Then the execution fails with "GREATER_THAN is not defined for (number, boolean)"

  Scenario: LESS_THAN of a string and a number is refused
    Given the following "register" data with key "bsn" for law "test_signaturen":
      | bsn       | rij                    |
      | 999993653 | {"a": "appel", "b": 5} |
    When I evaluate "kleiner" of "test_signaturen"
    Then the execution fails with "LESS_THAN is not defined for (string, number)"

  Scenario: GREATER_THAN_OR_EQUAL of two arrays is refused
    Given the following "register" data with key "bsn" for law "test_signaturen":
      | bsn       | rij                    |
      | 999993653 | {"a": [1], "b": [2]} |
    When I evaluate "groter_of_gelijk" of "test_signaturen"
    Then the execution fails with "GREATER_THAN_OR_EQUAL is not defined for (array, array)"

  Scenario: An ordering of two booleans is refused
    Given the following "register" data with key "bsn" for law "test_signaturen":
      | bsn       | rij                      |
      | 999993653 | {"a": true, "b": false} |
    When I evaluate "kleiner_of_gelijk" of "test_signaturen"
    Then the execution fails with "LESS_THAN_OR_EQUAL is not defined for (boolean, boolean)"

  # ---------------------------------------------------------------------------
  # AND, OR, NOT and the condition of an IF: booleans. A count is no verdict.
  # ---------------------------------------------------------------------------

  @signature:AND:boolean @signature:OR:boolean
  Scenario: AND and OR take booleans
    Given the following "register" data with key "bsn" for law "test_signaturen":
      | bsn       | rij                      |
      | 999993653 | {"a": true, "b": false} |
    When I evaluate outputs "beide, een_van_beide" of "test_signaturen"
    Then the execution succeeds
    Then output "beide" is false
    Then output "een_van_beide" is true

  Scenario: AND refuses a number as a condition
    Given the following "register" data with key "bsn" for law "test_signaturen":
      | bsn       | rij                 |
      | 999993653 | {"a": true, "b": 1} |
    When I evaluate "beide" of "test_signaturen"
    Then the execution fails with "AND is not defined for (number)"

  Scenario: AND stops at a false, so a later condition is not looked at
    Given the following "register" data with key "bsn" for law "test_signaturen":
      | bsn       | rij                  |
      | 999993653 | {"a": false, "b": 1} |
    When I evaluate "beide" of "test_signaturen"
    Then the execution succeeds
    Then output "beide" is false

  Scenario: OR refuses a string as a condition
    Given the following "register" data with key "bsn" for law "test_signaturen":
      | bsn       | rij                      |
      | 999993653 | {"a": false, "b": "ja"} |
    When I evaluate "een_van_beide" of "test_signaturen"
    Then the execution fails with "OR is not defined for (string)"

  Scenario: OR stops at a true, so a later condition is not looked at
    Given the following "register" data with key "bsn" for law "test_signaturen":
      | bsn       | rij                     |
      | 999993653 | {"a": true, "b": "ja"} |
    When I evaluate "een_van_beide" of "test_signaturen"
    Then the execution succeeds
    Then output "een_van_beide" is true

  @signature:NOT:boolean
  Scenario: NOT turns a boolean around
    Given the following "register" data with key "bsn" for law "test_signaturen":
      | bsn       | rij         |
      | 999993653 | {"a": true} |
    When I evaluate "niet" of "test_signaturen"
    Then the execution succeeds
    Then output "niet" is false

  Scenario: NOT refuses zero, which is no false
    Given the following "register" data with key "bsn" for law "test_signaturen":
      | bsn       | rij      |
      | 999993653 | {"a": 0} |
    When I evaluate "niet" of "test_signaturen"
    Then the execution fails with "NOT is not defined for (number)"

  Scenario: NOT refuses an empty string, which is no false
    Given the following "register" data with key "bsn" for law "test_signaturen":
      | bsn       | rij       |
      | 999993653 | {"a": ""} |
    When I evaluate "niet" of "test_signaturen"
    Then the execution fails with "NOT is not defined for (string)"

  @signature:IF:cases
  Scenario: IF takes the case whose condition is true
    Given the following "register" data with key "bsn" for law "test_signaturen":
      | bsn       | rij         |
      | 999993653 | {"a": true} |
    When I evaluate "keuze" of "test_signaturen"
    Then the execution succeeds
    Then output "keuze" equals "ja"

  Scenario: IF takes the default when no condition is true
    Given the following "register" data with key "bsn" for law "test_signaturen":
      | bsn       | rij          |
      | 999993653 | {"a": false} |
    When I evaluate "keuze" of "test_signaturen"
    Then the execution succeeds
    Then output "keuze" equals "nee"

  Scenario: IF refuses a number as a condition
    Given the following "register" data with key "bsn" for law "test_signaturen":
      | bsn       | rij      |
      | 999993653 | {"a": 1} |
    When I evaluate "keuze" of "test_signaturen"
    Then the execution fails with "IF is not defined for (number)"

  Scenario: IF refuses an empty list as a condition
    Given the following "register" data with key "bsn" for law "test_signaturen":
      | bsn       | rij       |
      | 999993653 | {"a": []} |
    When I evaluate "keuze" of "test_signaturen"
    Then the execution fails with "IF is not defined for (array)"

  # ---------------------------------------------------------------------------
  # EQUALS on lists and records: element by element. Different shapes are
  # unequal; a list against a record is two types.
  # ---------------------------------------------------------------------------

  Scenario: EQUALS of two equal lists is true
    Given the following "register" data with key "bsn" for law "test_signaturen":
      | bsn       | rij                            |
      | 999993653 | {"a": [1, "x"], "b": [1, "x"]} |
    When I evaluate "gelijk" of "test_signaturen"
    Then the execution succeeds
    Then output "gelijk" is true

  Scenario: EQUALS of two lists of different length is false
    Given the following "register" data with key "bsn" for law "test_signaturen":
      | bsn       | rij                     |
      | 999993653 | {"a": [1], "b": [1, 2]} |
    When I evaluate "gelijk" of "test_signaturen"
    Then the execution succeeds
    Then output "gelijk" is false

  Scenario: EQUALS of two records with different fields is false
    Given the following "register" data with key "bsn" for law "test_signaturen":
      | bsn       | rij                            |
      | 999993653 | {"a": {"x": 1}, "b": {"y": 1}} |
    When I evaluate "gelijk" of "test_signaturen"
    Then the execution succeeds
    Then output "gelijk" is false

  Scenario: EQUALS of a list and a record is refused
    Given the following "register" data with key "bsn" for law "test_signaturen":
      | bsn       | rij                       |
      | 999993653 | {"a": [1], "b": {"x": 1}} |
    When I evaluate "gelijk" of "test_signaturen"
    Then the execution fails with "EQUALS is not defined for (array, object)"

  # ---------------------------------------------------------------------------
  # A date makes a date of what stands beside it: a string in the form of a
  # date is that day, and any other string is no date.
  # ---------------------------------------------------------------------------

  Scenario: EQUALS of the reference date and the same day written as text is true
    Given the following "register" data with key "bsn" for law "test_signaturen":
      | bsn       | rij                 |
      | 999993653 | {"a": "2025-01-01"} |
    When I evaluate "is_peildatum" of "test_signaturen"
    Then the execution succeeds
    Then output "is_peildatum" is true

  Scenario: EQUALS of the reference date and another day is false
    Given the following "register" data with key "bsn" for law "test_signaturen":
      | bsn       | rij                 |
      | 999993653 | {"a": "2025-01-02"} |
    When I evaluate "is_peildatum" of "test_signaturen"
    Then the execution succeeds
    Then output "is_peildatum" is false

  Scenario: EQUALS of the reference date and a text that is no date is refused
    Given the following "register" data with key "bsn" for law "test_signaturen":
      | bsn       | rij             |
      | 999993653 | {"a": "morgen"} |
    When I evaluate "is_peildatum" of "test_signaturen"
    Then the execution fails with "it is not a date in the form YYYY-MM-DD"

  Scenario: EQUALS of the reference date and a day that does not exist is refused
    Given the following "register" data with key "bsn" for law "test_signaturen":
      | bsn       | rij                 |
      | 999993653 | {"a": "2025-02-30"} |
    When I evaluate "is_peildatum" of "test_signaturen"
    Then the execution fails with "it is not a date in the form YYYY-MM-DD"

  Scenario: EQUALS of the reference date and a number is refused
    Given the following "register" data with key "bsn" for law "test_signaturen":
      | bsn       | rij             |
      | 999993653 | {"a": 20250101} |
    When I evaluate "is_peildatum" of "test_signaturen"
    Then the execution fails with "EQUALS is not defined for (number, date)"

  Scenario: An ordering against the reference date reads a day written as text
    Given the following "register" data with key "bsn" for law "test_signaturen":
      | bsn       | rij                 |
      | 999993653 | {"a": "2024-12-31"} |
    When I evaluate "voor_peildatum" of "test_signaturen"
    Then the execution succeeds
    Then output "voor_peildatum" is true

  Scenario: An ordering of a text that is no date against the reference date is refused
    Given the following "register" data with key "bsn" for law "test_signaturen":
      | bsn       | rij               |
      | 999993653 | {"a": "gisteren"} |
    When I evaluate "voor_peildatum" of "test_signaturen"
    Then the execution fails with "it is not a date in the form YYYY-MM-DD"

  Scenario: An ordering of a number against the reference date is refused
    Given the following "register" data with key "bsn" for law "test_signaturen":
      | bsn       | rij         |
      | 999993653 | {"a": 2024} |
    When I evaluate "voor_peildatum" of "test_signaturen"
    Then the execution fails with "LESS_THAN is not defined for (number, date)"

  # ---------------------------------------------------------------------------
  # A FOREACH filter and the combines AND and OR take booleans as well.
  # ---------------------------------------------------------------------------

  @signature:FOREACH-filter:boolean
  Scenario: A FOREACH filter keeps the elements whose verdict is true
    Given the following "register" data with key "bsn" for law "test_signaturen":
      | bsn       | rij                                                                    |
      | 999993653 | {"rijen": [{"keur": true, "waarde": 5}, {"keur": false, "waarde": 7}]} |
    When I evaluate "som_van_gekeurde" of "test_signaturen"
    Then the execution succeeds
    Then output "som_van_gekeurde" equals 5

  Scenario: A FOREACH filter refuses a count as a verdict
    Given the following "register" data with key "bsn" for law "test_signaturen":
      | bsn       | rij                                   |
      | 999993653 | {"rijen": [{"keur": 1, "waarde": 5}]} |
    When I evaluate "som_van_gekeurde" of "test_signaturen"
    Then the execution fails with "FOREACH filter is not defined for (number)"

  @signature:combine-AND:boolean @signature:combine-OR:boolean
  Scenario: combine AND and OR reduce booleans
    Given the following "register" data with key "bsn" for law "test_signaturen":
      | bsn       | rij                                          |
      | 999993653 | {"rijen": [{"keur": true}, {"keur": false}]} |
    When I evaluate outputs "allen_gekeurd, een_gekeurd" of "test_signaturen"
    Then the execution succeeds
    Then output "allen_gekeurd" is false
    Then output "een_gekeurd" is true

  Scenario: combine AND refuses a string as a verdict
    Given the following "register" data with key "bsn" for law "test_signaturen":
      | bsn       | rij                         |
      | 999993653 | {"rijen": [{"keur": "ja"}]} |
    When I evaluate "allen_gekeurd" of "test_signaturen"
    Then the execution fails with "FOREACH combine AND is not defined for (string)"

  Scenario: combine OR refuses a number as a verdict
    Given the following "register" data with key "bsn" for law "test_signaturen":
      | bsn       | rij                      |
      | 999993653 | {"rijen": [{"keur": 0}]} |
    When I evaluate "een_gekeurd" of "test_signaturen"
    Then the execution fails with "FOREACH combine OR is not defined for (number)"

  # ---------------------------------------------------------------------------
  # An unknown operand is dealt with before the types are looked at.
  # ---------------------------------------------------------------------------

  Scenario: ADD of an unknown value and a string is unknown, not a type error
    Given the following "register" data with key "bsn" for law "test_signaturen":
      | bsn       | rij            |
      | 999993653 | {"a": "tekst"} |
    When I evaluate "met_optioneel" of "test_signaturen"
    Then the execution succeeds
    Then output "met_optioneel" is unknown

  Scenario: ADD of a known number and a string is still refused
    Given parameter "optioneel" is 5
    Given the following "register" data with key "bsn" for law "test_signaturen":
      | bsn       | rij            |
      | 999993653 | {"a": "tekst"} |
    When I evaluate "met_optioneel" of "test_signaturen"
    Then the execution fails with "ADD is not defined for (number, string)"
