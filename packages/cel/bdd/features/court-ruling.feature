Feature: A court ruling from another cell takes the place of a decision
  As a cell whose decision the court sets aside
  I want the court's ruling, received as a copy, to count in the place of my decision
  So that my answer follows the court without the court writing in my chronicle

  # The court is a cell of its own. It records the appeal and its ruling in
  # its own chronicle, under fictional article 11 of test_chronicle_effects
  # (after Awb 8:72 lid 3): the ruling replaces the contested decision. The
  # cell records that it received a copy (fictional article 12, after Awb
  # 8:79) with the existing receipt of a copy (RFC-053), which names the
  # decision of the cell the copy concerns. Played forward, the cell's
  # chronicle holds the receipt and, after it, the ruling as a record under
  # its own id `<court>/<id>`, known from the day of the receipt. Its effect
  # comes from article 11 on the day the court recorded it, and targets the
  # decision the receipt names: the court named it by its value, not by an
  # id in this chronicle.

  Background:
    Given parameter "aanvrager" is "burger_x"

  Scenario: The court sets aside the refusal and grants the card in its place
    Given the calculation date is "2025-03-01"
    When the cell records submission "aanvraag" under article "1" of "test_chronicle_effects" with:
      | ernstig_beperkt | false    |
      | aanvrager       | burger_x |
    And the cell decides "weigering" on submission "aanvraag"
    Given the calculation date is "2025-05-01"
    When the court records appeal "beroep" under article "11" of "test_chronicle_effects" with:
      | bestreden_besluit | weigering |
    And the court rules "uitspraak" on appeal "beroep" with:
      | ernstig_beperkt | true     |
      | aanvrager       | burger_x |
    Given the calculation date is "2025-05-10"
    When the cell receives ruling "uitspraak" as fact "afschrift_uitspraak_ontvangen" "ontvangst" under article "12" of "test_chronicle_effects"
    And I evaluate "heeft_geldige_kaart" of "test_reductie_geldige_kaart" over the chronicle
    Then output "heeft_geldige_kaart" is true
    And the records in force are "aanvraag, ontvangst, test_rechtbank/uitspraak"
    And the projection trace is:
      | record                   | event   | detail                             |
      | aanvraag                 | applied | -                                  |
      | weigering                | applied | -                                  |
      | ontvangst                | applied | -                                  |
      | test_rechtbank/uitspraak | rewound | replaces weigering                 |
      | aanvraag                 | applied | -                                  |
      | weigering                | skipped | undone by test_rechtbank/uitspraak |
      | ontvangst                | applied | -                                  |
      | test_rechtbank/uitspraak | applied | -                                  |
    When the cell answers "heeft_geldige_kaart"
    Then the answer of the cell is true

  Scenario: Before the copy is received, the refusal still counts
    Given the calculation date is "2025-03-01"
    When the cell records submission "aanvraag" under article "1" of "test_chronicle_effects" with:
      | ernstig_beperkt | false    |
      | aanvrager       | burger_x |
    And the cell decides "weigering" on submission "aanvraag"
    Given the calculation date is "2025-05-01"
    When the court records appeal "beroep" under article "11" of "test_chronicle_effects" with:
      | bestreden_besluit | weigering |
    And the court rules "uitspraak" on appeal "beroep" with:
      | ernstig_beperkt | true     |
      | aanvrager       | burger_x |
    Given the calculation date is "2025-05-10"
    When the cell receives ruling "uitspraak" as fact "afschrift_uitspraak_ontvangen" "ontvangst" under article "12" of "test_chronicle_effects"
    Given the calculation date is "2025-05-05"
    When I evaluate "heeft_geldige_kaart" of "test_reductie_geldige_kaart" over the chronicle
    Then output "heeft_geldige_kaart" is false
    And record "weigering" is applied
