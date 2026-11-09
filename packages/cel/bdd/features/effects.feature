Feature: A reduction plays the chronicle forward
  As a cell that records submissions, decisions and facts
  I want a reduction to play my chronicle forward and keep a state
  So that a decision the law undoes is skipped, and the answer is computed again

  # Exercises `projection` and `produces[].effects` (schema v0.8.15) against
  # the synthetic law test_chronicle_effects (packages/cel/bdd/fixtures),
  # which is loosely shaped after the disabled parking card and Awb 7:11 lid 2
  # and is otherwise fictional. The decision on an objection (article 2)
  # revokes the contested decision only when the objection is well-founded
  # (`when: herroepen`). Article 4 (a withdrawal of the card) and article 5
  # (handing the card in at the desk, recorded as a fact) are fictional
  # throughout: the corpus models neither.
  #
  # The reduction test_reductie_kaart_per_besluit (fixtures/cells/test_cel)
  # plays one aggregate per decision on an application: the decision and
  # every record about it, by `refers_to`, directly or through another
  # record (an objection, the decision on it, a card handed in). It keeps
  # whether the card is held (`in_bezit`) and how many grants counted
  # (`aantal_verleend`), and says nothing about revoking. The effects of the
  # law decide which records count: a record counts unless a later record
  # that itself counts revokes it. The read model test_reductie_geldige_kaart
  # reads the aggregates of the applicant. A trace table lists the steps of
  # each aggregate it read, by its root.

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
      | aggregate | record    | event   | detail                                                                     |
      | verlening | verlening | applied | test_chronicle_effects#7: aantal_verleend: 0 -> 1, in_bezit: false -> true |

  Scenario: An unfounded objection leaves the grant in place
    # Article 2 revokes only when its outcome `herroepen` is true, which
    # follows from the finding that the objection is well-founded.
    When the cell records submission "aanvraag" under article "1" of "test_chronicle_effects" with:
      | ernstig_beperkt | true |
      | aanvrager       | burger_x |
    And the cell decides "verlening" on submission "aanvraag"
    And the cell records submission "bezwaar" about "verlening" under article "2" of "test_chronicle_effects" with:
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
    And the cell records submission "bezwaar" about "verlening" under article "2" of "test_chronicle_effects" with:
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
    And the cell decides "nieuw_besluit" about "weigering" under article "3" of "test_chronicle_effects" with:
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
    And the cell decides "nieuw_besluit" about "verlening" under article "3" of "test_chronicle_effects" with:
      | bestreden_besluit | verlening |
      | ernstig_beperkt   | false     |
      | aanvrager         | burger_x          |
    And I evaluate "heeft_geldige_kaart" of "test_reductie_geldige_kaart" over the chronicle
    Then record "verlening" is skipped
    And output "heeft_geldige_kaart" is false

  Scenario: Revoking the first grant after the card was handed in and granted again
    # A revoked grant does not count, so its step never runs; nothing is
    # turned back: the card handed in and the second grant decide `in_bezit`.
    When the cell records submission "aanvraag" under article "1" of "test_chronicle_effects" with:
      | ernstig_beperkt | true |
      | aanvrager       | burger_x |
    And the cell decides "verlening" on submission "aanvraag"
    And the cell records fact "kaart_ingeleverd" as "inlevering" about "verlening" under article "5" of "test_chronicle_effects" with:
      | ingeleverd_op | 2025-03-01 |
      | aanvrager     | burger_x   |
    And the cell records submission "tweede_aanvraag" under article "1" of "test_chronicle_effects" with:
      | ernstig_beperkt | true |
      | aanvrager       | burger_x |
    And the cell decides "tweede_verlening" on submission "tweede_aanvraag"
    And the cell records submission "bezwaar" about "verlening" under article "2" of "test_chronicle_effects" with:
      | bestreden_besluit | verlening |
    And the cell decides "herroeping" on submission "bezwaar" with:
      | bezwaar_gegrond | true |
    And I evaluate "heeft_geldige_kaart" of "test_reductie_geldige_kaart" over the chronicle
    Then output "heeft_geldige_kaart" is true
    And property "in_bezit" was last set by "tweede_verlening"
    And the projection trace is:
      | aggregate        | record           | event   | detail                                                                     |
      | verlening        | verlening        | skipped | revoked by herroeping                                                      |
      | verlening        | inlevering       | applied | test_chronicle_effects#8: in_bezit: false -> false                         |
      | verlening        | bezwaar          | applied | -                                                                          |
      | verlening        | herroeping       | applied | -                                                                          |
      | tweede_verlening | tweede_verlening | applied | test_chronicle_effects#7: aantal_verleend: 0 -> 1, in_bezit: false -> true |

  Scenario: The answer reads two properties of the state
    # A card is a replacement when the applicant holds one and more than one
    # grant counted. Revoking the first grant leaves one.
    When the cell records submission "aanvraag" under article "1" of "test_chronicle_effects" with:
      | ernstig_beperkt | true |
      | aanvrager       | burger_x |
    And the cell decides "verlening" on submission "aanvraag"
    And the cell records fact "kaart_ingeleverd" as "inlevering" about "verlening" under article "5" of "test_chronicle_effects" with:
      | ingeleverd_op | 2025-03-01 |
      | aanvrager     | burger_x   |
    And the cell records submission "tweede_aanvraag" under article "1" of "test_chronicle_effects" with:
      | ernstig_beperkt | true |
      | aanvrager       | burger_x |
    And the cell decides "tweede_verlening" on submission "tweede_aanvraag"
    And I evaluate "kaart_is_vervanging" of "test_reductie_geldige_kaart" over the chronicle
    Then output "kaart_is_vervanging" is true
    When the cell records submission "bezwaar" about "verlening" under article "2" of "test_chronicle_effects" with:
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
    And the cell records submission "bezwaar" about "tweede_verlening" under article "2" of "test_chronicle_effects" with:
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
    And the cell decides "intrekking" about "verlening" under article "4" of "test_chronicle_effects" with:
      | ingetrokken_besluit | verlening |
    And I evaluate "heeft_geldige_kaart" of "test_reductie_geldige_kaart" over the chronicle
    Then record "verlening" is skipped
    And output "heeft_geldige_kaart" is false
    When the cell records submission "bezwaar" about "intrekking" under article "2" of "test_chronicle_effects" with:
      | bestreden_besluit | intrekking |
    And the cell decides "herroeping" on submission "bezwaar" with:
      | bezwaar_gegrond | true |
    And I evaluate "heeft_geldige_kaart" of "test_reductie_geldige_kaart" over the chronicle
    Then record "intrekking" is skipped
    And record "verlening" is applied
    And output "heeft_geldige_kaart" is true
    And the projection trace is:
      | aggregate | record     | event   | detail                                                                     |
      | verlening | verlening  | applied | test_chronicle_effects#7: aantal_verleend: 0 -> 1, in_bezit: false -> true |
      | verlening | intrekking | skipped | revoked by herroeping                                                      |
      | verlening | bezwaar    | applied | -                                                                          |
      | verlening | herroeping | applied | -                                                                          |

  Scenario: A decision revoked twice stays skipped when one revocation is undone
    # Two well-founded objections revoke the grant; a third revokes the first
    # revocation. The second revocation still counts, so the grant stays
    # skipped.
    When the cell records submission "aanvraag" under article "1" of "test_chronicle_effects" with:
      | ernstig_beperkt | true |
      | aanvrager       | burger_x |
    And the cell decides "verlening" on submission "aanvraag"
    And the cell records submission "bezwaar_1" about "verlening" under article "2" of "test_chronicle_effects" with:
      | bestreden_besluit | verlening |
    And the cell decides "herroeping_1" on submission "bezwaar_1" with:
      | bezwaar_gegrond | true |
    And the cell records submission "bezwaar_2" about "verlening" under article "2" of "test_chronicle_effects" with:
      | bestreden_besluit | verlening |
    And the cell decides "herroeping_2" on submission "bezwaar_2" with:
      | bezwaar_gegrond | true |
    And the cell records submission "bezwaar_3" about "herroeping_1" under article "2" of "test_chronicle_effects" with:
      | bestreden_besluit | herroeping_1 |
    And the cell decides "herroeping_3" on submission "bezwaar_3" with:
      | bezwaar_gegrond | true |
    And I evaluate "heeft_geldige_kaart" of "test_reductie_geldige_kaart" over the chronicle
    Then record "herroeping_1" is skipped
    And record "herroeping_2" is applied
    And record "verlening" is skipped
    And output "heeft_geldige_kaart" is false
    And the projection trace is:
      | aggregate | record       | event   | detail                  |
      | verlening | verlening    | skipped | revoked by herroeping_2 |
      | verlening | bezwaar_1    | applied | -                       |
      | verlening | herroeping_1 | skipped | revoked by herroeping_3 |
      | verlening | bezwaar_2    | applied | -                       |
      | verlening | herroeping_2 | applied | -                       |
      | verlening | bezwaar_3    | applied | -                       |
      | verlening | herroeping_3 | applied | -                       |

  Scenario: Replacing a replacement undoes its replacing
    # A record that is replaced undoes nothing any more: the first
    # replacement no longer revokes the grant, which counts again beside
    # the second replacement. A replacement that should keep the grant
    # replaced would say so with an effect of its own.
    When the cell records submission "aanvraag" under article "1" of "test_chronicle_effects" with:
      | ernstig_beperkt | true |
      | aanvrager       | burger_x |
    And the cell decides "verlening" on submission "aanvraag"
    And the cell decides "eerste_vervanging" about "verlening" under article "3" of "test_chronicle_effects" with:
      | bestreden_besluit | verlening |
      | ernstig_beperkt   | false     |
      | aanvrager         | burger_x          |
    And the cell decides "tweede_vervanging" about "eerste_vervanging" under article "3" of "test_chronicle_effects" with:
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
    # counts again and revokes the grant again.
    When the cell records submission "aanvraag" under article "1" of "test_chronicle_effects" with:
      | ernstig_beperkt | true |
      | aanvrager       | burger_x |
    And the cell decides "verlening" on submission "aanvraag"
    And the cell decides "eerste_vervanging" about "verlening" under article "3" of "test_chronicle_effects" with:
      | bestreden_besluit | verlening |
      | ernstig_beperkt   | false     |
      | aanvrager         | burger_x          |
    And the cell decides "tweede_vervanging" about "eerste_vervanging" under article "3" of "test_chronicle_effects" with:
      | bestreden_besluit | eerste_vervanging |
      | ernstig_beperkt   | true              |
      | aanvrager         | burger_x          |
    And the cell records submission "bezwaar" about "tweede_vervanging" under article "2" of "test_chronicle_effects" with:
      | bestreden_besluit | tweede_vervanging |
    And the cell decides "herroeping" on submission "bezwaar" with:
      | bezwaar_gegrond | true |
    And I evaluate "heeft_geldige_kaart" of "test_reductie_geldige_kaart" over the chronicle
    Then record "tweede_vervanging" is skipped
    And record "eerste_vervanging" is applied
    And record "verlening" is skipped
    And output "heeft_geldige_kaart" is false

  Scenario: A warning in the place of a withdrawal brings the card back
    # On objection the college gives a warning in the place of the withdrawal
    # (fictional article 6). The withdrawal undoes nothing any more.
    When the cell records submission "aanvraag" under article "1" of "test_chronicle_effects" with:
      | ernstig_beperkt | true |
      | aanvrager       | burger_x |
    And the cell decides "verlening" on submission "aanvraag"
    And the cell decides "intrekking" about "verlening" under article "4" of "test_chronicle_effects" with:
      | ingetrokken_besluit | verlening |
    And the cell decides "waarschuwing" about "intrekking" under article "6" of "test_chronicle_effects" with:
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
    And the cell decides "intrekking" about "verlening" under article "4" of "test_chronicle_effects" with:
      | ingetrokken_besluit | verlening |
    And the cell decides "eerste_waarschuwing" about "intrekking" under article "6" of "test_chronicle_effects" with:
      | bestreden_besluit | intrekking |
    And the cell decides "tweede_waarschuwing" about "intrekking" under article "6" of "test_chronicle_effects" with:
      | bestreden_besluit | intrekking |
    And the cell records submission "bezwaar" about "eerste_waarschuwing" under article "2" of "test_chronicle_effects" with:
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
    When the cell records submission "bezwaar" about "verlening" under article "2" of "test_chronicle_effects" with:
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
    And the cell records submission "bezwaar" about "verlening_x" under article "2" of "test_chronicle_effects" with:
      | bestreden_besluit | verlening_x |
    And the cell decides "herroeping" on submission "bezwaar" with:
      | bezwaar_gegrond | true |
    And the cell records submission "bezwaar_2" about "herroeping" under article "2" of "test_chronicle_effects" with:
      | bestreden_besluit | herroeping |
    And the cell decides "herroeping_2" on submission "bezwaar_2" with:
      | bezwaar_gegrond | true |
    And I evaluate "heeft_geldige_kaart" of "test_reductie_geldige_kaart" over the chronicle
    Then output "heeft_geldige_kaart" is true
    And the projection trace is:
      | aggregate   | record       | event   | detail                                                                     |
      | verlening_x | verlening_x  | applied | test_chronicle_effects#7: aantal_verleend: 0 -> 1, in_bezit: false -> true |
      | verlening_x | bezwaar      | applied | -                                                                          |
      | verlening_x | herroeping   | skipped | revoked by herroeping_2                                                    |
      | verlening_x | bezwaar_2    | applied | -                                                                          |
      | verlening_x | herroeping_2 | applied | -                                                                          |
    Given parameter "aanvrager" is "burger_y"
    When I evaluate "heeft_geldige_kaart" of "test_reductie_geldige_kaart" over the chronicle
    Then output "heeft_geldige_kaart" is true
    And the projection trace is:
      | aggregate   | record      | event   | detail                                                                     |
      | verlening_y | verlening_y | applied | test_chronicle_effects#7: aantal_verleend: 0 -> 1, in_bezit: false -> true |

  Scenario: A permit moved to another licence plate no longer holds for the old plate
    # Fictional test_parkeervergunning: article 2 moves a permit to another
    # plate in the place of the earlier one. The move is about the permit,
    # so it plays in the permit's aggregate: the permit no longer counts,
    # and the move puts the new plate in the state. The old plate has no
    # permit any more; the resident's permit is for the new plate.
    When the cell decides "vergunning" under article "1" of "test_parkeervergunning" with:
      | bewoner  | burger_x |
      | kenteken | AB-12-CD |
    And the cell decides "omzetting" about "vergunning" under article "2" of "test_parkeervergunning" with:
      | oude_vergunning | vergunning |
      | bewoner         | burger_x   |
      | kenteken        | XY-34-ZZ   |
    Given parameter "kenteken" is "AB-12-CD"
    When I evaluate "kenteken_heeft_vergunning" of "test_reductie_vergunning_per_kenteken" over the chronicle
    Then output "kenteken_heeft_vergunning" is false
    And the projection trace is:
      | aggregate  | record     | event   | detail                                                                          |
      | vergunning | vergunning | skipped | revoked by omzetting                                                            |
      | vergunning | omzetting  | applied | test_parkeervergunning#4: kenteken: null -> XY-34-ZZ, vergunning: false -> true |
    Given parameter "kenteken" is "XY-34-ZZ"
    When I evaluate "kenteken_heeft_vergunning" of "test_reductie_vergunning_per_kenteken" over the chronicle
    Then output "kenteken_heeft_vergunning" is true
    Given parameter "bewoner" is "burger_x"
    When I evaluate "kenteken_van_bewoner" of "test_reductie_vergunning_per_bewoner" over the chronicle
    Then output "kenteken_van_bewoner" equals "XY-34-ZZ"
    And record "vergunning" is skipped

  Scenario: Revoking the move brings the permit back to the old plate
    # The revocation is about the move, and so about the permit too: it
    # plays in the same aggregate. Undoing the move revives the permit for
    # the old plate.
    When the cell decides "vergunning" under article "1" of "test_parkeervergunning" with:
      | bewoner  | burger_x |
      | kenteken | AB-12-CD |
    And the cell decides "omzetting" about "vergunning" under article "2" of "test_parkeervergunning" with:
      | oude_vergunning | vergunning |
      | bewoner         | burger_x   |
      | kenteken        | XY-34-ZZ   |
    And the cell decides "herroeping" about "omzetting" under article "3" of "test_parkeervergunning" with:
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

  Scenario: A withdrawal without a submission is about the grant it withdraws
    # A withdrawal ex officio (fictional article 4) is taken on no
    # submission: its refers_to names the grant it affects, so it plays in
    # that grant's aggregate and revokes it.
    When the cell records submission "aanvraag" under article "1" of "test_chronicle_effects" with:
      | ernstig_beperkt | true     |
      | aanvrager       | burger_x |
    And the cell decides "verlening" on submission "aanvraag"
    And the cell decides "intrekking" about "verlening" under article "4" of "test_chronicle_effects" with:
      | ingetrokken_besluit | verlening |
    And I evaluate "heeft_geldige_kaart" of "test_reductie_geldige_kaart" over the chronicle
    Then output "heeft_geldige_kaart" is false
    And the projection trace is:
      | aggregate | record     | event   | detail                |
      | verlening | verlening  | skipped | revoked by intrekking |
      | verlening | intrekking | applied | -                     |

  Scenario: A decision revokes only a record it is about
    # The withdrawal is about the second grant but names the first in its
    # effect. From schema v0.8.15 a decision revokes only a record its
    # refers_to reaches, directly or through what that record refers to, so
    # the cell refuses to record it.
    When the cell records submission "aanvraag" under article "1" of "test_chronicle_effects" with:
      | ernstig_beperkt | true     |
      | aanvrager       | burger_x |
    And the cell decides "verlening" on submission "aanvraag"
    And the cell records submission "tweede_aanvraag" under article "1" of "test_chronicle_effects" with:
      | ernstig_beperkt | true     |
      | aanvrager       | burger_x |
    And the cell decides "tweede_verlening" on submission "tweede_aanvraag"
    And the cell decides "intrekking" about "tweede_verlening" under article "4" of "test_chronicle_effects" with:
      | ingetrokken_besluit | verlening |
    Then the cell refuses with "revokes 'verlening' (ingetrokken_besluit), which its refers_to does not reach"
    When I evaluate "heeft_geldige_kaart" of "test_reductie_geldige_kaart" over the chronicle
    Then output "heeft_geldige_kaart" is true

  Scenario: A policy takes the cell's answer, and a well-founded objection flips it
    # The normal route: the law (fictional article 9) leaves whether the
    # applicant holds a valid card to the college, whose policy binds that
    # term to the answer of its cell (`source.reduction`). The cell gives the
    # answer from test_reductie_geldige_kaart, which reads the aggregates of
    # the applicant the binding passes.
    When the cell records submission "aanvraag" under article "1" of "test_chronicle_effects" with:
      | ernstig_beperkt | true     |
      | aanvrager       | burger_x |
    And the cell decides "verlening" on submission "aanvraag"
    And I evaluate "mag_parkeren" of "test_chronicle_effects" with the cell answering "heeft_geldige_kaart" from "test_reductie_geldige_kaart"
    Then output "mag_parkeren" is true
    When the cell records submission "bezwaar" about "verlening" under article "2" of "test_chronicle_effects" with:
      | bestreden_besluit | verlening |
    And the cell decides "herroeping" on submission "bezwaar" with:
      | bezwaar_gegrond | true |
    And I evaluate "mag_parkeren" of "test_chronicle_effects" with the cell answering "heeft_geldige_kaart" from "test_reductie_geldige_kaart"
    Then output "mag_parkeren" is false

  Scenario: A read model compares the day asked about when it answers
    # Fictional test_bedrijfsvergunning: article 1 grants a permit valid up to
    # a day, article 2 says which permits count on a given day. Each decision
    # is an aggregate of test_reductie_vergunning_per_besluit (schema
    # v0.8.15); the read model passes their rows and the day to article 2:
    # the same chronicle answers differently for another day, and the same
    # question always gets the same answer.
    When the cell records submission "aanvraag_1" under article "1" of "test_bedrijfsvergunning" with:
      | bedrijf    | bedrijf_x  |
      | geldig_tot | 2025-06-30 |
    And the cell decides "vergunning_1" on submission "aanvraag_1"
    And the cell records submission "aanvraag_2" under article "1" of "test_bedrijfsvergunning" with:
      | bedrijf    | bedrijf_x  |
      | geldig_tot | 2026-06-30 |
    And the cell decides "vergunning_2" on submission "aanvraag_2"
    Given parameter "bedrijf" is "bedrijf_x"
    And parameter "peildatum" is "2025-03-01"
    When I evaluate "aantal_geldige_vergunningen" of "test_reductie_geldige_vergunningen_op_peildatum" over the chronicle
    Then output "aantal_geldige_vergunningen" equals 2
    Given parameter "peildatum" is "2025-12-01"
    When I evaluate "aantal_geldige_vergunningen" of "test_reductie_geldige_vergunningen_op_peildatum" over the chronicle
    Then output "aantal_geldige_vergunningen" equals 1
