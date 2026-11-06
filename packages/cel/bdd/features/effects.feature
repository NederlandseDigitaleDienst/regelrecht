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
    And parameter "aanvrager" is "burger_x"

  Scenario: A card granted on application is valid
    When the cell records submission "aanvraag" under article "1" of "test_chronicle_effects" with:
      | ernstig_beperkt | true |
      | aanvrager       | burger_x |
    And the cell decides "verlening" on submission "aanvraag"
    And I evaluate "heeft_geldige_kaart" of "test_reductie_geldige_kaart" over the chronicle
    Then output "heeft_geldige_kaart" is true
    And property "in_bezit" was last set by "verlening"
    And the projection trace is:
      | record    | event   | detail                                             |
      | verlening | applied | aantal_verleend: 0 -> 1, in_bezit: false -> true   |

  Scenario: An unfounded objection leaves the grant in place
    # Article 2 revokes only when its outcome `herroepen` is true, which
    # follows from the finding that the objection is well-founded.
    When the cell records submission "aanvraag" under article "1" of "test_chronicle_effects" with:
      | ernstig_beperkt | true |
      | aanvrager       | burger_x |
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
      | aanvrager       | burger_x |
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
      | aanvrager       | burger_x |
    And the cell decides "weigering" on submission "aanvraag"
    And the cell decides "nieuw_besluit" under article "3" of "test_chronicle_effects" with:
      | bestreden_besluit | weigering |
      | ernstig_beperkt   | true      |
      | aanvrager         | burger_x          |
    And I evaluate "heeft_geldige_kaart" of "test_reductie_geldige_kaart" over the chronicle
    Then record "weigering" is skipped
    And record "nieuw_besluit" is applied
    And output "heeft_geldige_kaart" is true

  Scenario: A replaced grant no longer counts
    When the cell records submission "aanvraag" under article "1" of "test_chronicle_effects" with:
      | ernstig_beperkt | true |
      | aanvrager       | burger_x |
    And the cell decides "verlening" on submission "aanvraag"
    And the cell decides "nieuw_besluit" under article "3" of "test_chronicle_effects" with:
      | bestreden_besluit | verlening |
      | ernstig_beperkt   | false     |
      | aanvrager         | burger_x          |
    And I evaluate "heeft_geldige_kaart" of "test_reductie_geldige_kaart" over the chronicle
    Then record "verlening" is skipped
    And output "heeft_geldige_kaart" is false

  Scenario: Revoking the first grant after the card was handed in and granted again
    # Undoing is replaying without the revoked grant, not turning its changes
    # back: the card handed in and the second grant still decide `in_bezit`.
    When the cell records submission "aanvraag" under article "1" of "test_chronicle_effects" with:
      | ernstig_beperkt | true |
      | aanvrager       | burger_x |
    And the cell decides "verlening" on submission "aanvraag"
    And the cell records fact "kaart_ingeleverd" as "inlevering" under article "5" of "test_chronicle_effects" with:
      | ingeleverd_op | 2025-03-01 |
      | aanvrager     | burger_x   |
    And the cell records submission "tweede_aanvraag" under article "1" of "test_chronicle_effects" with:
      | ernstig_beperkt | true |
      | aanvrager       | burger_x |
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
      | verlening        | applied | aantal_verleend: 0 -> 1, in_bezit: false -> true |
      | inlevering       | applied | in_bezit: true -> false                          |
      | tweede_verlening | applied | aantal_verleend: 1 -> 2, in_bezit: false -> true |
      | bezwaar          | applied | -                                                |
      | herroeping       | rewound | revokes verlening                                |
      | verlening        | skipped | undone by herroeping                             |
      | inlevering       | applied | in_bezit: false -> false                         |
      | tweede_verlening | applied | aantal_verleend: 0 -> 1, in_bezit: false -> true |
      | bezwaar          | applied | -                                                |
      | herroeping       | applied | -                                                |

  Scenario: The answer reads two properties of the state
    # A card is a replacement when the applicant holds one and more than one
    # grant counted. Revoking the first grant leaves one.
    When the cell records submission "aanvraag" under article "1" of "test_chronicle_effects" with:
      | ernstig_beperkt | true |
      | aanvrager       | burger_x |
    And the cell decides "verlening" on submission "aanvraag"
    And the cell records fact "kaart_ingeleverd" as "inlevering" under article "5" of "test_chronicle_effects" with:
      | ingeleverd_op | 2025-03-01 |
      | aanvrager     | burger_x   |
    And the cell records submission "tweede_aanvraag" under article "1" of "test_chronicle_effects" with:
      | ernstig_beperkt | true |
      | aanvrager       | burger_x |
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
      | aanvrager       | burger_x |
    And the cell decides "verlening" on submission "aanvraag"
    And I evaluate "heeft_geldige_kaart" of "test_reductie_geldige_kaart" over the chronicle
    Then output "heeft_geldige_kaart" is true
    When the cell records submission "tweede_aanvraag" under article "1" of "test_chronicle_effects" with:
      | ernstig_beperkt | true |
      | aanvrager       | burger_x |
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
      | aanvrager       | burger_x |
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
      | verlening  | applied | aantal_verleend: 0 -> 1, in_bezit: false -> true |
      | intrekking | rewound | revokes verlening                                |
      | verlening  | skipped | undone by intrekking                             |
      | intrekking | applied | -                                                |
      | bezwaar    | applied | -                                                |
      | herroeping | rewound | revokes intrekking                               |
      | verlening  | skipped | undone by intrekking                             |
      | intrekking | skipped | undone by herroeping                             |
      | intrekking | rewound | revives verlening                                |
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
      | aanvrager       | burger_x |
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
      | verlening    | applied | aantal_verleend: 0 -> 1, in_bezit: false -> true |
      | bezwaar_1    | applied | -                                                |
      | herroeping_1 | rewound | revokes verlening                                |
      | verlening    | skipped | undone by herroeping_1, herroeping_2             |
      | bezwaar_1    | applied | -                                                |
      | herroeping_1 | applied | -                                                |
      | bezwaar_2    | applied | -                                                |
      | herroeping_2 | applied | -                                                |
      | bezwaar_3    | applied | -                                                |
      | herroeping_3 | rewound | revokes herroeping_1                             |
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
      | aanvrager       | burger_x |
    And the cell decides "verlening" on submission "aanvraag"
    And the cell decides "eerste_vervanging" under article "3" of "test_chronicle_effects" with:
      | bestreden_besluit | verlening |
      | ernstig_beperkt   | false     |
      | aanvrager         | burger_x          |
    And the cell decides "tweede_vervanging" under article "3" of "test_chronicle_effects" with:
      | bestreden_besluit | eerste_vervanging |
      | ernstig_beperkt   | false             |
      | aanvrager         | burger_x          |
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
      | aanvrager       | burger_x |
    And the cell decides "verlening" on submission "aanvraag"
    And the cell decides "eerste_vervanging" under article "3" of "test_chronicle_effects" with:
      | bestreden_besluit | verlening |
      | ernstig_beperkt   | false     |
      | aanvrager         | burger_x          |
    And the cell decides "tweede_vervanging" under article "3" of "test_chronicle_effects" with:
      | bestreden_besluit | eerste_vervanging |
      | ernstig_beperkt   | true              |
      | aanvrager         | burger_x          |
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
      | aanvrager       | burger_x |
    And the cell decides "verlening" on submission "aanvraag"
    And the cell decides "intrekking" under article "4" of "test_chronicle_effects" with:
      | ingetrokken_besluit | verlening |
    And the cell decides "waarschuwing" under article "6" of "test_chronicle_effects" with:
      | bestreden_besluit | intrekking |
    And I evaluate "heeft_geldige_kaart" of "test_reductie_geldige_kaart" over the chronicle
    Then record "intrekking" is skipped
    And record "verlening" is applied
    And output "heeft_geldige_kaart" is true

  Scenario: A withdrawal replaced twice stays undone when one replacement is revoked
    # A withdrawal is replaced twice by a warning, and the first warning is
    # revoked. The second warning still counts, so the withdrawal stays
    # undone and the grant counts.
    When the cell records submission "aanvraag" under article "1" of "test_chronicle_effects" with:
      | ernstig_beperkt | true |
      | aanvrager       | burger_x |
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
      | aanvrager       | burger_x |
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

  Scenario: Two applicants in one chronicle each see only their own records
    # The objection and the decision on it carry no applicant; they belong to
    # the applicant whose grant they refer to, and so does the revocation of
    # that decision. Applicant Y sees none of it.
    When the cell records submission "aanvraag_x" under article "1" of "test_chronicle_effects" with:
      | ernstig_beperkt | true     |
      | aanvrager       | burger_x |
    And the cell decides "verlening_x" on submission "aanvraag_x"
    And the cell records submission "aanvraag_y" under article "1" of "test_chronicle_effects" with:
      | ernstig_beperkt | true     |
      | aanvrager       | burger_y |
    And the cell decides "verlening_y" on submission "aanvraag_y"
    And the cell records submission "bezwaar" under article "2" of "test_chronicle_effects" with:
      | bestreden_besluit | verlening_x |
    And the cell decides "herroeping" on submission "bezwaar" with:
      | bezwaar_gegrond | true |
    And the cell records submission "bezwaar_2" under article "2" of "test_chronicle_effects" with:
      | bestreden_besluit | herroeping |
    And the cell decides "herroeping_2" on submission "bezwaar_2" with:
      | bezwaar_gegrond | true |
    And I evaluate "heeft_geldige_kaart" of "test_reductie_geldige_kaart" over the chronicle
    Then output "heeft_geldige_kaart" is true
    And the projection trace is:
      | record       | event   | detail                                           |
      | verlening_x  | applied | aantal_verleend: 0 -> 1, in_bezit: false -> true |
      | bezwaar      | applied | -                                                |
      | herroeping   | rewound | revokes verlening_x                              |
      | verlening_x  | skipped | undone by herroeping                             |
      | bezwaar      | applied | -                                                |
      | herroeping   | applied | -                                                |
      | bezwaar_2    | applied | -                                                |
      | herroeping_2 | rewound | revokes herroeping                               |
      | verlening_x  | skipped | undone by herroeping                             |
      | bezwaar      | applied | -                                                |
      | herroeping   | skipped | undone by herroeping_2                           |
      | herroeping   | rewound | revives verlening_x                              |
      | verlening_x  | applied | aantal_verleend: 0 -> 1, in_bezit: false -> true |
      | bezwaar      | applied | -                                                |
      | herroeping   | skipped | undone by herroeping_2                           |
      | bezwaar_2    | applied | -                                                |
      | herroeping_2 | applied | -                                                |
    Given parameter "aanvrager" is "burger_y"
    When I evaluate "heeft_geldige_kaart" of "test_reductie_geldige_kaart" over the chronicle
    Then output "heeft_geldige_kaart" is true
    And the projection trace is:
      | record      | event   | detail                                           |
      | verlening_y | applied | aantal_verleend: 0 -> 1, in_bezit: false -> true |

  Scenario: A decision of another applicant in the place of a grant replaces it for both
    # A decision in the place of X's grant that carries applicant Y is Y's
    # record, but what it does with X's grant is law, whoever the question is
    # about: X's grant no longer counts. For X it counts for its effect only,
    # so its step gives X no card. For Y it is Y's own record, and the grant
    # it replaces is outside Y's records, so that effect is not Y's business.
    When the cell records submission "aanvraag_x" under article "1" of "test_chronicle_effects" with:
      | ernstig_beperkt | true     |
      | aanvrager       | burger_x |
    And the cell decides "verlening_x" on submission "aanvraag_x"
    And the cell decides "besluit_y" under article "3" of "test_chronicle_effects" with:
      | bestreden_besluit | verlening_x |
      | ernstig_beperkt   | true        |
      | aanvrager         | burger_y    |
    And I evaluate "heeft_geldige_kaart" of "test_reductie_geldige_kaart" over the chronicle
    Then output "heeft_geldige_kaart" is false
    And record "verlening_x" is skipped
    And the projection trace is:
      | record      | event        | detail                                           |
      | verlening_x | applied      | aantal_verleend: 0 -> 1, in_bezit: false -> true |
      | besluit_y   | rewound      | replaces verlening_x                             |
      | verlening_x | skipped      | undone by besluit_y                              |
      | besluit_y   | effects only | key differs: aanvrager                           |
    Given parameter "aanvrager" is "burger_y"
    When I evaluate "heeft_geldige_kaart" of "test_reductie_geldige_kaart" over the chronicle
    Then output "heeft_geldige_kaart" is true
    And the projection trace is:
      | record    | event   | detail                                           |
      | besluit_y | applied | aantal_verleend: 0 -> 1, in_bezit: false -> true |

  Scenario: A permit moved to another licence plate no longer holds for the old plate
    # Fictional test_parkeervergunning: article 2 moves a permit to another
    # plate in the place of the earlier one. Keyed by plate, the move carries
    # the new plate, so it counts for the old plate for its effect only: the
    # old plate has no permit any more. Keyed by resident, both records are
    # the resident's, and the permit is for the new plate.
    When the cell decides "vergunning" under article "1" of "test_parkeervergunning" with:
      | bewoner  | burger_x |
      | kenteken | AB-12-CD |
    And the cell decides "omzetting" under article "2" of "test_parkeervergunning" with:
      | oude_vergunning | vergunning |
      | bewoner         | burger_x   |
      | kenteken        | XY-34-ZZ   |
    Given parameter "kenteken" is "AB-12-CD"
    When I evaluate "kenteken_heeft_vergunning" of "test_reductie_vergunning_per_kenteken" over the chronicle
    Then output "kenteken_heeft_vergunning" is false
    And the projection trace is:
      | record     | event        | detail                      |
      | vergunning | applied      | vergunning: false -> true   |
      | omzetting  | rewound      | replaces vergunning         |
      | vergunning | skipped      | undone by omzetting         |
      | omzetting  | effects only | key differs: kenteken       |
    Given parameter "kenteken" is "XY-34-ZZ"
    When I evaluate "kenteken_heeft_vergunning" of "test_reductie_vergunning_per_kenteken" over the chronicle
    Then output "kenteken_heeft_vergunning" is true
    Given parameter "bewoner" is "burger_x"
    When I evaluate "kenteken_van_bewoner" of "test_reductie_vergunning_per_bewoner" over the chronicle
    Then output "kenteken_van_bewoner" equals "XY-34-ZZ"
    And record "vergunning" is skipped

  Scenario: Revoking the move brings the permit back to the old plate
    # The revocation carries no plate. It refers only to the move, a record
    # of the new plate, so for the old plate it too counts for its effect
    # only. Undoing the move revives the permit for the old plate.
    When the cell decides "vergunning" under article "1" of "test_parkeervergunning" with:
      | bewoner  | burger_x |
      | kenteken | AB-12-CD |
    And the cell decides "omzetting" under article "2" of "test_parkeervergunning" with:
      | oude_vergunning | vergunning |
      | bewoner         | burger_x   |
      | kenteken        | XY-34-ZZ   |
    And the cell decides "herroeping" under article "3" of "test_parkeervergunning" with:
      | herroepen_besluit | omzetting |
    Given parameter "kenteken" is "AB-12-CD"
    When I evaluate "kenteken_heeft_vergunning" of "test_reductie_vergunning_per_kenteken" over the chronicle
    Then output "kenteken_heeft_vergunning" is true
    And record "vergunning" is applied
    And record "omzetting" is skipped
    And property "vergunning" was last set by "vergunning"
    Given parameter "kenteken" is "XY-34-ZZ"
    When I evaluate "kenteken_heeft_vergunning" of "test_reductie_vergunning_per_kenteken" over the chronicle
    Then output "kenteken_heeft_vergunning" is false
