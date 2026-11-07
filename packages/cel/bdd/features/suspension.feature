Feature: A suspension revokes a decision for a while
  As a cell that records that the court suspended one of its decisions
  I want the decision not to count on the days the suspension holds
  So that an answer about such a day sees no decision, and an answer about another day does

  # Fictional article 13 of test_chronicle_effects, after Awb 8:81: the cell
  # records that the court suspended its decision from `schorsing_vanaf`
  # until `schorsing_tot`, as a fact. The fact revokes the decision for that
  # window only (schema v0.8.15, `from` and `until` on an effect). The day
  # an answer is about is the calculation date: on that day a record counts
  # unless a later record that itself counts on that day revokes it on that
  # day. A suspended record is gone from the whole fold for such an answer,
  # so the count of grants drops too, as for a revocation. In practice the
  # ruling reaches the cell as a copy (see court-ruling.feature); the schema
  # has no kind of submission yet for the request for a provisional measure,
  # so here the cell records the fact directly.

  Background:
    Given the calculation date is "2025-03-01"
    And parameter "aanvrager" is "burger_x"
    When the cell records submission "aanvraag" under article "1" of "test_chronicle_effects" with:
      | ernstig_beperkt | true     |
      | aanvrager       | burger_x |
    And the cell decides "verlening" on submission "aanvraag"
    Given the calculation date is "2025-04-01"
    When the cell records fact "besluit_geschorst" as "schorsing" under article "13" of "test_chronicle_effects" with:
      | geschorst_besluit | verlening  |
      | schorsing_vanaf   | 2025-04-15 |
      | schorsing_tot     | 2025-06-01 |

  Scenario: The grant does not count on a day the suspension holds
    Given the calculation date is "2025-05-01"
    When I evaluate "heeft_geldige_kaart" of "test_reductie_geldige_kaart" over the chronicle
    Then output "heeft_geldige_kaart" is false
    And the projection trace is:
      | record    | event   | detail               |
      | aanvraag  | applied | -                    |
      | verlening | skipped | revoked by schorsing |
      | schorsing | applied | -                    |

  Scenario: The suspension holds from its first day, not before
    Given the calculation date is "2025-04-14"
    When I evaluate "heeft_geldige_kaart" of "test_reductie_geldige_kaart" over the chronicle
    Then output "heeft_geldige_kaart" is true
    And record "verlening" is applied
    Given the calculation date is "2025-04-15"
    When I evaluate "heeft_geldige_kaart" of "test_reductie_geldige_kaart" over the chronicle
    Then output "heeft_geldige_kaart" is false

  Scenario: On the day the suspension ends, the grant counts again
    Given the calculation date is "2025-06-01"
    When I evaluate "heeft_geldige_kaart" of "test_reductie_geldige_kaart" over the chronicle
    Then output "heeft_geldige_kaart" is true
    And record "verlening" is applied

  Scenario: A suspension without an end holds on every later day
    When the cell records fact "besluit_geschorst" as "schorsing_zonder_einde" under article "13" of "test_chronicle_effects" with:
      | geschorst_besluit | verlening  |
      | schorsing_vanaf   | 2025-07-01 |
      | schorsing_tot     | null       |
    Given the calculation date is "2026-01-01"
    When I evaluate "heeft_geldige_kaart" of "test_reductie_geldige_kaart" over the chronicle
    Then output "heeft_geldige_kaart" is false
    And the projection trace is:
      | record                 | event   | detail                            |
      | aanvraag               | applied | -                                 |
      | verlening              | skipped | revoked by schorsing_zonder_einde |
      | schorsing              | applied | -                                 |
      | schorsing_zonder_einde | applied | -                                 |
