Feature: Effects of a decision on another decision
  As a cell that records decisions
  I want the law to say what a decision does with an earlier one
  So that a reduction sees only the decisions whose consequences still count

  # Exercises `produces[].effects` (schema v0.8.14) against the synthetic law
  # test_chronicle_effects (packages/cel/bdd/fixtures), which is loosely shaped
  # after the disabled parking card and Awb 7:11 lid 2 and is otherwise
  # fictional. Article 4, a withdrawal of the card, is fictional throughout:
  # the corpus models no such withdrawal.
  #
  # The cell plays the effects in the order the decisions were recorded. The
  # reduction test_reductie_geldige_kaart (fixtures/cells/test_cel) is
  # given only the decisions that count, and holds no rule about revoking or
  # replacing.

  Background:
    Given the calculation date is "2025-03-01"

  Scenario: A card granted on application is valid
    When the cell records submission "aanvraag" under article "1" of "test_chronicle_effects" with:
      | ernstig_beperkt | true |
    And the cell decides "verlening" on submission "aanvraag"
    Then the consequences of decision "verlening" count
    When I evaluate "heeft_geldige_kaart" of "test_reductie_geldige_kaart" over the decisions that count as "besluiten"
    Then output "heeft_geldige_kaart" is true

  Scenario: Revoking the grant on objection takes the card away
    When the cell records submission "aanvraag" under article "1" of "test_chronicle_effects" with:
      | ernstig_beperkt | true |
    And the cell decides "verlening" on submission "aanvraag"
    And the cell records submission "bezwaar" under article "2" of "test_chronicle_effects" with:
      | bestreden_besluit | verlening |
    And the cell decides "herroeping" on submission "bezwaar"
    Then the consequences of decision "verlening" do not count
    And the consequences of decision "herroeping" count
    When I evaluate "heeft_geldige_kaart" of "test_reductie_geldige_kaart" over the decisions that count as "besluiten"
    Then output "heeft_geldige_kaart" is false

  Scenario: A decision taken in the place of another counts instead of it
    # A refusal, and on objection a grant in its place. The reduction would
    # say true with or without the refusal; the next scenario shows a
    # replaced grant that no longer counts.
    When the cell records submission "aanvraag" under article "1" of "test_chronicle_effects" with:
      | ernstig_beperkt | false |
    And the cell decides "weigering" on submission "aanvraag"
    And the cell decides "nieuw_besluit" under article "3" of "test_chronicle_effects" with:
      | bestreden_besluit | weigering |
      | ernstig_beperkt   | true      |
    Then the consequences of decision "weigering" do not count
    And the consequences of decision "nieuw_besluit" count
    When I evaluate "heeft_geldige_kaart" of "test_reductie_geldige_kaart" over the decisions that count as "besluiten"
    Then output "heeft_geldige_kaart" is true

  Scenario: A replaced grant no longer counts
    When the cell records submission "aanvraag" under article "1" of "test_chronicle_effects" with:
      | ernstig_beperkt | true |
    And the cell decides "verlening" on submission "aanvraag"
    And the cell decides "nieuw_besluit" under article "3" of "test_chronicle_effects" with:
      | bestreden_besluit | verlening |
      | ernstig_beperkt   | false     |
    Then the consequences of decision "verlening" do not count
    When I evaluate "heeft_geldige_kaart" of "test_reductie_geldige_kaart" over the decisions that count as "besluiten"
    Then output "heeft_geldige_kaart" is false

  Scenario: When a withdrawal is revoked, the card counts again
    When the cell records submission "aanvraag" under article "1" of "test_chronicle_effects" with:
      | ernstig_beperkt | true |
    And the cell decides "verlening" on submission "aanvraag"
    And the cell decides "intrekking" under article "4" of "test_chronicle_effects" with:
      | ingetrokken_besluit | verlening |
    Then the consequences of decision "verlening" do not count
    When I evaluate "heeft_geldige_kaart" of "test_reductie_geldige_kaart" over the decisions that count as "besluiten"
    Then output "heeft_geldige_kaart" is false
    When the cell records submission "bezwaar" under article "2" of "test_chronicle_effects" with:
      | bestreden_besluit | intrekking |
    And the cell decides "herroeping" on submission "bezwaar"
    Then the consequences of decision "intrekking" do not count
    And the consequences of decision "verlening" count
    When I evaluate "heeft_geldige_kaart" of "test_reductie_geldige_kaart" over the decisions that count as "besluiten"
    Then output "heeft_geldige_kaart" is true
