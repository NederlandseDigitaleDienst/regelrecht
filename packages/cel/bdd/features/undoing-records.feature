Feature: Any record may undo an earlier record
  As a cell that records applications, facts and decisions
  I want the law to say of any record that it undoes or replaces an earlier one
  So that a withdrawn application or a rectified fact no longer counts, without inverting anything

  # Since schema v0.8.15 an effect sits on any item an article produces and on
  # any fact it requires, and names any earlier record. An effect touches only
  # the record it names: a record that refers to it keeps counting. The
  # reduction test_reductie_aanvragen reads the applications, the decisions
  # on them and the income stated, as far as they are in force. Article 7 of test_chronicle_effects (a
  # withdrawn application, after Awb 4:5 and 6:21) and article 8 (a rectified
  # income, after AVG 16) are fictional.

  Background:
    Given the calculation date is "2025-03-01"
    And parameter "aanvrager" is "burger_x"

  Scenario: A rectified fact takes the place of the wrong one and is not counted twice
    When the cell records fact "inkomen_opgegeven" as "opgave_2024" under article "8" of "test_chronicle_effects" with:
      | jaar      | 2024     |
      | bedrag    | 30000    |
      | aanvrager | burger_x |
    And the cell records fact "inkomen_opgegeven" as "opgave_2023" under article "8" of "test_chronicle_effects" with:
      | jaar      | 2023     |
      | bedrag    | 20000    |
      | aanvrager | burger_x |
    And the cell records fact "inkomen_gerectificeerd" as "rectificatie" under article "8" of "test_chronicle_effects" with:
      | gerectificeerde_opgave | opgave_2024 |
      | jaar                   | 2024        |
      | bedrag                 | 25000       |
      | aanvrager              | burger_x    |
    And I evaluate "totaal_inkomen" of "test_reductie_aanvragen" over the chronicle
    Then output "totaal_inkomen" equals 45000
    And record "opgave_2024" is skipped
    And the records in force are "opgave_2023, rectificatie"

  Scenario: The applicant withdraws the application before the decision
    When the cell records submission "aanvraag" under article "1" of "test_chronicle_effects" with:
      | ernstig_beperkt | true     |
      | aanvrager       | burger_x |
    And I evaluate "aanvraag_loopt" of "test_reductie_aanvragen" over the chronicle
    Then output "aanvraag_loopt" is true
    When the cell records fact "aanvraag_ingetrokken" as "intrekking" under article "7" of "test_chronicle_effects" with:
      | ingetrokken_aanvraag | aanvraag |
    And I evaluate "aanvraag_loopt" of "test_reductie_aanvragen" over the chronicle
    Then output "aanvraag_loopt" is false
    And the projection trace is:
      | record     | event   | detail                                                                       |
      | aanvraag   | applied | - |
      | intrekking | rewound | undoes aanvraag                                                              |
      | aanvraag   | skipped | undone by intrekking                                                         |
      | intrekking | applied | -                                                                            |

  Scenario: An application withdrawn after the decision leaves the decision in place
    # The withdrawal undoes the application only. The grant refers to it,
    # but the law gives the withdrawal no effect on the grant.
    When the cell records submission "aanvraag" under article "1" of "test_chronicle_effects" with:
      | ernstig_beperkt | true     |
      | aanvrager       | burger_x |
    And the cell decides "verlening" on submission "aanvraag"
    And the cell records fact "aanvraag_ingetrokken" as "intrekking" under article "7" of "test_chronicle_effects" with:
      | ingetrokken_aanvraag | aanvraag |
    And I evaluate "aanvraag_loopt" of "test_reductie_aanvragen" over the chronicle
    Then record "aanvraag" is skipped
    And record "verlening" is applied
    And output "aanvraag_loopt" is false
    When I evaluate "heeft_geldige_kaart" of "test_reductie_geldige_kaart" over the chronicle
    Then output "heeft_geldige_kaart" is true
    And record "verlening" is applied

  Scenario: An application stays pending while another is decided on
    # Each application is a row of its own; an application is pending while
    # no decision refers to it, so a decision on the first leaves the second
    # pending.
    When the cell records submission "aanvraag_1" under article "1" of "test_chronicle_effects" with:
      | ernstig_beperkt | true     |
      | aanvrager       | burger_x |
    And the cell records submission "aanvraag_2" under article "1" of "test_chronicle_effects" with:
      | ernstig_beperkt | true     |
      | aanvrager       | burger_x |
    And the cell decides "besluit_1" on submission "aanvraag_1"
    And I evaluate "aanvraag_loopt" of "test_reductie_aanvragen" over the chronicle
    Then output "aanvraag_loopt" is true
    And the records in force are "aanvraag_1, aanvraag_2, besluit_1"
    When the cell decides "besluit_2" on submission "aanvraag_2"
    And I evaluate "aanvraag_loopt" of "test_reductie_aanvragen" over the chronicle
    Then output "aanvraag_loopt" is false
