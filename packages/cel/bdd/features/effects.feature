Feature: A reduction plays the chronicle forward
  As a cell that records submissions, decisions and facts
  I want a reduction to play my chronicle forward and keep a state
  So that a decision the law undoes is skipped, and the answer is computed again

  # Exercises `projection` and `produces[].effects` (schema v0.8.14) against
  # the synthetic law test_chronicle_effects (packages/cel/bdd/fixtures),
  # which is loosely shaped after the disabled parking card and Awb 7:11 lid 2
  # and is otherwise fictional. The decision on an objection (article 2)
  # revokes the contested decision only when the objection is well-founded
  # (`when: herroepen`). Article 4 (a withdrawal of the card) and article 5
  # (handing the card in at the desk, recorded as a fact) are fictional
  # throughout: the corpus models neither.
  #
  # The reduction test_reductie_geldige_kaart (fixtures/cells/test_cel) keeps
  # two properties: whether the applicant holds a card (`in_bezit`) and how
  # many grants counted (`aantal_verleend`). It says what a grant, a decision
  # in the place of another and a card handed in do with them, and nothing
  # about revoking. The engine carries out the effects of the law by playing
  # the chronicle again from the start, skipping a record that is undone.

  Background:
    Given the calculation date is "2025-03-01"

  Scenario: A card granted on application is valid
    When the cell records submission "aanvraag" under article "1" of "test_chronicle_effects" with:
      | ernstig_beperkt | true |
    And the cell decides "verlening" on submission "aanvraag"
    And I evaluate "heeft_geldige_kaart" of "test_reductie_geldige_kaart" over the chronicle
    Then output "heeft_geldige_kaart" is true
    And property "in_bezit" was last set by "verlening"
    And the projection trace is:
      | record    | event   | detail                                             |
      | aanvraag  | applied | -                                                  |
      | verlening | applied | aantal_verleend: 0 -> 1, in_bezit: false -> true   |

  Scenario: An unfounded objection leaves the grant in place
    # Article 2 revokes only when its outcome `herroepen` is true, which
    # follows from the finding that the objection is well-founded.
    When the cell records submission "aanvraag" under article "1" of "test_chronicle_effects" with:
      | ernstig_beperkt | true |
    And the cell decides "verlening" on submission "aanvraag"
    And the cell records submission "bezwaar" under article "2" of "test_chronicle_effects" with:
      | bestreden_besluit | verlening |
    And the cell decides "beslissing_op_bezwaar" on submission "bezwaar" with:
      | bezwaar_gegrond | false |
    And I evaluate "heeft_geldige_kaart" of "test_reductie_geldige_kaart" over the chronicle
    Then record "verlening" is applied
    And output "heeft_geldige_kaart" is true

  Scenario: A well-founded objection revokes the grant and takes the card away
    When the cell records submission "aanvraag" under article "1" of "test_chronicle_effects" with:
      | ernstig_beperkt | true |
    And the cell decides "verlening" on submission "aanvraag"
    And the cell records submission "bezwaar" under article "2" of "test_chronicle_effects" with:
      | bestreden_besluit | verlening |
    And the cell decides "herroeping" on submission "bezwaar" with:
      | bezwaar_gegrond | true |
    And I evaluate "heeft_geldige_kaart" of "test_reductie_geldige_kaart" over the chronicle
    Then record "verlening" is skipped
    And record "herroeping" is applied
    And output "heeft_geldige_kaart" is false

  Scenario: A decision taken in the place of a refusal grants the card
    When the cell records submission "aanvraag" under article "1" of "test_chronicle_effects" with:
      | ernstig_beperkt | false |
    And the cell decides "weigering" on submission "aanvraag"
    And the cell decides "nieuw_besluit" under article "3" of "test_chronicle_effects" with:
      | bestreden_besluit | weigering |
      | ernstig_beperkt   | true      |
    And I evaluate "heeft_geldige_kaart" of "test_reductie_geldige_kaart" over the chronicle
    Then record "weigering" is skipped
    And record "nieuw_besluit" is applied
    And output "heeft_geldige_kaart" is true

  Scenario: A replaced grant no longer counts
    When the cell records submission "aanvraag" under article "1" of "test_chronicle_effects" with:
      | ernstig_beperkt | true |
    And the cell decides "verlening" on submission "aanvraag"
    And the cell decides "nieuw_besluit" under article "3" of "test_chronicle_effects" with:
      | bestreden_besluit | verlening |
      | ernstig_beperkt   | false     |
    And I evaluate "heeft_geldige_kaart" of "test_reductie_geldige_kaart" over the chronicle
    Then record "verlening" is skipped
    And output "heeft_geldige_kaart" is false

  Scenario: Revoking the first grant after the card was handed in and granted again
    # Undoing is replaying without the revoked grant, not turning its changes
    # back: the card handed in and the second grant still decide `in_bezit`.
    When the cell records submission "aanvraag" under article "1" of "test_chronicle_effects" with:
      | ernstig_beperkt | true |
    And the cell decides "verlening" on submission "aanvraag"
    And the cell records fact "kaart_ingeleverd" as "inlevering" under article "5" of "test_chronicle_effects" with:
      | ingeleverd_op | 2025-03-01 |
    And the cell records submission "tweede_aanvraag" under article "1" of "test_chronicle_effects" with:
      | ernstig_beperkt | true |
    And the cell decides "tweede_verlening" on submission "tweede_aanvraag"
    And the cell records submission "bezwaar" under article "2" of "test_chronicle_effects" with:
      | bestreden_besluit | verlening |
    And the cell decides "herroeping" on submission "bezwaar" with:
      | bezwaar_gegrond | true |
    And I evaluate "heeft_geldige_kaart" of "test_reductie_geldige_kaart" over the chronicle
    Then output "heeft_geldige_kaart" is true
    And property "in_bezit" was last set by "tweede_verlening"
    And the projection trace is:
      | record           | event   | detail                                           |
      | aanvraag         | applied | -                                                |
      | verlening        | applied | aantal_verleend: 0 -> 1, in_bezit: false -> true |
      | inlevering       | applied | in_bezit: true -> false                          |
      | tweede_aanvraag  | applied | -                                                |
      | tweede_verlening | applied | aantal_verleend: 1 -> 2, in_bezit: false -> true |
      | bezwaar          | applied | -                                                |
      | herroeping       | rewound | revokes verlening                                |
      | aanvraag         | applied | -                                                |
      | verlening        | skipped | undone by herroeping                             |
      | inlevering       | applied | in_bezit: false -> false                         |
      | tweede_aanvraag  | applied | -                                                |
      | tweede_verlening | applied | aantal_verleend: 0 -> 1, in_bezit: false -> true |
      | bezwaar          | applied | -                                                |
      | herroeping       | applied | -                                                |

  Scenario: The answer reads two properties of the state
    # A card is a replacement when the applicant holds one and more than one
    # grant counted. Revoking the first grant leaves one.
    When the cell records submission "aanvraag" under article "1" of "test_chronicle_effects" with:
      | ernstig_beperkt | true |
    And the cell decides "verlening" on submission "aanvraag"
    And the cell records fact "kaart_ingeleverd" as "inlevering" under article "5" of "test_chronicle_effects" with:
      | ingeleverd_op | 2025-03-01 |
    And the cell records submission "tweede_aanvraag" under article "1" of "test_chronicle_effects" with:
      | ernstig_beperkt | true |
    And the cell decides "tweede_verlening" on submission "tweede_aanvraag"
    And I evaluate "kaart_is_vervanging" of "test_reductie_geldige_kaart" over the chronicle
    Then output "kaart_is_vervanging" is true
    When the cell records submission "bezwaar" under article "2" of "test_chronicle_effects" with:
      | bestreden_besluit | verlening |
    And the cell decides "herroeping" on submission "bezwaar" with:
      | bezwaar_gegrond | true |
    And I evaluate "kaart_is_vervanging" of "test_reductie_geldige_kaart" over the chronicle
    Then output "kaart_is_vervanging" is false
    And property "aantal_verleend" was last set by "tweede_verlening"

  Scenario: Revoking a second grant leaves the first grant in place
    # The card is already granted; a second application grants it again. A
    # well-founded objection revokes the second grant only: the first grant
    # still counts, so the applicant still has a valid card.
    When the cell records submission "aanvraag" under article "1" of "test_chronicle_effects" with:
      | ernstig_beperkt | true |
    And the cell decides "verlening" on submission "aanvraag"
    And I evaluate "heeft_geldige_kaart" of "test_reductie_geldige_kaart" over the chronicle
    Then output "heeft_geldige_kaart" is true
    When the cell records submission "tweede_aanvraag" under article "1" of "test_chronicle_effects" with:
      | ernstig_beperkt | true |
    And the cell decides "tweede_verlening" on submission "tweede_aanvraag"
    And the cell records submission "bezwaar" under article "2" of "test_chronicle_effects" with:
      | bestreden_besluit | tweede_verlening |
    And the cell decides "herroeping" on submission "bezwaar" with:
      | bezwaar_gegrond | true |
    And I evaluate "heeft_geldige_kaart" of "test_reductie_geldige_kaart" over the chronicle
    Then record "tweede_verlening" is skipped
    And record "verlening" is applied
    And output "heeft_geldige_kaart" is true

  Scenario: When a withdrawal is revoked, the card counts again
    When the cell records submission "aanvraag" under article "1" of "test_chronicle_effects" with:
      | ernstig_beperkt | true |
    And the cell decides "verlening" on submission "aanvraag"
    And the cell decides "intrekking" under article "4" of "test_chronicle_effects" with:
      | ingetrokken_besluit | verlening |
    And I evaluate "heeft_geldige_kaart" of "test_reductie_geldige_kaart" over the chronicle
    Then record "verlening" is skipped
    And output "heeft_geldige_kaart" is false
    When the cell records submission "bezwaar" under article "2" of "test_chronicle_effects" with:
      | bestreden_besluit | intrekking |
    And the cell decides "herroeping" on submission "bezwaar" with:
      | bezwaar_gegrond | true |
    And I evaluate "heeft_geldige_kaart" of "test_reductie_geldige_kaart" over the chronicle
    Then record "intrekking" is skipped
    And record "verlening" is applied
    And output "heeft_geldige_kaart" is true
    And the projection trace is:
      | record     | event   | detail                                           |
      | aanvraag   | applied | -                                                |
      | verlening  | applied | aantal_verleend: 0 -> 1, in_bezit: false -> true |
      | intrekking | rewound | revokes verlening                                |
      | aanvraag   | applied | -                                                |
      | verlening  | skipped | undone by intrekking                             |
      | intrekking | applied | -                                                |
      | bezwaar    | applied | -                                                |
      | herroeping | rewound | revokes intrekking                               |
      | aanvraag   | applied | -                                                |
      | verlening  | skipped | undone by intrekking                             |
      | intrekking | skipped | undone by herroeping                             |
      | intrekking | rewound | revives verlening                                |
      | aanvraag   | applied | -                                                |
      | verlening  | applied | aantal_verleend: 0 -> 1, in_bezit: false -> true |
      | intrekking | skipped | undone by herroeping                             |
      | bezwaar    | applied | -                                                |
      | herroeping | applied | -                                                |

  Scenario: A decision revoked twice stays skipped when one revocation is undone
    # Two well-founded objections revoke the grant; a third revokes the first
    # revocation. The second revocation still counts, so the grant stays
    # skipped.
    When the cell records submission "aanvraag" under article "1" of "test_chronicle_effects" with:
      | ernstig_beperkt | true |
    And the cell decides "verlening" on submission "aanvraag"
    And the cell records submission "bezwaar_1" under article "2" of "test_chronicle_effects" with:
      | bestreden_besluit | verlening |
    And the cell decides "herroeping_1" on submission "bezwaar_1" with:
      | bezwaar_gegrond | true |
    And the cell records submission "bezwaar_2" under article "2" of "test_chronicle_effects" with:
      | bestreden_besluit | verlening |
    And the cell decides "herroeping_2" on submission "bezwaar_2" with:
      | bezwaar_gegrond | true |
    And the cell records submission "bezwaar_3" under article "2" of "test_chronicle_effects" with:
      | bestreden_besluit | herroeping_1 |
    And the cell decides "herroeping_3" on submission "bezwaar_3" with:
      | bezwaar_gegrond | true |
    And I evaluate "heeft_geldige_kaart" of "test_reductie_geldige_kaart" over the chronicle
    Then record "herroeping_1" is skipped
    And record "herroeping_2" is applied
    And record "verlening" is skipped
    And output "heeft_geldige_kaart" is false
    And the projection trace is:
      | record       | event   | detail                                           |
      | aanvraag     | applied | -                                                |
      | verlening    | applied | aantal_verleend: 0 -> 1, in_bezit: false -> true |
      | bezwaar_1    | applied | -                                                |
      | herroeping_1 | rewound | revokes verlening                                |
      | aanvraag     | applied | -                                                |
      | verlening    | skipped | undone by herroeping_1, herroeping_2             |
      | bezwaar_1    | applied | -                                                |
      | herroeping_1 | applied | -                                                |
      | bezwaar_2    | applied | -                                                |
      | herroeping_2 | applied | -                                                |
      | bezwaar_3    | applied | -                                                |
      | herroeping_3 | rewound | revokes herroeping_1                             |
      | aanvraag     | applied | -                                                |
      | verlening    | skipped | undone by herroeping_2                           |
      | bezwaar_1    | applied | -                                                |
      | herroeping_1 | skipped | undone by herroeping_3                           |
      | bezwaar_2    | applied | -                                                |
      | herroeping_2 | applied | -                                                |
      | bezwaar_3    | applied | -                                                |
      | herroeping_3 | applied | -                                                |

  Scenario: Replacing a replacement undoes its replacing
    # A record that is replaced undoes nothing any more: the first
    # replacement no longer replaces the grant, which counts again beside
    # the second replacement. A replacement that should keep the grant
    # replaced would say so with an effect of its own.
    When the cell records submission "aanvraag" under article "1" of "test_chronicle_effects" with:
      | ernstig_beperkt | true |
    And the cell decides "verlening" on submission "aanvraag"
    And the cell decides "eerste_vervanging" under article "3" of "test_chronicle_effects" with:
      | bestreden_besluit | verlening |
      | ernstig_beperkt   | false     |
    And the cell decides "tweede_vervanging" under article "3" of "test_chronicle_effects" with:
      | bestreden_besluit | eerste_vervanging |
      | ernstig_beperkt   | false             |
    And I evaluate "heeft_geldige_kaart" of "test_reductie_geldige_kaart" over the chronicle
    Then record "verlening" is applied
    And record "eerste_vervanging" is skipped
    And record "tweede_vervanging" is applied
    And output "heeft_geldige_kaart" is true

  Scenario: Revoking the last replacement revives the one it replaced, not the grant
    # A record that is revoked undoes nothing any more. The first replacement
    # counts again and replaces the grant again.
    When the cell records submission "aanvraag" under article "1" of "test_chronicle_effects" with:
      | ernstig_beperkt | true |
    And the cell decides "verlening" on submission "aanvraag"
    And the cell decides "eerste_vervanging" under article "3" of "test_chronicle_effects" with:
      | bestreden_besluit | verlening |
      | ernstig_beperkt   | false     |
    And the cell decides "tweede_vervanging" under article "3" of "test_chronicle_effects" with:
      | bestreden_besluit | eerste_vervanging |
      | ernstig_beperkt   | true              |
    And the cell records submission "bezwaar" under article "2" of "test_chronicle_effects" with:
      | bestreden_besluit | tweede_vervanging |
    And the cell decides "herroeping" on submission "bezwaar" with:
      | bezwaar_gegrond | true |
    And I evaluate "heeft_geldige_kaart" of "test_reductie_geldige_kaart" over the chronicle
    Then record "tweede_vervanging" is skipped
    And record "eerste_vervanging" is applied
    And record "verlening" is skipped
    And output "heeft_geldige_kaart" is false

  Scenario: A warning in the place of a withdrawal brings the card back
    # On objection the college replaces the withdrawal with a warning
    # (fictional article 6). The withdrawal undoes nothing any more.
    When the cell records submission "aanvraag" under article "1" of "test_chronicle_effects" with:
      | ernstig_beperkt | true |
    And the cell decides "verlening" on submission "aanvraag"
    And the cell decides "intrekking" under article "4" of "test_chronicle_effects" with:
      | ingetrokken_besluit | verlening |
    And the cell decides "waarschuwing" under article "6" of "test_chronicle_effects" with:
      | bestreden_besluit | intrekking |
    And I evaluate "heeft_geldige_kaart" of "test_reductie_geldige_kaart" over the chronicle
    Then record "intrekking" is skipped
    And record "verlening" is applied
    And output "heeft_geldige_kaart" is true

  Scenario: Whether a record counts does not depend on the order it was found in
    # A withdrawal is replaced twice by a warning, and the first warning is
    # revoked. The second warning still counts, so the withdrawal stays
    # undone and the grant counts.
    When the cell records submission "aanvraag" under article "1" of "test_chronicle_effects" with:
      | ernstig_beperkt | true |
    And the cell decides "verlening" on submission "aanvraag"
    And the cell decides "intrekking" under article "4" of "test_chronicle_effects" with:
      | ingetrokken_besluit | verlening |
    And the cell decides "eerste_waarschuwing" under article "6" of "test_chronicle_effects" with:
      | bestreden_besluit | intrekking |
    And the cell decides "tweede_waarschuwing" under article "6" of "test_chronicle_effects" with:
      | bestreden_besluit | intrekking |
    And the cell records submission "bezwaar" under article "2" of "test_chronicle_effects" with:
      | bestreden_besluit | eerste_waarschuwing |
    And the cell decides "herroeping" on submission "bezwaar" with:
      | bezwaar_gegrond | true |
    And I evaluate "heeft_geldige_kaart" of "test_reductie_geldige_kaart" over the chronicle
    Then record "verlening" is applied
    And record "intrekking" is skipped
    And record "eerste_waarschuwing" is skipped
    And record "tweede_waarschuwing" is applied
    And output "heeft_geldige_kaart" is true

  Scenario: A record recorded after the calculation date is not played
    When the cell records submission "aanvraag" under article "1" of "test_chronicle_effects" with:
      | ernstig_beperkt | true |
    And the cell decides "verlening" on submission "aanvraag"
    Given the calculation date is "2025-04-01"
    When the cell records submission "bezwaar" under article "2" of "test_chronicle_effects" with:
      | bestreden_besluit | verlening |
    And the cell decides "herroeping" on submission "bezwaar" with:
      | bezwaar_gegrond | true |
    Given the calculation date is "2025-03-15"
    When I evaluate "heeft_geldige_kaart" of "test_reductie_geldige_kaart" over the chronicle
    Then output "heeft_geldige_kaart" is true
    And record "verlening" is applied
    Given the calculation date is "2025-04-01"
    When I evaluate "heeft_geldige_kaart" of "test_reductie_geldige_kaart" over the chronicle
    Then output "heeft_geldige_kaart" is false
