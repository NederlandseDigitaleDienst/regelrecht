Feature: Afwijzing van subsidieaanvragen
  Een aanvraag wordt afgewezen wanneer de partij niet aan de voorwaarden
  voldoet: te weinig zetels of leden, of schending van de
  transparantieregels (anonieme giften, giften van niet-ingezetenen,
  meldplicht, openbaarmaking).

  Background:
    Given the calculation date is "2026-06-01"

  Scenario: Landelijke partij zonder kamerzetels wordt afgewezen
    Given an application with the following data:
      | niveau                           | LANDELIJK |
      | orgaan                           | GEMEENTERAAD |
      | aantal_kamerzetels               | 0         |
      | aantal_betalende_leden           | 5000      |
      | aantal_raadszetels               | 0         |
      | inwoneraantal_gemeente           | 0         |
      | ontvangt_anonieme_giften         | false     |
      | ontvangt_giften_niet_ingezetenen | false     |
      | voldoet_aan_meldplicht_giften    | true      |
      | financien_openbaar_op_website    | true      |
    When the subsidiebesluit is executed
    Then the subsidie is afgewezen
    And the subsidiebedrag is "0" eurocent
    And no betaalopdracht is required

  Scenario: Landelijke partij met minder dan duizend leden wordt afgewezen
    Given an application with the following data:
      | niveau                           | LANDELIJK |
      | orgaan                           | GEMEENTERAAD |
      | aantal_kamerzetels               | 3         |
      | aantal_betalende_leden           | 999       |
      | aantal_raadszetels               | 0         |
      | inwoneraantal_gemeente           | 0         |
      | ontvangt_anonieme_giften         | false     |
      | ontvangt_giften_niet_ingezetenen | false     |
      | voldoet_aan_meldplicht_giften    | true      |
      | financien_openbaar_op_website    | true      |
    When the subsidiebesluit is executed
    Then the subsidie is afgewezen
    And no betaalopdracht is required

  Scenario: Partij die giften van niet-ingezetenen ontvangt wordt afgewezen
    Given an application with the following data:
      | niveau                           | LANDELIJK |
      | orgaan                           | GEMEENTERAAD |
      | aantal_kamerzetels               | 10        |
      | aantal_betalende_leden           | 5000      |
      | aantal_raadszetels               | 0         |
      | inwoneraantal_gemeente           | 0         |
      | ontvangt_anonieme_giften         | false     |
      | ontvangt_giften_niet_ingezetenen | true      |
      | voldoet_aan_meldplicht_giften    | true      |
      | financien_openbaar_op_website    | true      |
    When the subsidiebesluit is executed
    Then the subsidie is afgewezen
    And no betaalopdracht is required

  Scenario: Partij die de meldplicht voor grote giften schendt wordt afgewezen
    Given an application with the following data:
      | niveau                           | DECENTRAAL |
      | orgaan                           | GEMEENTERAAD |
      | aantal_kamerzetels               | 0          |
      | aantal_betalende_leden           | 0          |
      | aantal_raadszetels               | 4          |
      | inwoneraantal_gemeente           | 80000      |
      | ontvangt_anonieme_giften         | false      |
      | ontvangt_giften_niet_ingezetenen | false      |
      | voldoet_aan_meldplicht_giften    | false      |
      | financien_openbaar_op_website    | true       |
    When the subsidiebesluit is executed
    Then the subsidie is afgewezen
    And no betaalopdracht is required

  Scenario: Partij die haar financien niet openbaar maakt wordt afgewezen
    Given an application with the following data:
      | niveau                           | DECENTRAAL |
      | orgaan                           | GEMEENTERAAD |
      | aantal_kamerzetels               | 0          |
      | aantal_betalende_leden           | 0          |
      | aantal_raadszetels               | 4          |
      | inwoneraantal_gemeente           | 80000      |
      | ontvangt_anonieme_giften         | false      |
      | ontvangt_giften_niet_ingezetenen | false      |
      | voldoet_aan_meldplicht_giften    | true       |
      | financien_openbaar_op_website    | false      |
    When the subsidiebesluit is executed
    Then the subsidie is afgewezen
    And no betaalopdracht is required

  Scenario: Decentrale partij zonder raadszetels wordt afgewezen
    Given an application with the following data:
      | niveau                           | DECENTRAAL |
      | orgaan                           | GEMEENTERAAD |
      | aantal_kamerzetels               | 0          |
      | aantal_betalende_leden           | 0          |
      | aantal_raadszetels               | 0          |
      | inwoneraantal_gemeente           | 50000      |
      | ontvangt_anonieme_giften         | false      |
      | ontvangt_giften_niet_ingezetenen | false      |
      | voldoet_aan_meldplicht_giften    | true       |
      | financien_openbaar_op_website    | true       |
    When the subsidiebesluit is executed
    Then the subsidie is afgewezen
    And no betaalopdracht is required
