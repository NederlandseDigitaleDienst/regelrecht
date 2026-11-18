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
      | bsn         | rij              |
      | "999993653" | {"a": 2, "b": 3} |
    When I evaluate "som" of "test_signaturen"
    Then the execution succeeds
    Then output "som" equals 5

  @signature:ADD:string
  Scenario: ADD of two strings joins them
    Given the following "register" data with key "bsn" for law "test_signaturen":
      | bsn         | rij                        |
      | "999993653" | {"a": "af", "b": "spraak"} |
    When I evaluate "tekst" of "test_signaturen"
    Then the execution succeeds
    Then output "tekst" equals "afspraak"

  @signature:ADD:array
  Scenario: ADD of two arrays joins them in order
    Given the following "register" data with key "bsn" for law "test_signaturen":
      | bsn         | rij                                                        |
      | "999993653" | {"a": ["x", "y"], "b": ["z"], "verwacht": ["x", "y", "z"]} |
    When I evaluate "lijst_klopt" of "test_signaturen"
    Then the execution succeeds
    Then output "lijst_klopt" is true
  Scenario: ADD of two arrays keeps their order
    Given the following "register" data with key "bsn" for law "test_signaturen":
      | bsn         | rij                                                        |
      | "999993653" | {"a": ["x", "y"], "b": ["z"], "verwacht": ["z", "x", "y"]} |
    When I evaluate "lijst_klopt" of "test_signaturen"
    Then the execution succeeds
    Then output "lijst_klopt" is false

  Scenario: ADD of a number and a string is refused
    Given the following "register" data with key "bsn" for law "test_signaturen":
      | bsn         | rij                |
      | "999993653" | {"a": 2, "b": "3"} |
    When I evaluate "som" of "test_signaturen"
    Then the execution fails with "ADD is not defined for (number, string)"

  Scenario: ADD of a boolean and a number is refused
    Given the following "register" data with key "bsn" for law "test_signaturen":
      | bsn         | rij                 |
      | "999993653" | {"a": true, "b": 1} |
    When I evaluate "som" of "test_signaturen"
    Then the execution fails with "ADD is not defined for (boolean, number)"

  Scenario: ADD of a string and an array is refused
    Given the following "register" data with key "bsn" for law "test_signaturen":
      | bsn         | rij                    |
      | "999993653" | {"a": "x", "b": ["y"]} |
    When I evaluate "lijst" of "test_signaturen"
    Then the execution fails with "ADD is not defined for (string, array)"

  # ---------------------------------------------------------------------------
  # SUBTRACT, MULTIPLY, DIVIDE, MIN, MAX: numbers only.
  # ---------------------------------------------------------------------------

  @signature:SUBTRACT:number @signature:MULTIPLY:number @signature:DIVIDE:number @signature:MAX:number @signature:MIN:number
  Scenario: The other arithmetic takes numbers
    Given the following "register" data with key "bsn" for law "test_signaturen":
      | bsn         | rij               |
      | "999993653" | {"a": 10, "b": 4} |
    When I evaluate outputs "verschil, product, quotient, grootste, kleinste" of "test_signaturen"
    Then the execution succeeds
    Then output "verschil" equals 6
    Then output "product" equals 40
    Then output "quotient" equals 2.5
    Then output "grootste" equals 10
    Then output "kleinste" equals 4

  Scenario: SUBTRACT of two strings is refused
    Given the following "register" data with key "bsn" for law "test_signaturen":
      | bsn         | rij                   |
      | "999993653" | {"a": "10", "b": "4"} |
    When I evaluate "verschil" of "test_signaturen"
    Then the execution fails with "SUBTRACT is not defined for (string, string)"

  Scenario: MULTIPLY of a number and a string is refused
    Given the following "register" data with key "bsn" for law "test_signaturen":
      | bsn         | rij                 |
      | "999993653" | {"a": 10, "b": "4"} |
    When I evaluate "product" of "test_signaturen"
    Then the execution fails with "MULTIPLY is not defined for (number, string)"

  Scenario: DIVIDE of a number and a boolean is refused
    Given the following "register" data with key "bsn" for law "test_signaturen":
      | bsn         | rij                  |
      | "999993653" | {"a": 10, "b": true} |
    When I evaluate "quotient" of "test_signaturen"
    Then the execution fails with "DIVIDE is not defined for (number, boolean)"

  Scenario: Dividing by zero is an error
    Given the following "register" data with key "bsn" for law "test_signaturen":
      | bsn         | rij               |
      | "999993653" | {"a": 10, "b": 0} |
    When I evaluate "quotient" of "test_signaturen"
    Then the execution fails with "Division by zero"

  Scenario: MAX of a number and a string is refused
    Given the following "register" data with key "bsn" for law "test_signaturen":
      | bsn         | rij                 |
      | "999993653" | {"a": 10, "b": "4"} |
    When I evaluate "grootste" of "test_signaturen"
    Then the execution fails with "MAX is not defined for (number, string)"

  Scenario: MIN of two strings is refused
    Given the following "register" data with key "bsn" for law "test_signaturen":
      | bsn         | rij                  |
      | "999993653" | {"a": "a", "b": "b"} |
    When I evaluate "kleinste" of "test_signaturen"
    Then the execution fails with "MIN is not defined for (string, string)"

  # ---------------------------------------------------------------------------
  # No operation rounds: a division is exact.
  # ---------------------------------------------------------------------------

  Scenario: A third times three is one
    Given the following "register" data with key "bsn" for law "test_signaturen":
      | bsn         | rij              |
      | "999993653" | {"a": 1, "b": 3} |
    When I evaluate "terug" of "test_signaturen"
    Then the execution succeeds
    Then output "terug" equals 1

  # ---------------------------------------------------------------------------
  # ROUND: a number. A tie goes away from zero.
  # ---------------------------------------------------------------------------

  @signature:ROUND:number
  Scenario: ROUND rounds a tie away from zero
    Given the following "register" data with key "bsn" for law "test_signaturen":
      | bsn         | rij        |
      | "999993653" | {"a": 2.5} |
    When I evaluate "afgerond" of "test_signaturen"
    Then the execution succeeds
    Then output "afgerond" equals 3

  Scenario: ROUND rounds a negative tie away from zero
    Given the following "register" data with key "bsn" for law "test_signaturen":
      | bsn         | rij         |
      | "999993653" | {"a": -2.5} |
    When I evaluate "afgerond" of "test_signaturen"
    Then the execution succeeds
    Then output "afgerond" equals -3

  @signature:CEIL:number @signature:FLOOR:number
  Scenario: CEIL rounds up and FLOOR rounds down, also below zero
    Given the following "register" data with key "bsn" for law "test_signaturen":
      | bsn         | rij         |
      | "999993653" | {"a": -2.5} |
    When I evaluate outputs "naar_boven, naar_beneden" of "test_signaturen"
    Then the execution succeeds
    Then output "naar_boven" equals -2
    Then output "naar_beneden" equals -3

  Scenario: CEIL of a boolean is refused
    Given the following "register" data with key "bsn" for law "test_signaturen":
      | bsn         | rij         |
      | "999993653" | {"a": true} |
    When I evaluate "naar_boven" of "test_signaturen"
    Then the execution fails with "CEIL is not defined for (boolean)"

  Scenario: FLOOR of a string is refused
    Given the following "register" data with key "bsn" for law "test_signaturen":
      | bsn         | rij          |
      | "999993653" | {"a": "2.5"} |
    When I evaluate "naar_beneden" of "test_signaturen"
    Then the execution fails with "FLOOR is not defined for (string)"

  Scenario: ROUND of a string is refused
    Given the following "register" data with key "bsn" for law "test_signaturen":
      | bsn         | rij          |
      | "999993653" | {"a": "2.5"} |
    When I evaluate "afgerond" of "test_signaturen"
    Then the execution fails with "ROUND is not defined for (string)"

  # ---------------------------------------------------------------------------
  # EQUALS: two values of one type. An absence is tested, never compared.
  # ---------------------------------------------------------------------------

  @signature:EQUALS:alike
  Scenario: EQUALS of two equal numbers is true
    Given the following "register" data with key "bsn" for law "test_signaturen":
      | bsn         | rij              |
      | "999993653" | {"a": 5, "b": 5} |
    When I evaluate "gelijk" of "test_signaturen"
    Then the execution succeeds
    Then output "gelijk" is true

  Scenario: EQUALS of a whole number and the same number with a fraction part is true
    Given the following "register" data with key "bsn" for law "test_signaturen":
      | bsn         | rij                |
      | "999993653" | {"a": 5, "b": 5.0} |
    When I evaluate "gelijk" of "test_signaturen"
    Then the execution succeeds
    Then output "gelijk" is true

  Scenario: EQUALS of two different strings is false
    Given the following "register" data with key "bsn" for law "test_signaturen":
      | bsn         | rij                     |
      | "999993653" | {"a": "ja", "b": "nee"} |
    When I evaluate "gelijk" of "test_signaturen"
    Then the execution succeeds
    Then output "gelijk" is false

  Scenario: EQUALS of two booleans compares them
    Given the following "register" data with key "bsn" for law "test_signaturen":
      | bsn         | rij                    |
      | "999993653" | {"a": true, "b": true} |
    When I evaluate "gelijk" of "test_signaturen"
    Then the execution succeeds
    Then output "gelijk" is true

  Scenario: EQUALS of a number and a string is refused, not false
    Given the following "register" data with key "bsn" for law "test_signaturen":
      | bsn         | rij                |
      | "999993653" | {"a": 5, "b": "5"} |
    When I evaluate "gelijk" of "test_signaturen"
    Then the execution fails with "EQUALS is not defined for (number, string)"

  Scenario: EQUALS of a boolean and a number is refused
    Given the following "register" data with key "bsn" for law "test_signaturen":
      | bsn         | rij                 |
      | "999993653" | {"a": true, "b": 1} |
    When I evaluate "gelijk" of "test_signaturen"
    Then the execution fails with "EQUALS is not defined for (boolean, number)"

  Scenario: EQUALS of an absence and a value is false
    Given the following "register" data with key "bsn" for law "test_signaturen":
      | bsn         | rij                 |
      | "999993653" | {"a": null, "b": 5} |
    When I evaluate "gelijk" of "test_signaturen"
    Then the execution succeeds
    Then output "gelijk" is false

  Scenario: EQUALS of two absences is true
    Given the following "register" data with key "bsn" for law "test_signaturen":
      | bsn         | rij                    |
      | "999993653" | {"a": null, "b": null} |
    When I evaluate "gelijk" of "test_signaturen"
    Then the execution succeeds
    Then output "gelijk" is true

  # ---------------------------------------------------------------------------
  # Orderings: two numbers, or two dates.
  # ---------------------------------------------------------------------------

  @signature:GREATER_THAN:number-number @signature:LESS_THAN:number-number @signature:GREATER_THAN_OR_EQUAL:number-number @signature:LESS_THAN_OR_EQUAL:number-number
  Scenario: An ordering compares two numbers
    Given the following "register" data with key "bsn" for law "test_signaturen":
      | bsn         | rij              |
      | "999993653" | {"a": 7, "b": 5} |
    When I evaluate outputs "groter, kleiner_of_gelijk, kleiner, groter_of_gelijk" of "test_signaturen"
    Then the execution succeeds
    Then output "groter" is true
    Then output "kleiner_of_gelijk" is false
    Then output "kleiner" is false
    Then output "groter_of_gelijk" is true

  @signature:GREATER_THAN:date-date @signature:LESS_THAN:date-date @signature:GREATER_THAN_OR_EQUAL:date-date @signature:LESS_THAN_OR_EQUAL:date-date
  Scenario: An ordering compares two dates, the later one being greater
    Given the following "register" data with key "bsn" for law "test_signaturen":
      | bsn         | rij                                    |
      | "999993653" | {"a": "2025-06-01", "b": "2025-01-31"} |
    When I evaluate outputs "groter, kleiner_of_gelijk, kleiner, groter_of_gelijk" of "test_signaturen"
    Then the execution succeeds
    Then output "groter" is true
    Then output "kleiner_of_gelijk" is false
    Then output "kleiner" is false
    Then output "groter_of_gelijk" is true

  Scenario: An ordering of two equal numbers
    Given the following "register" data with key "bsn" for law "test_signaturen":
      | bsn         | rij              |
      | "999993653" | {"a": 5, "b": 5} |
    When I evaluate outputs "groter, kleiner_of_gelijk, kleiner, groter_of_gelijk" of "test_signaturen"
    Then the execution succeeds
    Then output "groter" is false
    Then output "kleiner_of_gelijk" is true
    Then output "kleiner" is false
    Then output "groter_of_gelijk" is true

  Scenario: An ordering of two strings that are no dates is refused
    Given the following "register" data with key "bsn" for law "test_signaturen":
      | bsn         | rij                         |
      | "999993653" | {"a": "appel", "b": "peer"} |
    When I evaluate "groter" of "test_signaturen"
    Then the execution fails with "GREATER_THAN is not defined for (string, string)"

  Scenario: An ordering of a number and a boolean is refused
    Given the following "register" data with key "bsn" for law "test_signaturen":
      | bsn         | rij                 |
      | "999993653" | {"a": 7, "b": true} |
    When I evaluate "groter" of "test_signaturen"
    Then the execution fails with "GREATER_THAN is not defined for (number, boolean)"

  Scenario: LESS_THAN of a string and a number is refused
    Given the following "register" data with key "bsn" for law "test_signaturen":
      | bsn         | rij                    |
      | "999993653" | {"a": "appel", "b": 5} |
    When I evaluate "kleiner" of "test_signaturen"
    Then the execution fails with "LESS_THAN is not defined for (string, number)"

  Scenario: GREATER_THAN_OR_EQUAL of two arrays is refused
    Given the following "register" data with key "bsn" for law "test_signaturen":
      | bsn         | rij                  |
      | "999993653" | {"a": [1], "b": [2]} |
    When I evaluate "groter_of_gelijk" of "test_signaturen"
    Then the execution fails with "GREATER_THAN_OR_EQUAL is not defined for (array, array)"

  Scenario: An ordering of two booleans is refused
    Given the following "register" data with key "bsn" for law "test_signaturen":
      | bsn         | rij                     |
      | "999993653" | {"a": true, "b": false} |
    When I evaluate "kleiner_of_gelijk" of "test_signaturen"
    Then the execution fails with "LESS_THAN_OR_EQUAL is not defined for (boolean, boolean)"

  # ---------------------------------------------------------------------------
  # AND, OR, NOT and the condition of an IF: booleans. A count is no verdict.
  # ---------------------------------------------------------------------------

  @signature:AND:boolean @signature:OR:boolean
  Scenario: AND and OR take booleans
    Given the following "register" data with key "bsn" for law "test_signaturen":
      | bsn         | rij                     |
      | "999993653" | {"a": true, "b": false} |
    When I evaluate outputs "beide, een_van_beide" of "test_signaturen"
    Then the execution succeeds
    Then output "beide" is false
    Then output "een_van_beide" is true

  Scenario: AND refuses a number as a condition
    Given the following "register" data with key "bsn" for law "test_signaturen":
      | bsn         | rij                 |
      | "999993653" | {"a": true, "b": 1} |
    When I evaluate "beide" of "test_signaturen"
    Then the execution fails with "AND is not defined for (number)"

  Scenario: AND stops at a false, so a later condition is not looked at
    Given the following "register" data with key "bsn" for law "test_signaturen":
      | bsn         | rij                  |
      | "999993653" | {"a": false, "b": 1} |
    When I evaluate "beide" of "test_signaturen"
    Then the execution succeeds
    Then output "beide" is false

  Scenario: OR refuses a string as a condition
    Given the following "register" data with key "bsn" for law "test_signaturen":
      | bsn         | rij                     |
      | "999993653" | {"a": false, "b": "ja"} |
    When I evaluate "een_van_beide" of "test_signaturen"
    Then the execution fails with "OR is not defined for (string)"

  Scenario: OR stops at a true, so a later condition is not looked at
    Given the following "register" data with key "bsn" for law "test_signaturen":
      | bsn         | rij                    |
      | "999993653" | {"a": true, "b": "ja"} |
    When I evaluate "een_van_beide" of "test_signaturen"
    Then the execution succeeds
    Then output "een_van_beide" is true

  @signature:NOT:boolean
  Scenario: NOT turns a boolean around
    Given the following "register" data with key "bsn" for law "test_signaturen":
      | bsn         | rij         |
      | "999993653" | {"a": true} |
    When I evaluate "niet" of "test_signaturen"
    Then the execution succeeds
    Then output "niet" is false

  Scenario: NOT refuses zero, which is no false
    Given the following "register" data with key "bsn" for law "test_signaturen":
      | bsn         | rij      |
      | "999993653" | {"a": 0} |
    When I evaluate "niet" of "test_signaturen"
    Then the execution fails with "NOT is not defined for (number)"

  Scenario: NOT refuses an empty string, which is no false
    Given the following "register" data with key "bsn" for law "test_signaturen":
      | bsn         | rij       |
      | "999993653" | {"a": ""} |
    When I evaluate "niet" of "test_signaturen"
    Then the execution fails with "NOT is not defined for (string)"

  @signature:IF:cases
  Scenario: IF takes the case whose condition is true
    Given the following "register" data with key "bsn" for law "test_signaturen":
      | bsn         | rij         |
      | "999993653" | {"a": true} |
    When I evaluate "keuze" of "test_signaturen"
    Then the execution succeeds
    Then output "keuze" equals "ja"

  Scenario: IF takes the default when no condition is true
    Given the following "register" data with key "bsn" for law "test_signaturen":
      | bsn         | rij          |
      | "999993653" | {"a": false} |
    When I evaluate "keuze" of "test_signaturen"
    Then the execution succeeds
    Then output "keuze" equals "nee"

  Scenario: IF refuses a number as a condition
    Given the following "register" data with key "bsn" for law "test_signaturen":
      | bsn         | rij      |
      | "999993653" | {"a": 1} |
    When I evaluate "keuze" of "test_signaturen"
    Then the execution fails with "IF is not defined for (number)"

  Scenario: IF refuses an empty list as a condition
    Given the following "register" data with key "bsn" for law "test_signaturen":
      | bsn         | rij       |
      | "999993653" | {"a": []} |
    When I evaluate "keuze" of "test_signaturen"
    Then the execution fails with "IF is not defined for (array)"

  # ---------------------------------------------------------------------------
  # EQUALS on lists and records: element by element. Different shapes are
  # unequal; a list against a record is two types.
  # ---------------------------------------------------------------------------

  Scenario: EQUALS of two equal lists is true
    Given the following "register" data with key "bsn" for law "test_signaturen":
      | bsn         | rij                            |
      | "999993653" | {"a": [1, "x"], "b": [1, "x"]} |
    When I evaluate "gelijk" of "test_signaturen"
    Then the execution succeeds
    Then output "gelijk" is true

  Scenario: EQUALS of two lists of different length is false
    Given the following "register" data with key "bsn" for law "test_signaturen":
      | bsn         | rij                     |
      | "999993653" | {"a": [1], "b": [1, 2]} |
    When I evaluate "gelijk" of "test_signaturen"
    Then the execution succeeds
    Then output "gelijk" is false

  Scenario: EQUALS of two records with different fields is false
    Given the following "register" data with key "bsn" for law "test_signaturen":
      | bsn         | rij                            |
      | "999993653" | {"a": {"x": 1}, "b": {"y": 1}} |
    When I evaluate "gelijk" of "test_signaturen"
    Then the execution succeeds
    Then output "gelijk" is false

  Scenario: EQUALS of a list and a record is refused
    Given the following "register" data with key "bsn" for law "test_signaturen":
      | bsn         | rij                       |
      | "999993653" | {"a": [1], "b": {"x": 1}} |
    When I evaluate "gelijk" of "test_signaturen"
    Then the execution fails with "EQUALS is not defined for (array, object)"

  # ---------------------------------------------------------------------------
  # A date makes a date of what stands beside it: a string in the form of a
  # date is that day, and any other string is no date.
  # ---------------------------------------------------------------------------

  Scenario: EQUALS of the reference date and the same day written as text is true
    Given the following "register" data with key "bsn" for law "test_signaturen":
      | bsn         | rij                 |
      | "999993653" | {"a": "2025-01-01"} |
    When I evaluate "is_peildatum" of "test_signaturen"
    Then the execution succeeds
    Then output "is_peildatum" is true

  Scenario: EQUALS of the reference date and another day is false
    Given the following "register" data with key "bsn" for law "test_signaturen":
      | bsn         | rij                 |
      | "999993653" | {"a": "2025-01-02"} |
    When I evaluate "is_peildatum" of "test_signaturen"
    Then the execution succeeds
    Then output "is_peildatum" is false

  Scenario: EQUALS of the reference date and a text that is no date is refused
    Given the following "register" data with key "bsn" for law "test_signaturen":
      | bsn         | rij             |
      | "999993653" | {"a": "morgen"} |
    When I evaluate "is_peildatum" of "test_signaturen"
    Then the execution fails with "it is not a date in the form YYYY-MM-DD"

  Scenario: EQUALS of the reference date and a day that does not exist is refused
    Given the following "register" data with key "bsn" for law "test_signaturen":
      | bsn         | rij                 |
      | "999993653" | {"a": "2025-02-30"} |
    When I evaluate "is_peildatum" of "test_signaturen"
    Then the execution fails with "it is not a date in the form YYYY-MM-DD"

  Scenario: EQUALS of the reference date and a number is refused
    Given the following "register" data with key "bsn" for law "test_signaturen":
      | bsn         | rij             |
      | "999993653" | {"a": 20250101} |
    When I evaluate "is_peildatum" of "test_signaturen"
    Then the execution fails with "EQUALS is not defined for (number, date)"

  Scenario: An ordering against the reference date reads a day written as text
    Given the following "register" data with key "bsn" for law "test_signaturen":
      | bsn         | rij                 |
      | "999993653" | {"a": "2024-12-31"} |
    When I evaluate "voor_peildatum" of "test_signaturen"
    Then the execution succeeds
    Then output "voor_peildatum" is true

  Scenario: An ordering of a text that is no date against the reference date is refused
    Given the following "register" data with key "bsn" for law "test_signaturen":
      | bsn         | rij               |
      | "999993653" | {"a": "gisteren"} |
    When I evaluate "voor_peildatum" of "test_signaturen"
    Then the execution fails with "it is not a date in the form YYYY-MM-DD"

  Scenario: An ordering of a number against the reference date is refused
    Given the following "register" data with key "bsn" for law "test_signaturen":
      | bsn         | rij         |
      | "999993653" | {"a": 2024} |
    When I evaluate "voor_peildatum" of "test_signaturen"
    Then the execution fails with "LESS_THAN is not defined for (number, date)"

  # ---------------------------------------------------------------------------
  # A FOREACH filter and the combines AND and OR take booleans as well.
  # ---------------------------------------------------------------------------

  @signature:FOREACH-filter:boolean
  Scenario: A FOREACH filter keeps the elements whose verdict is true
    Given the following "register" data with key "bsn" for law "test_signaturen":
      | bsn         | rij                                                                    |
      | "999993653" | {"rijen": [{"keur": true, "waarde": 5}, {"keur": false, "waarde": 7}]} |
    When I evaluate "som_van_gekeurde" of "test_signaturen"
    Then the execution succeeds
    Then output "som_van_gekeurde" equals 5

  Scenario: A FOREACH filter refuses a count as a verdict
    Given the following "register" data with key "bsn" for law "test_signaturen":
      | bsn         | rij                                   |
      | "999993653" | {"rijen": [{"keur": 1, "waarde": 5}]} |
    When I evaluate "som_van_gekeurde" of "test_signaturen"
    Then the execution fails with "FOREACH filter is not defined for (number)"

  @signature:combine-AND:boolean @signature:combine-OR:boolean
  Scenario: combine AND and OR reduce booleans
    Given the following "register" data with key "bsn" for law "test_signaturen":
      | bsn         | rij                                          |
      | "999993653" | {"rijen": [{"keur": true}, {"keur": false}]} |
    When I evaluate outputs "allen_gekeurd, een_gekeurd" of "test_signaturen"
    Then the execution succeeds
    Then output "allen_gekeurd" is false
    Then output "een_gekeurd" is true

  Scenario: combine AND refuses a string as a verdict
    Given the following "register" data with key "bsn" for law "test_signaturen":
      | bsn         | rij                         |
      | "999993653" | {"rijen": [{"keur": "ja"}]} |
    When I evaluate "allen_gekeurd" of "test_signaturen"
    Then the execution fails with "FOREACH combine AND is not defined for (string)"

  Scenario: combine OR refuses a number as a verdict
    Given the following "register" data with key "bsn" for law "test_signaturen":
      | bsn         | rij                      |
      | "999993653" | {"rijen": [{"keur": 0}]} |
    When I evaluate "een_gekeurd" of "test_signaturen"
    Then the execution fails with "FOREACH combine OR is not defined for (number)"

  # ---------------------------------------------------------------------------
  # An unknown operand is dealt with before the types are looked at.
  # ---------------------------------------------------------------------------

  Scenario: ADD of an unknown value and a string is unknown, not a type error
    Given the following "register" data with key "bsn" for law "test_signaturen":
      | bsn         | rij            |
      | "999993653" | {"a": "tekst"} |
    When I evaluate "met_optioneel" of "test_signaturen"
    Then the execution succeeds
    Then output "met_optioneel" is unknown

  Scenario: ADD of a known number and a string is still refused
    Given parameter "optioneel" is 5
    Given the following "register" data with key "bsn" for law "test_signaturen":
      | bsn         | rij            |
      | "999993653" | {"a": "tekst"} |
    When I evaluate "met_optioneel" of "test_signaturen"
    Then the execution fails with "ADD is not defined for (number, string)"

  # ---------------------------------------------------------------------------
  # IN: a subject and an array of the same type. Elements are looked at in
  # order, up to the first that equals the subject.
  # ---------------------------------------------------------------------------

  @signature:IN:member-of
  Scenario: IN is true when an element equals the subject
    Given the following "register" data with key "bsn" for law "test_signaturen":
      | bsn         | rij                          |
      | "999993653" | {"a": 5, "lijst": [3, 5, 8]} |
    When I evaluate "komt_voor" of "test_signaturen"
    Then the execution succeeds
    Then output "komt_voor" is true

  Scenario: IN is false when no element equals the subject
    Given the following "register" data with key "bsn" for law "test_signaturen":
      | bsn         | rij                          |
      | "999993653" | {"a": 4, "lijst": [3, 5, 8]} |
    When I evaluate "komt_voor" of "test_signaturen"
    Then the execution succeeds
    Then output "komt_voor" is false

  Scenario: IN over an empty list is false
    Given the following "register" data with key "bsn" for law "test_signaturen":
      | bsn         | rij                   |
      | "999993653" | {"a": 4, "lijst": []} |
    When I evaluate "komt_voor" of "test_signaturen"
    Then the execution succeeds
    Then output "komt_voor" is false

  Scenario: IN over an absent list is false
    Given the following "register" data with key "bsn" for law "test_signaturen":
      | bsn         | rij                     |
      | "999993653" | {"a": 4, "lijst": null} |
    When I evaluate "komt_voor" of "test_signaturen"
    Then the execution succeeds
    Then output "komt_voor" is false

  Scenario: IN finds an absent subject among elements that hold an absence
    Given the following "register" data with key "bsn" for law "test_signaturen":
      | bsn         | rij                             |
      | "999993653" | {"a": null, "lijst": [1, null]} |
    When I evaluate "komt_voor" of "test_signaturen"
    Then the execution succeeds
    Then output "komt_voor" is true

  Scenario: IN does not find an absent subject among values
    Given the following "register" data with key "bsn" for law "test_signaturen":
      | bsn         | rij                          |
      | "999993653" | {"a": null, "lijst": [1, 2]} |
    When I evaluate "komt_voor" of "test_signaturen"
    Then the execution succeeds
    Then output "komt_voor" is false

  Scenario: IN compares as EQUALS does, a whole number equalling the same number with a fraction part
    Given the following "register" data with key "bsn" for law "test_signaturen":
      | bsn         | rij                      |
      | "999993653" | {"a": 5, "lijst": [5.0]} |
    When I evaluate "komt_voor" of "test_signaturen"
    Then the execution succeeds
    Then output "komt_voor" is true

  Scenario: IN refuses an element of another type than the subject
    Given the following "register" data with key "bsn" for law "test_signaturen":
      | bsn         | rij                      |
      | "999993653" | {"a": 5, "lijst": ["5"]} |
    When I evaluate "komt_voor" of "test_signaturen"
    Then the execution fails with "IN is not defined for (number, string)"

  Scenario: IN stops at the first match, so a later element of another type is not looked at
    Given the following "register" data with key "bsn" for law "test_signaturen":
      | bsn         | rij                            |
      | "999993653" | {"a": 5, "lijst": [5, "vijf"]} |
    When I evaluate "komt_voor" of "test_signaturen"
    Then the execution succeeds
    Then output "komt_voor" is true

  Scenario: IN reaches an element of another type before a match and refuses it
    Given the following "register" data with key "bsn" for law "test_signaturen":
      | bsn         | rij                            |
      | "999993653" | {"a": 5, "lijst": ["vijf", 5]} |
    When I evaluate "komt_voor" of "test_signaturen"
    Then the execution fails with "IN is not defined for (number, string)"

  Scenario: IN refuses a value that is no list
    Given the following "register" data with key "bsn" for law "test_signaturen":
      | bsn         | rij                  |
      | "999993653" | {"a": 5, "lijst": 5} |
    When I evaluate "komt_voor" of "test_signaturen"
    Then the execution fails with "IN is not defined for (number, number)"

  # ---------------------------------------------------------------------------
  # LIST and RECORD build a value from values of any type.
  # ---------------------------------------------------------------------------

  @signature:LIST:values
  Scenario: LIST keeps its items in order, whatever their type
    Given the following "register" data with key "bsn" for law "test_signaturen":
      | bsn         | rij                                            |
      | "999993653" | {"a": 1, "b": "twee", "verwacht": [1, "twee"]} |
    When I evaluate "lijst_van_twee_klopt" of "test_signaturen"
    Then the execution succeeds
    Then output "lijst_van_twee_klopt" is true

  Scenario: LIST keeps an absent item as an element
    Given the following "register" data with key "bsn" for law "test_signaturen":
      | bsn         | rij                                        |
      | "999993653" | {"a": 1, "b": null, "verwacht": [1, null]} |
    When I evaluate "lijst_van_twee_klopt" of "test_signaturen"
    Then the execution succeeds
    Then output "lijst_van_twee_klopt" is true

  Scenario: LIST with its items the other way round is another list
    Given the following "register" data with key "bsn" for law "test_signaturen":
      | bsn         | rij                                            |
      | "999993653" | {"a": 1, "b": "twee", "verwacht": ["twee", 1]} |
    When I evaluate "lijst_van_twee_klopt" of "test_signaturen"
    Then the execution succeeds
    Then output "lijst_van_twee_klopt" is false

  @signature:RECORD:values
  Scenario: RECORD gives an object with a field for each name
    Given the following "register" data with key "bsn" for law "test_signaturen":
      | bsn         | rij                                                                |
      | "999993653" | {"a": 1, "b": "twee", "verwacht": {"eerste": 1, "tweede": "twee"}} |
    When I evaluate "record_klopt" of "test_signaturen"
    Then the execution succeeds
    Then output "record_klopt" is true

  Scenario: RECORD keeps an absent value in its field
    Given the following "register" data with key "bsn" for law "test_signaturen":
      | bsn         | rij                                                            |
      | "999993653" | {"a": 1, "b": null, "verwacht": {"eerste": 1, "tweede": null}} |
    When I evaluate "record_klopt" of "test_signaturen"
    Then the execution succeeds
    Then output "record_klopt" is true

  # ---------------------------------------------------------------------------
  # FOREACH: an array, and what combine makes of its results.
  # ---------------------------------------------------------------------------

  @signature:FOREACH:collection
  Scenario: FOREACH without combine gives the results in order
    Given the following "register" data with key "bsn" for law "test_signaturen":
      | bsn         | rij                                                           |
      | "999993653" | {"rijen": [{"waarde": 3}, {"waarde": 5}], "verwacht": [3, 5]} |
    When I evaluate "waarden_kloppen" of "test_signaturen"
    Then the execution succeeds
    Then output "waarden_kloppen" is true

  Scenario: FOREACH without combine keeps an absent result as an element
    Given the following "register" data with key "bsn" for law "test_signaturen":
      | bsn         | rij                                                                 |
      | "999993653" | {"rijen": [{"waarde": 3}, {"waarde": null}], "verwacht": [3, null]} |
    When I evaluate "waarden_kloppen" of "test_signaturen"
    Then the execution succeeds
    Then output "waarden_kloppen" is true

  Scenario: FOREACH over an absent collection iterates nothing
    Given the following "register" data with key "bsn" for law "test_signaturen":
      | bsn         | rij                             |
      | "999993653" | {"rijen": null, "verwacht": []} |
    When I evaluate "waarden_kloppen" of "test_signaturen"
    Then the execution succeeds
    Then output "waarden_kloppen" is true

  Scenario: FOREACH over a value that is no array iterates once over it
    Given the following "register" data with key "bsn" for law "test_signaturen":
      | bsn         | rij                      |
      | "999993653" | {"rijen": {"waarde": 7}} |
    When I evaluate "som_van_alle" of "test_signaturen"
    Then the execution succeeds
    Then output "som_van_alle" equals 7

  @signature:combine-ADD:number
  Scenario: combine ADD sums numbers
    Given the following "register" data with key "bsn" for law "test_signaturen":
      | bsn         | rij                                       |
      | "999993653" | {"rijen": [{"waarde": 3}, {"waarde": 5}]} |
    When I evaluate "som_van_alle" of "test_signaturen"
    Then the execution succeeds
    Then output "som_van_alle" equals 8

  Scenario: combine ADD over no results is zero
    Given the following "register" data with key "bsn" for law "test_signaturen":
      | bsn         | rij           |
      | "999993653" | {"rijen": []} |
    When I evaluate "som_van_alle" of "test_signaturen"
    Then the execution succeeds
    Then output "som_van_alle" equals 0

  Scenario: combine ADD does not join strings
    Given the following "register" data with key "bsn" for law "test_signaturen":
      | bsn         | rij                                           |
      | "999993653" | {"rijen": [{"waarde": "a"}, {"waarde": "b"}]} |
    When I evaluate "som_van_alle" of "test_signaturen"
    Then the execution fails with "FOREACH combine ADD is not defined for (string, string)"

  Scenario: combine ADD does not join lists
    Given the following "register" data with key "bsn" for law "test_signaturen":
      | bsn         | rij                                           |
      | "999993653" | {"rijen": [{"waarde": [1]}, {"waarde": [2]}]} |
    When I evaluate "som_van_alle" of "test_signaturen"
    Then the execution fails with "FOREACH combine ADD is not defined for (array, array)"

  @signature:combine-MIN:number
  Scenario: combine MIN gives the lowest number
    Given the following "register" data with key "bsn" for law "test_signaturen":
      | bsn         | rij                                       |
      | "999993653" | {"rijen": [{"waarde": 3}, {"waarde": 5}]} |
    When I evaluate "laagste" of "test_signaturen"
    Then the execution succeeds
    Then output "laagste" equals 3

  @signature:combine-MAX:number
  Scenario: combine MAX gives the highest number
    Given the following "register" data with key "bsn" for law "test_signaturen":
      | bsn         | rij                                       |
      | "999993653" | {"rijen": [{"waarde": 3}, {"waarde": 5}]} |
    When I evaluate "hoogste" of "test_signaturen"
    Then the execution succeeds
    Then output "hoogste" equals 5

  Scenario: combine MIN over no results is absent
    Given the following "register" data with key "bsn" for law "test_signaturen":
      | bsn         | rij           |
      | "999993653" | {"rijen": []} |
    When I evaluate "laagste" of "test_signaturen"
    Then the execution succeeds
    Then output "laagste" is absent

  Scenario: combine MIN refuses a string
    Given the following "register" data with key "bsn" for law "test_signaturen":
      | bsn         | rij                          |
      | "999993653" | {"rijen": [{"waarde": "a"}]} |
    When I evaluate "laagste" of "test_signaturen"
    Then the execution fails with "FOREACH combine MIN is not defined for (string)"

  Scenario: combine MAX refuses a boolean
    Given the following "register" data with key "bsn" for law "test_signaturen":
      | bsn         | rij                           |
      | "999993653" | {"rijen": [{"waarde": true}]} |
    When I evaluate "hoogste" of "test_signaturen"
    Then the execution fails with "FOREACH combine MAX is not defined for (boolean)"

  @signature:combine-ONLY:values
  Scenario: combine ONLY gives its one element
    Given the following "register" data with key "bsn" for law "test_signaturen":
      | bsn         | rij                        |
      | "999993653" | {"rijen": [{"waarde": 5}]} |
    When I evaluate "enige" of "test_signaturen"
    Then the execution succeeds
    Then output "enige" equals 5

  Scenario: combine ONLY over no results is absent
    Given the following "register" data with key "bsn" for law "test_signaturen":
      | bsn         | rij           |
      | "999993653" | {"rijen": []} |
    When I evaluate "enige" of "test_signaturen"
    Then the execution succeeds
    Then output "enige" is absent

  Scenario: combine ONLY refuses more than one element
    Given the following "register" data with key "bsn" for law "test_signaturen":
      | bsn         | rij                                       |
      | "999993653" | {"rijen": [{"waarde": 5}, {"waarde": 6}]} |
    When I evaluate "enige" of "test_signaturen"
    Then the execution fails with "ONLY takes at most one"

  @signature:combine-DISTINCT:values
  Scenario: combine DISTINCT gives the elements that differ, in the order each first occurs
    Given the following "register" data with key "bsn" for law "test_signaturen":
      | bsn         | rij                                                                          |
      | "999993653" | {"rijen": [{"waarde": 5}, {"waarde": 3}, {"waarde": 5}], "verwacht": [5, 3]} |
    When I evaluate "soorten_kloppen" of "test_signaturen"
    Then the execution succeeds
    Then output "soorten_kloppen" is true

  Scenario: combine DISTINCT keeps an absent element once
    Given the following "register" data with key "bsn" for law "test_signaturen":
      | bsn         | rij                                                                                   |
      | "999993653" | {"rijen": [{"waarde": null}, {"waarde": 3}, {"waarde": null}], "verwacht": [null, 3]} |
    When I evaluate "soorten_kloppen" of "test_signaturen"
    Then the execution succeeds
    Then output "soorten_kloppen" is true

  Scenario: combine DISTINCT over no results is an empty list
    Given the following "register" data with key "bsn" for law "test_signaturen":
      | bsn         | rij                           |
      | "999993653" | {"rijen": [], "verwacht": []} |
    When I evaluate "soorten_kloppen" of "test_signaturen"
    Then the execution succeeds
    Then output "soorten_kloppen" is true

  Scenario: combine DISTINCT refuses elements of different types
    Given the following "register" data with key "bsn" for law "test_signaturen":
      | bsn         | rij                                                         |
      | "999993653" | {"rijen": [{"waarde": 5}, {"waarde": "5"}], "verwacht": []} |
    When I evaluate "soorten_kloppen" of "test_signaturen"
    Then the execution fails with "FOREACH combine DISTINCT is not defined for (number, string)"

  Scenario: combine AND stops at a false, so a later element is not evaluated
    # The second row has no `keur`: evaluating it would fail.
    Given the following "register" data with key "bsn" for law "test_signaturen":
      | bsn         | rij                                        |
      | "999993653" | {"rijen": [{"keur": false}, {"ander": 1}]} |
    When I evaluate "allen_gekeurd" of "test_signaturen"
    Then the execution succeeds
    Then output "allen_gekeurd" is false

  Scenario: combine AND reaches an element that is no verdict before a false and refuses it
    Given the following "register" data with key "bsn" for law "test_signaturen":
      | bsn         | rij                                       |
      | "999993653" | {"rijen": [{"keur": 1}, {"keur": false}]} |
    When I evaluate "allen_gekeurd" of "test_signaturen"
    Then the execution fails with "FOREACH combine AND is not defined for (number)"

  Scenario: combine OR stops at a true, so a later element is not evaluated
    # The second row has no `keur`: evaluating it would fail.
    Given the following "register" data with key "bsn" for law "test_signaturen":
      | bsn         | rij                                       |
      | "999993653" | {"rijen": [{"keur": true}, {"ander": 1}]} |
    When I evaluate "een_gekeurd" of "test_signaturen"
    Then the execution succeeds
    Then output "een_gekeurd" is true

  Scenario: combine AND over no results is true and combine OR is false
    Given the following "register" data with key "bsn" for law "test_signaturen":
      | bsn         | rij           |
      | "999993653" | {"rijen": []} |
    When I evaluate "allen_gekeurd" of "test_signaturen"
    Then the execution succeeds
    Then output "allen_gekeurd" is true

  # ---------------------------------------------------------------------------
  # Fields: a path leads through records. The field of an absence is absent; a
  # field a record does not have, and a path into another value, are errors.
  # ---------------------------------------------------------------------------

  Scenario: The field of an absent record is absent
    Given the following "register" data with key "bsn" for law "test_signaturen":
      | bsn         | rij         |
      | "999993653" | {"a": null} |
    When I evaluate "onderdeel_ontbreekt" of "test_signaturen"
    Then the execution succeeds
    Then output "onderdeel_ontbreekt" is true

  Scenario: A field that holds an absence is absent
    Given the following "register" data with key "bsn" for law "test_signaturen":
      | bsn         | rij                        |
      | "999993653" | {"a": {"onderdeel": null}} |
    When I evaluate "onderdeel_ontbreekt" of "test_signaturen"
    Then the execution succeeds
    Then output "onderdeel_ontbreekt" is true

  Scenario: A field that holds a value is not absent
    Given the following "register" data with key "bsn" for law "test_signaturen":
      | bsn         | rij                     |
      | "999993653" | {"a": {"onderdeel": 1}} |
    When I evaluate "onderdeel_ontbreekt" of "test_signaturen"
    Then the execution succeeds
    Then output "onderdeel_ontbreekt" is false

  Scenario: A field the record does not have is an error
    Given the following "register" data with key "bsn" for law "test_signaturen":
      | bsn         | rij                 |
      | "999993653" | {"a": {"ander": 1}} |
    When I evaluate "onderdeel_ontbreekt" of "test_signaturen"
    Then the execution fails with "Variable not found"

  Scenario: A path into a number is an error
    Given the following "register" data with key "bsn" for law "test_signaturen":
      | bsn         | rij      |
      | "999993653" | {"a": 5} |
    When I evaluate "onderdeel_ontbreekt" of "test_signaturen"
    Then the execution fails with "Type mismatch: expected object, got int"

  Scenario: A path into a list is an error
    Given the following "register" data with key "bsn" for law "test_signaturen":
      | bsn         | rij                       |
      | "999993653" | {"a": [{"onderdeel": 1}]} |
    When I evaluate "onderdeel_ontbreekt" of "test_signaturen"
    Then the execution fails with "Type mismatch: expected object, got array"

  Scenario: combine AND evaluates the element after one that does not settle the answer
    Given the following "register" data with key "bsn" for law "test_signaturen":
      | bsn         | rij                                       |
      | "999993653" | {"rijen": [{"keur": true}, {"ander": 1}]} |
    When I evaluate "allen_gekeurd" of "test_signaturen"
    Then the execution fails with "Variable not found"

  # ---------------------------------------------------------------------------
  # Dates. A date is a calendar day; a string in the form YYYY-MM-DD is that
  # day where a date is required. A count is a whole number, never cut off.
  # ---------------------------------------------------------------------------

  @signature:DATE:number-number-number
  Scenario: DATE makes a date of a year, a month and a day
    Given the following "register" data with key "bsn" for law "test_signaturen":
      | bsn         | rij                          |
      | "999993653" | {"a": 2025, "b": 3, "c": 15} |
    When I evaluate "datum" of "test_signaturen"
    Then the execution succeeds
    Then output "datum" equals "2025-03-15"

  Scenario: DATE refuses a day that does not exist
    Given the following "register" data with key "bsn" for law "test_signaturen":
      | bsn         | rij                          |
      | "999993653" | {"a": 2025, "b": 2, "c": 30} |
    When I evaluate "datum" of "test_signaturen"
    Then the execution fails with "is not a calendar day"

  Scenario: DATE refuses a month that does not exist
    Given the following "register" data with key "bsn" for law "test_signaturen":
      | bsn         | rij                          |
      | "999993653" | {"a": 2025, "b": 13, "c": 1} |
    When I evaluate "datum" of "test_signaturen"
    Then the execution fails with "is not a calendar day"

  Scenario: DATE knows a leap day
    Given the following "register" data with key "bsn" for law "test_signaturen":
      | bsn         | rij                          |
      | "999993653" | {"a": 2024, "b": 2, "c": 29} |
    When I evaluate "datum" of "test_signaturen"
    Then the execution succeeds
    Then output "datum" equals "2024-02-29"

  Scenario: DATE refuses a fraction, where it used to cut it off
    Given the following "register" data with key "bsn" for law "test_signaturen":
      | bsn         | rij                           |
      | "999993653" | {"a": 2025, "b": 2.5, "c": 1} |
    When I evaluate "datum" of "test_signaturen"
    Then the execution fails with "DATE 'month' must be a whole number"

  Scenario: DATE refuses a part that is no number
    Given the following "register" data with key "bsn" for law "test_signaturen":
      | bsn         | rij                            |
      | "999993653" | {"a": "2025", "b": 3, "c": 15} |
    When I evaluate "datum" of "test_signaturen"
    Then the execution fails with "DATE is not defined for (string, number, number)"

  @signature:DATE_ADD:date-and-counts
  Scenario: DATE_ADD adds months and keeps the day of the month
    Given the following "register" data with key "bsn" for law "test_signaturen":
      | bsn         | rij                         |
      | "999993653" | {"a": "2025-01-15", "b": 2} |
    When I evaluate "verschoven" of "test_signaturen"
    Then the execution succeeds
    Then output "verschoven" equals "2025-03-15"

  Scenario: DATE_ADD moves a day past the end of the month to its last day
    Given the following "register" data with key "bsn" for law "test_signaturen":
      | bsn         | rij                         |
      | "999993653" | {"a": "2025-01-31", "b": 1} |
    When I evaluate "verschoven" of "test_signaturen"
    Then the execution succeeds
    Then output "verschoven" equals "2025-02-28"

  Scenario: DATE_ADD takes a negative count
    Given the following "register" data with key "bsn" for law "test_signaturen":
      | bsn         | rij                          |
      | "999993653" | {"a": "2025-03-31", "b": -1} |
    When I evaluate "verschoven" of "test_signaturen"
    Then the execution succeeds
    Then output "verschoven" equals "2025-02-28"

  Scenario: DATE_ADD refuses a fraction, where it used to cut it off
    Given the following "register" data with key "bsn" for law "test_signaturen":
      | bsn         | rij                           |
      | "999993653" | {"a": "2025-01-15", "b": 1.5} |
    When I evaluate "verschoven" of "test_signaturen"
    Then the execution fails with "DATE_ADD 'months' must be a whole number"

  Scenario: DATE_ADD refuses a number as its date
    Given the following "register" data with key "bsn" for law "test_signaturen":
      | bsn         | rij                     |
      | "999993653" | {"a": 20250115, "b": 1} |
    When I evaluate "verschoven" of "test_signaturen"
    Then the execution fails with "DATE_ADD is not defined for (number)"

  Scenario: DATE_ADD refuses a text that is no date
    Given the following "register" data with key "bsn" for law "test_signaturen":
      | bsn         | rij                     |
      | "999993653" | {"a": "morgen", "b": 1} |
    When I evaluate "verschoven" of "test_signaturen"
    Then the execution fails with "'morgen' is not a date in the form YYYY-MM-DD"

  Scenario: DATE_ADD refuses a date that is not in canonical form
    Given the following "register" data with key "bsn" for law "test_signaturen":
      | bsn         | rij                        |
      | "999993653" | {"a": "2025-1-15", "b": 1} |
    When I evaluate "verschoven" of "test_signaturen"
    Then the execution fails with "'2025-1-15' is not a date in the form YYYY-MM-DD"

  @signature:DATE_DIFF:dates-and-unit
  Scenario: DATE_DIFF counts the complete days from the first date to the second
    Given the following "register" data with key "bsn" for law "test_signaturen":
      | bsn         | rij                                    |
      | "999993653" | {"a": "2025-01-01", "b": "2025-01-31"} |
    When I evaluate "dagen_tussen" of "test_signaturen"
    Then the execution succeeds
    Then output "dagen_tussen" equals 30

  Scenario: DATE_DIFF is negative when the second date is the earlier one
    Given the following "register" data with key "bsn" for law "test_signaturen":
      | bsn         | rij                                    |
      | "999993653" | {"a": "2025-01-31", "b": "2025-01-01"} |
    When I evaluate "dagen_tussen" of "test_signaturen"
    Then the execution succeeds
    Then output "dagen_tussen" equals -30

  Scenario: DATE_DIFF counts a month as complete on the last day of a shorter month
    Given the following "register" data with key "bsn" for law "test_signaturen":
      | bsn         | rij                                    |
      | "999993653" | {"a": "2025-01-31", "b": "2025-02-28"} |
    When I evaluate "maanden_tussen" of "test_signaturen"
    Then the execution succeeds
    Then output "maanden_tussen" equals 1

  Scenario: DATE_DIFF does not count a month that is not complete
    Given the following "register" data with key "bsn" for law "test_signaturen":
      | bsn         | rij                                    |
      | "999993653" | {"a": "2025-01-31", "b": "2025-02-27"} |
    When I evaluate "maanden_tussen" of "test_signaturen"
    Then the execution succeeds
    Then output "maanden_tussen" equals 0

  Scenario: DATE_DIFF refuses a number as a date
    Given the following "register" data with key "bsn" for law "test_signaturen":
      | bsn         | rij                                |
      | "999993653" | {"a": 20250101, "b": "2025-01-31"} |
    When I evaluate "dagen_tussen" of "test_signaturen"
    Then the execution fails with "DATE_DIFF is not defined for (number, date)"

  @signature:AGE:date-date
  Scenario: AGE counts the complete years, the birthday included
    Given the following "register" data with key "bsn" for law "test_signaturen":
      | bsn         | rij                                    |
      | "999993653" | {"a": "2000-06-15", "b": "2025-06-15"} |
    When I evaluate "leeftijd" of "test_signaturen"
    Then the execution succeeds
    Then output "leeftijd" equals 25

  Scenario: AGE does not count the year before the birthday
    Given the following "register" data with key "bsn" for law "test_signaturen":
      | bsn         | rij                                    |
      | "999993653" | {"a": "2000-06-15", "b": "2025-06-14"} |
    When I evaluate "leeftijd" of "test_signaturen"
    Then the execution succeeds
    Then output "leeftijd" equals 24

  Scenario: AGE of someone born on a leap day completes a year on 28 February
    Given the following "register" data with key "bsn" for law "test_signaturen":
      | bsn         | rij                                    |
      | "999993653" | {"a": "2004-02-29", "b": "2025-02-28"} |
    When I evaluate "leeftijd" of "test_signaturen"
    Then the execution succeeds
    Then output "leeftijd" equals 21

  Scenario: AGE is negative before birth
    Given the following "register" data with key "bsn" for law "test_signaturen":
      | bsn         | rij                                    |
      | "999993653" | {"a": "2030-01-01", "b": "2025-01-01"} |
    When I evaluate "leeftijd" of "test_signaturen"
    Then the execution succeeds
    Then output "leeftijd" equals -5

  Scenario: AGE refuses a number as a date of birth
    Given the following "register" data with key "bsn" for law "test_signaturen":
      | bsn         | rij                            |
      | "999993653" | {"a": 2000, "b": "2025-06-15"} |
    When I evaluate "leeftijd" of "test_signaturen"
    Then the execution fails with "AGE is not defined for (number, date)"

  Scenario: AGE refuses a text that is no date
    Given the following "register" data with key "bsn" for law "test_signaturen":
      | bsn         | rij                                 |
      | "999993653" | {"a": "vroeger", "b": "2025-06-15"} |
    When I evaluate "leeftijd" of "test_signaturen"
    Then the execution fails with "'vroeger' is not a date in the form YYYY-MM-DD"

  @signature:DATE_PART:date-and-part
  Scenario: DATE_PART reads the year of a date
    Given the following "register" data with key "bsn" for law "test_signaturen":
      | bsn         | rij                 |
      | "999993653" | {"a": "2025-03-15"} |
    When I evaluate "jaar" of "test_signaturen"
    Then the execution succeeds
    Then output "jaar" equals 2025

  Scenario: DATE_PART refuses a number
    Given the following "register" data with key "bsn" for law "test_signaturen":
      | bsn         | rij         |
      | "999993653" | {"a": 2025} |
    When I evaluate "jaar" of "test_signaturen"
    Then the execution fails with "DATE_PART is not defined for (number)"

  Scenario: DATE_PART of an absent date is an error, not a type error
    Given the following "register" data with key "bsn" for law "test_signaturen":
      | bsn         | rij         |
      | "999993653" | {"a": null} |
    When I evaluate "jaar" of "test_signaturen"
    Then the execution fails with "DATE_PART: operand is null (absent)"

  @signature:DAY_OF_WEEK:date
  Scenario: DAY_OF_WEEK gives 0 for a Monday
    Given the following "register" data with key "bsn" for law "test_signaturen":
      | bsn         | rij                 |
      | "999993653" | {"a": "2025-01-06"} |
    When I evaluate "weekdag" of "test_signaturen"
    Then the execution succeeds
    Then output "weekdag" equals 0

  Scenario: DAY_OF_WEEK gives 6 for a Sunday
    Given the following "register" data with key "bsn" for law "test_signaturen":
      | bsn         | rij                 |
      | "999993653" | {"a": "2025-01-05"} |
    When I evaluate "weekdag" of "test_signaturen"
    Then the execution succeeds
    Then output "weekdag" equals 6

  Scenario: DAY_OF_WEEK refuses a boolean
    Given the following "register" data with key "bsn" for law "test_signaturen":
      | bsn         | rij         |
      | "999993653" | {"a": true} |
    When I evaluate "weekdag" of "test_signaturen"
    Then the execution fails with "DAY_OF_WEEK is not defined for (boolean)"

  @signature:START_OF:date-and-part
  Scenario: START_OF goes back to the first day of the month
    Given the following "register" data with key "bsn" for law "test_signaturen":
      | bsn         | rij                 |
      | "999993653" | {"a": "2025-03-14"} |
    When I evaluate "begin_maand" of "test_signaturen"
    Then the execution succeeds
    Then output "begin_maand" equals "2025-03-01"

  Scenario: START_OF refuses a number
    Given the following "register" data with key "bsn" for law "test_signaturen":
      | bsn         | rij         |
      | "999993653" | {"a": 2025} |
    When I evaluate "begin_maand" of "test_signaturen"
    Then the execution fails with "START_OF is not defined for (number)"

  Scenario: DATE_ADD adds years first and months after, moving the day back after each
    Given the following "register" data with key "bsn" for law "test_signaturen":
      | bsn         | rij                                 |
      | "999993653" | {"a": "2024-02-29", "b": 1, "c": 1} |
    When I evaluate "ver_verschoven" of "test_signaturen"
    Then the execution succeeds
    Then output "ver_verschoven" equals "2025-03-28"

  Scenario: DATE_ADD adds weeks and days
    Given the following "register" data with key "bsn" for law "test_signaturen":
      | bsn         | rij                                 |
      | "999993653" | {"a": "2025-01-01", "b": 2, "c": 3} |
    When I evaluate "kort_verschoven" of "test_signaturen"
    Then the execution succeeds
    Then output "kort_verschoven" equals "2025-01-18"

  Scenario: DATE_ADD refuses a count that is no number
    Given the following "register" data with key "bsn" for law "test_signaturen":
      | bsn         | rij                           |
      | "999993653" | {"a": "2025-01-15", "b": "1"} |
    When I evaluate "verschoven" of "test_signaturen"
    Then the execution fails with "DATE_ADD 'months' is not defined for (string)"

  Scenario: DATE_ADD refuses a result past the year 9999
    Given the following "register" data with key "bsn" for law "test_signaturen":
      | bsn         | rij                                 |
      | "999993653" | {"a": "9999-12-31", "b": 0, "c": 1} |
    When I evaluate "kort_verschoven" of "test_signaturen"
    Then the execution fails with "outside the years 0001 to 9999"

  Scenario: DATE refuses a year outside 0001 to 9999
    Given the following "register" data with key "bsn" for law "test_signaturen":
      | bsn         | rij                      |
      | "999993653" | {"a": 0, "b": 1, "c": 1} |
    When I evaluate "datum" of "test_signaturen"
    Then the execution fails with "is not a calendar day"

  Scenario: DATE refuses a negative part
    Given the following "register" data with key "bsn" for law "test_signaturen":
      | bsn         | rij                          |
      | "999993653" | {"a": 2025, "b": -1, "c": 1} |
    When I evaluate "datum" of "test_signaturen"
    Then the execution fails with "is not a calendar day"

  Scenario: A text for a day before the year 0001 is no date
    Given the following "register" data with key "bsn" for law "test_signaturen":
      | bsn         | rij                 |
      | "999993653" | {"a": "0000-01-01"} |
    When I evaluate "jaar" of "test_signaturen"
    Then the execution fails with "outside the years 0001 to 9999"

  Scenario: DATE_DIFF counts the complete years
    Given the following "register" data with key "bsn" for law "test_signaturen":
      | bsn         | rij                                    |
      | "999993653" | {"a": "2000-06-15", "b": "2025-06-14"} |
    When I evaluate "jaren_tussen" of "test_signaturen"
    Then the execution succeeds
    Then output "jaren_tussen" equals 24

  Scenario: DATE_DIFF completes a year from 29 February on 28 February of a common year
    Given the following "register" data with key "bsn" for law "test_signaturen":
      | bsn         | rij                                    |
      | "999993653" | {"a": "2024-02-29", "b": "2025-02-28"} |
    When I evaluate "jaren_tussen" of "test_signaturen"
    Then the execution succeeds
    Then output "jaren_tussen" equals 1

  Scenario: DATE_DIFF in months is the negative of the count the other way round
    Given the following "register" data with key "bsn" for law "test_signaturen":
      | bsn         | rij                                    |
      | "999993653" | {"a": "2025-02-28", "b": "2025-01-31"} |
    When I evaluate "maanden_tussen" of "test_signaturen"
    Then the execution succeeds
    Then output "maanden_tussen" equals -1

  Scenario: DATE_DIFF takes the reference date as a date
    Given the following "register" data with key "bsn" for law "test_signaturen":
      | bsn         | rij                 |
      | "999993653" | {"a": "2024-12-31"} |
    When I evaluate "dagen_tot_peildatum" of "test_signaturen"
    Then the execution succeeds
    Then output "dagen_tot_peildatum" equals 1

  Scenario: DATE_DIFF refuses a text that is no date, whichever operand it is
    Given the following "register" data with key "bsn" for law "test_signaturen":
      | bsn         | rij                               |
      | "999993653" | {"a": "2025-01-01", "b": "later"} |
    When I evaluate "dagen_tussen" of "test_signaturen"
    Then the execution fails with "'later' is not a date in the form YYYY-MM-DD"

  Scenario: DATE_PART refuses a text that is no date
    Given the following "register" data with key "bsn" for law "test_signaturen":
      | bsn         | rij           |
      | "999993653" | {"a": "ooit"} |
    When I evaluate "jaar" of "test_signaturen"
    Then the execution fails with "'ooit' is not a date in the form YYYY-MM-DD"

  Scenario: START_OF refuses a text that is no date
    Given the following "register" data with key "bsn" for law "test_signaturen":
      | bsn         | rij           |
      | "999993653" | {"a": "ooit"} |
    When I evaluate "begin_maand" of "test_signaturen"
    Then the execution fails with "'ooit' is not a date in the form YYYY-MM-DD"

  Scenario: DAY_OF_WEEK refuses a text that is no date
    Given the following "register" data with key "bsn" for law "test_signaturen":
      | bsn         | rij           |
      | "999993653" | {"a": "ooit"} |
    When I evaluate "weekdag" of "test_signaturen"
    Then the execution fails with "'ooit' is not a date in the form YYYY-MM-DD"

  Scenario: START_OF of an absent date is an error, not a type error
    Given the following "register" data with key "bsn" for law "test_signaturen":
      | bsn         | rij         |
      | "999993653" | {"a": null} |
    When I evaluate "begin_maand" of "test_signaturen"
    Then the execution fails with "START_OF: operand is null (absent)"

  Scenario: DATE_PART of a date nobody has is unknown
    Given the following "register" data with key "bsn" for law "test_signaturen":
      | bsn         | rij      |
      | "999993653" | {"a": 1} |
    When I evaluate "jaar_van_peildag" of "test_signaturen"
    Then the execution succeeds
    Then output "jaar_van_peildag" is unknown

  Scenario: START_OF of a date nobody has is unknown
    Given the following "register" data with key "bsn" for law "test_signaturen":
      | bsn         | rij      |
      | "999993653" | {"a": 1} |
    When I evaluate "begin_maand_van_peildag" of "test_signaturen"
    Then the execution succeeds
    Then output "begin_maand_van_peildag" is unknown

  Scenario: DATE_PART reads a date that is passed as a parameter
    Given parameter "peildag" is "2024-07-01"
    Given the following "register" data with key "bsn" for law "test_signaturen":
      | bsn         | rij      |
      | "999993653" | {"a": 1} |
    When I evaluate "jaar_van_peildag" of "test_signaturen"
    Then the execution succeeds
    Then output "jaar_van_peildag" equals 2024
