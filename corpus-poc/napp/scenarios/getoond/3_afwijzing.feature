Feature: Afwijzing van subsidieaanvragen
  Een aanvraag wordt afgewezen wanneer de partij niet aan de voorwaarden
  voldoet: te weinig zetels of leden, of schending van de
  transparantieregels (anonieme giften, giften van niet-ingezetenen,
  meldplicht, openbaarmaking).

  Background:
    Given the calculation date is "2026-06-01"

  Scenario: Partij die anonieme giften ontvangt wordt afgewezen
    Given an application with the following data:
      | niveau                           | LANDELIJK |
      | orgaan                           | GEMEENTERAAD |
      | aantal_kamerzetels               | 10        |
      | aantal_betalende_leden           | 5000      |
      | aantal_raadszetels               | 0         |
      | inwoneraantal_gemeente           | 0         |
      | ontvangt_anonieme_giften         | true      |
      | ontvangt_giften_niet_ingezetenen | false     |
      | voldoet_aan_meldplicht_giften    | true      |
      | financien_openbaar_op_website    | true      |
    When the subsidiebesluit is executed
    Then the subsidie is afgewezen
    And no betaalopdracht is required
