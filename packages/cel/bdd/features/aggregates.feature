Feature: One aggregate per decision, read as rows
  As a cell that records decisions and what refers to them
  I want a projection that plays one aggregate per decision
  So that a read model can count the decisions, each with its own state

  # Exercises `projection.per` and `source.projections` (schema v0.8.17)
  # against the synthetic law test_chronicle_effects. The reduction
  # test_reductie_kaart_per_verlening (fixtures/cells/test_cel) plays one
  # aggregate per decision on an application under article 1: the decision,
  # and the records that refer to it, such as a decision on an objection
  # against it (article 2). The read model test_reductie_kaarten_van_aanvrager
  # reads the aggregates of the applicant and counts them; it plays nothing
  # itself. Which records count follows from the effects of the law, as in
  # any projection.

  Background:
    Given the calculation date is "2025-03-01"
    And parameter "aanvrager" is "burger_x"
    When the cell records submission "aanvraag_1" under article "1" of "test_chronicle_effects" with:
      | ernstig_beperkt | true     |
      | aanvrager       | burger_x |
    And the cell decides "verlening_1" on submission "aanvraag_1"
    And the cell records submission "aanvraag_2" under article "1" of "test_chronicle_effects" with:
      | ernstig_beperkt | true     |
      | aanvrager       | burger_x |
    And the cell decides "verlening_2" on submission "aanvraag_2"

  Scenario: Each decision is an aggregate of its own
    When I evaluate "aantal_kaarten" of "test_reductie_kaarten_van_aanvrager" over the chronicle
    Then output "aantal_kaarten" equals 2

  Scenario: An aggregate and its read model share one file
    # test_reductie_kaarten_in_een_bestand (schema v0.8.19) holds the
    # aggregate per decision in its article verlening and the read model in
    # its article aantal_kaarten, which names that article.
    When I evaluate "kaarten_in_een_bestand" of "test_reductie_kaarten_in_een_bestand" over the chronicle
    Then output "kaarten_in_een_bestand" equals 2

  Scenario: Another applicant's decisions are other aggregates
    When the cell records submission "aanvraag_y" under article "1" of "test_chronicle_effects" with:
      | ernstig_beperkt | true     |
      | aanvrager       | burger_y |
    And the cell decides "verlening_y" on submission "aanvraag_y"
    And I evaluate "aantal_kaarten" of "test_reductie_kaarten_van_aanvrager" over the chronicle
    Then output "aantal_kaarten" equals 2

  Scenario: A record of another kind writes the aggregate of the decision it refers to
    # The decision on an unfounded objection refers to the grant it
    # contests, so it plays in that grant's aggregate and its step writes it.
    When the cell records submission "bezwaar" about "verlening_1" under article "2" of "test_chronicle_effects" with:
      | bestreden_besluit | verlening_1 |
    And the cell decides "beslissing" on submission "bezwaar" with:
      | bezwaar_gegrond | false |
    And I evaluate "aantal_met_bezwaar" of "test_reductie_kaarten_van_aanvrager" over the chronicle
    Then output "aantal_met_bezwaar" equals 1
    When I evaluate "aantal_kaarten" of "test_reductie_kaarten_van_aanvrager" over the chronicle
    Then output "aantal_kaarten" equals 2

  Scenario: A revoked decision applies no step in its aggregate
    # The well-founded objection revokes the first grant. Its aggregate
    # stays, and the decision on the objection still writes it, but the
    # grant itself sets nothing.
    When the cell records submission "bezwaar" about "verlening_1" under article "2" of "test_chronicle_effects" with:
      | bestreden_besluit | verlening_1 |
    And the cell decides "herroeping" on submission "bezwaar" with:
      | bezwaar_gegrond | true |
    And I evaluate "aantal_kaarten" of "test_reductie_kaarten_van_aanvrager" over the chronicle
    Then output "aantal_kaarten" equals 1
    When I evaluate "aantal_met_bezwaar" of "test_reductie_kaarten_van_aanvrager" over the chronicle
    Then output "aantal_met_bezwaar" equals 1

  Scenario: A decision whose revocation is revoked counts again
    # An objection against the revocation is well-founded too: it revokes
    # the revocation, so the first grant counts again in its aggregate.
    When the cell records submission "bezwaar" about "verlening_1" under article "2" of "test_chronicle_effects" with:
      | bestreden_besluit | verlening_1 |
    And the cell decides "herroeping" on submission "bezwaar" with:
      | bezwaar_gegrond | true |
    And the cell records submission "tweede_bezwaar" about "herroeping" under article "2" of "test_chronicle_effects" with:
      | bestreden_besluit | herroeping |
    And the cell decides "tweede_herroeping" on submission "tweede_bezwaar" with:
      | bezwaar_gegrond | true |
    And I evaluate "aantal_kaarten" of "test_reductie_kaarten_van_aanvrager" over the chronicle
    Then output "aantal_kaarten" equals 2

  Scenario: A decision recorded after the calculation date is no aggregate yet
    Given the calculation date is "2025-04-01"
    When the cell records submission "aanvraag_3" under article "1" of "test_chronicle_effects" with:
      | ernstig_beperkt | true     |
      | aanvrager       | burger_x |
    And the cell decides "verlening_3" on submission "aanvraag_3"
    Given the calculation date is "2025-03-01"
    When I evaluate "aantal_kaarten" of "test_reductie_kaarten_van_aanvrager" over the chronicle
    Then output "aantal_kaarten" equals 2
    Given the calculation date is "2025-04-01"
    When I evaluate "aantal_kaarten" of "test_reductie_kaarten_van_aanvrager" over the chronicle
    Then output "aantal_kaarten" equals 3

  Scenario: A projection names the fact of its records when an article requires two
    # Article 4 of test_chronicle_corrections requires the rent of a place
    # and its termination. The reduction test_reductie_plaats_per_huur
    # addresses each by its fact name (schema v0.8.18): one aggregate per
    # rent, and the termination that refers to it ends that rent alone.
    Given parameter "bewoner" is "burger_x"
    When the cell records fact "plaats_gehuurd" as "huur_1" under article "4" of "test_chronicle_corrections" with:
      | bewoner      | burger_x |
      | plaatsnummer | P-12     |
    And the cell records fact "plaats_gehuurd" as "huur_2" under article "4" of "test_chronicle_corrections" with:
      | bewoner      | burger_x |
      | plaatsnummer | P-14     |
    And I evaluate "aantal_plaatsen" of "test_reductie_plaatsen_van_bewoner" over the chronicle
    Then output "aantal_plaatsen" equals 2
    When the cell records fact "huur_opgezegd" as "opzegging" about "huur_1" under article "4" of "test_chronicle_corrections" with:
      | einde_huur | 2025-02-28 |
    And I evaluate "aantal_plaatsen" of "test_reductie_plaatsen_van_bewoner" over the chronicle
    Then output "aantal_plaatsen" equals 1
