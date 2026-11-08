Feature: A suspension is a step with a period, not a revocation
  As a cell that records that the court suspended a card for a while
  I want the period in the state of my reduction, and the law to say what it means on a day
  So that the card is not valid on a day the suspension holds, while the grant keeps counting

  # A revocation takes a record out of the whole history (ex tunc). A
  # suspension (fictional article 13 of test_chronicle_effects, after Awb
  # 8:81) holds from a day on and ends, so it revokes nothing. Its step in
  # test_reductie_kaart_op_peildatum puts the period in the state; lifting
  # it (article 14, after Awb 8:87) ends it on a day, never later than it
  # already ended. Whether the card is
  # valid on the day the question names is article 15 of the law: the
  # reduction passes it the state and that day. Which records count never
  # depends on the day asked about. In practice the measure reaches the cell
  # as a copy (see court-ruling.feature); here the cell records the fact
  # directly. All questions are asked on 2025-07-01, when every record is
  # known, about the day under "peildatum".

  Background:
    Given the calculation date is "2025-03-01"
    And parameter "aanvrager" is "burger_x"
    When the cell records submission "aanvraag" under article "1" of "test_chronicle_effects" with:
      | ernstig_beperkt | true     |
      | aanvrager       | burger_x |
    And the cell decides "verlening" on submission "aanvraag"
    Given the calculation date is "2025-04-01"
    When the cell records fact "besluit_geschorst" as "schorsing" under article "13" of "test_chronicle_effects" with:
      | aanvrager       | burger_x   |
      | schorsing_vanaf | 2025-04-15 |
      | schorsing_tot   | 2025-06-01 |

  Scenario: The card is not valid on a day the suspension holds, and the grant still counts
    Given the calculation date is "2025-07-01"
    And parameter "peildatum" is "2025-05-01"
    When I evaluate "heeft_geldige_kaart_op_peildatum" of "test_reductie_kaart_op_peildatum" over the chronicle
    Then output "heeft_geldige_kaart_op_peildatum" is false
    And record "verlening" is applied
    And property "geschorst_vanaf" was last set by "schorsing"
    And the projection trace is:
      | record    | event   | detail                                                                     |
      | aanvraag  | applied | -                                                                          |
      | verlening | applied | test_chronicle_effects#7: aantal_verleend: 0 -> 1, in_bezit: false -> true |
      | schorsing | applied | geschorst_tot: null -> 2025-06-01, geschorst_vanaf: null -> 2025-04-15     |

  Scenario: The suspension holds from its first day, not before
    Given the calculation date is "2025-07-01"
    And parameter "peildatum" is "2025-04-14"
    When I evaluate "heeft_geldige_kaart_op_peildatum" of "test_reductie_kaart_op_peildatum" over the chronicle
    Then output "heeft_geldige_kaart_op_peildatum" is true
    Given parameter "peildatum" is "2025-04-15"
    When I evaluate "heeft_geldige_kaart_op_peildatum" of "test_reductie_kaart_op_peildatum" over the chronicle
    Then output "heeft_geldige_kaart_op_peildatum" is false

  Scenario: On the day the suspension ends, the card is valid again
    Given the calculation date is "2025-07-01"
    And parameter "peildatum" is "2025-06-01"
    When I evaluate "heeft_geldige_kaart_op_peildatum" of "test_reductie_kaart_op_peildatum" over the chronicle
    Then output "heeft_geldige_kaart_op_peildatum" is true

  Scenario: Lifting the suspension is a later step that ends the period
    Given the calculation date is "2025-05-10"
    When the cell records fact "schorsing_opgeheven" as "opheffing" under article "14" of "test_chronicle_effects" with:
      | aanvrager    | burger_x   |
      | opgeheven_op | 2025-05-10 |
    Given the calculation date is "2025-07-01"
    And parameter "peildatum" is "2025-05-15"
    When I evaluate "heeft_geldige_kaart_op_peildatum" of "test_reductie_kaart_op_peildatum" over the chronicle
    Then output "heeft_geldige_kaart_op_peildatum" is true
    And property "geschorst_tot" was last set by "opheffing"
    Given parameter "peildatum" is "2025-05-05"
    When I evaluate "heeft_geldige_kaart_op_peildatum" of "test_reductie_kaart_op_peildatum" over the chronicle
    Then output "heeft_geldige_kaart_op_peildatum" is false

  Scenario: A lifting after the suspension ended does not lengthen it
    # Article 14 gives the earlier of the end the suspension had and the day
    # it is lifted; the step only takes that output.
    Given the calculation date is "2025-07-01"
    When the cell records fact "schorsing_opgeheven" as "late_opheffing" under article "14" of "test_chronicle_effects" with:
      | aanvrager    | burger_x   |
      | opgeheven_op | 2025-07-01 |
    Given parameter "peildatum" is "2025-06-15"
    When I evaluate "heeft_geldige_kaart_op_peildatum" of "test_reductie_kaart_op_peildatum" over the chronicle
    Then output "heeft_geldige_kaart_op_peildatum" is true
    And the projection trace is:
      | record         | event   | detail                                                                     |
      | aanvraag       | applied | -                                                                          |
      | verlening      | applied | test_chronicle_effects#7: aantal_verleend: 0 -> 1, in_bezit: false -> true |
      | schorsing      | applied | geschorst_tot: null -> 2025-06-01, geschorst_vanaf: null -> 2025-04-15     |
      | late_opheffing | applied | test_chronicle_effects#14: geschorst_tot: 2025-06-01 -> 2025-06-01         |

  Scenario: A changed suspension overwrites the period, also with no end
    When the cell records fact "besluit_geschorst" as "nieuwe_schorsing" under article "13" of "test_chronicle_effects" with:
      | aanvrager       | burger_x   |
      | schorsing_vanaf | 2025-07-01 |
      | schorsing_tot   | null       |
    Given the calculation date is "2025-07-01"
    And parameter "peildatum" is "2025-05-01"
    When I evaluate "heeft_geldige_kaart_op_peildatum" of "test_reductie_kaart_op_peildatum" over the chronicle
    Then output "heeft_geldige_kaart_op_peildatum" is true
    Given parameter "peildatum" is "2026-01-01"
    When I evaluate "heeft_geldige_kaart_op_peildatum" of "test_reductie_kaart_op_peildatum" over the chronicle
    Then output "heeft_geldige_kaart_op_peildatum" is false
    And property "geschorst_vanaf" was last set by "nieuwe_schorsing"

  Scenario: Lifting a suspension without an end ends it on the day of lifting
    When the cell records fact "besluit_geschorst" as "nieuwe_schorsing" under article "13" of "test_chronicle_effects" with:
      | aanvrager       | burger_x   |
      | schorsing_vanaf | 2025-04-15 |
      | schorsing_tot   | null       |
    Given the calculation date is "2025-08-01"
    When the cell records fact "schorsing_opgeheven" as "opheffing" under article "14" of "test_chronicle_effects" with:
      | aanvrager    | burger_x   |
      | opgeheven_op | 2025-08-01 |
    Given the calculation date is "2025-09-01"
    And parameter "peildatum" is "2025-07-31"
    When I evaluate "heeft_geldige_kaart_op_peildatum" of "test_reductie_kaart_op_peildatum" over the chronicle
    Then output "heeft_geldige_kaart_op_peildatum" is false
    Given parameter "peildatum" is "2025-08-01"
    When I evaluate "heeft_geldige_kaart_op_peildatum" of "test_reductie_kaart_op_peildatum" over the chronicle
    Then output "heeft_geldige_kaart_op_peildatum" is true

  Scenario: When a lifting is revoked, the period it ended comes back
    # The lifting's step never runs, so the state keeps the period of the
    # suspension, as if it was never lifted.
    Given the calculation date is "2025-05-10"
    When the cell records fact "schorsing_opgeheven" as "opheffing" under article "14" of "test_chronicle_effects" with:
      | aanvrager    | burger_x   |
      | opgeheven_op | 2025-05-10 |
    And the cell records fact "schorsing_vervallen" as "vervallen" about "opheffing" under article "16" of "test_chronicle_effects" with:
      | vervallen_op | 2025-05-10 |
    Given the calculation date is "2025-07-01"
    And parameter "peildatum" is "2025-05-15"
    When I evaluate "heeft_geldige_kaart_op_peildatum" of "test_reductie_kaart_op_peildatum" over the chronicle
    Then output "heeft_geldige_kaart_op_peildatum" is false
    And record "opheffing" is skipped
    And property "geschorst_tot" was last set by "schorsing"

  Scenario: When a second suspension is revoked, the first period comes back
    When the cell records fact "besluit_geschorst" as "nieuwe_schorsing" under article "13" of "test_chronicle_effects" with:
      | aanvrager       | burger_x   |
      | schorsing_vanaf | 2025-07-01 |
      | schorsing_tot   | null       |
    And the cell records fact "schorsing_vervallen" as "vervallen" about "nieuwe_schorsing" under article "16" of "test_chronicle_effects" with:
      | vervallen_op | 2025-04-01 |
    Given the calculation date is "2025-07-01"
    And parameter "peildatum" is "2025-05-01"
    When I evaluate "heeft_geldige_kaart_op_peildatum" of "test_reductie_kaart_op_peildatum" over the chronicle
    Then output "heeft_geldige_kaart_op_peildatum" is false
    And record "nieuwe_schorsing" is skipped
    And property "geschorst_vanaf" was last set by "schorsing"
    Given parameter "peildatum" is "2026-01-01"
    When I evaluate "heeft_geldige_kaart_op_peildatum" of "test_reductie_kaart_op_peildatum" over the chronicle
    Then output "heeft_geldige_kaart_op_peildatum" is true

  Scenario: A suspension that is revoked never enters the state
    # Fictional article 16: the suspension never held (ex tunc). A revoked
    # record does not count, so its step never runs, the period never
    # enters the state, and the card is valid on a day inside the former
    # period. Counting and the fold are separate layers.
    When the cell records fact "schorsing_vervallen" as "vervallen" about "schorsing" under article "16" of "test_chronicle_effects" with:
      | vervallen_op | 2025-04-01 |
    Given the calculation date is "2025-07-01"
    And parameter "peildatum" is "2025-05-01"
    When I evaluate "heeft_geldige_kaart_op_peildatum" of "test_reductie_kaart_op_peildatum" over the chronicle
    Then output "heeft_geldige_kaart_op_peildatum" is true
    And record "schorsing" is skipped
    And the projection trace is:
      | record    | event   | detail                                                                     |
      | aanvraag  | applied | -                                                                          |
      | verlening | applied | test_chronicle_effects#7: aantal_verleend: 0 -> 1, in_bezit: false -> true |
      | schorsing | skipped | revoked by vervallen                                                       |
      | vervallen | applied | -                                                                          |
