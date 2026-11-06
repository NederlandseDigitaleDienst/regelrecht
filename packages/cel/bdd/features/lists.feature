Feature: A reduction reads several grants at once
  As a reduction that tracks several grants at once
  I want one row per grant in force, each with its own dates
  So that two grants that overlap each keep their own end date

  # Each grant in force is a row of its own (schema v0.8.14), with its first
  # and last day. The reduction test_reductie_verleningen reads the grants of
  # fictional article 9 and the cards handed back; a card handed back ends
  # its grant on the day it was handed back.

  Background:
    Given parameter "aanvrager" is "burger_x"

  Scenario: Two grants with their own end dates, one handed back early
    Given the calculation date is "2025-01-01"
    When the cell decides "verlening_1" under article "9" of "test_chronicle_effects" with:
      | aanvrager    | burger_x   |
      | ingangsdatum | 2025-01-01 |
      | einddatum    | 2025-07-01 |
    Given the calculation date is "2025-03-01"
    When the cell decides "verlening_2" under article "9" of "test_chronicle_effects" with:
      | aanvrager    | burger_x   |
      | ingangsdatum | 2025-03-01 |
      | einddatum    | 2026-03-01 |
    Given the calculation date is "2025-08-01"
    When I evaluate "heeft_geldige_kaart_op_dag" of "test_reductie_verleningen" over the chronicle
    Then output "heeft_geldige_kaart_op_dag" is true
    And the records in force are "verlening_1, verlening_2"
    Given the calculation date is "2025-05-01"
    When the cell records fact "kaart_teruggegeven" as "teruggave" under article "9" of "test_chronicle_effects" with:
      | teruggegeven_kaart | verlening_2 |
      | teruggegeven_op    | 2025-05-01  |
      | aanvrager          | burger_x    |
    Given the calculation date is "2025-08-01"
    When I evaluate "heeft_geldige_kaart_op_dag" of "test_reductie_verleningen" over the chronicle
    Then output "heeft_geldige_kaart_op_dag" is false
    And the records in force are "verlening_1, verlening_2, teruggave"
    Given the calculation date is "2025-04-15"
    When I evaluate "heeft_geldige_kaart_op_dag" of "test_reductie_verleningen" over the chronicle as known on "2025-08-01"
    Then output "heeft_geldige_kaart_op_dag" is true
