Feature: A cell corrects a fact it recorded
  As a cell that never changes its chronicle
  I want to record a correction as a fact of its own
  So that a reduction reads the corrected value and the history stays

  # Exercises the synthetic register test_chronicle_corrections
  # (packages/cel/bdd/fixtures) and the reduction test_reductie_adres. A
  # correction of one field is a fact of its own with a step of its own: the
  # step sets that field and leaves the others (a fold step). A record that
  # was wrong from the start is revoked by the fact that says so and refers
  # to it, as the law declares (`requires[].effects`, `revokes:
  # refers_to`); a new registration then holds the right values. A correction from a later day on ("met ingang van") is a fold
  # step too: the record puts the day in the state and the answer compares
  # it with the day asked about. This feature does not exercise that.

  Background:
    Given the calculation date is "2025-03-01"
    And parameter "bewoner" is "burger_x"

  Scenario: A correction of the postcode leaves the house number as registered
    When the cell records fact "ingeschreven" as "inschrijving" under article "1" of "test_chronicle_corrections" with:
      | bewoner    | burger_x |
      | postcode   | 1000AA   |
      | huisnummer | 12       |
    And the cell records fact "postcode_gecorrigeerd" as "correctie" under article "2" of "test_chronicle_corrections" with:
      | bewoner  | burger_x |
      | postcode | 1000AB   |
    And I evaluate "postcode" of "test_reductie_adres" over the chronicle
    Then output "postcode" equals "1000AB"
    When I evaluate "huisnummer" of "test_reductie_adres" over the chronicle
    Then output "huisnummer" equals 12
    And property "postcode" was last set by "correctie"
    And property "huisnummer" was last set by "inschrijving"
    And record "inschrijving" is applied

  Scenario: A registration wrong from the start no longer counts, and a new one takes its place
    When the cell records fact "ingeschreven" as "inschrijving" under article "1" of "test_chronicle_corrections" with:
      | bewoner    | burger_x |
      | postcode   | 1000AA   |
      | huisnummer | 12       |
    And the cell records fact "inschrijving_onjuist" as "onjuist" about "inschrijving" under article "3" of "test_chronicle_corrections" with:
      | reden | verkeerd huisnummer opgegeven |
    And I evaluate "postcode" of "test_reductie_adres" over the chronicle
    Then record "inschrijving" is skipped
    And output "postcode" is absent
    When the cell records fact "ingeschreven" as "nieuwe_inschrijving" under article "1" of "test_chronicle_corrections" with:
      | bewoner    | burger_x |
      | postcode   | 2000BB   |
      | huisnummer | 3        |
    And I evaluate "postcode" of "test_reductie_adres" over the chronicle
    Then output "postcode" equals "2000BB"
    When I evaluate "huisnummer" of "test_reductie_adres" over the chronicle
    Then output "huisnummer" equals 3
    And property "postcode" was last set by "nieuwe_inschrijving"

  Scenario: A correction of the postcode still holds when the registration before it was wrong from the start
    # The correction of the postcode refers to no record, so it counts on
    # its own: the postcode it records holds, while the house number of the
    # revoked registration does not, so the state keeps its start value.
    When the cell records fact "ingeschreven" as "inschrijving" under article "1" of "test_chronicle_corrections" with:
      | bewoner    | burger_x |
      | postcode   | 1000AA   |
      | huisnummer | 12       |
    And the cell records fact "postcode_gecorrigeerd" as "correctie" under article "2" of "test_chronicle_corrections" with:
      | bewoner  | burger_x |
      | postcode | 1000AB   |
    And the cell records fact "inschrijving_onjuist" as "onjuist" about "inschrijving" under article "3" of "test_chronicle_corrections" with:
      | reden | verkeerd huisnummer opgegeven |
    And I evaluate "postcode" of "test_reductie_adres" over the chronicle
    Then record "inschrijving" is skipped
    And record "correctie" is applied
    And output "postcode" equals "1000AB"
    When I evaluate "huisnummer" of "test_reductie_adres" over the chronicle
    Then output "huisnummer" equals 0
