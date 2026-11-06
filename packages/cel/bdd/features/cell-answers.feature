Feature: A cell answers from a reduction that plays its chronicle forward
  As a cell whose answers a policy binds to an open term
  I want my answer to come from the reduction that plays my chronicle forward
  So that a decision reads every record the law left in place

  # The cell's catalogue (fixtures/cells/test_cel/cel.yaml) names the reduction
  # test_reductie_geldige_kaart for the answer `heeft_geldige_kaart`. A
  # fictional policy (fixtures/regulation/test_beleid_ontheffing) binds the
  # open term of the permit in article 13 of test_chronicle_effects to that
  # answer. The steps "the cell answers" and "the cell itself decides" run
  # the cell's own code, the path the engine takes when it asks the cell.

  Background:
    Given the calculation date is "2025-03-01"
    And parameter "aanvrager" is "burger_x"

  Scenario: The cell answers from the records in force
    When the cell records submission "aanvraag" under article "1" of "test_chronicle_effects" with:
      | ernstig_beperkt | true     |
      | aanvrager       | burger_x |
    And the cell decides "verlening" on submission "aanvraag"
    And the cell answers "heeft_geldige_kaart"
    Then the answer of the cell is true
    When the cell records submission "bezwaar" under article "2" of "test_chronicle_effects" with:
      | bestreden_besluit | verlening |
    And the cell decides "herroeping" on submission "bezwaar" with:
      | bezwaar_gegrond | true |
    And the cell answers "heeft_geldige_kaart"
    Then the answer of the cell is false

  Scenario: A decision the cell takes reads the answer of the projection
    # The permit is granted to whoever holds a valid card. The cell decides
    # it itself, and the policy asks the cell, which plays its chronicle.
    When the cell records submission "aanvraag" under article "1" of "test_chronicle_effects" with:
      | ernstig_beperkt | true     |
      | aanvrager       | burger_x |
    And the cell decides "verlening" on submission "aanvraag"
    And the cell records submission "ontheffingsaanvraag" under article "13" of "test_chronicle_effects" with:
      | aanvrager | burger_x |
    And the cell itself decides "ontheffing" on submission "ontheffingsaanvraag"
    Then record "ontheffing" has output "ontheffing_verleend" true

  Scenario: A decision the cell takes after the grant was revoked refuses the permit
    When the cell records submission "aanvraag" under article "1" of "test_chronicle_effects" with:
      | ernstig_beperkt | true     |
      | aanvrager       | burger_x |
    And the cell decides "verlening" on submission "aanvraag"
    And the cell records submission "bezwaar" under article "2" of "test_chronicle_effects" with:
      | bestreden_besluit | verlening |
    And the cell decides "herroeping" on submission "bezwaar" with:
      | bezwaar_gegrond | true |
    And the cell records submission "ontheffingsaanvraag" under article "13" of "test_chronicle_effects" with:
      | aanvrager | burger_x |
    And the cell itself decides "ontheffing" on submission "ontheffingsaanvraag"
    Then record "ontheffing" has output "ontheffing_verleend" false

  Scenario: Another applicant's grant does not answer for this one
    When the cell records submission "aanvraag" under article "1" of "test_chronicle_effects" with:
      | ernstig_beperkt | true     |
      | aanvrager       | burger_y |
    And the cell decides "verlening" on submission "aanvraag"
    And the cell answers "heeft_geldige_kaart"
    Then the answer of the cell is false

  Scenario: A decision that may replace the contested one plays the whole chronicle
    # Article 14 may take a decision in the place of the grant it decides an
    # objection against, so the cell reads its other answers without that
    # grant. A projection still plays the whole chronicle: it is never
    # filtered, and the effects of the law decide which records count. A
    # provisional measure suspends the grant only from 1 April. On 1 March
    # the grant counts, so the card is valid; without the grant in the
    # chronicle the projection would find no card, and the measure would
    # name a record that is not there.
    When the cell records submission "aanvraag" under article "1" of "test_chronicle_effects" with:
      | ernstig_beperkt | true     |
      | aanvrager       | burger_x |
    And the cell decides "verlening" on submission "aanvraag"
    And the cell decides "voorziening" under article "10" of "test_chronicle_effects" with:
      | bestreden_besluit | verlening  |
      | schorsing_vanaf   | 2025-04-01 |
      | schorsing_tot     | 2025-06-01 |
    And the cell itself records submission "bezwaar" against "verlening" under article "14" of "test_chronicle_effects"
    And the cell itself decides "beslissing_op_bezwaar" on submission "bezwaar"
    Then record "beslissing_op_bezwaar" has output "kaart_nog_geldig" true
    Given the calculation date is "2025-04-15"
    When the cell itself records submission "bezwaar_2" against "verlening" under article "14" of "test_chronicle_effects"
    And the cell itself decides "beslissing_op_bezwaar_2" on submission "bezwaar_2"
    Then record "beslissing_op_bezwaar_2" has output "kaart_nog_geldig" false
