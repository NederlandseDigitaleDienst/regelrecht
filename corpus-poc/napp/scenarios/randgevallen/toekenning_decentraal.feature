Feature: Subsidie voor decentrale politieke partijen
  Als decentrale politieke partij met ten minste een zetel in de
  gemeenteraad, provinciale staten, de eilandsraad of het algemeen bestuur
  van een waterschap wil ik subsidie ontvangen, berekend als aantal zetels
  maal een bedrag per zetel uit het Besluit subsidiëring decentrale
  politieke partijen (AMvB op grond van artikel 26; voor gemeenteraadszetels
  afhankelijk van het inwoneraantal). Bij verlening hoort een voorschot
  van 80% (artikel 17).

  Background:
    Given the calculation date is "2026-06-01"

  Scenario: Lokale partij met 3 zetels in een kleine gemeente
    Given an application with the following data:
      | niveau                           | DECENTRAAL |
      | orgaan                           | GEMEENTERAAD |
      | aantal_kamerzetels               | 0          |
      | aantal_betalende_leden           | 0          |
      | aantal_raadszetels               | 3          |
      | inwoneraantal_gemeente           | 30000      |
      | ontvangt_anonieme_giften         | false      |
      | ontvangt_giften_niet_ingezetenen | false      |
      | voldoet_aan_meldplicht_giften    | true       |
      | financien_openbaar_op_website    | true       |
    When the subsidiebesluit is executed
    Then the subsidie is toegekend
    And the subsidiebedrag is "197400" eurocent

  Scenario: Stadspartij met 7 zetels in een grote stad
    Given an application with the following data:
      | niveau                           | DECENTRAAL |
      | orgaan                           | GEMEENTERAAD |
      | aantal_kamerzetels               | 0          |
      | aantal_betalende_leden           | 0          |
      | aantal_raadszetels               | 7          |
      | inwoneraantal_gemeente           | 400000     |
      | ontvangt_anonieme_giften         | false      |
      | ontvangt_giften_niet_ingezetenen | false      |
      | voldoet_aan_meldplicht_giften    | true       |
      | financien_openbaar_op_website    | true       |
    When the subsidiebesluit is executed
    Then the subsidie is toegekend
    And the subsidiebedrag is "1129100" eurocent

  Scenario: Partij in gemeente precies op de staffelgrens van 375000 inwoners
    Given an application with the following data:
      | niveau                           | DECENTRAAL |
      | orgaan                           | GEMEENTERAAD |
      | aantal_kamerzetels               | 0          |
      | aantal_betalende_leden           | 0          |
      | aantal_raadszetels               | 2          |
      | inwoneraantal_gemeente           | 375000     |
      | ontvangt_anonieme_giften         | false      |
      | ontvangt_giften_niet_ingezetenen | false      |
      | voldoet_aan_meldplicht_giften    | true       |
      | financien_openbaar_op_website    | true       |
    When the subsidiebesluit is executed
    Then the subsidie is toegekend
    And the subsidiebedrag is "240000" eurocent

  Scenario: Partij met 17 statenzetels in een provincie
    Given an application with the following data:
      | niveau                           | DECENTRAAL |
      | orgaan                           | PROVINCIALE_STATEN |
      | aantal_kamerzetels               | 0          |
      | aantal_betalende_leden           | 0          |
      | aantal_raadszetels               | 17         |
      | inwoneraantal_gemeente           | 0          |
      | ontvangt_anonieme_giften         | false      |
      | ontvangt_giften_niet_ingezetenen | false      |
      | voldoet_aan_meldplicht_giften    | true       |
      | financien_openbaar_op_website    | true       |
    When the subsidiebesluit is executed
    Then the subsidie is toegekend
    And the subsidiebedrag is "2742100" eurocent

  Scenario: Partij met 4 zetels in het algemeen bestuur van een waterschap
    Given an application with the following data:
      | niveau                           | DECENTRAAL |
      | orgaan                           | WATERSCHAP |
      | aantal_kamerzetels               | 0          |
      | aantal_betalende_leden           | 0          |
      | aantal_raadszetels               | 4          |
      | inwoneraantal_gemeente           | 0          |
      | ontvangt_anonieme_giften         | false      |
      | ontvangt_giften_niet_ingezetenen | false      |
      | voldoet_aan_meldplicht_giften    | true       |
      | financien_openbaar_op_website    | true       |
    When the subsidiebesluit is executed
    Then the subsidie is toegekend
    And the subsidiebedrag is "645200" eurocent

  Scenario: Eilandspartij met 3 zetels in de eilandsraad
    Given an application with the following data:
      | niveau                           | DECENTRAAL  |
      | orgaan                           | EILANDSRAAD |
      | aantal_kamerzetels               | 0           |
      | aantal_betalende_leden           | 0           |
      | aantal_raadszetels               | 3           |
      | inwoneraantal_gemeente           | 0           |
      | ontvangt_anonieme_giften         | false       |
      | ontvangt_giften_niet_ingezetenen | false       |
      | voldoet_aan_meldplicht_giften    | true        |
      | financien_openbaar_op_website    | true        |
    When the subsidiebesluit is executed
    Then the subsidie is toegekend
    And the subsidiebedrag is "197400" eurocent
    And a betaalopdracht of "157920" eurocent is required
