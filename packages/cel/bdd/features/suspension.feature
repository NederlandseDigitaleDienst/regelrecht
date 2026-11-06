Feature: A provisional measure suspends a decision for a while
  As a cell that records a provisional measure of the court
  I want the suspended decision not to count on the days the suspension holds
  So that an answer about such a day sees no decision, and an answer about another day does

  # Fictional article 10 of test_chronicle_effects, after Awb 8:81: the
  # provisional measure undoes the contested decision from `schorsing_vanaf`
  # until `schorsing_tot` (schema v0.8.15, `from` and `until` on an effect).
  # The law says so, not the reduction: test_reductie_geldige_kaart has no
  # step for the measure. For an answer about a day in that window the
  # decision is gone from the history; for an answer about another day it
  # counts. In practice the measure reaches the cell as a copy from the
  # court's cell (see court-ruling.feature); here the cell records it
  # directly.

  Background:
    Given parameter "aanvrager" is "burger_x"

  Scenario: The grant does not count while it is suspended
    Given the calculation date is "2025-03-01"
    When the cell records submission "aanvraag" under article "1" of "test_chronicle_effects" with:
      | ernstig_beperkt | true     |
      | aanvrager       | burger_x |
    And the cell decides "verlening" on submission "aanvraag"
    Given the calculation date is "2025-04-01"
    When the cell decides "voorziening" under article "10" of "test_chronicle_effects" with:
      | bestreden_besluit | verlening  |
      | schorsing_vanaf   | 2025-04-01 |
      | schorsing_tot     | 2025-06-01 |
    Given the calculation date is "2025-05-01"
    When I evaluate "heeft_geldige_kaart" of "test_reductie_geldige_kaart" over the chronicle
    Then output "heeft_geldige_kaart" is false
    And the projection trace is:
      | record      | event   | detail                |
      | aanvraag    | applied | -                     |
      | verlening   | applied | -                     |
      | voorziening | rewound | undoes verlening      |
      | aanvraag    | applied | -                     |
      | verlening   | skipped | undone by voorziening |
      | voorziening | applied | -                     |
    When the cell answers "heeft_geldige_kaart"
    Then the answer of the cell is false
    Given the calculation date is "2025-06-01"
    When I evaluate "heeft_geldige_kaart" of "test_reductie_geldige_kaart" over the chronicle
    Then output "heeft_geldige_kaart" is true
    And record "verlening" is applied

  Scenario: An answer about a day before the suspension, as known later
    Given the calculation date is "2025-03-01"
    When the cell records submission "aanvraag" under article "1" of "test_chronicle_effects" with:
      | ernstig_beperkt | true     |
      | aanvrager       | burger_x |
    And the cell decides "verlening" on submission "aanvraag"
    Given the calculation date is "2025-04-01"
    When the cell decides "voorziening" under article "10" of "test_chronicle_effects" with:
      | bestreden_besluit | verlening  |
      | schorsing_vanaf   | 2025-04-01 |
      | schorsing_tot     | 2025-06-01 |
    Given the calculation date is "2025-03-15"
    When I evaluate "heeft_geldige_kaart" of "test_reductie_geldige_kaart" over the chronicle as known on "2025-05-01"
    Then output "heeft_geldige_kaart" is true
    And the projection trace is:
      | record      | event   | detail |
      | aanvraag    | applied | -      |
      | verlening   | applied | -      |
      | voorziening | applied | -      |
