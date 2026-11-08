Feature: A ruling received as a copy revokes the decision it annuls
  As a cell whose decision the court annuls
  I want the receipt of the court's ruling to revoke my decision
  So that my answer follows the court without the court writing in my chronicle

  # Schema v0.8.15 lets a fact have effects (`requires[].effects`), as a
  # decision has them (`produces[].effects`). The court is a cell of its own:
  # it records the appeal and its ruling in its own chronicle, under fictional
  # article 10 of test_chronicle_effects (after Awb 8:72 lid 1). The cell
  # records that it received a copy of the ruling with `Cell::record_receipt`
  # (RFC-053), as the fact of fictional article 11 (after Awb 8:79). The
  # receipt refers to the decision the copy concerns and takes from the copy
  # whether the ruling annuls it. Article 11 says that the receipt revokes
  # the decision it refers to when the ruling annuls it. The reduction
  # test_reductie_geldige_kaart has no step for any of this.

  Background:
    Given the calculation date is "2025-03-01"
    And parameter "aanvrager" is "burger_x"
    When the cell records submission "aanvraag" under article "1" of "test_chronicle_effects" with:
      | ernstig_beperkt | true     |
      | aanvrager       | burger_x |
    And the cell decides "verlening" on submission "aanvraag"

  Scenario: The receipt of a ruling that annuls the grant revokes it
    Given the calculation date is "2025-05-01"
    When the court records appeal "beroep" under article "10" of "test_chronicle_effects" with:
      | bestreden_besluit | verlening |
    And the court rules "uitspraak" on appeal "beroep" with:
      | beroep_gegrond | true |
    Given the calculation date is "2025-05-10"
    When the cell receives ruling "uitspraak" as fact "afschrift_uitspraak_ontvangen" "ontvangst" under article "11" of "test_chronicle_effects"
    And I evaluate "heeft_geldige_kaart" of "test_reductie_geldige_kaart" over the chronicle
    Then output "heeft_geldige_kaart" is false
    And the projection trace is:
      | record    | event   | detail               |
      | aanvraag  | applied | -                    |
      | verlening | skipped | revoked by ontvangst |
      | ontvangst | applied | -                    |

  Scenario: The receipt of a ruling that dismisses the appeal revokes nothing
    Given the calculation date is "2025-05-01"
    When the court records appeal "beroep" under article "10" of "test_chronicle_effects" with:
      | bestreden_besluit | verlening |
    And the court rules "uitspraak" on appeal "beroep" with:
      | beroep_gegrond | false |
    Given the calculation date is "2025-05-10"
    When the cell receives ruling "uitspraak" as fact "afschrift_uitspraak_ontvangen" "ontvangst" under article "11" of "test_chronicle_effects"
    And I evaluate "heeft_geldige_kaart" of "test_reductie_geldige_kaart" over the chronicle
    Then output "heeft_geldige_kaart" is true
    And record "verlening" is applied

  Scenario: Before the copy is received, the grant still counts
    # The court ruled, but the cell does not know it until it receives the
    # copy: an answer given on a day before the receipt (the calculation
    # date) does not know the receipt yet, so it sees the grant.
    Given the calculation date is "2025-05-01"
    When the court records appeal "beroep" under article "10" of "test_chronicle_effects" with:
      | bestreden_besluit | verlening |
    And the court rules "uitspraak" on appeal "beroep" with:
      | beroep_gegrond | true |
    Given the calculation date is "2025-05-10"
    When the cell receives ruling "uitspraak" as fact "afschrift_uitspraak_ontvangen" "ontvangst" under article "11" of "test_chronicle_effects"
    Given the calculation date is "2025-05-05"
    When I evaluate "heeft_geldige_kaart" of "test_reductie_geldige_kaart" over the chronicle
    Then output "heeft_geldige_kaart" is true
    And record "verlening" is applied

  Scenario: When the ruling is annulled on further appeal, the grant counts again
    # Fictional article 12: the cell records that the appeal court annulled
    # the ruling it received a copy of and held the appeal unfounded after
    # all. The fact refers to the receipt and revokes it. A revoked record revokes nothing, so the grant
    # counts again.
    Given the calculation date is "2025-05-01"
    When the court records appeal "beroep" under article "10" of "test_chronicle_effects" with:
      | bestreden_besluit | verlening |
    And the court rules "uitspraak" on appeal "beroep" with:
      | beroep_gegrond | true |
    Given the calculation date is "2025-05-10"
    When the cell receives ruling "uitspraak" as fact "afschrift_uitspraak_ontvangen" "ontvangst" under article "11" of "test_chronicle_effects"
    Given the calculation date is "2025-09-01"
    When the cell records fact "uitspraak_vernietigd_in_hoger_beroep" as "hoger_beroep" about "ontvangst" under article "12" of "test_chronicle_effects" with:
      | uitspraak_in_hoger_beroep_op | 2025-09-01 |
    And I evaluate "heeft_geldige_kaart" of "test_reductie_geldige_kaart" over the chronicle
    Then output "heeft_geldige_kaart" is true
    And record "ontvangst" is skipped
    And the projection trace is:
      | record       | event   | detail                                                                     |
      | aanvraag     | applied | -                                                                          |
      | verlening    | applied | test_chronicle_effects#7: aantal_verleend: 0 -> 1, in_bezit: false -> true |
      | ontvangst    | skipped | revoked by hoger_beroep                                                    |
      | hoger_beroep | applied | -                                                                          |
